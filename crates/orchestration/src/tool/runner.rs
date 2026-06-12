use crate::settings::model::LspSettings;
use crate::tool::blocking_ops::split_selector;
pub(super) use crate::tool::blocking_ops::{
    apply_read_selector, BlockingBatchContext, BlockingRunOutcome, BlockingToolOps,
};
use crate::tool::cache::{cache_key, CacheEntry, CacheValidation, ToolResultCache};
use crate::tool::errors::ToolError;
use crate::tool::output::{ArtifactStore, ToolArtifactRecord};
use crate::tool::registry::{BuiltinToolKind, ToolRegistry, ToolRegistryError};
use engine::{EditBatch, FileChangeRecord, ToolCall, ToolOutputMeta, ToolResult};
use reqwest::Client;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub result: ToolResult,
    pub artifact: Option<ToolArtifactRecord>,
    pub file_changes: Vec<FileChangeRecord>,
    pub edit_batch: Option<EditBatch>,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub node_id: engine::NodeId,
    /// Identifies the transcript receiving the result: the node id for node
    /// turns, a unique session id for subagent invocations. Used by the
    /// result cache to decide whether a repeated call can be answered with a
    /// content-omitting stub (the transcript already holds the content).
    pub conversation_id: String,
    /// LSP format-on-write and diagnostics for edit tools in this invocation.
    pub lsp: LspSettings,
}

#[derive(Debug)]
pub struct ToolRunner {
    registry: ToolRegistry,
    pub(super) http: Client,
    pub(super) cwd: PathBuf,
    artifacts: ArtifactStore,
    pub(super) cancel_token: CancellationToken,
    pub(super) snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    cache: ToolResultCache,
}

#[derive(Debug, Error)]
pub enum ToolRunnerError {
    #[error(transparent)]
    Registry(#[from] ToolRegistryError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error("{0}")]
    InvalidArguments(String),
    #[error("blocking tool task failed: {0}")]
    BlockingTask(String),
}

impl ToolRunner {
    pub fn new(
        registry: ToolRegistry,
        cwd: PathBuf,
        artifacts: ArtifactStore,
        cancel_token: CancellationToken,
        snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    ) -> Self {
        Self {
            registry,
            http: Client::new(),
            cwd,
            artifacts,
            cancel_token,
            snapshot_store,
            cache: ToolResultCache::new(),
        }
    }

    /// Invalidate epoch-validated cache entries after files changed outside
    /// tool execution (e.g. edit-batch reverts).
    pub fn bump_cache_epoch(&self) {
        self.cache.bump_write_epoch();
    }

    pub fn snapshot_store(
        &self,
    ) -> Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore> {
        self.snapshot_store.clone()
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn artifacts(&self) -> &ArtifactStore {
        &self.artifacts
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub async fn execute(
        &self,
        call: ToolCall,
        ctx: Option<ToolExecutionContext>,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let kind = self.registry.get(&call.name)?.kind;
        if let Some(context) = &ctx {
            if let Some(record) = self.serve_cached(kind, &call, context) {
                return Ok(record);
            }
        }
        let cache_ctx = ctx.clone();
        let result = self.dispatch(kind, call.clone(), ctx).await;
        if matches!(
            kind,
            BuiltinToolKind::Write
                | BuiltinToolKind::Edit
                | BuiltinToolKind::ApplyPatch
                | BuiltinToolKind::Bash
        ) {
            self.cache.bump_write_epoch();
        }
        if let (Some(context), Ok(record)) = (&cache_ctx, &result) {
            self.maybe_cache(kind, &call, context, record);
        }
        result
    }

    /// Whether results of this call are reproducible from on-disk state alone
    /// (and therefore safe to serve from the per-run cache once validated).
    fn is_cacheable(&self, kind: BuiltinToolKind, call: &ToolCall) -> bool {
        match kind {
            BuiltinToolKind::Search | BuiltinToolKind::Find | BuiltinToolKind::AstGrep => true,
            BuiltinToolKind::Read => self
                .read_call_path(call)
                .is_some_and(|path| !path.starts_with("http://") && !path.starts_with("https://")),
            _ => false,
        }
    }

    fn read_call_path<'a>(&self, call: &'a ToolCall) -> Option<&'a str> {
        call.arguments.get("path").and_then(Value::as_str)
    }

    fn serve_cached(
        &self,
        kind: BuiltinToolKind,
        call: &ToolCall,
        ctx: &ToolExecutionContext,
    ) -> Option<ToolExecutionRecord> {
        if !self.is_cacheable(kind, call) {
            return None;
        }
        let key = cache_key(&call.name, &call.arguments);
        let hit = self.cache.lookup(&key, &ctx.conversation_id)?;
        let (content, artifact_ids, output_meta) = if hit.same_conversation {
            (
                format!(
                    "[cached] Result identical to your earlier `{}` call ({}) in this conversation; inputs unchanged, content omitted.",
                    call.name, hit.tool_call_id
                ),
                Vec::new(),
                None,
            )
        } else {
            (
                format!(
                    "[cached] Unchanged since node '{}' ran this exact `{}` call earlier in the run.\n\n{}",
                    hit.node_id, call.name, hit.content
                ),
                hit.artifact_ids,
                hit.output_meta,
            )
        };
        Some(ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                content,
                is_error: false,
                artifact_ids,
                output_meta,
            },
            artifact: None,
            file_changes: Vec::new(),
            edit_batch: None,
        })
    }

    fn maybe_cache(
        &self,
        kind: BuiltinToolKind,
        call: &ToolCall,
        ctx: &ToolExecutionContext,
        record: &ToolExecutionRecord,
    ) {
        if record.result.is_error || !self.is_cacheable(kind, call) {
            return;
        }
        let validation = match kind {
            BuiltinToolKind::Read => {
                let Some(path) = self.read_call_path(call) else {
                    return;
                };
                let (path, _) = split_selector(path);
                let absolute = if Path::new(&path).is_absolute() {
                    PathBuf::from(&path)
                } else {
                    self.cwd.join(&path)
                };
                match fs::metadata(&absolute) {
                    Ok(meta) if meta.is_file() => {
                        let Ok(modified) = meta.modified() else {
                            return;
                        };
                        CacheValidation::FileStamp {
                            path: absolute,
                            modified,
                            len: meta.len(),
                        }
                    }
                    // Directory listings cannot be stamped; tie to write epoch.
                    Ok(_) => CacheValidation::WriteEpoch(self.cache.current_epoch()),
                    Err(_) => return,
                }
            }
            _ => CacheValidation::WriteEpoch(self.cache.current_epoch()),
        };
        self.cache.insert(
            cache_key(&call.name, &call.arguments),
            CacheEntry {
                conversation_id: ctx.conversation_id.clone(),
                node_id: ctx.node_id.0.clone(),
                tool_call_id: call.id.clone(),
                content: record.result.content.clone(),
                artifact_ids: record.result.artifact_ids.clone(),
                output_meta: record.result.output_meta.clone(),
                validation,
            },
        );
    }

    pub(super) async fn finalize_bash_record(
        &self,
        call: ToolCall,
        outcome: crate::tools::bash::BashExecutionOutcome,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let (content, artifact, output_meta) =
            self.store_output_text(&call.name, outcome.output).await?;
        Ok(ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content,
                is_error: outcome.is_error,
                artifact_ids: artifact
                    .as_ref()
                    .map(|record| vec![record.artifact_id.clone()])
                    .unwrap_or_default(),
                output_meta,
            },
            artifact,
            file_changes: Vec::new(),
            edit_batch: None,
        })
    }

    pub(super) async fn finalize_record(
        &self,
        call: ToolCall,
        raw_output: String,
        file_changes: Vec<FileChangeRecord>,
        edit_batch: Option<EditBatch>,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let (content, artifact, output_meta) =
            self.store_output_text(&call.name, raw_output).await?;
        Ok(ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content,
                is_error: false,
                artifact_ids: artifact
                    .as_ref()
                    .map(|record| vec![record.artifact_id.clone()])
                    .unwrap_or_default(),
                output_meta,
            },
            artifact,
            file_changes,
            edit_batch,
        })
    }

    async fn store_output_text(
        &self,
        tool_name: &str,
        raw_output: String,
    ) -> Result<(String, Option<ToolArtifactRecord>, Option<ToolOutputMeta>), ToolRunnerError> {
        const INLINE_LIMIT: usize = 50_000;
        if raw_output.len() <= INLINE_LIMIT {
            return self
                .artifacts
                .store_text(tool_name, raw_output)
                .map_err(ToolRunnerError::Tool);
        }
        let tool_name = tool_name.to_string();
        let artifacts = self.artifacts.clone();
        tokio::task::spawn_blocking(move || artifacts.store_text(&tool_name, raw_output))
            .await
            .map_err(|error| ToolRunnerError::BlockingTask(error.to_string()))?
            .map_err(ToolRunnerError::Tool)
    }

    pub fn denied(&self, call: ToolCall, reason: impl Into<String>) -> ToolExecutionRecord {
        self.failed_record(call, reason, Vec::new(), None)
    }

    pub(super) fn failed_record(
        &self,
        call: ToolCall,
        reason: impl Into<String>,
        file_changes: Vec<FileChangeRecord>,
        edit_batch: Option<EditBatch>,
    ) -> ToolExecutionRecord {
        ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content: reason.into(),
                is_error: true,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
            artifact: None,
            file_changes,
            edit_batch,
        }
    }
}

#[path = "dispatch.rs"]
mod dispatch;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::LspSettings as PersistedLspSettings;

    fn runner(root: &Path) -> ToolRunner {
        let registry = ToolRegistry::new();
        let artifacts = ArtifactStore::new(root.join("artifacts")).unwrap();
        ToolRunner::new(
            registry,
            root.to_path_buf(),
            artifacts,
            CancellationToken::new(),
            Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new()),
        )
    }

    fn ctx(node: &str, conversation: &str) -> Option<ToolExecutionContext> {
        Some(ToolExecutionContext {
            node_id: engine::NodeId(node.to_string()),
            conversation_id: conversation.to_string(),
            lsp: PersistedLspSettings::default(),
        })
    }

    #[tokio::test]
    async fn read_file_selector_returns_numbered_lines() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "a\nb\nc\n").unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(
                ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "note.txt:2-3"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("2:b"));
        assert!(record.result.content.contains("3:c"));
    }

    #[tokio::test]
    async fn read_file_without_selector_announces_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let lines = (1..=305)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.path().join("big.txt"), lines).unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(
                ToolCall {
                    id: "call-trunc".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "big.txt"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("300:line-300"));
        assert!(!record.result.content.contains("301:line-301"));
        assert!(record
            .result
            .content
            .contains("truncated at line 300 of 305"));
        assert!(record
            .result
            .content
            .contains("use :{start}-{end} or :raw to read more"));
    }

    #[tokio::test]
    async fn search_finds_matching_lines() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\nbeta\n").unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(
                ToolCall {
                    id: "call-2".to_string(),
                    name: "search".to_string(),
                    arguments: serde_json::json!({"pattern": "beta", "paths": "note.txt"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("note.txt:2:beta"));
    }

    #[tokio::test]
    async fn write_creates_file_under_execution_folder() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(
                ToolCall {
                    id: "call-write".to_string(),
                    name: "write".to_string(),
                    arguments: serde_json::json!({"path": "new.txt", "content": "hello\n"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("Created new.txt"));
        assert_eq!(
            fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "hello\n"
        );
        assert_eq!(record.file_changes.len(), 1);
        assert_eq!(record.file_changes[0].path, "new.txt");
        assert_eq!(record.file_changes[0].op, engine::FileChangeOp::Create);
        assert!(record.file_changes[0].diff_summary.is_some());
    }

    #[tokio::test]
    async fn edit_replaces_text_in_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\nbeta\n").unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(
                ToolCall {
                    id: "call-edit".to_string(),
                    name: "edit".to_string(),
                    arguments: serde_json::json!({
                        "path": "note.txt",
                        "edits": [{"old_text": "beta", "new_text": "gamma"}]
                    }),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("Updated note.txt"));
        assert_eq!(
            fs::read_to_string(dir.path().join("note.txt")).unwrap(),
            "alpha\ngamma\n"
        );
        assert_eq!(record.file_changes.len(), 1);
        assert_eq!(record.file_changes[0].path, "note.txt");
        assert_eq!(record.file_changes[0].op, engine::FileChangeOp::Update);
    }

    #[tokio::test]
    async fn write_rejects_path_outside_execution_folder() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runner(dir.path());
        let error = runner
            .execute(
                ToolCall {
                    id: "call-escape".to_string(),
                    name: "write".to_string(),
                    arguments: serde_json::json!({"path": "../escape.txt", "content": "nope"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("path escapes execution folder"));
    }

    #[tokio::test]
    async fn write_rejects_no_op_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        let error = runner
            .execute(
                ToolCall {
                    id: "call-noop".to_string(),
                    name: "write".to_string(),
                    arguments: serde_json::json!({"path": "note.txt", "content": "alpha\n"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("No changes would be made"));
        assert_eq!(
            fs::read_to_string(dir.path().join("note.txt")).unwrap(),
            "alpha\n"
        );
    }

    #[tokio::test]
    async fn edit_rejects_path_outside_execution_folder() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        let error = runner
            .execute(
                ToolCall {
                    id: "call-edit-escape".to_string(),
                    name: "edit".to_string(),
                    arguments: serde_json::json!({
                        "path": "../escape.txt",
                        "edits": [{"old_text": "alpha", "new_text": "beta"}]
                    }),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("path escapes execution folder"));
    }

    #[tokio::test]
    async fn apply_patch_creates_file_under_execution_folder() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runner(dir.path());
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+hello\n*** End Patch\n";
        let record = runner
            .execute(
                ToolCall {
                    id: "call-patch".to_string(),
                    name: "apply_patch".to_string(),
                    arguments: serde_json::json!({"input": patch}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("Created new.txt"));
        assert_eq!(
            fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "hello\n"
        );
        assert_eq!(record.file_changes.len(), 1);
        assert_eq!(record.file_changes[0].path, "new.txt");
        assert_eq!(record.file_changes[0].op, engine::FileChangeOp::Create);
        assert!(record.file_changes[0].diff_summary.is_some());
    }

    fn read_call(id: &str, path: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: "read".to_string(),
            arguments: serde_json::json!({ "path": path }),
            intent: None,
        }
    }

    #[tokio::test]
    async fn repeated_read_across_nodes_serves_cached_content_with_provenance() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\nbeta\n").unwrap();
        let runner = runner(dir.path());
        let first = runner
            .execute(read_call("call-1", "note.txt"), ctx("recon", "recon"))
            .await
            .unwrap();
        assert!(!first.result.content.contains("[cached]"));
        let second = runner
            .execute(read_call("call-2", "note.txt"), ctx("plan", "plan"))
            .await
            .unwrap();
        assert!(second
            .result
            .content
            .starts_with("[cached] Unchanged since node 'recon'"));
        assert!(second.result.content.contains("1:alpha"));
        assert_eq!(second.result.tool_call_id, "call-2");
    }

    #[tokio::test]
    async fn repeated_read_in_same_conversation_omits_content() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        runner
            .execute(read_call("call-1", "note.txt"), ctx("recon", "recon"))
            .await
            .unwrap();
        let second = runner
            .execute(read_call("call-2", "note.txt"), ctx("recon", "recon"))
            .await
            .unwrap();
        assert!(second.result.content.contains("content omitted"));
        assert!(second.result.content.contains("call-1"));
        assert!(!second.result.content.contains("alpha"));
    }

    #[tokio::test]
    async fn read_cache_invalidated_when_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        runner
            .execute(read_call("call-1", "note.txt"), ctx("recon", "recon"))
            .await
            .unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\ngamma\n").unwrap();
        let second = runner
            .execute(read_call("call-2", "note.txt"), ctx("plan", "plan"))
            .await
            .unwrap();
        assert!(!second.result.content.contains("[cached]"));
        assert!(second.result.content.contains("2:gamma"));
    }

    #[tokio::test]
    async fn search_cache_invalidated_by_write_tool() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        let search = |id: &str| ToolCall {
            id: id.to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"pattern": "alpha", "paths": "."}),
            intent: None,
        };
        runner
            .execute(search("call-1"), ctx("recon", "recon"))
            .await
            .unwrap();
        let hit = runner
            .execute(search("call-2"), ctx("plan", "plan"))
            .await
            .unwrap();
        assert!(hit.result.content.contains("[cached]"));
        runner
            .execute(
                ToolCall {
                    id: "call-write".to_string(),
                    name: "write".to_string(),
                    arguments: serde_json::json!({"path": "other.txt", "content": "alpha too\n"}),
                    intent: None,
                },
                ctx("implement", "implement"),
            )
            .await
            .unwrap();
        let after_write = runner
            .execute(search("call-3"), ctx("verify", "verify"))
            .await
            .unwrap();
        assert!(!after_write.result.content.contains("[cached]"));
        assert!(after_write.result.content.contains("other.txt"));
    }

    #[tokio::test]
    async fn bump_cache_epoch_invalidates_search_entries() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        let search = |id: &str| ToolCall {
            id: id.to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"pattern": "alpha", "paths": "."}),
            intent: None,
        };
        runner
            .execute(search("call-1"), ctx("recon", "recon"))
            .await
            .unwrap();
        runner.bump_cache_epoch();
        let after_bump = runner
            .execute(search("call-2"), ctx("plan", "plan"))
            .await
            .unwrap();
        assert!(!after_bump.result.content.contains("[cached]"));
    }

    #[tokio::test]
    async fn execute_without_context_bypasses_cache() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        runner
            .execute(read_call("call-1", "note.txt"), None)
            .await
            .unwrap();
        let second = runner
            .execute(read_call("call-2", "note.txt"), None)
            .await
            .unwrap();
        assert!(!second.result.content.contains("[cached]"));
    }

    #[tokio::test]
    async fn partial_apply_patch_returns_file_changes_without_ledger_leak() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runner(dir.path());
        let patch = "*** Begin Patch\n*** Add File: good.txt\n+hello\n*** Update File: missing.txt\n@ old\n-old\n+new\n*** End Patch\n";
        let record = runner
            .execute(
                ToolCall {
                    id: "call-partial".to_string(),
                    name: "apply_patch".to_string(),
                    arguments: serde_json::json!({"input": patch}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.is_error);
        assert!(record.result.content.contains("Created good.txt"));
        assert_eq!(record.file_changes.len(), 1);
        assert_eq!(record.file_changes[0].path, "good.txt");
        assert_eq!(
            fs::read_to_string(dir.path().join("good.txt")).unwrap(),
            "hello\n"
        );

        let write_record = runner
            .execute(
                ToolCall {
                    id: "call-after-partial".to_string(),
                    name: "write".to_string(),
                    arguments: serde_json::json!({"path": "after.txt", "content": "ok\n"}),
                    intent: None,
                },
                None,
            )
            .await
            .unwrap();
        assert!(!write_record.result.is_error);
        assert_eq!(write_record.file_changes.len(), 1);
        assert_eq!(write_record.file_changes[0].path, "after.txt");
    }
}
