//! Patch filesystem wrapper that runs LSP writethrough after each write.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use super::config::LspSettings;
use super::diagnostics::FileDiagnosticsResult;
use super::writethrough::after_write;
use crate::tools::edit::normalize::{normalize_to_lf, strip_bom};
use crate::tools::edit::patch::{PatchFileSystem, StdPatchFileSystem};

pub struct WritethroughPatchFileSystem {
    inner: StdPatchFileSystem,
    settings: LspSettings,
    diagnostics: Mutex<Vec<FileDiagnosticsResult>>,
    normalized_after_write: Mutex<HashMap<PathBuf, String>>,
}

impl WritethroughPatchFileSystem {
    pub fn new(settings: LspSettings) -> Self {
        Self {
            inner: StdPatchFileSystem,
            settings,
            diagnostics: Mutex::new(Vec::new()),
            normalized_after_write: Mutex::new(HashMap::new()),
        }
    }

    pub fn take_diagnostics(&self) -> Vec<FileDiagnosticsResult> {
        self.diagnostics.lock().drain(..).collect()
    }

    #[must_use]
    pub fn normalized_content(&self, path: &Path) -> Option<String> {
        self.normalized_after_write.lock().get(path).cloned()
    }
}

impl PatchFileSystem for WritethroughPatchFileSystem {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.inner.read(path)
    }

    fn read_binary(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.inner.read_binary(path)
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        self.inner.write(path, content)?;
        if let Some(result) = after_write(path, &self.settings) {
            self.diagnostics.lock().push(result);
        }
        if let Ok(raw) = std::fs::read_to_string(path) {
            let normalized = normalize_to_lf(&strip_bom(&raw).text);
            self.normalized_after_write
                .lock()
                .insert(path.to_path_buf(), normalized);
        }
        Ok(())
    }

    fn delete(&self, path: &Path) -> io::Result<()> {
        self.inner.delete(path)
    }

    fn mkdir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.mkdir_all(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        self.inner.exists(path)
    }
}
