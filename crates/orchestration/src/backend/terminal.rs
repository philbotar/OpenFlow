use crate::terminal::{TerminalEvent, TerminalStart};
use tokio::sync::mpsc::UnboundedReceiver;

use super::{AppBackend, BackendError};

impl AppBackend {
    pub fn start_terminal(
        &self,
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<(TerminalStart, UnboundedReceiver<TerminalEvent>), BackendError> {
        self.terminal.start(cwd, cols, rows).map_err(|message| {
            log::warn!("terminal.start_failed: {message}");
            BackendError::ProjectOperation(message)
        })
    }

    pub fn write_terminal(&self, session_id: &str, data: &str) -> Result<(), BackendError> {
        self.terminal
            .write(session_id, data)
            .map_err(BackendError::ProjectOperation)
    }

    pub fn resize_terminal(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), BackendError> {
        self.terminal
            .resize(session_id, cols, rows)
            .map_err(BackendError::ProjectOperation)
    }

    pub fn stop_terminal(&self, session_id: &str) -> Result<(), BackendError> {
        self.terminal
            .stop(session_id)
            .map_err(BackendError::ProjectOperation)
    }

    pub fn stop_all_terminals(&self) {
        self.terminal.stop_all();
    }
}
