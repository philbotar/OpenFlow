use crate::tool::errors::ToolError;
use engine::{ToolOutputMeta, ToolTruncation, ToolTruncationStrategy};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const INLINE_OUTPUT_BYTE_LIMIT: usize = 50_000;
const DEFAULT_HEAD_BYTES: usize = 20_000;
const DEFAULT_TAIL_BYTES: usize = 20_000;

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
            ToolError::Failed(format!("failed to create artifact dir: {error}"))
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
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
            ToolError::Failed(format!("failed to spill tool output to artifact: {error}"))
        })?;

        let head_len = DEFAULT_HEAD_BYTES.min(text.len());
        let tail_len = DEFAULT_TAIL_BYTES.min(text.len().saturating_sub(head_len));
        let head = &text[..head_len];
        let tail = &text[text.len() - tail_len..];
        let truncated = format!(
            "{head}\n… output truncated; read artifact {} for full output …\n{tail}",
            artifact_id
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
        assert!(artifact.is_some());
        assert!(meta.and_then(|value| value.truncation).is_some());
    }
}
