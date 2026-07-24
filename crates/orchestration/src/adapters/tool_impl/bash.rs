//! Bash command execution for the agent `bash` tool (oh-my-pi–aligned semantics).

use crate::tool::ToolExecutionUpdate;
use crate::tools::edit::path::{resolve_writable, PathEscapeError};
use crate::tools::errors::ToolError;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

const LIVE_UPDATE_TAIL_BYTES: usize = 12_000;

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const MIN_TIMEOUT_SECS: u64 = 1;
const MAX_TIMEOUT_SECS: u64 = 3600;

static BASH_ENV_NAME_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("env name regex is valid"));

/// Non-interactive defaults so shell commands do not block on pagers or prompts.
const NON_INTERACTIVE_ENV: &[(&str, &str)] = &[
    ("PAGER", "cat"),
    ("GIT_PAGER", "cat"),
    ("MANPAGER", "cat"),
    ("SYSTEMD_PAGER", "cat"),
    ("BAT_PAGER", "cat"),
    ("DELTA_PAGER", "cat"),
    ("GH_PAGER", "cat"),
    ("GLAB_PAGER", "cat"),
    ("PSQL_PAGER", "cat"),
    ("MYSQL_PAGER", "cat"),
    ("AWS_PAGER", ""),
    ("HOMEBREW_PAGER", "cat"),
    ("LESS", "FRX"),
    ("TERM", "dumb"),
    ("GPG_TTY", "not a tty"),
    ("NO_COLOR", "1"),
    ("PYTHONUNBUFFERED", "1"),
    ("GIT_EDITOR", "true"),
    ("VISUAL", "true"),
    ("EDITOR", "true"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("SSH_ASKPASS", "/usr/bin/false"),
    ("CI", "1"),
    ("npm_config_yes", "true"),
    ("npm_config_update_notifier", "false"),
    ("npm_config_fund", "false"),
    ("npm_config_audit", "false"),
    ("npm_config_progress", "false"),
    ("PNPM_DISABLE_SELF_UPDATE_CHECK", "true"),
    ("PNPM_UPDATE_NOTIFIER", "false"),
    ("YARN_ENABLE_TELEMETRY", "0"),
    ("YARN_ENABLE_PROGRESS_BARS", "0"),
    ("CARGO_TERM_PROGRESS_WHEN", "never"),
    ("DEBIAN_FRONTEND", "noninteractive"),
    ("PIP_NO_INPUT", "1"),
    ("PIP_DISABLE_PIP_VERSION_CHECK", "1"),
    ("TF_INPUT", "0"),
    ("TF_IN_AUTOMATION", "1"),
    ("GH_PROMPT_DISABLED", "1"),
    ("COMPOSER_NO_INTERACTION", "1"),
    ("CLOUDSDK_CORE_DISABLE_PROMPTS", "1"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashExecutionOutcome {
    pub output: String,
    pub exit_code: Option<i32>,
    pub is_error: bool,
}

#[derive(Debug, Deserialize)]
struct BashArgs {
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    cwd: Option<String>,
}

/// Clamp timeout to the allowed bash range (default 300s, max 3600s).
#[must_use]
pub fn clamp_bash_timeout(raw: Option<u64>) -> u64 {
    raw.unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
}

fn tail_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut start = text.len().saturating_sub(max_bytes);
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

fn merge_stdout_stderr(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}{stderr}"),
    }
}

fn combined_output_from_bytes(stdout_bytes: &[u8], stderr_bytes: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout_bytes);
    let stderr = String::from_utf8_lossy(stderr_bytes);
    merge_stdout_stderr(&stdout, &stderr)
}

fn emit_bash_update(
    update_tx: &Option<tokio::sync::mpsc::UnboundedSender<ToolExecutionUpdate>>,
    stdout_bytes: &[u8],
    stderr_bytes: &[u8],
) {
    let Some(update_tx) = update_tx else {
        return;
    };
    let combined = combined_output_from_bytes(stdout_bytes, stderr_bytes);
    let content = tail_text(&combined, LIVE_UPDATE_TAIL_BYTES);
    let _ = update_tx.send(ToolExecutionUpdate {
        content,
        output_meta: None,
    });
}

pub async fn execute_bash(
    execution_cwd: &Path,
    args: Value,
    cancel_token: &CancellationToken,
    update_tx: Option<tokio::sync::mpsc::UnboundedSender<ToolExecutionUpdate>>,
) -> Result<BashExecutionOutcome, ToolError> {
    let args: BashArgs = serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
        tool: "bash".to_string(),
        problem: error.to_string(),
        hint: "required field: command (string); optional timeout, cwd, env".to_string(),
    })?;
    if args.command.trim().is_empty() {
        return Err(ToolError::InvalidArgs {
            tool: "bash".to_string(),
            problem: "command must not be empty".to_string(),
            hint: "provide a shell command string".to_string(),
        });
    }

    let requested_timeout = args.timeout;
    let timeout_secs = clamp_bash_timeout(requested_timeout);
    let work_dir = resolve_bash_cwd(execution_cwd, args.cwd.as_deref())?;
    let env = normalize_bash_env(args.env)?;

    let started = Instant::now();
    let raw = match run_shell_command(
        &args.command,
        &work_dir,
        env.as_ref(),
        timeout_secs,
        cancel_token,
        &update_tx,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(ToolError::Timeout {
            partial_output: Some(body),
            after_secs,
            ..
        }) => {
            let wall_time_secs = started.elapsed().as_secs_f64();
            let mut output_lines = Vec::new();
            let trimmed = body.trim_end();
            if trimmed.is_empty() {
                output_lines.push("(no output)".to_string());
            } else {
                output_lines.push(trimmed.to_string());
            }
            output_lines.push(String::new());
            output_lines.push(format!("(timed out after {after_secs}s)"));
            output_lines.push(format!("Wall time: {wall_time_secs:.2} seconds"));
            return Ok(BashExecutionOutcome {
                output: output_lines.join("\n"),
                exit_code: None,
                is_error: true,
            });
        }
        Err(error) => return Err(error),
    };
    let wall_time_secs = started.elapsed().as_secs_f64();

    let mut notices = Vec::new();
    if let Some(requested) = requested_timeout.filter(|value| *value != timeout_secs) {
        notices.push(format!(
            "Timeout clamped to {timeout_secs}s (requested {requested}s; allowed range {MIN_TIMEOUT_SECS}-{MAX_TIMEOUT_SECS}s)."
        ));
    }
    notices.push(format!("Wall time: {wall_time_secs:.2} seconds"));

    let failed_exit = raw.exit_code.is_some_and(|code| code != 0);
    if failed_exit {
        if let Some(code) = raw.exit_code {
            notices.push(format!("Command exited with code {code}"));
        }
    }

    let mut output_lines = Vec::new();
    let body = raw.combined_output.trim_end();
    if body.is_empty() {
        output_lines.push("(no output)".to_string());
    } else {
        output_lines.push(body.to_string());
    }
    if !notices.is_empty() {
        output_lines.push(String::new());
        output_lines.extend(notices);
    }

    Ok(BashExecutionOutcome {
        output: output_lines.join("\n"),
        exit_code: raw.exit_code,
        is_error: failed_exit || raw.cancelled,
    })
}

#[derive(Debug)]
struct RawShellOutcome {
    combined_output: String,
    exit_code: Option<i32>,
    cancelled: bool,
}

async fn read_stream_incremental<R, F>(reader: &mut R, mut append: F) -> std::io::Result<()>
where
    R: AsyncReadExt + Unpin,
    F: FnMut(&[u8]),
{
    let mut chunk = [0u8; 4096];
    loop {
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        append(&chunk[..n]);
    }
    Ok(())
}

async fn read_stream_incremental_shared<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    target: Arc<Mutex<Vec<u8>>>,
    stdout_bytes: Arc<Mutex<Vec<u8>>>,
    stderr_bytes: Arc<Mutex<Vec<u8>>>,
    update_tx: Option<tokio::sync::mpsc::UnboundedSender<ToolExecutionUpdate>>,
) -> std::io::Result<()> {
    read_stream_incremental(reader, |chunk| {
        target
            .lock()
            .expect("pipe buffer lock")
            .extend_from_slice(chunk);
        let stdout = stdout_bytes.lock().expect("stdout lock").clone();
        let stderr = stderr_bytes.lock().expect("stderr lock").clone();
        emit_bash_update(&update_tx, &stdout, &stderr);
    })
    .await
}

async fn run_shell_command(
    command: &str,
    cwd: &Path,
    extra_env: Option<&HashMap<String, String>>,
    timeout_secs: u64,
    cancel_token: &CancellationToken,
    update_tx: &Option<tokio::sync::mpsc::UnboundedSender<ToolExecutionUpdate>>,
) -> Result<RawShellOutcome, ToolError> {
    run_shell_command_until(
        command,
        cwd,
        extra_env,
        async move {
            tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
            timeout_secs
        },
        cancel_token,
        update_tx,
    )
    .await
}

async fn run_shell_command_until<F>(
    command: &str,
    cwd: &Path,
    extra_env: Option<&HashMap<String, String>>,
    timeout: F,
    cancel_token: &CancellationToken,
    update_tx: &Option<tokio::sync::mpsc::UnboundedSender<ToolExecutionUpdate>>,
) -> Result<RawShellOutcome, ToolError>
where
    F: Future<Output = u64>,
{
    if cancel_token.is_cancelled() {
        return Ok(RawShellOutcome {
            combined_output: "Command cancelled".to_string(),
            exit_code: None,
            cancelled: true,
        });
    }

    let mut child = build_shell_command(command, cwd, extra_env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| ToolError::failed(format!("bash failed to start: {error}")))?;

    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| ToolError::failed("bash stdout unavailable".to_string()))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| ToolError::failed("bash stderr unavailable".to_string()))?;

    let stdout_bytes = Arc::new(Mutex::new(Vec::new()));
    let stderr_bytes = Arc::new(Mutex::new(Vec::new()));
    let live_update_tx = update_tx.clone();

    let mut stdout_reader = {
        let stdout_bytes = Arc::clone(&stdout_bytes);
        let stderr_bytes = Arc::clone(&stderr_bytes);
        let update_tx = live_update_tx.clone();
        tokio::spawn(async move {
            read_stream_incremental_shared(
                &mut stdout_pipe,
                Arc::clone(&stdout_bytes),
                stdout_bytes,
                stderr_bytes,
                update_tx,
            )
            .await
        })
    };
    let mut stderr_reader = {
        let stderr_bytes = Arc::clone(&stderr_bytes);
        let stdout_bytes = Arc::clone(&stdout_bytes);
        let update_tx = live_update_tx;
        tokio::spawn(async move {
            read_stream_incremental_shared(
                &mut stderr_pipe,
                Arc::clone(&stderr_bytes),
                stdout_bytes,
                stderr_bytes,
                update_tx,
            )
            .await
        })
    };

    tokio::pin!(timeout);
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            kill_process_group(&mut child).await;
            stdout_reader.abort();
            stderr_reader.abort();
            Ok(RawShellOutcome {
                combined_output: "Command cancelled".to_string(),
                exit_code: None,
                cancelled: true,
            })
        }
        status = child.wait() => {
            let stdout_res = stdout_reader.await.unwrap_or(Ok(()));
            let stderr_res = stderr_reader.await.unwrap_or(Ok(()));
            stdout_res.map_err(|error| ToolError::failed(format!("bash read failed: {error}")))?;
            stderr_res.map_err(|error| {
                ToolError::failed(format!("bash stderr read failed: {error}"))
            })?;
            let status = status
                .map_err(|error| ToolError::failed(format!("bash failed: {error}")))?;
            let stdout = stdout_bytes.lock().expect("stdout lock").clone();
            let stderr = stderr_bytes.lock().expect("stderr lock").clone();
            Ok(RawShellOutcome {
                combined_output: combined_output_from_bytes(&stdout, &stderr),
                exit_code: status.code(),
                cancelled: false,
            })
        }
        after_secs = &mut timeout => {
            kill_process_group(&mut child).await;
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            let readers_finished = tokio::time::timeout(Duration::from_secs(2), async {
                let _ = (&mut stdout_reader).await;
                let _ = (&mut stderr_reader).await;
            })
            .await
            .is_ok();
            if !readers_finished {
                stdout_reader.abort();
                stderr_reader.abort();
            }
            let stdout = stdout_bytes.lock().expect("stdout lock").clone();
            let stderr = stderr_bytes.lock().expect("stderr lock").clone();
            let combined_output = combined_output_from_bytes(&stdout, &stderr);
            Err(ToolError::Timeout {
                tool: "bash".to_string(),
                after_secs,
                hint: "increase timeout or split the command into smaller steps".to_string(),
                partial_output: Some(combined_output),
            })
        }
    }
}

fn build_shell_command(
    command: &str,
    cwd: &Path,
    extra_env: Option<&HashMap<String, String>>,
) -> Command {
    let mut command_builder = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-lc").arg(command);
        cmd
    };
    #[cfg(unix)]
    command_builder.process_group(0);
    command_builder.current_dir(cwd);
    command_builder.env_remove("BASH_ENV");
    for (key, value) in NON_INTERACTIVE_ENV {
        command_builder.env(key, *value);
    }
    if let Some(extra) = extra_env {
        for (key, value) in extra {
            command_builder.env(key, value);
        }
    }
    command_builder
}

#[cfg(unix)]
async fn kill_process_group(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;
        let pgid = pid as i32;
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
        let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
    }
    let _ = child.kill().await;
}

#[cfg(not(unix))]
async fn kill_process_group(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
}

fn resolve_bash_cwd(execution_cwd: &Path, cwd: Option<&str>) -> Result<PathBuf, ToolError> {
    match cwd {
        Some(path) if !path.trim().is_empty() => {
            resolve_writable(execution_cwd, path).map_err(|PathEscapeError(message)| {
                if message.contains("path escapes execution folder") {
                    ToolError::PermissionDenied {
                        what: message,
                        hint: "paths must stay under the execution folder; use a relative path"
                            .to_string(),
                    }
                } else {
                    ToolError::failed(message)
                }
            })
        }
        _ => execution_cwd
            .canonicalize()
            .map_err(|error| ToolError::failed(format!("invalid execution cwd: {error}"))),
    }
}

fn normalize_bash_env(
    env: Option<HashMap<String, String>>,
) -> Result<Option<HashMap<String, String>>, ToolError> {
    let Some(env) = env else {
        return Ok(None);
    };
    if env.is_empty() {
        return Ok(None);
    }
    let mut normalized = HashMap::with_capacity(env.len());
    for (key, value) in env {
        if !BASH_ENV_NAME_PATTERN.is_match(&key) {
            return Err(ToolError::InvalidArgs {
                tool: "bash".to_string(),
                problem: format!("invalid env name: {key}"),
                hint: "env keys must be valid environment variable names".to_string(),
            });
        }
        normalized.insert(key, value);
    }
    Ok(Some(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_text_keeps_last_bytes_on_utf8_boundary() {
        let text = format!("{}{}", "a".repeat(40), "z".repeat(40));
        assert_eq!(tail_text(&text, 10), "zzzzzzzzzz");
    }

    #[test]
    fn tail_text_returns_full_text_when_under_limit() {
        assert_eq!(tail_text("abc", 10), "abc");
    }

    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn clamp_bash_timeout_honours_bounds() {
        assert_eq!(clamp_bash_timeout(None), 300);
        assert_eq!(clamp_bash_timeout(Some(0)), 1);
        assert_eq!(clamp_bash_timeout(Some(9999)), 3600);
    }

    #[test]
    fn normalize_bash_env_rejects_invalid_names() {
        let err = normalize_bash_env(Some(HashMap::from([("1BAD".to_string(), "x".to_string())])))
            .unwrap_err();
        assert!(err.to_string().contains("[invalid_args]"));
        assert!(err.to_string().contains("invalid env name"));
    }

    #[test]
    fn resolve_bash_cwd_defaults_to_execution_folder() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let resolved = resolve_bash_cwd(&cwd, None).expect("resolve");
        assert_eq!(resolved, cwd);
    }

    #[test]
    fn resolve_bash_cwd_rejects_escape() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let err = resolve_bash_cwd(&cwd, Some("../outside")).unwrap_err();
        assert!(err.to_string().contains("escapes execution folder"));
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn bash_timeout_preserves_output_emitted_before_timeout() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let cancellation = CancellationToken::new();
        let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel();
        let (timeout_tx, timeout_rx) = tokio::sync::oneshot::channel();
        let command = tokio::spawn(async move {
            run_shell_command_until(
                "printf 'BEFORE_TIMEOUT\n'; sleep 30",
                &cwd,
                None,
                async {
                    let _ = timeout_rx.await;
                    1
                },
                &cancellation,
                &Some(update_tx),
            )
            .await
        });

        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let update = update_rx.recv().await.expect("bash update");
                if update.content.contains("BEFORE_TIMEOUT") {
                    break;
                }
            }
        })
        .await
        .expect("bash should emit output");
        timeout_tx.send(()).expect("trigger timeout");

        let err = command
            .await
            .expect("bash task")
            .expect_err("should timeout");
        match err {
            ToolError::Timeout { partial_output, .. } => {
                let partial = partial_output.expect("partial output");
                assert!(partial.contains("BEFORE_TIMEOUT"));
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn bash_incremental_read_collects_output_before_exit() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = run_shell_command(
            "echo line1; echo line2",
            &cwd,
            None,
            30,
            &CancellationToken::new(),
            &None,
        )
        .await
        .expect("success");
        assert!(outcome.combined_output.contains("line1"));
        assert!(outcome.combined_output.contains("line2"));
    }

    #[cfg(unix)]
    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn bash_timeout_kills_process_group() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let result = run_shell_command(
            "sleep 6042",
            &cwd,
            None,
            1,
            &CancellationToken::new(),
            &None,
        )
        .await;
        assert!(
            matches!(result, Err(ToolError::Timeout { .. })),
            "expected timeout, got {result:?}"
        );
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn execute_bash_runs_command_in_cwd() {
        let temp = TempDir::new().expect("tempdir");
        fs::write(temp.path().join("marker.txt"), "ok").expect("write");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = execute_bash(
            &cwd,
            serde_json::json!({"command": "cat marker.txt"}),
            &CancellationToken::new(),
            None,
        )
        .await
        .expect("bash");
        assert!(outcome.output.contains("ok"));
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.is_error);
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn execute_bash_surfaces_non_zero_exit_as_error() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = execute_bash(
            &cwd,
            serde_json::json!({"command": "exit 7"}),
            &CancellationToken::new(),
            None,
        )
        .await
        .expect("bash");
        assert_eq!(outcome.exit_code, Some(7));
        assert!(outcome.is_error);
        assert!(outcome.output.contains("Command exited with code 7"));
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn execute_bash_merges_stderr() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = execute_bash(
            &cwd,
            serde_json::json!({"command": "echo out; echo err 1>&2"}),
            &CancellationToken::new(),
            None,
        )
        .await
        .expect("bash");
        assert!(outcome.output.contains("out"));
        assert!(outcome.output.contains("err"));
    }
}
