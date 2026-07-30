use engine::{validate_markdown_handoff, HandoffArtifact, HandoffFormat, HandoffSpec, NodeId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const MAX_HANDOFF_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum HandoffError {
    #[error("{0}")]
    Invalid(String),
    #[error("handoff storage failed: {0}")]
    Storage(String),
}

impl HandoffError {
    #[must_use]
    pub const fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }
}

pub struct StoredHandoff {
    pub artifact: HandoffArtifact,
    pub output: Value,
}

/// Run-scoped node handoff persistence.
#[derive(Debug, Clone)]
pub struct HandoffStore {
    root: PathBuf,
}

impl HandoffStore {
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Validate and persist one completed node handoff.
    ///
    /// # Errors
    ///
    /// Returns [`HandoffError::Invalid`] for malformed Markdown handoffs and
    /// [`HandoffError::Storage`] for filesystem failures.
    pub fn materialize(
        &self,
        node_id: &NodeId,
        spec: &HandoffSpec,
        output: &Value,
        assistant_message: Option<&str>,
    ) -> Result<StoredHandoff, HandoffError> {
        let segment = encode_path_segment(node_id);
        let directory = self.root.join(&segment);
        fs::create_dir_all(&directory).map_err(|error| HandoffError::Storage(error.to_string()))?;

        let (format, filename, media_type, content, downstream_output) = match spec {
            HandoffSpec::Markdown { template } => {
                let markdown = output
                    .get("markdown")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        HandoffError::Invalid(
                            "Markdown handoff requires output.markdown".to_string(),
                        )
                    })?;
                validate_markdown_handoff(template, markdown)
                    .map_err(|error| HandoffError::Invalid(error.to_string()))?;
                let summary = assistant_message
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("See the Markdown handoff artifact.");
                (
                    HandoffFormat::Markdown,
                    "HANDOFF.md",
                    "text/markdown",
                    markdown.as_bytes().to_vec(),
                    json!({ "summary": summary }),
                )
            }
            HandoffSpec::Json => {
                let mut content = serde_json::to_vec_pretty(output)
                    .map_err(|error| HandoffError::Invalid(error.to_string()))?;
                content.push(b'\n');
                (
                    HandoffFormat::Json,
                    "HANDOFF.json",
                    "application/json",
                    content,
                    output.clone(),
                )
            }
            HandoffSpec::Legacy => {
                return Err(HandoffError::Invalid(
                    "legacy node output does not use a handoff artifact".to_string(),
                ));
            }
        };

        if content.len() > MAX_HANDOFF_BYTES {
            return Err(HandoffError::Invalid(format!(
                "handoff exceeds the {MAX_HANDOFF_BYTES} byte limit"
            )));
        }

        let path = directory.join(filename);
        write_handoff(&path, &content)?;
        let sha256 = format!("{:x}", Sha256::digest(&content));

        Ok(StoredHandoff {
            artifact: HandoffArtifact {
                format,
                uri: format!("run://handoffs/{segment}/{filename}"),
                media_type: media_type.to_string(),
                sha256,
                size_bytes: content.len(),
            },
            output: downstream_output,
        })
    }
}

#[must_use]
pub fn handoff_root_for_artifact_root(artifact_root: &Path) -> PathBuf {
    if artifact_root
        .file_name()
        .is_some_and(|name| name == "artifacts")
    {
        return artifact_root
            .parent()
            .unwrap_or(artifact_root)
            .join("handoffs");
    }
    artifact_root.join("handoffs")
}

/// Resolve an immutable handoff URI under the current run root.
///
/// # Errors
///
/// Returns [`HandoffError::Invalid`] for malformed or traversal-like URIs.
pub fn resolve_handoff_uri(artifact_root: &Path, uri: &str) -> Result<PathBuf, HandoffError> {
    let relative = uri
        .strip_prefix("run://handoffs/")
        .ok_or_else(|| HandoffError::Invalid(format!("invalid handoff URI: {uri}")))?;
    let segments = relative.split('/').collect::<Vec<_>>();
    if segments.len() != 2
        || segments.iter().any(|segment| segment.is_empty())
        || !matches!(segments[1], "HANDOFF.md" | "HANDOFF.json")
        || segments[0] == "."
        || segments[0] == ".."
    {
        return Err(HandoffError::Invalid(format!("invalid handoff URI: {uri}")));
    }
    Ok(handoff_root_for_artifact_root(artifact_root)
        .join(segments[0])
        .join(segments[1]))
}

fn write_handoff(path: &Path, content: &[u8]) -> Result<(), HandoffError> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, content).map_err(|error| HandoffError::Storage(error.to_string()))?;
    fs::rename(&temporary, path).map_err(|error| HandoffError::Storage(error.to_string()))
}

fn encode_path_segment(node_id: &NodeId) -> String {
    let mut encoded = String::new();
    for byte in node_id.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.') {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
