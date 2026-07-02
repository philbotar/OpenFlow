use super::{AppBackend, BackendError};

impl AppBackend {
    pub fn backend_err(&self, error: BackendError) -> BackendError {
        log::warn!("backend error: {error}");
        error
    }

    pub(super) fn persistence_err(&self, code: &str, error: BackendError) -> BackendError {
        log::warn!("{code}: {error}");
        error
    }
}
