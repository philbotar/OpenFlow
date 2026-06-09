//! Storage seam for the hashline patcher.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteResult {
    pub text: String,
}

#[derive(Debug, Error)]
#[error("File not found: {0}")]
pub struct NotFoundError(pub String);

pub fn is_not_found(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<NotFoundError>().is_some()
}

/// Minimal sync filesystem contract for hashline patching.
pub trait HashlineFilesystem: Send + Sync {
    fn read_text(&self, path: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
    fn write_text(
        &self,
        path: &str,
        content: &str,
    ) -> Result<WriteResult, Box<dyn std::error::Error + Send + Sync>>;
    fn exists(&self, path: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        match self.read_text(path) {
            Ok(_) => Ok(true),
            Err(error) if is_not_found(error.as_ref()) => Ok(false),
            Err(error) => Err(error),
        }
    }
    fn canonical_path(&self, path: &str) -> String {
        path.to_string()
    }
    fn preflight_write(&self, _path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// In-memory filesystem for tests and sandboxes.
#[derive(Debug, Clone, Default)]
pub struct InMemoryFilesystem {
    files: Arc<Mutex<HashMap<String, String>>>,
}

impl InMemoryFilesystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_files<I, P, C>(initial: I) -> Self
    where
        I: IntoIterator<Item = (P, C)>,
        P: Into<String>,
        C: Into<String>,
    {
        let mut map = HashMap::new();
        for (path, content) in initial {
            map.insert(path.into(), content.into());
        }
        Self {
            files: Arc::new(Mutex::new(map)),
        }
    }

    pub fn set(&self, path: &str, content: impl Into<String>) {
        self.files.lock().insert(path.to_string(), content.into());
    }

    pub fn get(&self, path: &str) -> Option<String> {
        self.files.lock().get(path).cloned()
    }

    pub fn delete(&self, path: &str) -> bool {
        self.files.lock().remove(path).is_some()
    }

    pub fn clear(&self) {
        self.files.lock().clear();
    }
}

impl HashlineFilesystem for InMemoryFilesystem {
    fn read_text(&self, path: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.files
            .lock()
            .get(path)
            .cloned()
            .ok_or_else(|| Box::new(NotFoundError(path.to_string())) as _)
    }

    fn write_text(
        &self,
        path: &str,
        content: &str,
    ) -> Result<WriteResult, Box<dyn std::error::Error + Send + Sync>> {
        self.set(path, content);
        Ok(WriteResult {
            text: content.to_string(),
        })
    }

    fn exists(&self, path: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.files.lock().contains_key(path))
    }

    fn canonical_path(&self, path: &str) -> String {
        Path::new(path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(path))
            .to_string_lossy()
            .into_owned()
    }
}
