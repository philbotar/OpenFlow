//! Per-kind builtin tool dispatch for [`super::ToolRunner`].

use super::{
    apply_read_selector, BlockingBatchContext, BlockingRunOutcome, BlockingToolOps, LspSettings,
    ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError,
};
use crate::tool::errors::ToolError;
use crate::tool::registry::BuiltinToolKind;
use engine::ToolCall;
use serde::Deserialize;
use serde_json::Value;

impl ToolRunner {
    pub(super) async fn dispatch(
        &self,
        kind: BuiltinToolKind,
        call: ToolCall,
        ctx: Option<ToolExecutionContext>,
    ) -> Result<ToolExecutionRecord, ToolRunnerError> {
        match kind {
            BuiltinToolKind::Read => {
                let raw = self.read(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None).await
            }
            BuiltinToolKind::AstGrep => {
                let raw = self.ast_grep(call.arguments.clone()).await?;
                self.finalize_record(call, raw, Vec::new(), None).await
            }
            BuiltinToolKind::Bash => {
                let outcome = crate::tools::bash::execute_bash(
                    &self.cwd,
                    call.arguments.clone(),
                    &self.cancel_token,
                )
                .await?;
                self.finalize_bash_record(call, outcome).await
            }
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
        tokio::task::spawn_blocking(move || {
            BlockingToolOps::read_local_at(cwd, snapshots, &args.path)
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
}
