#[path = "grep.rs"]
mod grep;

use crate::lsp::LspSettings;
use crate::tool_errors::ToolError;
use crate::tool_output::{ArtifactStore, ToolArtifactRecord};
use crate::tool_registry::{BuiltinToolKind, ToolRegistry, ToolRegistryError};
use engine::{EditBatch, FileChangeRecord, ToolCall, ToolResult};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

static LINE_SELECTOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(?:-\d+)?$").expect("line selector regex is valid"));

const DEFAULT_READ_LINE_LIMIT: usize = 300;

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
}

#[derive(Debug)]
pub struct ToolRunner {
    registry: ToolRegistry,
    http: Client,
    cwd: PathBuf,
    artifacts: ArtifactStore,
    cancel_token: CancellationToken,
    snapshot_store: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    lsp: LspSettings,
}

struct BlockingRunOutcome {
    output: Result<String, ToolError>,
    file_changes: Vec<FileChangeRecord>,
    edit_batch: Option<EditBatch>,
}

struct BlockingBatchContext {
    node_id: String,
    tool_call_id: String,
    tool_name: String,
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
        lsp: LspSettings,
    ) -> Self {
        Self {
            registry,
            http: Client::new(),
            cwd,
            artifacts,
            cancel_token,
            snapshot_store,
            lsp,
        }
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
        let registered = self.registry.get(&call.name)?;
        match registered.kind {
            BuiltinToolKind::Read => {
                let raw = self.read(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None)
            }
            BuiltinToolKind::AstGrep => {
                let raw = self.ast_grep(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None)
            }
            BuiltinToolKind::Search
            | BuiltinToolKind::Find
            | BuiltinToolKind::Write
            | BuiltinToolKind::Edit
            | BuiltinToolKind::ApplyPatch => {
                let batch_ctx = ctx.map(|context| BlockingBatchContext {
                    node_id: context.node_id.0,
                    tool_call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                });
                let outcome = self
                    .run_blocking(registered.kind, call.arguments.clone(), batch_ctx)
                    .await?;
                match outcome.output {
                    Ok(raw) => {
                        self.finalize_record(call, raw, outcome.file_changes, outcome.edit_batch)
                    }
                    Err(error)
                        if outcome.file_changes.is_empty() && outcome.edit_batch.is_none() =>
                    {
                        Err(ToolRunnerError::Tool(error))
                    }
                    Err(error) => Ok(self.failed_record(
                        call,
                        error.to_string(),
                        outcome.file_changes,
                        outcome.edit_batch,
                    )),
                }
            }
            BuiltinToolKind::DeclareSubagents | BuiltinToolKind::CallSubagent => {
                Err(ToolRunnerError::InvalidArguments(format!(
                    "Tool '{}' is a runtime builtin and should not reach the filesystem runner",
                    call.name
                )))
            }
        }
    }

    async fn ast_grep(&self, args: Value) -> Result<String, ToolRunnerError> {
        #[derive(Deserialize)]
        struct AstGrepArgs {
            pat: String,
            paths: Vec<String>,
        }
        let args: AstGrepArgs = serde_json::from_value(args).map_err(|error| {
            ToolRunnerError::Tool(ToolError::Failed(format!("invalid ast_grep args: {error}")))
        })?;
        let mut command = tokio::process::Command::new("ast-grep");
        command
            .arg("scan")
            .arg("--pattern")
            .arg(&args.pat)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for path in &args.paths {
            command.arg(path);
        }
        let mut child = command.spawn().map_err(|error| {
            ToolRunnerError::Tool(ToolError::Failed(format!("ast_grep failed: {error}")))
        })?;
        let mut stdout_pipe = child.stdout.take().ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::Failed("ast_grep stdout unavailable".to_string()))
        })?;
        let mut stderr_pipe = child.stderr.take().ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::Failed("ast_grep stderr unavailable".to_string()))
        })?;
        tokio::select! {
            biased;
            _ = self.cancel_token.cancelled() => {
                let _ = child.kill().await;
                Err(ToolRunnerError::Tool(ToolError::Failed(
                    "ast_grep cancelled".to_string(),
                )))
            }
            result = async {
                let mut stdout_bytes = Vec::new();
                let mut stderr_bytes = Vec::new();
                let (stdout_res, stderr_res, status) = tokio::join!(
                    tokio::io::AsyncReadExt::read_to_end(&mut stdout_pipe, &mut stdout_bytes),
                    tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut stderr_bytes),
                    child.wait(),
                );
                stdout_res.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::Failed(format!("ast_grep read failed: {error}")))
                })?;
                stderr_res.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::Failed(format!(
                        "ast_grep stderr read failed: {error}"
                    )))
                })?;
                let status = status.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::Failed(format!("ast_grep failed: {error}")))
                })?;
                if !status.success() {
                    return Err(ToolRunnerError::Tool(ToolError::Failed(
                        String::from_utf8_lossy(&stderr_bytes).trim().to_string(),
                    )));
                }
                Ok(String::from_utf8_lossy(&stdout_bytes).to_string())
            } => result,
        }
    }

    async fn run_blocking(
        &self,
        kind: BuiltinToolKind,
        args: Value,
        batch_ctx: Option<BlockingBatchContext>,
    ) -> Result<BlockingRunOutcome, ToolRunnerError> {
        let cwd = self.cwd.clone();
        let snapshots = self.snapshot_store.clone();
        let lsp = self.lsp.clone();
        tokio::task::spawn_blocking(move || {
            let edit_batch = batch_ctx.and_then(|context| {
                crate::tools::edit::batch::capture_edit_batch(
                    &cwd,
                    &context.node_id,
                    &context.tool_call_id,
                    &context.tool_name,
                    kind,
                    &args,
                )
            });
            let ledger = crate::tools::edit::ledger::FileChangeLedger::new();
            let ops = BlockingToolOps::new(cwd, ledger.clone(), snapshots, lsp);
            let output = match kind {
                BuiltinToolKind::Search => ops.search(args),
                BuiltinToolKind::Find => ops.find(args),
                BuiltinToolKind::Write => ops.write(args),
                BuiltinToolKind::Edit => ops.edit(args),
                BuiltinToolKind::ApplyPatch => ops.apply_patch(args),
                BuiltinToolKind::AstGrep => Err(ToolError::Failed(
                    "ast_grep must use async runner".to_string(),
                )),
                _ => Err(ToolError::Failed(
                    "blocking runner received a non-blocking tool".to_string(),
                )),
            };
            let mut file_changes = ledger.take();
            if let Some(ref batch) = edit_batch {
                for change in &mut file_changes {
                    change.batch_id = Some(batch.batch_id.clone());
                }
            }
            BlockingRunOutcome {
                output,
                file_changes,
                edit_batch,
            }
        })
        .await
        .map_err(|error| ToolRunnerError::BlockingTask(error.to_string()))
    }

    fn finalize_record(
        &self,
        call: ToolCall,
        raw_output: String,
        file_changes: Vec<FileChangeRecord>,
        edit_batch: Option<EditBatch>,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let (content, artifact, output_meta) = self.artifacts.store_text(&call.name, raw_output)?;
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

    pub fn denied(&self, call: ToolCall, reason: impl Into<String>) -> ToolExecutionRecord {
        self.failed_record(call, reason, Vec::new(), None)
    }

    fn failed_record(
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

    async fn read(&self, args: Value) -> Result<String, ToolRunnerError> {
        #[derive(Deserialize)]
        struct ReadArgs {
            path: String,
        }
        let args: ReadArgs = serde_json::from_value(args).map_err(|error| {
            ToolRunnerError::Tool(ToolError::Failed(format!("invalid read args: {error}")))
        })?;
        if args.path.starts_with("http://") || args.path.starts_with("https://") {
            return self
                .read_url(&args.path)
                .await
                .map_err(ToolRunnerError::from);
        }

        let cwd = self.cwd.clone();
        let snapshots = self.snapshot_store.clone();
        let lsp = self.lsp.clone();
        tokio::task::spawn_blocking(move || {
            BlockingToolOps::new(
                cwd,
                crate::tools::edit::ledger::FileChangeLedger::new(),
                snapshots,
                lsp,
            )
            .read_local(&args.path)
        })
        .await
        .map_err(|error| ToolRunnerError::BlockingTask(error.to_string()))?
        .map_err(ToolRunnerError::from)
    }

    async fn read_url(&self, url: &str) -> Result<String, ToolError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|error| ToolError::Failed(format!("read failed for {url}: {error}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| ToolError::Failed(format!("read failed for {url}: {error}")))?;
        if !status.is_success() {
            return Err(ToolError::Failed(format!(
                "read failed for {url}: HTTP {status}"
            )));
        }
        Ok(apply_read_selector(url, &text, None))
    }
}

struct BlockingToolOps {
    cwd: PathBuf,
    ledger: crate::tools::edit::ledger::FileChangeLedger,
    snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    lsp: LspSettings,
}

impl BlockingToolOps {
    fn new(
        cwd: PathBuf,
        ledger: crate::tools::edit::ledger::FileChangeLedger,
        snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
        lsp: LspSettings,
    ) -> Self {
        Self {
            cwd,
            ledger,
            snapshots,
            lsp,
        }
    }

    fn read_local(&self, path: &str) -> Result<String, ToolError> {
        let (path, selector) = split_selector(path);
        let absolute = self.resolve_local(&path);
        let metadata = fs::metadata(&absolute)
            .map_err(|error| ToolError::Failed(format!("read failed for {}: {error}", path)))?;
        if metadata.is_dir() {
            return self.read_directory(&absolute);
        }
        let text = fs::read_to_string(&absolute)
            .map_err(|error| ToolError::Failed(format!("read failed for {}: {error}", path)))?;
        if let Ok(canonical) =
            crate::tools::edit::file_snapshot_store::canonical_snapshot_path(&self.cwd, &path)
        {
            let _ = crate::tools::edit::file_snapshot_store::record_file_snapshot(
                self.snapshots.as_ref(),
                &canonical,
                &text,
            );
        }
        Ok(apply_read_selector(&path, &text, selector.as_deref()))
    }

    fn read_directory(&self, path: &Path) -> Result<String, ToolError> {
        let mut entries = fs::read_dir(path)
            .map_err(|error| {
                ToolError::Failed(format!("read failed for {}: {error}", path.display()))
            })?
            .filter_map(Result::ok)
            .map(|entry| {
                let file_type = entry.file_type().ok();
                let mut name = entry.file_name().to_string_lossy().to_string();
                if file_type.as_ref().is_some_and(|kind| kind.is_dir()) {
                    name.push('/');
                }
                name
            })
            .collect::<Vec<_>>();
        entries.sort();
        Ok(entries.into_iter().take(200).collect::<Vec<_>>().join("\n"))
    }

    fn search(&self, args: Value) -> Result<String, ToolError> {
        grep::search(&self.cwd, args)
    }

    fn write(&self, args: Value) -> Result<String, ToolError> {
        crate::tools::edit::write::execute_write(
            self.cwd.clone(),
            args,
            self.ledger.clone(),
            self.lsp.clone(),
        )
    }

    fn edit(&self, args: Value) -> Result<String, ToolError> {
        crate::tools::edit::edit_tool::execute_edit(
            self.cwd.clone(),
            args,
            self.ledger.clone(),
            self.snapshots.clone(),
            self.lsp.clone(),
        )
    }

    fn apply_patch(&self, args: Value) -> Result<String, ToolError> {
        crate::tools::edit::apply_patch_tool::execute_apply_patch(
            self.cwd.clone(),
            args,
            self.ledger.clone(),
            self.lsp.clone(),
        )
    }

    fn find(&self, args: Value) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        struct FindArgs {
            paths: StringOrMany,
        }
        let args: FindArgs = serde_json::from_value(args)
            .map_err(|error| ToolError::Failed(format!("invalid find args: {error}")))?;
        let mut matches = Vec::new();
        for pattern in args.paths.into_vec() {
            for entry in self.expand_paths(&pattern)? {
                matches.push(entry.display().to_string());
            }
        }
        matches.sort();
        matches.dedup();
        Ok(matches.into_iter().take(200).collect::<Vec<_>>().join("\n"))
    }

    fn resolve_local(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }

    fn expand_paths(&self, pattern: &str) -> Result<Vec<PathBuf>, ToolError> {
        let absolute = self.resolve_local(pattern);
        if absolute.exists() {
            if absolute.is_dir() {
                return Ok(WalkDir::new(&absolute)
                    .into_iter()
                    .filter_map(Result::ok)
                    .map(|entry| entry.path().to_path_buf())
                    .collect());
            }
            return Ok(vec![absolute]);
        }

        let pattern = self.cwd.join(pattern).display().to_string();
        let mut matches = Vec::new();
        for entry in glob::glob(&pattern)
            .map_err(|error| ToolError::Failed(format!("invalid glob pattern: {error}")))?
        {
            let path = entry.map_err(|error| ToolError::Failed(format!("glob failed: {error}")))?;
            if path.is_dir() {
                matches.extend(
                    WalkDir::new(&path)
                        .into_iter()
                        .filter_map(Result::ok)
                        .map(|entry| entry.path().to_path_buf()),
                );
            } else {
                matches.push(path);
            }
        }
        Ok(matches)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrMany {
    One(String),
    Many(Vec<String>),
}

impl StringOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

fn split_selector(path: &str) -> (String, Option<String>) {
    if let Some(index) = path.rfind(':') {
        let suffix = &path[index + 1..];
        if suffix == "raw" || LINE_SELECTOR.is_match(suffix) {
            return (path[..index].to_string(), Some(suffix.to_string()));
        }
    }
    (path.to_string(), None)
}

fn apply_read_selector(label: &str, text: &str, selector: Option<&str>) -> String {
    match selector {
        Some("raw") => text.to_string(),
        Some(range) => {
            let (start, end) = parse_range(range);
            let lines = text.lines().collect::<Vec<_>>();
            let start_index = start.saturating_sub(1);
            let end_index = end.min(lines.len());
            let slice = lines[start_index.min(lines.len())..end_index]
                .iter()
                .enumerate()
                .map(|(offset, line)| format!("{}:{}", start_index + offset + 1, line))
                .collect::<Vec<_>>();
            format!("¶{label}\n{}", slice.join("\n"))
        }
        None => {
            let all_lines: Vec<_> = text.lines().collect();
            let total_lines = all_lines.len();
            let shown = all_lines
                .iter()
                .take(DEFAULT_READ_LINE_LIMIT)
                .enumerate()
                .map(|(index, line)| format!("{}:{}", index + 1, line))
                .collect::<Vec<_>>();
            let mut output = format!("¶{label}\n{}", shown.join("\n"));
            if total_lines > DEFAULT_READ_LINE_LIMIT {
                output.push_str(&format!(
                    "\n… truncated at line {DEFAULT_READ_LINE_LIMIT} of {total_lines}; use :{{start}}-{{end}} or :raw to read more …"
                ));
            }
            output
        }
    }
}

fn parse_range(range: &str) -> (usize, usize) {
    if let Some((start, end)) = range.split_once('-') {
        let start = start.parse::<usize>().unwrap_or(1);
        let end = end.parse::<usize>().unwrap_or(start);
        (start.max(1), end.max(start))
    } else {
        let value = range.parse::<usize>().unwrap_or(1);
        (value.max(1), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(root: &Path) -> ToolRunner {
        let registry = ToolRegistry::new();
        let artifacts = ArtifactStore::new(root.join("artifacts")).unwrap();
        ToolRunner::new(
            registry,
            root.to_path_buf(),
            artifacts,
            CancellationToken::new(),
            Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new()),
            LspSettings::default(),
        )
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
