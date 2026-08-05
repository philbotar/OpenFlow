use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalStart {
    pub session_id: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEvent {
    pub session_id: String,
    pub kind: TerminalEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEventKind {
    Output { data: String },
    Exit { status: Option<i32> },
    Error { message: String },
}

struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct TerminalManager {
    sessions: Mutex<HashMap<String, TerminalSession>>,
}

impl TerminalManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn start(
        &self,
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<(TerminalStart, UnboundedReceiver<TerminalEvent>), String> {
        let cwd = resolve_terminal_cwd(cwd)?;
        let shell = crate::mcp::environment::user_shell();
        let effective_path = crate::mcp::environment::effective_path().await;
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system
            .openpty(size)
            .map_err(|error| format!("failed to open terminal pty: {error}"))?;
        let command = build_terminal_command(&shell, &cwd, effective_path.as_deref());
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("failed to spawn terminal shell: {error}"))?;
        drop(pair.slave);

        let session_id = Uuid::new_v4().to_string();
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("failed to open terminal reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("failed to open terminal writer: {error}"))?;
        let (tx, rx) = unbounded_channel();
        spawn_reader(session_id.clone(), &tx, reader);

        self.sessions.lock().insert(
            session_id.clone(),
            TerminalSession {
                master: pair.master,
                writer,
                child,
            },
        );

        Ok((
            TerminalStart {
                session_id,
                cwd: cwd.to_string_lossy().to_string(),
            },
            rx,
        ))
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("terminal session not found: {session_id}"))?;
        session
            .writer
            .write_all(data.as_bytes())
            .map_err(|error| format!("failed to write terminal input: {error}"))?;
        session
            .writer
            .flush()
            .map_err(|error| format!("failed to flush terminal input: {error}"))
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("terminal session not found: {session_id}"))?;
        session
            .master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("failed to resize terminal: {error}"))
    }

    pub fn stop(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock();
        if let Some(mut session) = sessions.remove(session_id) {
            kill_terminal_process_groups(&*session.master, &*session.child);
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
        Ok(())
    }

    pub fn stop_all(&self) {
        let ids = self.sessions.lock().keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let _ = self.stop(&id);
        }
    }
}

fn kill_terminal_process_groups(master: &dyn MasterPty, child: &dyn Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;

        // A PTY shell can put the foreground command in a different process
        // group from the shell. Kill both groups before killing the shell.
        let mut process_groups = Vec::with_capacity(2);
        if let Some(pgid) = master.process_group_leader() {
            process_groups.push(pgid);
        }
        if let Some(pid) = child.process_id().and_then(|pid| i32::try_from(pid).ok()) {
            process_groups.push(pid);
        }
        process_groups.sort_unstable();
        process_groups.dedup();

        for pgid in process_groups {
            if pgid > 0 {
                let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
                let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
            }
        }
    }

    #[cfg(not(unix))]
    let _ = (master, child);
}

fn build_terminal_command(
    shell: &OsStr,
    cwd: &Path,
    effective_path: Option<&OsStr>,
) -> CommandBuilder {
    let mut command = CommandBuilder::new(shell);
    command.cwd(cwd);
    if let Some(path) = effective_path {
        command.env("PATH", path);
    }
    command
}

fn spawn_reader(
    session_id: String,
    tx: &UnboundedSender<TerminalEvent>,
    mut reader: Box<dyn Read + Send>,
) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let data = String::from_utf8_lossy(&buffer[..count]).to_string();
                    let _ = tx.send(TerminalEvent {
                        session_id: session_id.clone(),
                        kind: TerminalEventKind::Output { data },
                    });
                }
                Err(error) => {
                    let _ = tx.send(TerminalEvent {
                        session_id: session_id.clone(),
                        kind: TerminalEventKind::Error {
                            message: error.to_string(),
                        },
                    });
                    break;
                }
            }
        }
        let _ = tx.send(TerminalEvent {
            session_id,
            kind: TerminalEventKind::Exit { status: None },
        });
    });
}

pub fn resolve_terminal_cwd(cwd: Option<&str>) -> Result<PathBuf, String> {
    match cwd.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        Some(path) => {
            let expanded = if let Some(rest) = path.strip_prefix("~/") {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(rest)
            } else if path == "~" {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"))
            } else {
                PathBuf::from(path)
            };
            let canonical = expanded.canonicalize().map_err(|error| {
                format!("terminal cwd is not a valid directory ({path}): {error}")
            })?;
            if !canonical.is_dir() {
                return Err(format!("terminal cwd is not a directory: {path}"));
            }
            Ok(canonical)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_terminal_command, resolve_terminal_cwd};
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn resolve_terminal_cwd_uses_process_directory_when_unset() {
        let cwd = resolve_terminal_cwd(None).expect("fallback cwd");
        assert!(cwd.is_dir());
    }

    #[test]
    fn resolve_terminal_cwd_rejects_missing_directory() {
        let error = resolve_terminal_cwd(Some("/definitely/not/a/real/openflow/terminal/path"))
            .expect_err("missing cwd should fail");
        assert!(error.contains("terminal cwd is not a valid directory"));
    }

    #[test]
    fn terminal_command_uses_login_shell_path() {
        let command = build_terminal_command(
            OsStr::new("/bin/sh"),
            Path::new("."),
            Some(OsStr::new("/custom/bin:/usr/bin")),
        );

        assert_eq!(
            command.get_env("PATH"),
            Some(OsStr::new("/custom/bin:/usr/bin"))
        );
    }

    #[cfg(unix)]
    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn stopping_terminal_kills_foreground_command_descendants() {
        use nix::sys::signal::{kill, killpg, Signal};
        use nix::unistd::Pid;
        use std::time::Duration;
        use tempfile::TempDir;

        let temp = TempDir::new().expect("tempdir");
        let manager = super::TerminalManager::new();
        let (terminal, _events) = manager
            .start(Some(temp.path().to_str().expect("temp path")), 80, 24)
            .await
            .expect("terminal");

        manager
            .write(
                &terminal.session_id,
                "sh -c 'sleep 30 & echo $! > child.pid; wait'\n",
            )
            .expect("write command");

        let child_pid_result = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(contents) = tokio::fs::read_to_string(temp.path().join("child.pid")).await
                {
                    if let Ok(pid) = contents.trim().parse::<i32>() {
                        break pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;

        manager.stop(&terminal.session_id).expect("stop terminal");

        let child_pid = child_pid_result.expect("child pid");

        let child_is_alive = |pid| kill(Pid::from_raw(pid), None).is_ok();
        let cleaned = tokio::time::timeout(Duration::from_secs(2), async {
            while child_is_alive(child_pid) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .is_ok();
        if !cleaned {
            let _ = killpg(Pid::from_raw(child_pid), Signal::SIGKILL);
            let _ = kill(Pid::from_raw(child_pid), Signal::SIGKILL);
        }

        assert!(
            cleaned,
            "stopping the PTY left a foreground descendant alive"
        );
    }
}
