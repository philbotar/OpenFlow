use crate::settings::model::LspSettings;
use crate::tool::blocking_ops::split_selector;
pub(super) use crate::tool::blocking_ops::{
    apply_read_selector, BlockingBatchContext, BlockingRunOutcome, BlockingToolOps,
};
use crate::tool::cache::{cache_key, CacheEntry, CacheValidation, ToolResultCache};
use crate::tool::errors::ToolError;
use crate::tool::output::{ArtifactStore, ToolArtifactRecord};
use crate::tool::registry::{BuiltinToolKind, ToolRegistry, ToolRegistryError};
use engine::{EditBatch, FileChangeRecord, ReadRecord, ToolCall, ToolOutputMeta, ToolResult};
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
    pub reads: Vec<ReadRecord>,
    pub edit_batch: Option<EditBatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionUpdate {
    pub content: String,
    pub output_meta: Option<ToolOutputMeta>,
}

#[derive(Clone)]
pub struct ToolExecutionContext {
    pub node_id: engine::NodeId,
    /// Identifies the transcript receiving the result: the node id for node
    /// turns, a unique session id for subagent invocations. Used by the
    /// result cache to decide whether a repeated call can be answered with a
    /// content-omitting stub (the transcript already holds the content).
    pub conversation_id: String,
    /// LSP format-on-write and diagnostics for edit tools in this invocation.
    pub lsp: LspSettings,
    pub update_tx: Option<tokio::sync::mpsc::UnboundedSender<ToolExecutionUpdate>>,
}

impl std::fmt::Debug for ToolExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolExecutionContext")
            .field("node_id", &self.node_id)
            .field("conversation_id", &self.conversation_id)
            .field("lsp", &self.lsp)
            .field("update_tx", &self.update_tx.is_some())
            .finish()
    }
}

#[derive(Debug)]
pub struct ToolRunner {
    registry: ToolRegistry,
    mcp_clients: Option<crate::adapters::mcp::McpRunClients>,
    pub(super) http: Client,
    pub(super) cwd: PathBuf,
    artifacts: ArtifactStore,
    pub(super) cancel_token: CancellationToken,
    pub(super) snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    cache: ToolResultCache,
    pub(super) search: crate::settings::model::SearchSettings,
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
    #[error(transparent)]
    Mcp(#[from] crate::adapters::mcp::McpError),
}

impl ToolRunnerError {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Tool(error) => error.is_retryable(),
            Self::Registry(_) | Self::InvalidArguments(_) | Self::BlockingTask(_) => false,
            Self::Mcp(error) => matches!(error, crate::adapters::mcp::McpError::Transport(_)),
        }
    }
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
            mcp_clients: None,
            http: Client::new(),
            cwd,
            artifacts,
            cancel_token,
            snapshot_store,
            cache: ToolResultCache::new(),
            search: crate::settings::model::SearchSettings::default(),
        }
    }

    #[must_use]
    pub fn with_search_settings(mut self, search: crate::settings::model::SearchSettings) -> Self {
        self.search = search;
        self
    }

    #[must_use]
    pub fn with_mcp_clients(mut self, mcp_clients: crate::adapters::mcp::McpRunClients) -> Self {
        self.mcp_clients = Some(mcp_clients);
        self
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
            if let Some(mut record) = self.serve_cached(kind, &call, context) {
                self.enrich_read_record(kind, &call, &mut record);
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
        let mut result = result;
        if let Ok(record) = &mut result {
            self.enrich_read_record(kind, &call, record);
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

    fn enrich_read_record(
        &self,
        kind: BuiltinToolKind,
        call: &ToolCall,
        record: &mut ToolExecutionRecord,
    ) {
        if kind != BuiltinToolKind::Read || !record.reads.is_empty() {
            return;
        }
        let Some(path) = self.read_call_path(call) else {
            return;
        };
        if let Some(read_record) = crate::tool::read::capture::capture_read_record(&self.cwd, path)
        {
            record.reads.push(read_record);
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
            let lines: Vec<&str> = hit.content.lines().collect();
            let head = lines.first().copied().unwrap_or("(empty)");
            let line_count = lines.len();
            (
                format!(
                    "[cached] Identical to the result of tool call {} earlier in this conversation; inputs unchanged. Refer back to that result — do not repeat this call. First line: \"{head}\" ({line_count} lines total).",
                    hit.tool_call_id
                ),
                Vec::new(),
                None,
            )
        } else {
            (
                format!(
                    "[cached] Unchanged since node '{}' ran this exact `{}` call earlier in the run; content below is still valid.\n\n{}",
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
            reads: Vec::new(),
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
            reads: Vec::new(),
            edit_batch: None,
        })
    }

    fn is_artifact_read(call: &ToolCall) -> bool {
        call.name == "read"
            && call
                .arguments
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| split_selector(path).0.starts_with("artifact:"))
    }

    pub(super) async fn finalize_record(
        &self,
        call: ToolCall,
        raw_output: String,
        file_changes: Vec<FileChangeRecord>,
        edit_batch: Option<EditBatch>,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let (content, artifact, output_meta) = if Self::is_artifact_read(&call) {
            (raw_output, None, None)
        } else {
            self.store_output_text(&call.name, raw_output).await?
        };
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
            reads: Vec::new(),
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
            reads: Vec::new(),
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
    use crate::tool::registry::{BuiltinToolKind, RegisteredTool};

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
            update_tx: None,
        })
    }

    #[cfg_attr(miri, ignore)]
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
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("2:b"));
        assert!(record.result.content.contains("3:c"));
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn read_file_without_selector_announces_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let lines = (1..=3005)
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
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("3000:line-3000"));
        assert!(!record.result.content.contains("3001:line-3001"));
        assert!(record
            .result
            .content
            .contains("truncated at line 3000 of 3005"));
        assert!(record
            .result
            .content
            .contains("use :{start}-{end} or :raw to read more"));
    }

    #[cfg_attr(miri, ignore)]
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
                },
                None,
            )
            .await
            .unwrap();
        assert!(record.result.content.contains("note.txt:2:beta"));
    }

    #[cfg_attr(miri, ignore)]
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

    #[cfg_attr(miri, ignore)]
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

    #[cfg_attr(miri, ignore)]
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
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("path escapes execution folder"));
    }

    #[cfg_attr(miri, ignore)]
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

    #[cfg_attr(miri, ignore)]
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
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("path escapes execution folder"));
    }

    #[cfg_attr(miri, ignore)]
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
        }
    }

    #[cfg_attr(miri, ignore)]
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

    #[cfg_attr(miri, ignore)]
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
        assert!(second.result.content.contains("do not repeat this call"));
        assert!(second.result.content.contains("call-1"));
        assert!(second.result.content.contains("First line:"));
        assert!(!second.result.content.contains("alpha"));
    }

    #[cfg_attr(miri, ignore)]
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

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn search_cache_invalidated_by_write_tool() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        let search = |id: &str| ToolCall {
            id: id.to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"pattern": "alpha", "paths": "."}),
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

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn bump_cache_epoch_invalidates_search_entries() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\n").unwrap();
        let runner = runner(dir.path());
        let search = |id: &str| ToolCall {
            id: id.to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"pattern": "alpha", "paths": "."}),
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

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn read_artifact_round_trip_and_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let runner = runner(dir.path());
        let big = "x".repeat(60_000);
        let spilled = runner.artifacts().store_text("bash", big.clone()).unwrap();
        let artifact_id = spilled.1.expect("artifact record").artifact_id.clone();

        let full = runner
            .execute(
                read_call("call-artifact", &format!("artifact:{artifact_id}:raw")),
                None,
            )
            .await
            .unwrap();
        assert_eq!(full.result.content, big);

        let slice = runner
            .execute(
                read_call("call-slice", &format!("artifact:{artifact_id}:1-1")),
                None,
            )
            .await
            .unwrap();
        assert!(slice.result.content.contains("1:"));

        let error = runner
            .execute(read_call("call-missing", "artifact:missing-id"), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("[not_found]"));
        assert!(error
            .to_string()
            .contains("artifacts only live for the current run"));
    }

    #[cfg_attr(miri, ignore)]
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

    #[cfg_attr(miri, ignore)]
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
                },
                None,
            )
            .await
            .unwrap();
        assert!(!write_record.result.is_error);
        assert_eq!(write_record.file_changes.len(), 1);
        assert_eq!(write_record.file_changes[0].path, "after.txt");
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn mcp_execute_without_clients_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = ToolRegistry::new();
        registry
            .extend_mcp(vec![RegisteredTool {
                definition: engine::ToolDefinition {
                    name: "mcp/test/echo".into(),
                    description: "echo".into(),
                    input_schema: serde_json::json!({"type":"object","properties":{}}),
                    tier: engine::ToolTier::Write,
                    concurrency: engine::ToolConcurrency::Shared,
                },
                kind: BuiltinToolKind::Mcp,
            }])
            .unwrap();
        let runner = ToolRunner::new(
            registry,
            dir.path().to_path_buf(),
            ArtifactStore::new(dir.path().join("artifacts")).unwrap(),
            CancellationToken::new(),
            Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new()),
        );
        let err = runner
            .execute(
                ToolCall {
                    id: "call-mcp".into(),
                    name: "mcp/test/echo".into(),
                    arguments: serde_json::json!({}),
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolRunnerError::Mcp(_)));
    }
}
