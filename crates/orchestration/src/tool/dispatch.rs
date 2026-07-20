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
    if let Some(artifact_id) = base.strip_prefix("artifact:") {
        return ReadTarget::Artifact {
            artifact_id: artifact_id.to_string(),
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
                let raw = clients
                    .call_namespaced(&call.name, call.arguments.clone())
                    .await?;
                self.finalize_record(call, raw, Vec::new(), None).await
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
                    "required field: path (string); supports local paths, URLs, and artifact:{id}"
                        .to_string(),
            })
        })?;
        match parse_read_target(&args.path) {
            ReadTarget::Artifact {
                artifact_id,
                selector,
            } => self.read_artifact(&artifact_id, selector, &args.path),
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
        #[derive(Deserialize)]
        struct WritePlanArtifactArgs {
            markdown: String,
        }

        let args: WritePlanArtifactArgs =
            serde_json::from_value(call.arguments.clone()).map_err(|error| {
                ToolRunnerError::Tool(ToolError::InvalidArgs {
                    tool: call.name.clone(),
                    problem: error.to_string(),
                    hint: "required field: markdown (string)".to_string(),
                })
            })?;
        let artifact = self
            .artifacts
            .store_plan_markdown(args.markdown)
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
