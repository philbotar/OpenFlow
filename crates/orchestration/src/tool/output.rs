use crate::tool::errors::ToolError;
use engine::{ToolOutputMeta, ToolTruncation, ToolTruncationStrategy};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const INLINE_OUTPUT_BYTE_LIMIT: usize = 50_000;
const DEFAULT_HEAD_BYTES: usize = 20_000;
const DEFAULT_TAIL_BYTES: usize = 20_000;

/// Per-run artifact storage. Artifacts live under the run's artifact root for the
/// duration of the run; there is no garbage collection until the run ends.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolArtifactRecord {
    pub artifact_id: String,
    pub tool_name: String,
    pub path: String,
    pub size_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub fn new(root: PathBuf) -> Result<Self, ToolError> {
        fs::create_dir_all(&root).map_err(|error| {
            ToolError::failed(format!("failed to create artifact dir: {error}"))
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a spilled artifact by id (`{id}-*.txt` under the artifact root).
    pub fn path_for(&self, artifact_id: &str) -> Option<PathBuf> {
        let pattern = self.root.join(format!("{artifact_id}-*.txt"));
        glob::glob(pattern.to_str()?)
            .ok()?
            .filter_map(Result::ok)
            .next()
    }

    pub fn store_text(
        &self,
        tool_name: &str,
        text: String,
    ) -> Result<(String, Option<ToolArtifactRecord>, Option<ToolOutputMeta>), ToolError> {
        if text.len() <= INLINE_OUTPUT_BYTE_LIMIT {
            return Ok((text, None, None));
        }

        let artifact_id = Uuid::new_v4().to_string();
        let file_name = format!("{artifact_id}-{tool_name}.txt");
        let path = self.root.join(file_name);
        fs::write(&path, &text).map_err(|error| {
            ToolError::failed(format!("failed to spill tool output to artifact: {error}"))
        })?;

        let total_bytes = text.len();
        let total_lines = text.lines().count();
        let head_len = DEFAULT_HEAD_BYTES.min(text.len());
        let tail_len = DEFAULT_TAIL_BYTES.min(text.len().saturating_sub(head_len));
        let head = &text[..head_len];
        let tail = &text[text.len() - tail_len..];
        let truncated = format!(
            "{head}\n… output truncated ({total_bytes} bytes, {total_lines} lines); call read with path \"artifact:{artifact_id}\" (supports :start-end selectors) for the full output …\n{tail}",
        );
        let meta = ToolOutputMeta {
            truncation: Some(ToolTruncation {
                strategy: ToolTruncationStrategy::Middle,
                total_bytes: text.len(),
                shown_bytes: truncated.len(),
                elided_bytes: text.len().saturating_sub(truncated.len()),
                total_lines: Some(text.lines().count()),
                shown_lines: Some(truncated.lines().count()),
                elided_lines: Some(
                    text.lines()
                        .count()
                        .saturating_sub(truncated.lines().count()),
                ),
            }),
            source_url: None,
        };
        Ok((
            truncated,
            Some(ToolArtifactRecord {
                artifact_id,
                tool_name: tool_name.to_string(),
                path: path.display().to_string(),
                size_bytes: text.len(),
            }),
            Some(meta),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_text_stays_inline() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let (text, artifact, meta) = store.store_text("read", "hello".to_string()).unwrap();
        assert_eq!(text, "hello");
        assert!(artifact.is_none());
        assert!(meta.is_none());
    }

    #[test]
    fn large_text_spills_to_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let big = "x".repeat(60_000);
        let (text, artifact, meta) = store.store_text("read", big).unwrap();
        assert!(text.contains("output truncated"));
        assert!(text.contains("artifact:"));
        assert!(artifact.is_some());
        assert!(meta.and_then(|value| value.truncation).is_some());
    }

    #[test]
    fn path_for_resolves_spilled_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let big = "y".repeat(60_000);
        let (_, artifact, _) = store.store_text("bash", big).unwrap();
        let record = artifact.expect("artifact record");
        let resolved = store.path_for(&record.artifact_id).expect("resolved path");
        assert!(resolved.is_file());
        let content = fs::read_to_string(resolved).unwrap();
        assert_eq!(content.len(), 60_000);
    }

    #[test]
    fn path_for_returns_none_for_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        assert!(store.path_for("nonexistent-id").is_none());
    }
}
