use crate::adapters::storage::json_file_store::{write_json_file, OPENFLOW_DATA_DIR_SLUG};
use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use crate::settings::ports::SettingsStore;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// #region agent log
const AGENT_DEBUG_LOG_PATH: &str =
    "/Users/philipbotar/Developer/Step-through-agentic-workflow/.cursor/debug-64d565.log";

fn agent_debug_log(
    hypothesis_id: &str,
    location: &str,
    message: &str,
    data: serde_json::Value,
) {
    let payload = serde_json::json!({
        "sessionId": "64d565",
        "hypothesisId": hypothesis_id,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()),
    });
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(AGENT_DEBUG_LOG_PATH)
    {
        let _ = writeln!(file, "{payload}");
    }
}
// #endregion

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
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join("settings.json")
    }

    /// # Errors
    /// Returns an error if the settings file cannot be read or parsed.
    pub fn load(&self) -> io::Result<AppSettings> {
        let path_display = self.path.display().to_string();
        if !self.path.exists() {
            // #region agent log
            agent_debug_log(
                "A",
                "settings_store.rs:load",
                "settings file missing, using defaults",
                serde_json::json!({ "path": path_display }),
            );
            // #endregion
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.path)?;
        let has_providers_key = text.contains("\"providers\"");
        match parse_settings_json(&text) {
            Ok(settings) => {
                // #region agent log
                agent_debug_log(
                    "A",
                    "settings_store.rs:load",
                    "settings parsed",
                    serde_json::json!({
                        "path": path_display,
                        "hasProvidersKey": has_providers_key,
                        "providerCount": settings.providers.len(),
                    }),
                );
                // #endregion
                Ok(settings)
            }
            Err(error) => {
                // #region agent log
                agent_debug_log(
                    "A",
                    "settings_store.rs:load",
                    "settings parse failed, bootstrapping defaults",
                    serde_json::json!({
                        "path": path_display,
                        "hasProvidersKey": has_providers_key,
                        "error": error.to_string(),
                    }),
                );
                // #endregion
                let bak_path = self.path.with_extension("json.bak");
                let _ = fs::rename(&self.path, &bak_path);
                let defaults = AppSettings::default();
                let _ = write_json_file(&self.path, &defaults, "settings");
                Ok(defaults)
            }
        }
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
        write_json_file(&self.path, &to_save, "settings")
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

    #[test]
    fn invalid_settings_file_bootstraps_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"active_provider":"openai","openai":{"display_name":"Legacy"}}"#,
        )
        .unwrap();
        let store = FileSettingsStore::new(&path);

        let settings = store.load().unwrap();

        assert_eq!(settings, AppSettings::default());
        assert!(path.exists());
        assert!(path.with_extension("json.bak").exists());
    }

    #[test]
    fn atomic_save_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = FileSettingsStore::new(&path);

        store.save(&AppSettings::default()).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }
}
