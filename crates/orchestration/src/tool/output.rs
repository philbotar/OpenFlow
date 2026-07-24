use crate::tool::errors::ToolError;
use engine::{ToolOutputMeta, ToolTruncation, ToolTruncationStrategy};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const INLINE_OUTPUT_BYTE_LIMIT: usize = 50_000;
const DEFAULT_HEAD_BYTES: usize = 20_000;
const DEFAULT_TAIL_BYTES: usize = 20_000;
pub const MAX_PLAN_ARTIFACT_BYTES: usize = 256 * 1024;
const PLAN_DRAFT_FILE_NAME: &str = "PLAN.md";

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

/// Immutable Markdown artifact written by the Plan Mode capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanArtifact {
    pub record: ToolArtifactRecord,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
    plan_artifact_id: Arc<Mutex<Option<String>>>,
}

impl ArtifactStore {
    pub fn new(root: PathBuf) -> Result<Self, ToolError> {
        fs::create_dir_all(&root).map_err(|error| {
            ToolError::failed(format!("failed to create artifact dir: {error}"))
        })?;
        let plan_artifact_id = fs::read_dir(&root).ok().and_then(|entries| {
            entries.filter_map(Result::ok).find_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let artifact_id = name.strip_suffix("-plan.md")?;
                (Uuid::parse_str(artifact_id).ok()?.to_string() == artifact_id)
                    .then(|| artifact_id.to_string())
            })
        });
        Ok(Self {
            root,
            plan_artifact_id: Arc::new(Mutex::new(plan_artifact_id)),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Return the host path backing the mutable run-local plan draft.
    ///
    /// The path stops being writable as soon as a plan artifact is sealed.
    pub fn plan_draft_path(&self) -> Result<PathBuf, ToolError> {
        let plan_artifact_id = self
            .plan_artifact_id
            .lock()
            .map_err(|_| ToolError::failed("plan artifact lock is unavailable".to_string()))?;
        if plan_artifact_id.is_some() {
            return Err(ToolError::failed(
                "a plan artifact has already been sealed for this run".to_string(),
            ));
        }
        Ok(self.root.join(PLAN_DRAFT_FILE_NAME))
    }

    /// Atomically move the completed run-local draft to its immutable,
    /// host-selected artifact path.
    pub fn seal_plan_draft(&self) -> Result<PlanArtifact, ToolError> {
        let mut plan_artifact_id = self
            .plan_artifact_id
            .lock()
            .map_err(|_| ToolError::failed("plan artifact lock is unavailable".to_string()))?;
        if let Some(artifact_id) = plan_artifact_id.as_deref() {
            return self.read_sealed_plan_artifact(artifact_id);
        }

        let draft_path = self.root.join(PLAN_DRAFT_FILE_NAME);
        let markdown = fs::read_to_string(&draft_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound {
                    what: format!("plan draft not found at {}", engine::PLAN_DRAFT_PATH),
                    hint: format!(
                        "create {} with write, then update it with edit before sealing",
                        engine::PLAN_DRAFT_PATH
                    ),
                }
            } else {
                ToolError::failed(format!("failed to read plan draft: {error}"))
            }
        })?;
        if markdown.trim().is_empty() {
            return Err(ToolError::failed(format!(
                "plan draft at {} is empty",
                engine::PLAN_DRAFT_PATH
            )));
        }
        if markdown.len() > MAX_PLAN_ARTIFACT_BYTES {
            return Err(ToolError::failed(format!(
                "plan artifact exceeds the {} byte limit",
                MAX_PLAN_ARTIFACT_BYTES
            )));
        }

        let artifact_id = Uuid::new_v4().to_string();
        let path = self.root.join(format!("{artifact_id}-plan.md"));
        fs::rename(&draft_path, &path)
            .map_err(|error| ToolError::failed(format!("failed to seal plan artifact: {error}")))?;
        *plan_artifact_id = Some(artifact_id.clone());

        let mut digest = Sha256::new();
        digest.update(markdown.as_bytes());
        Ok(PlanArtifact {
            record: ToolArtifactRecord {
                artifact_id,
                tool_name: "openflow_write_plan_artifact".to_string(),
                path: path.display().to_string(),
                size_bytes: markdown.len(),
            },
            sha256: format!("{:x}", digest.finalize()),
        })
    }

    fn read_sealed_plan_artifact(&self, artifact_id: &str) -> Result<PlanArtifact, ToolError> {
        let path = self.root.join(format!("{artifact_id}-plan.md"));
        let markdown = fs::read_to_string(&path).map_err(|error| {
            ToolError::failed(format!(
                "sealed plan artifact {artifact_id} is unavailable: {error}"
            ))
        })?;
        let mut digest = Sha256::new();
        digest.update(markdown.as_bytes());
        Ok(PlanArtifact {
            record: ToolArtifactRecord {
                artifact_id: artifact_id.to_string(),
                tool_name: "openflow_write_plan_artifact".to_string(),
                path: path.display().to_string(),
                size_bytes: markdown.len(),
            },
            sha256: format!("{:x}", digest.finalize()),
        })
    }

    /// Resolve a host-created artifact by id under the artifact root.
    pub fn path_for(&self, artifact_id: &str) -> Option<PathBuf> {
        let uuid = Uuid::parse_str(artifact_id).ok()?;
        if uuid.to_string() != artifact_id {
            return None;
        }
        ["txt", "md"].into_iter().find_map(|extension| {
            let pattern = self.root.join(format!("{artifact_id}-*.{extension}"));
            glob::glob(pattern.to_str()?)
                .ok()?
                .filter_map(Result::ok)
                .next()
        })
    }

    /// Write a plan exactly once to a host-selected Markdown artifact path.
    pub fn store_plan_markdown(&self, markdown: String) -> Result<PlanArtifact, ToolError> {
        if markdown.len() > MAX_PLAN_ARTIFACT_BYTES {
            return Err(ToolError::failed(format!(
                "plan artifact exceeds the {} byte limit",
                MAX_PLAN_ARTIFACT_BYTES
            )));
        }
        let mut plan_artifact_id = self
            .plan_artifact_id
            .lock()
            .map_err(|_| ToolError::failed("plan artifact lock is unavailable".to_string()))?;
        if plan_artifact_id.is_some() {
            return Err(ToolError::failed(
                "a plan artifact has already been sealed for this run".to_string(),
            ));
        }
        let artifact_id = Uuid::new_v4().to_string();
        let path = self.root.join(format!("{artifact_id}-plan.md"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                ToolError::failed(format!("failed to create plan artifact: {error}"))
            })?;
        file.write_all(markdown.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| {
                ToolError::failed(format!("failed to write plan artifact: {error}"))
            })?;
        *plan_artifact_id = Some(artifact_id.clone());

        let mut digest = Sha256::new();
        digest.update(markdown.as_bytes());
        Ok(PlanArtifact {
            record: ToolArtifactRecord {
                artifact_id,
                tool_name: "openflow_write_plan_artifact".to_string(),
                path: path.display().to_string(),
                size_bytes: markdown.len(),
            },
            sha256: format!("{:x}", digest.finalize()),
        })
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

    #[test]
    fn plan_markdown_is_host_named_immutable_and_readable_as_an_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let markdown = "# Approved plan\n\nImplement the narrow slice.".to_string();
        let artifact = store.store_plan_markdown(markdown.clone()).unwrap();

        assert_eq!(artifact.record.tool_name, "openflow_write_plan_artifact");
        assert_eq!(artifact.record.size_bytes, markdown.len());
        assert_eq!(artifact.sha256.len(), 64);
        assert!(artifact.record.path.ends_with("-plan.md"));
        assert!(Uuid::parse_str(&artifact.record.artifact_id).is_ok());
        let resolved = store
            .path_for(&artifact.record.artifact_id)
            .expect("artifact path");
        assert_eq!(fs::read_to_string(resolved).unwrap(), markdown);
    }

    #[test]
    fn plan_markdown_rejects_content_over_the_fixed_limit() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let error = store
            .store_plan_markdown("x".repeat(MAX_PLAN_ARTIFACT_BYTES + 1))
            .unwrap_err();

        assert!(matches!(
            error,
            ToolError::ExecutionFailed { detail, .. }
                if detail.contains("byte limit") && detail.contains(&MAX_PLAN_ARTIFACT_BYTES.to_string())
        ));
        assert!(fs::read_dir(store.root()).unwrap().next().is_none());
    }

    #[test]
    fn plan_markdown_is_sealed_once_per_run_artifact_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let first = store.store_plan_markdown("# First".to_string()).unwrap();
        let error = store
            .store_plan_markdown("# Second".to_string())
            .unwrap_err();

        assert!(error.to_string().contains("already been sealed"));
        assert_eq!(
            fs::read_to_string(store.path_for(&first.record.artifact_id).unwrap()).unwrap(),
            "# First"
        );
        assert_eq!(fs::read_dir(store.root()).unwrap().count(), 1);
    }

    #[test]
    fn repeated_plan_draft_seal_returns_the_existing_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().join("artifacts")).unwrap();
        let markdown = "# Approved plan\n\nImplement the narrow slice.\n";
        let draft_path = store.plan_draft_path().unwrap();
        fs::write(&draft_path, markdown).unwrap();

        let artifact = store.seal_plan_draft().unwrap();

        assert!(!draft_path.exists());
        assert_eq!(
            fs::read_to_string(store.path_for(&artifact.record.artifact_id).unwrap()).unwrap(),
            markdown
        );
        assert_eq!(artifact.sha256.len(), 64);
        assert!(store
            .plan_draft_path()
            .unwrap_err()
            .to_string()
            .contains("already been sealed"));
        assert_eq!(store.seal_plan_draft().unwrap(), artifact);
    }
}
