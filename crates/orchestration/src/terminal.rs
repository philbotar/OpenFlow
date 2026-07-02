use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
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

    pub fn start(
        &self,
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<(TerminalStart, UnboundedReceiver<TerminalEvent>), String> {
        let cwd = resolve_terminal_cwd(cwd)?;
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
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut command = CommandBuilder::new(shell);
        command.cwd(&cwd);
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
    use super::resolve_terminal_cwd;

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
}
