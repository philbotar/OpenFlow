use crate::mcp::installer::PackageInstallPlan;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
const OUTPUT_LIMIT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageInstallStatus {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInstallOutcome {
    pub status: PackageInstallStatus,
    pub exit_code: Option<i32>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub output_truncated: bool,
    pub duration_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum PackageInstallerError {
    #[error("failed to create MCP install directory")]
    CreateDirectory,
    #[error("MCP package runtime `{runtime}` was not found")]
    RuntimeNotFound { runtime: String },
    #[error("failed to start MCP package installer")]
    Spawn,
    #[error("failed while waiting for MCP package installer")]
    Wait,
    #[error("failed to collect MCP package installer output")]
    Output,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PackageInstaller;

impl PackageInstaller {
    pub async fn install(
        &self,
        plan: &PackageInstallPlan,
        cancel: &CancellationToken,
    ) -> Result<PackageInstallOutcome, PackageInstallerError> {
        self.install_with_timeout(plan, cancel, INSTALL_TIMEOUT)
            .await
    }

    async fn install_with_timeout(
        &self,
        plan: &PackageInstallPlan,
        cancel: &CancellationToken,
        timeout: Duration,
    ) -> Result<PackageInstallOutcome, PackageInstallerError> {
        tokio::fs::create_dir_all(&plan.target_dir)
            .await
            .map_err(|_| PackageInstallerError::CreateDirectory)?;
        let started = Instant::now();
        let mut command = build_command(plan);
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                PackageInstallerError::RuntimeNotFound {
                    runtime: plan.executable.clone(),
                }
            } else {
                PackageInstallerError::Spawn
            }
        })?;
        #[cfg(unix)]
        let mut kill_guard = ProcessGroupKillGuard::new(&child);
        let stdout = child.stdout.take().ok_or(PackageInstallerError::Output)?;
        let stderr = child.stderr.take().ok_or(PackageInstallerError::Output)?;
        let stdout_task = tokio::spawn(read_capped(stdout));
        let stderr_task = tokio::spawn(read_capped(stderr));

        let (status, exit_code) = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                kill_process_group(&mut child).await;
                (PackageInstallStatus::Cancelled, None)
            }
            () = tokio::time::sleep(timeout) => {
                kill_process_group(&mut child).await;
                (PackageInstallStatus::TimedOut, None)
            }
            result = child.wait() => {
                let result = result.map_err(|_| PackageInstallerError::Wait)?;
                let status = if result.success() {
                    PackageInstallStatus::Succeeded
                } else {
                    PackageInstallStatus::Failed
                };
                (status, result.code())
            }
        };
        #[cfg(unix)]
        kill_guard.disarm();

        let (stdout, stdout_truncated) = stdout_task
            .await
            .map_err(|_| PackageInstallerError::Output)??;
        let (stderr, stderr_truncated) = stderr_task
            .await
            .map_err(|_| PackageInstallerError::Output)??;
        Ok(PackageInstallOutcome {
            status,
            exit_code,
            stdout_tail: sanitize_output(&stdout),
            stderr_tail: sanitize_output(&stderr),
            output_truncated: stdout_truncated || stderr_truncated,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

fn build_command(plan: &PackageInstallPlan) -> Command {
    let mut command = Command::new(&plan.executable);
    command.args(&plan.args);
    command.envs(&plan.environment);
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    command
}

async fn read_capped<R>(mut reader: R) -> Result<(Vec<u8>, bool), PackageInstallerError>
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|_| PackageInstallerError::Output)?;
        if read == 0 {
            break;
        }
        if read >= OUTPUT_LIMIT_BYTES {
            retained.clear();
            retained.extend_from_slice(&buffer[read - OUTPUT_LIMIT_BYTES..read]);
            truncated = true;
            continue;
        }
        let overflow = retained
            .len()
            .saturating_add(read)
            .saturating_sub(OUTPUT_LIMIT_BYTES);
        if overflow > 0 {
            retained.drain(..overflow);
            truncated = true;
        }
        retained.extend_from_slice(&buffer[..read]);
    }
    Ok((retained, truncated))
}

fn sanitize_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if ["token", "password", "secret", "authorization"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "[redacted]".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(unix)]
struct ProcessGroupKillGuard {
    pgid: Option<i32>,
}

#[cfg(unix)]
impl ProcessGroupKillGuard {
    fn new(child: &tokio::process::Child) -> Self {
        Self {
            pgid: child.id().and_then(|pid| i32::try_from(pid).ok()),
        }
    }

    fn disarm(&mut self) {
        self.pgid = None;
    }
}

#[cfg(unix)]
impl Drop for ProcessGroupKillGuard {
    fn drop(&mut self) {
        let Some(pgid) = self.pgid else {
            return;
        };
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
        let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
    }
}

#[cfg(unix)]
async fn kill_process_group(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;
        let pgid = i32::try_from(pid).unwrap_or(i32::MAX);
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
        let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
    }
    let _ = child.kill().await;
}

#[cfg(not(unix))]
async fn kill_process_group(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;

    fn shell_plan(target: &Path, script: &str) -> PackageInstallPlan {
        PackageInstallPlan {
            executable: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
            environment: BTreeMap::new(),
            target_dir: target.to_path_buf(),
            display_command: "test command".to_string(),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn package_install_cancellation_kills_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("child-finished");
        let script = format!("(sleep 1; touch '{}') & wait", marker.display());
        let plan = shell_plan(dir.path(), &script);
        let cancel = CancellationToken::new();
        let cancel_task = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            cancel_task.cancel();
        });

        let outcome = PackageInstaller
            .install_with_timeout(&plan, &cancel, Duration::from_secs(2))
            .await
            .unwrap();

        assert_eq!(outcome.status, PackageInstallStatus::Cancelled);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(
            !marker.exists(),
            "child process survived installer cancellation"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn package_install_output_is_capped_and_sanitized() {
        let dir = tempfile::tempdir().unwrap();
        let plan = shell_plan(
            dir.path(),
            "head -c 70000 /dev/zero | tr '\\0' x; printf '\\ntoken=do-not-leak\\n'",
        );

        let outcome = PackageInstaller
            .install_with_timeout(&plan, &CancellationToken::new(), Duration::from_secs(2))
            .await
            .unwrap();

        assert_eq!(outcome.status, PackageInstallStatus::Succeeded);
        assert!(outcome.output_truncated);
        assert!(outcome.stdout_tail.ends_with("[redacted]"));
        assert!(!outcome.stdout_tail.contains("do-not-leak"));
        assert!(outcome.stdout_tail.len() <= OUTPUT_LIMIT_BYTES);
    }
}
