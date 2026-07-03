//! Shared serde shapes for write-tier edit tools.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct ToolPathArg {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WriteToolArgs {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EditToolArgs {
    pub path: String,
    pub edits: Vec<EditEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EditEntry {
    pub old_text: String,
    pub new_text: String,
    #[serde(default)]
    pub all: bool,
}

/// Hashline `edit` and `apply_patch` both take a single envelope string.
#[derive(Debug, Deserialize)]
pub(crate) struct PatchEnvelopeArgs {
    pub input: String,
}
