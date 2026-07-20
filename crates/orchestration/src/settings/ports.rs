use std::io;

pub use crate::settings::model::{
    AppSettings, LspSettings, ProviderProfile, ProviderTransport, SkillSummary,
};

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> io::Result<AppSettings>;
    fn save(&self, settings: &AppSettings) -> io::Result<()>;
    /// Write settings as-is (no merge of preserved secrets).
    fn save_raw(&self, settings: &AppSettings) -> io::Result<()>;
}

pub trait SkillCatalog: Send + Sync {
    fn discover(&self, search_paths: &[String]) -> io::Result<Vec<SkillSummary>>;
}
