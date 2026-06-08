//! Persistence seam for node templates.

use crate::template::Template;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateStoreError {
    #[error("cannot determine local data directory")]
    DataDirUnavailable,
    #[error("cannot create directory: {path}")]
    CannotCreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot read templates file: {path}")]
    CannotRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write templates to: {path}")]
    CannotWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot serialize templates")]
    Serialize(#[from] serde_json::Error),
    #[error("template not found: {id}")]
    NotFound { id: String },
}

/// Persistence contract for node templates.
pub trait TemplateStore {
    fn list(&self) -> Vec<Template>;

    /// # Errors
    /// Returns an error when the template cannot be persisted.
    fn add(&self, template: Template) -> Result<(), TemplateStoreError>;

    /// # Errors
    /// Returns an error when the template cannot be removed from disk.
    fn remove(&self, id: &str) -> Result<(), TemplateStoreError>;

    /// # Errors
    /// Returns [`TemplateStoreError::NotFound`] when no template matches `template.id`.
    fn update(&self, template: Template) -> Result<(), TemplateStoreError>;
}
