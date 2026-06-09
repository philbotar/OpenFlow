use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use crate::settings::ports::SettingsStore;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn parse_settings_json(text: &str) -> Result<AppSettings, serde_json::Error> {
    serde_json::from_str::<AppSettings>(text).map(AppSettings::normalized)
}

#[derive(Debug, Clone)]
pub struct FileSettingsStore {
    path: PathBuf,
}

impl FileSettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("step-through-agentic-workflow")
            .join("settings.json")
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read or parsed.
    pub fn load(&self) -> io::Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.path)?;
        parse_settings_json(&text).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("settings JSON invalid: {error}"),
            )
        })
    }

    /// # Errors
    /// Returns an error if the settings cannot be serialized or written to disk.
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        let mut to_save = settings.clone();
        if self.path.exists() {
            let text = fs::read_to_string(&self.path)?;
            if let Ok(existing) = parse_settings_json(&text) {
                merge_preserved_api_keys(&mut to_save, &existing);
            }
        }
        self.write(&to_save)
    }

    fn write(&self, settings: &AppSettings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(settings).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("settings serialization failed: {e}"),
            )
        })?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SettingsStore for FileSettingsStore {
    fn load(&self) -> io::Result<AppSettings> {
        FileSettingsStore::load(self)
    }

    fn save(&self, settings: &AppSettings) -> io::Result<()> {
        FileSettingsStore::save(self, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::LspSettings;
    use crate::settings::ports::ProviderTransport;
    use providers::ProviderId;
    use tempfile::tempdir;

    #[test]
    fn default_settings_include_builtin_provider_profiles() {
        let settings = AppSettings::default();

        assert_eq!(settings.active_provider, ProviderId::from("openai"));
        assert!(settings.providers.contains_key(&ProviderId::from("openai")));
        assert!(settings
            .providers
            .contains_key(&ProviderId::from("anthropic")));
        let openai = settings
            .providers
            .get(&ProviderId::from("openai"))
            .expect("openai profile");
        assert_eq!(openai.display_name, "OpenAI");
        assert_eq!(openai.transport, ProviderTransport::Responses);
    }

    #[test]
    fn redacted_settings_clear_api_keys() {
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = "sk-secret".to_string();

        let redacted = settings.redacted();

        assert!(settings.active_profile().api_key == "sk-secret");
        assert!(redacted.active_profile().api_key.is_empty());
    }

    #[test]
    fn save_from_redacted_snapshot_preserves_existing_api_keys() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = "sk-persisted".to_string();
        store.save(&settings).unwrap();

        let mut redacted = store.load().unwrap().redacted();
        redacted
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .known_models
            .push("new-model".to_string());
        store.save(&redacted).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(
            loaded
                .providers
                .get(&ProviderId::from("openai"))
                .expect("openai profile")
                .api_key,
            "sk-persisted"
        );
    }

    #[test]
    fn missing_settings_file_loads_default_settings() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));

        let settings = store.load().unwrap();

        assert_eq!(settings, AppSettings::default());
    }

    #[test]
    fn settings_roundtrip_restores_identical_state() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings {
            active_provider: ProviderId::from("anthropic"),
            ..Default::default()
        };
        settings
            .providers
            .get_mut(&ProviderId::from("anthropic"))
            .expect("anthropic profile")
            .known_models = vec!["claude-custom".to_string()];

        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, settings);
        assert_eq!(loaded.lsp, LspSettings::default());
    }
}
