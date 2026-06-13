//! Bash command execution for the agent `bash` tool (oh-my-pi–aligned semantics).

use crate::tools::edit::path::{resolve_writable, PathEscapeError};
use crate::tools::errors::ToolError;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

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

pub async fn execute_bash(
    execution_cwd: &Path,
    args: Value,
    cancel_token: &CancellationToken,
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
    let raw = run_shell_command(
        &args.command,
        &work_dir,
        env.as_ref(),
        timeout_secs,
        cancel_token,
    )
    .await?;
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

struct RawShellOutcome {
    combined_output: String,
    exit_code: Option<i32>,
    cancelled: bool,
}

async fn run_shell_command(
    command: &str,
    cwd: &Path,
    extra_env: Option<&HashMap<String, String>>,
    timeout_secs: u64,
    cancel_token: &CancellationToken,
) -> Result<RawShellOutcome, ToolError> {
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

    let timeout = Duration::from_secs(timeout_secs);
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            kill_process_group(&mut child).await;
            Ok(RawShellOutcome {
                combined_output: "Command cancelled".to_string(),
                exit_code: None,
                cancelled: true,
            })
        }
        result = async {
            let mut stdout_bytes = Vec::new();
            let mut stderr_bytes = Vec::new();
            let (stdout_res, stderr_res, status) = tokio::join!(
                tokio::io::AsyncReadExt::read_to_end(&mut stdout_pipe, &mut stdout_bytes),
                tokio::io::AsyncReadExt::read_to_end(&mut stderr_pipe, &mut stderr_bytes),
                child.wait(),
            );
            stdout_res.map_err(|error| ToolError::failed(format!("bash read failed: {error}")))?;
            stderr_res.map_err(|error| {
                ToolError::failed(format!("bash stderr read failed: {error}"))
            })?;
            let status = status
                .map_err(|error| ToolError::failed(format!("bash failed: {error}")))?;
            let stdout = String::from_utf8_lossy(&stdout_bytes);
            let stderr = String::from_utf8_lossy(&stderr_bytes);
            let combined_output = merge_stdout_stderr(&stdout, &stderr);
            Ok(RawShellOutcome {
                combined_output,
                exit_code: status.code(),
                cancelled: false,
            })
        } => result,
        _ = tokio::time::sleep(timeout) => {
            kill_process_group(&mut child).await;
            Err(ToolError::Timeout {
                tool: "bash".to_string(),
                after_secs: timeout_secs,
                hint: "increase timeout or split the command into smaller steps".to_string(),
                partial_output: None,
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
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
    }
    let _ = child.kill().await;
}

#[cfg(not(unix))]
async fn kill_process_group(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
}

fn merge_stdout_stderr(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => format!("{stdout}{stderr}"),
    }
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

    #[cfg(unix)]
    #[tokio::test]
    async fn bash_timeout_kills_process_group() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let result = run_shell_command(
            "sleep 6042 & sleep 6042",
            &cwd,
            None,
            1,
            &CancellationToken::new(),
        )
        .await;
        std::thread::sleep(std::time::Duration::from_millis(500));
        let still_running = std::process::Command::new("pgrep")
            .arg("-f")
            .arg("sleep 6042")
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
        assert!(
            !still_running,
            "grandchild sleep should be killed with process group"
        );
        assert!(result.is_err() || result.unwrap().exit_code.is_none());
    }

    #[tokio::test]
    async fn execute_bash_runs_command_in_cwd() {
        let temp = TempDir::new().expect("tempdir");
        fs::write(temp.path().join("marker.txt"), "ok").expect("write");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = execute_bash(
            &cwd,
            serde_json::json!({"command": "cat marker.txt"}),
            &CancellationToken::new(),
        )
        .await
        .expect("bash");
        assert!(outcome.output.contains("ok"));
        assert_eq!(outcome.exit_code, Some(0));
        assert!(!outcome.is_error);
    }

    #[tokio::test]
    async fn execute_bash_surfaces_non_zero_exit_as_error() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = execute_bash(
            &cwd,
            serde_json::json!({"command": "exit 7"}),
            &CancellationToken::new(),
        )
        .await
        .expect("bash");
        assert_eq!(outcome.exit_code, Some(7));
        assert!(outcome.is_error);
        assert!(outcome.output.contains("Command exited with code 7"));
    }

    #[tokio::test]
    async fn execute_bash_merges_stderr() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let outcome = execute_bash(
            &cwd,
            serde_json::json!({"command": "echo out; echo err 1>&2"}),
            &CancellationToken::new(),
        )
        .await
        .expect("bash");
        assert!(outcome.output.contains("out"));
        assert!(outcome.output.contains("err"));
    }
}
