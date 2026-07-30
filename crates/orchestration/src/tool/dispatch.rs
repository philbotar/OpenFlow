//! Per-kind builtin tool dispatch for [`super::ToolRunner`].

use super::{
    apply_read_selector, BlockingBatchContext, BlockingRunOutcome, BlockingToolOps, LspSettings,
    ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError,
};
use crate::tool::blocking_ops::split_selector;
use crate::tool::errors::ToolError;
use crate::tool::read::selector::ReadSelector;
use crate::tool::registry::BuiltinToolKind;
use engine::{ToolCall, ToolResult};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;

enum ReadTarget {
    Artifact {
        artifact_id: String,
        selector: ReadSelector,
    },
    Handoff {
        uri: String,
        selector: ReadSelector,
    },
    PlanDraft {
        selector: ReadSelector,
    },
    Url {
        url: String,
        selector: ReadSelector,
    },
    Local {
        path: String,
    },
}

fn parse_read_target(path: &str) -> ReadTarget {
    let (base, selector) = split_selector(path);
    if base == engine::PLAN_DRAFT_PATH {
        return ReadTarget::PlanDraft { selector };
    }
    if let Some(artifact_id) = base.strip_prefix("artifact:") {
        return ReadTarget::Artifact {
            artifact_id: artifact_id.to_string(),
            selector,
        };
    }
    if base.starts_with("run://handoffs/") {
        return ReadTarget::Handoff {
            uri: base,
            selector,
        };
    }
    if base.starts_with("http://") || base.starts_with("https://") {
        return ReadTarget::Url {
            url: base,
            selector,
        };
    }
    ReadTarget::Local {
        path: path.to_string(),
    }
}

fn map_http_status_error(url: &str, status: StatusCode) -> ToolError {
    if matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE) {
        ToolError::NotFound {
            what: format!("read failed for {url}: HTTP {status}"),
            hint: "check the URL is reachable and returns 2xx".to_string(),
        }
    } else {
        ToolError::failed(format!("read failed for {url}: HTTP {status}"))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Deserialize)]
struct TodoItemArgs {
    content: String,
    status: TodoStatus,
}

#[derive(Debug, Deserialize)]
struct UpdateTodoListArgs {
    todos: Vec<TodoItemArgs>,
}

fn update_todo_list(args: Value) -> Result<String, ToolError> {
    let args: UpdateTodoListArgs = serde_json::from_value(args).map_err(|error| {
        ToolError::InvalidArgs {
            tool: "openflow_update_todo_list".to_string(),
            problem: error.to_string(),
            hint: "required field: todos (1-12 items with content and pending, in_progress, or completed status)".to_string(),
        }
    })?;
    if !(1..=12).contains(&args.todos.len()) {
        return Err(ToolError::InvalidArgs {
            tool: "openflow_update_todo_list".to_string(),
            problem: format!("todos must contain 1-12 items, got {}", args.todos.len()),
            hint: "send the complete current phase checklist with 1-12 items".to_string(),
        });
    }

    let mut completed = 0;
    let mut in_progress = 0;
    let mut current = None;
    for (index, todo) in args.todos.iter().enumerate() {
        let content = todo.content.trim();
        if content.is_empty() || content.chars().count() > 160 {
            return Err(ToolError::InvalidArgs {
                tool: "openflow_update_todo_list".to_string(),
                problem: format!(
                    "todos[{index}].content must contain 1-160 non-whitespace characters"
                ),
                hint: "use a short action-oriented phase label".to_string(),
            });
        }
        match todo.status {
            TodoStatus::Pending => {}
            TodoStatus::InProgress => {
                in_progress += 1;
                current = Some(content);
            }
            TodoStatus::Completed => completed += 1,
        }
    }
    if in_progress > 1 {
        return Err(ToolError::InvalidArgs {
            tool: "openflow_update_todo_list".to_string(),
            problem: "only one todo may be in_progress".to_string(),
            hint: "mark other unfinished phases pending".to_string(),
        });
    }

    let mut result = format!(
        "Checklist updated: {completed}/{} completed.",
        args.todos.len()
    );
    if let Some(content) = current {
        result.push_str(&format!(" In progress: {content}"));
    }
    Ok(result)
}

impl ToolRunner {
    pub(super) async fn dispatch(
        &self,
        kind: BuiltinToolKind,
        call: ToolCall,
        ctx: Option<ToolExecutionContext>,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        if matches!(kind, BuiltinToolKind::Write | BuiltinToolKind::Edit)
            && call
                .arguments
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| path == engine::PLAN_DRAFT_PATH)
        {
            return self.run_plan_draft_mutation(kind, call).await;
        }
        match kind {
            BuiltinToolKind::Read => {
                let raw = self.read(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None).await
            }
            BuiltinToolKind::AstGrep => {
                let raw = self.ast_grep(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None).await
            }
            BuiltinToolKind::AstEdit => {
                let lsp = ctx
                    .as_ref()
                    .map(|context| context.lsp.clone())
                    .unwrap_or_default();
                let outcome = crate::tools::ast_edit::execute_ast_edit(
                    &self.cwd,
                    call.arguments.clone(),
                    &self.cancel_token,
                    lsp,
                )
                .await;
                match outcome.output {
                    Ok(raw) => {
                        self.finalize_record(call, raw, outcome.file_changes, None)
                            .await
                    }
                    Err(error) if outcome.file_changes.is_empty() => {
                        Err(ToolRunnerError::Tool(error))
                    }
                    Err(error) => {
                        Ok(self.failed_record(call, error.to_string(), outcome.file_changes, None))
                    }
                }
            }
            BuiltinToolKind::Bash => {
                let update_tx = ctx.as_ref().and_then(|context| context.update_tx.clone());
                let outcome = crate::tools::bash::execute_bash(
                    &self.cwd,
                    call.arguments.clone(),
                    &self.cancel_token,
                    update_tx,
                )
                .await?;
                self.finalize_bash_record(call, outcome).await
            }
            BuiltinToolKind::WebSearch => {
                let raw = self.web_search(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None).await
            }
            BuiltinToolKind::UpdateTodoList => {
                let raw = update_todo_list(call.arguments.clone())?;
                self.finalize_record(call, raw, Vec::new(), None).await
            }
            BuiltinToolKind::WritePlanArtifact => self.write_plan_artifact(call),
            BuiltinToolKind::Search
            | BuiltinToolKind::Find
            | BuiltinToolKind::Write
            | BuiltinToolKind::Edit
            | BuiltinToolKind::ApplyPatch => {
                let lsp = ctx
                    .as_ref()
                    .map(|context| context.lsp.clone())
                    .unwrap_or_default();
                let batch_ctx = ctx.map(|context| BlockingBatchContext {
                    node_id: context.node_id.0,
                    tool_call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                });
                let outcome = self
                    .run_blocking(kind, call.arguments.clone(), batch_ctx, lsp)
                    .await?;
                match outcome.output {
                    Ok(raw) => {
                        self.finalize_record(call, raw, outcome.file_changes, outcome.edit_batch)
                            .await
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
            BuiltinToolKind::Mcp => {
                let clients = self.mcp_clients.as_ref().ok_or_else(|| {
                    ToolRunnerError::Mcp(crate::adapters::mcp::McpError::ServerNotConnected {
                        server_id: call.name.clone(),
                    })
                })?;
                let outcome = clients
                    .call_namespaced(&call.name, call.arguments.clone())
                    .await?;
                self.finalize_record_with_status(
                    call,
                    outcome.content,
                    outcome.is_error,
                    Vec::new(),
                    None,
                )
                .await
            }
        }
    }

    pub(super) async fn run_blocking(
        &self,
        kind: BuiltinToolKind,
        args: Value,
        batch_ctx: Option<BlockingBatchContext>,
        lsp: LspSettings,
    ) -> Result<BlockingRunOutcome, ToolRunnerError> {
        let cwd = self.cwd.clone();
        let snapshots = self.snapshot_store.clone();
        tokio::task::spawn_blocking(move || {
            BlockingToolOps::run_blocking(cwd, snapshots, lsp, kind, args, batch_ctx)
        })
        .await
        .map_err(|error| ToolRunnerError::BlockingTask(error.to_string()))
    }

    async fn read(&self, args: Value) -> Result<String, ToolRunnerError> {
        #[derive(Deserialize)]
        struct ReadArgs {
            path: String,
        }
        let args: ReadArgs = serde_json::from_value(args).map_err(|error| {
            ToolRunnerError::Tool(ToolError::InvalidArgs {
                tool: "read".to_string(),
                problem: error.to_string(),
                hint:
                    "required field: path (string); supports local paths, URLs, artifact:{id}, and run://handoffs/...".to_string(),
            })
        })?;
        match parse_read_target(&args.path) {
            ReadTarget::Artifact {
                artifact_id,
                selector,
            } => self.read_artifact(&artifact_id, selector, &args.path),
            ReadTarget::Handoff { uri, selector } => self.read_handoff(&uri, selector),
            ReadTarget::PlanDraft { selector } => self.read_plan_draft(selector),
            ReadTarget::Url { url, selector } => self
                .read_url(&url, selector)
                .await
                .map_err(ToolRunnerError::from),
            ReadTarget::Local { path } => {
                let cwd = self.cwd.clone();
                let snapshots = self.snapshot_store.clone();
                tokio::task::spawn_blocking(move || {
                    BlockingToolOps::read_local_at(cwd, snapshots, &path)
                })
                .await
                .map_err(|error| ToolRunnerError::BlockingTask(error.to_string()))?
                .map_err(ToolRunnerError::from)
            }
        }
    }

    fn write_plan_artifact(&self, call: ToolCall) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let artifact = self
            .artifacts
            .seal_plan_draft()
            .map_err(ToolRunnerError::Tool)?;
        let content = format!(
            "artifact:{}\nsha256:{}\nbytes:{}",
            artifact.record.artifact_id, artifact.sha256, artifact.record.size_bytes
        );
        Ok(ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content,
                is_error: false,
                artifact_ids: vec![artifact.record.artifact_id.clone()],
                output_meta: None,
            },
            artifact: Some(artifact.record),
            file_changes: Vec::new(),
            reads: Vec::new(),
            edit_batch: None,
        })
    }

    async fn run_plan_draft_mutation(
        &self,
        kind: BuiltinToolKind,
        call: ToolCall,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let draft_path = self
            .artifacts
            .plan_draft_path()
            .map_err(ToolRunnerError::Tool)?;
        let internal_name = draft_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ToolRunnerError::Tool(ToolError::failed(
                    "plan draft path is not valid UTF-8".to_string(),
                ))
            })?;
        let mut arguments = call.arguments.clone();
        arguments["path"] = Value::String(internal_name.to_string());
        let cwd = self.artifacts.root().to_path_buf();
        let snapshots = self.snapshot_store.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            BlockingToolOps::run_blocking(
                cwd,
                snapshots,
                LspSettings::default(),
                kind,
                arguments,
                None,
            )
        })
        .await
        .map_err(|error| ToolRunnerError::BlockingTask(error.to_string()))?;
        let raw = outcome.output.map_err(ToolRunnerError::Tool)?;
        self.finalize_record(
            call,
            raw.replacen(internal_name, engine::PLAN_DRAFT_PATH, 1),
            Vec::new(),
            None,
        )
        .await
    }

    fn read_plan_draft(&self, selector: ReadSelector) -> Result<String, ToolRunnerError> {
        let path = self
            .artifacts
            .plan_draft_path()
            .map_err(ToolRunnerError::Tool)?;
        let markdown = std::fs::read_to_string(&path).map_err(|error| {
            ToolRunnerError::Tool(if error.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound {
                    what: format!("plan draft not found at {}", engine::PLAN_DRAFT_PATH),
                    hint: format!(
                        "create {} with write before reading it",
                        engine::PLAN_DRAFT_PATH
                    ),
                }
            } else {
                ToolError::failed(format!("failed to read plan draft: {error}"))
            })
        })?;
        Ok(apply_read_selector(
            engine::PLAN_DRAFT_PATH,
            &markdown,
            selector,
        ))
    }

    fn read_artifact(
        &self,
        artifact_id: &str,
        selector: ReadSelector,
        label: &str,
    ) -> Result<String, ToolRunnerError> {
        let artifact_path = self.artifacts.path_for(artifact_id).ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::NotFound {
                what: format!("artifact not found: {artifact_id}"),
                hint: "artifacts only live for the current run".to_string(),
            })
        })?;
        let text = std::fs::read_to_string(&artifact_path).map_err(|error| {
            ToolRunnerError::Tool(ToolError::failed(format!(
                "read failed for artifact:{artifact_id}: {error}"
            )))
        })?;
        Ok(apply_read_selector(label, &text, selector))
    }

    fn read_handoff(&self, uri: &str, selector: ReadSelector) -> Result<String, ToolRunnerError> {
        let path = crate::run::handoff::resolve_handoff_uri(self.artifacts.root(), uri)
            .map_err(|error| ToolRunnerError::Tool(ToolError::failed(error.to_string())))?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            ToolRunnerError::Tool(if error.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound {
                    what: format!("handoff not found: {uri}"),
                    hint: "handoffs only live for the current durable run".to_string(),
                }
            } else {
                ToolError::failed(format!("read failed for {uri}: {error}"))
            })
        })?;
        Ok(apply_read_selector(uri, &text, selector))
    }

    async fn read_url(&self, url: &str, selector: ReadSelector) -> Result<String, ToolError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|error| ToolError::failed(format!("read failed for {url}: {error}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| ToolError::failed(format!("read failed for {url}: {error}")))?;
        if !status.is_success() {
            return Err(map_http_status_error(url, status));
        }
        Ok(apply_read_selector(url, &text, selector))
    }

    async fn ast_grep(&self, args: Value) -> Result<String, ToolRunnerError> {
        #[derive(Deserialize)]
        struct AstGrepArgs {
            pat: String,
            paths: Vec<String>,
        }
        let args: AstGrepArgs = serde_json::from_value(args).map_err(|error| {
            ToolRunnerError::Tool(ToolError::InvalidArgs {
                tool: "ast_grep".to_string(),
                problem: error.to_string(),
                hint: "required fields: pat (string), paths (array of strings)".to_string(),
            })
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
            ToolRunnerError::Tool(ToolError::failed(format!("ast_grep failed: {error}")))
        })?;
        let mut stdout_pipe = child.stdout.take().ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::failed("ast_grep stdout unavailable".to_string()))
        })?;
        let mut stderr_pipe = child.stderr.take().ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::failed("ast_grep stderr unavailable".to_string()))
        })?;
        tokio::select! {
            biased;
            _ = self.cancel_token.cancelled() => {
                let _ = child.kill().await;
                Err(ToolRunnerError::Tool(ToolError::Cancelled {
                    tool: "ast_grep".to_string(),
                }))
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
                    ToolRunnerError::Tool(ToolError::failed(format!("ast_grep read failed: {error}")))
                })?;
                stderr_res.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::failed(format!(
                        "ast_grep stderr read failed: {error}"
                    )))
                })?;
                let status = status.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::failed(format!("ast_grep failed: {error}")))
                })?;
                if !status.success() {
                    return Err(ToolRunnerError::Tool(ToolError::failed(
                        String::from_utf8_lossy(&stderr_bytes).trim().to_string(),
                    )));
                }
                Ok(String::from_utf8_lossy(&stdout_bytes).to_string())
            } => result,
        }
    }

    async fn web_search(&self, args: Value) -> Result<String, ToolRunnerError> {
        let args = crate::tool::web_search::parse_args(args).map_err(ToolRunnerError::Tool)?;
        let binary =
            crate::tool::web_search::resolve_binary(&self.search).map_err(ToolRunnerError::Tool)?;
        let mut command = tokio::process::Command::new(&binary);
        command
            .args(crate::tool::web_search::cli_args(&args))
            .envs(crate::tool::web_search::key_env_vars(&self.search))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            ToolRunnerError::Tool(ToolError::failed(format!("web_search failed: {error}")))
        })?;
        let mut stdout_pipe = child.stdout.take().ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::failed(
                "web_search stdout unavailable".to_string(),
            ))
        })?;
        let mut stderr_pipe = child.stderr.take().ok_or_else(|| {
            ToolRunnerError::Tool(ToolError::failed(
                "web_search stderr unavailable".to_string(),
            ))
        })?;
        tokio::select! {
            biased;
            _ = self.cancel_token.cancelled() => {
                let _ = child.kill().await;
                Err(ToolRunnerError::Tool(ToolError::Cancelled {
                    tool: "web_search".to_string(),
                }))
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
                    ToolRunnerError::Tool(ToolError::failed(format!(
                        "web_search read failed: {error}"
                    )))
                })?;
                stderr_res.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::failed(format!(
                        "web_search stderr read failed: {error}"
                    )))
                })?;
                let status = status.map_err(|error| {
                    ToolRunnerError::Tool(ToolError::failed(format!(
                        "web_search failed: {error}"
                    )))
                })?;
                if !status.success() {
                    return Err(ToolRunnerError::Tool(
                        crate::tool::web_search::map_exit_failure(
                            status.code(),
                            &String::from_utf8_lossy(&stderr_bytes),
                        ),
                    ));
                }
                Ok(String::from_utf8_lossy(&stdout_bytes).to_string())
            } => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_target_treats_url_selectors_as_url_plus_selector() {
        let target = parse_read_target("https://example.test/note.txt:2-3");
        assert!(matches!(
            target,
            ReadTarget::Url { url, selector }
                if url == "https://example.test/note.txt"
                    && selector == ReadSelector::Lines {
                        ranges: vec![crate::tool::read::selector::LineRange {
                            start: 2,
                            end: Some(3)
                        }],
                        raw: false,
                    }
        ));
    }

    #[test]
    fn read_target_recognizes_run_handoff_uri() {
        let target = parse_read_target("run://handoffs/research/HANDOFF.md:10-20");
        assert!(matches!(
            target,
            ReadTarget::Handoff { uri, selector }
                if uri == "run://handoffs/research/HANDOFF.md"
                    && selector == ReadSelector::Lines {
                        ranges: vec![crate::tool::read::selector::LineRange {
                            start: 10,
                            end: Some(20)
                        }],
                        raw: false,
                    }
        ));
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn read_opens_materialized_run_handoff() {
        let dir = tempfile::tempdir().unwrap();
        let artifact_root = dir.path().join("artifacts");
        let handoff_store = crate::run::handoff::HandoffStore::new(
            crate::run::handoff::handoff_root_for_artifact_root(&artifact_root),
        );
        let stored = handoff_store
            .materialize(
                &engine::NodeId::from("research"),
                &engine::HandoffSpec::Json,
                &serde_json::json!({"summary": "verified"}),
                None,
            )
            .unwrap();
        let runner = ToolRunner::new(
            crate::tool::registry::ToolRegistry::new(),
            dir.path().to_path_buf(),
            crate::tool::output::ArtifactStore::new(artifact_root).unwrap(),
            tokio_util::sync::CancellationToken::new(),
            std::sync::Arc::new(
                crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new(),
            ),
        );

        let output = runner
            .read(serde_json::json!({ "path": stored.artifact.uri }))
            .await
            .unwrap();

        assert!(output.contains(r#""summary": "verified""#));
    }

    #[test]
    fn http_status_error_only_uses_not_found_for_missing_resources() {
        let not_found =
            map_http_status_error("https://example.test/missing", StatusCode::NOT_FOUND);
        assert!(matches!(not_found, ToolError::NotFound { .. }));

        let server_error = map_http_status_error(
            "https://example.test/boom",
            StatusCode::INTERNAL_SERVER_ERROR,
        );
        assert!(matches!(server_error, ToolError::ExecutionFailed { .. }));
    }

    #[test]
    fn todo_list_accepts_one_active_phase_and_summarizes_progress() {
        let result = update_todo_list(serde_json::json!({
            "todos": [
                { "content": "Trace current behavior", "status": "completed" },
                { "content": "Implement checklist", "status": "in_progress" },
                { "content": "Verify focused gates", "status": "pending" }
            ],
            "_i": "Share current progress"
        }))
        .unwrap();

        assert_eq!(
            result,
            "Checklist updated: 1/3 completed. In progress: Implement checklist"
        );
    }

    #[test]
    fn todo_list_rejects_multiple_active_phases() {
        let error = update_todo_list(serde_json::json!({
            "todos": [
                { "content": "Implement checklist", "status": "in_progress" },
                { "content": "Verify focused gates", "status": "in_progress" }
            ]
        }))
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("only one todo may be in_progress"));
    }

    // ponytail: ToolRunner::new builds reqwest→aws-lc (FFI Miri rejects); also spawns subprocess
    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn web_search_runs_fake_binary_with_injected_env() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-search");
        std::fs::write(
            &fake,
            "#!/bin/sh\nprintf '{\"args\":\"%s\",\"brave\":\"%s\"}' \"$*\" \"$SEARCH_KEYS_BRAVE\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let settings = crate::settings::model::SearchSettings {
            binary_path: fake.display().to_string(),
            keys: [("brave".to_string(), "bk-123".to_string())]
                .into_iter()
                .collect(),
            ..crate::settings::model::SearchSettings::default()
        };

        let mut registry = crate::tool::registry::ToolRegistry::new();
        registry.register_web_search();
        let runner = ToolRunner::new(
            registry,
            dir.path().to_path_buf(),
            crate::tool::output::ArtifactStore::new(dir.path().join("artifacts")).unwrap(),
            tokio_util::sync::CancellationToken::new(),
            std::sync::Arc::new(
                crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new(),
            ),
        )
        .with_search_settings(settings);

        let raw = runner
            .web_search(serde_json::json!({"query": "rust rfc", "count": 3}))
            .await
            .unwrap();
        assert!(raw.contains("rust rfc --json -c 3"));
        assert!(raw.contains("\"brave\":\"bk-123\""));
    }
}
