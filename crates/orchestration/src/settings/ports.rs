use std::io;

use crate::mcp::ports::McpSecretRef;

pub use crate::settings::model::{
    AppSettings, LspSettings, ProviderProfile, ProviderTransport, SkillSummary,
};

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> io::Result<AppSettings>;
    fn save(&self, settings: &AppSettings) -> io::Result<()>;
    /// Write settings as-is (no merge of preserved secrets).
    fn save_raw(&self, settings: &AppSettings) -> io::Result<()>;

    fn set_mcp_secret(&self, _secret_ref: &McpSecretRef, _value: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "MCP secret storage is unavailable",
        ))
    }

    fn get_mcp_secret(&self, _secret_ref: &McpSecretRef) -> io::Result<Option<String>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "MCP secret storage is unavailable",
        ))
    }

    fn delete_mcp_secret(&self, _secret_ref: &McpSecretRef) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "MCP secret storage is unavailable",
        ))
    }
}

pub trait SkillCatalog: Send + Sync {
    fn discover(&self, search_paths: &[String]) -> io::Result<Vec<SkillSummary>>;
}
