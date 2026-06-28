use crate::adapters::storage::json_file_store::{write_json_file, OPENFLOW_DATA_DIR_SLUG};
use crate::settings::model::{merge_preserved_api_keys, AppSettings};
use crate::settings::ports::SettingsStore;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.path)?;
        match parse_settings_json(&text) {
            Ok(settings) => Ok(settings),
            Err(_error) => {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let stamped = self
                    .path
                    .with_file_name(format!("settings.json.bak.{stamp}"));
                fs::rename(&self.path, &stamped)?;
                let defaults = AppSettings::default();
                write_json_file(&self.path, &defaults, "settings")?;
                Ok(defaults)
            }
        }
    }

    /// # Errors
    /// Returns an error if the settings cannot be serialized or written to disk.
    pub fn save_raw(&self, settings: &AppSettings) -> io::Result<()> {
        write_json_file(&self.path, settings, "settings")
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
        self.save_raw(&to_save)
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

    fn save_raw(&self, settings: &AppSettings) -> io::Result<()> {
        FileSettingsStore::save_raw(self, settings)
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
        let bak_files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("settings.json.bak"))
            .collect();
        assert_eq!(bak_files.len(), 1);
    }

    #[test]
    fn bootstrap_preserves_existing_bak_with_timestamp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let bak = dir.path().join("settings.json.bak");
        fs::write(&path, "{not valid json").unwrap();
        fs::write(&bak, r#"{"preserved":true}"#).unwrap();

        let store = FileSettingsStore::new(path.clone());
        let loaded = store.load().expect("load defaults after corrupt");
        assert_eq!(loaded, AppSettings::default());
        assert!(bak.exists(), "original .bak must not be overwritten");
        let bak_files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("settings.json.bak"))
            .collect();
        assert!(bak_files.len() >= 2, "timestamped backup created");
    }

    #[test]
    #[cfg(unix)]
    fn bootstrap_write_failure_returns_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{bad").unwrap();

        let mut perms = fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(dir.path(), perms).unwrap();

        let store = FileSettingsStore::new(&path);
        let result = store.load();

        let mut restore = fs::metadata(dir.path()).unwrap().permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(dir.path(), restore);

        assert!(result.is_err());
    }

    #[test]
    fn save_raw_clears_api_key_without_merge() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key = "sk-persisted".to_string();
        store.save(&settings).unwrap();

        let mut cleared = store.load().unwrap();
        cleared
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key
            .clear();
        store.save_raw(&cleared).unwrap();

        assert!(
            store
                .load()
                .unwrap()
                .providers
                .get(&ProviderId::from("openai"))
                .expect("openai profile")
                .api_key
                .is_empty()
        );
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
