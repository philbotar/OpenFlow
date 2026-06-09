//! LSP-aware write pipeline for file edit tools (Phase 8).
//!
//! Ships incrementally with CLI formatters; full language-server client is future work.

pub mod config;
pub mod diagnostics;
pub mod formatters;
pub mod patch_fs;
pub mod writethrough;

pub use config::LspSettings;
pub use diagnostics::{append_writethrough_to_output, FileDiagnosticsResult, FormatResult};
pub use patch_fs::WritethroughPatchFileSystem;
pub use writethrough::after_write;
