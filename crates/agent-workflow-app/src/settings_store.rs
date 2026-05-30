use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKind {
    OpenAi,
    OpenAiCompatible,
}

impl AiProviderKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "ChatGPT / OpenAI",
            Self::OpenAiCompatible => "OpenAI-compatible API",
        }
    }

    #[must_use]
    pub const fn env_key(self) -> &'static str {
        match self {
            Self::OpenAi => "OPENAI_API_KEY",
            Self::OpenAiCompatible => "OPENAI_COMPATIBLE_API_KEY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTransport {
    Responses,
    ChatCompletions,
}

impl ProviderTransport {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Responses => "Responses API",
            Self::ChatCompletions => "Chat Completions API",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub display_name: String,
    pub base_url: String,
    pub transport: ProviderTransport,
    #[serde(default = "default_responses_path")]
    pub responses_path: String,
    #[serde(default = "default_chat_completions_path")]
    pub chat_completions_path: String,
    pub known_models: Vec<String>,
    #[serde(skip)]
    pub new_model_input: String,
}

fn default_responses_path() -> String {
    "v1/responses".to_string()
}

fn default_chat_completions_path() -> String {
    "v1/chat/completions".to_string()
}

impl ProviderProfile {
    #[must_use]
    pub fn openai_default() -> Self {
        Self {
            display_name: "ChatGPT / OpenAI".to_string(),
            base_url: "https://api.openai.com".to_string(),
            transport: ProviderTransport::Responses,
            responses_path: default_responses_path(),
            chat_completions_path: default_chat_completions_path(),
            known_models: vec![
                "gpt-4o".into(),
                "gpt-4o-mini".into(),
                "gpt-4.5".into(),
                "o3".into(),
            ],
            new_model_input: String::new(),
        }
    }

    #[must_use]
    pub fn compatible_default() -> Self {
        Self {
            display_name: "OpenAI-compatible API".to_string(),
            base_url: "http://localhost:11434".to_string(),
            transport: ProviderTransport::ChatCompletions,
            responses_path: default_responses_path(),
            chat_completions_path: default_chat_completions_path(),
            known_models: vec!["llama3.1".into(), "qwen2.5".into(), "mistral".into()],
            new_model_input: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub active_provider: AiProviderKind,
    pub openai: ProviderProfile,
    #[serde(rename = "openai_compatible")]
    pub compatible: ProviderProfile,
}

impl AppSettings {
    #[must_use]
    pub const fn active_profile(&self) -> &ProviderProfile {
        match self.active_provider {
            AiProviderKind::OpenAi => &self.openai,
            AiProviderKind::OpenAiCompatible => &self.compatible,
        }
    }

    #[must_use]
    pub const fn active_profile_mut(&mut self) -> &mut ProviderProfile {
        match self.active_provider {
            AiProviderKind::OpenAi => &mut self.openai,
            AiProviderKind::OpenAiCompatible => &mut self.compatible,
        }
    }

    #[must_use]
    pub fn active_models(&self) -> &[String] {
        &self.active_profile().known_models
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            active_provider: AiProviderKind::OpenAi,
            openai: ProviderProfile::openai_default(),
            compatible: ProviderProfile::compatible_default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacySettings {
    known_models: Vec<String>,
}

impl From<LegacySettings> for AppSettings {
    fn from(value: LegacySettings) -> Self {
        Self {
            active_provider: AiProviderKind::OpenAi,
            openai: ProviderProfile {
                known_models: if value.known_models.is_empty() {
                    ProviderProfile::openai_default().known_models
                } else {
                    value.known_models
                },
                ..ProviderProfile::openai_default()
            },
            compatible: ProviderProfile::compatible_default(),
        }
    }
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
        match serde_json::from_str::<AppSettings>(&text) {
            Ok(settings) => Ok(settings),
            Err(current_error) => serde_json::from_str::<LegacySettings>(&text)
                .map(AppSettings::from)
                .map_err(|legacy_error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "settings JSON invalid: current schema: {current_error}; legacy schema: {legacy_error}"
                        ),
                    )
                }),
        }
    }

    /// # Errors
    /// Returns an error if the settings cannot be serialized or written to disk.
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_settings_include_provider_profiles() {
        let settings = AppSettings::default();

        assert_eq!(settings.active_provider, AiProviderKind::OpenAi);
        assert_eq!(settings.openai.display_name, "ChatGPT / OpenAI");
        assert_eq!(settings.openai.base_url, "https://api.openai.com");
        assert_eq!(settings.openai.transport, ProviderTransport::Responses);
        assert!(settings
            .openai
            .known_models
            .contains(&"gpt-4.5".to_string()));
        assert_eq!(settings.compatible.display_name, "OpenAI-compatible API");
        assert_eq!(settings.compatible.base_url, "http://localhost:11434");
        assert_eq!(
            settings.compatible.transport,
            ProviderTransport::ChatCompletions
        );
        assert_eq!(settings.openai.responses_path, "v1/responses");
        assert_eq!(settings.openai.chat_completions_path, "v1/chat/completions");
        assert_eq!(settings.compatible.responses_path, "v1/responses");
        assert_eq!(
            settings.compatible.chat_completions_path,
            "v1/chat/completions"
        );
        assert!(settings
            .compatible
            .known_models
            .contains(&"llama3.1".to_string()));
        assert!(settings.openai.new_model_input.is_empty());
        assert!(settings.compatible.new_model_input.is_empty());
    }

    #[test]
    fn active_profile_tracks_active_provider() {
        let mut settings = AppSettings {
            active_provider: AiProviderKind::OpenAi,
            ..Default::default()
        };

        assert_eq!(settings.active_profile().display_name, "ChatGPT / OpenAI");

        settings.active_provider = AiProviderKind::OpenAiCompatible;
        assert_eq!(
            settings.active_profile().display_name,
            "OpenAI-compatible API"
        );
        settings
            .active_profile_mut()
            .known_models
            .push("custom-model".to_string());

        assert!(settings
            .compatible
            .known_models
            .contains(&"custom-model".to_string()));
        assert!(!settings
            .openai
            .known_models
            .contains(&"custom-model".to_string()));
    }

    #[test]
    fn loads_legacy_known_models_into_openai_profile() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "known_models": ["legacy-a", "legacy-b"]
}"#,
        )
        .unwrap();
        let store = FileSettingsStore::new(path);

        let settings = store.load().unwrap();

        assert_eq!(settings.active_provider, AiProviderKind::OpenAi);
        assert_eq!(
            settings.openai.known_models,
            vec!["legacy-a".to_string(), "legacy-b".to_string()]
        );
        assert_eq!(
            settings.compatible.known_models,
            ProviderProfile::compatible_default().known_models
        );
    }

    #[test]
    fn saves_provider_settings_without_transient_model_inputs() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let settings = AppSettings {
            active_provider: AiProviderKind::OpenAiCompatible,
            compatible: ProviderProfile {
                base_url: "https://models.example.test".to_string(),
                transport: ProviderTransport::Responses,
                responses_path: "custom/responses".to_string(),
                chat_completions_path: "custom/chat/completions".to_string(),
                known_models: vec!["vendor-model".to_string()],
                new_model_input: "draft-model".to_string(),
                ..ProviderProfile::compatible_default()
            },
            ..Default::default()
        };

        store.save(&settings).unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();
        let loaded = store.load().unwrap();

        assert!(raw.contains("\"active_provider\""));
        assert!(raw.contains("\"openai_compatible\""));
        assert!(!raw.contains("draft-model"));
        assert_eq!(loaded.active_provider, AiProviderKind::OpenAiCompatible);
        assert_eq!(loaded.compatible.base_url, "https://models.example.test");
        assert_eq!(loaded.compatible.transport, ProviderTransport::Responses);
        assert_eq!(loaded.compatible.responses_path, "custom/responses");
        assert_eq!(
            loaded.compatible.chat_completions_path,
            "custom/chat/completions"
        );
        assert_eq!(
            loaded.compatible.known_models,
            vec!["vendor-model".to_string()]
        );
        assert!(loaded.compatible.new_model_input.is_empty());
    }

    #[test]
    fn missing_settings_file_loads_default_settings() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));

        let settings = store.load().unwrap();

        assert_eq!(settings, AppSettings::default());
    }

    #[test]
    fn invalid_settings_json_returns_invalid_data_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let store = FileSettingsStore::new(path);

        let error = store.load().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("settings JSON invalid"));
    }

    #[test]
    fn atomic_save_does_not_leave_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = FileSettingsStore::new(&path);
        let settings = AppSettings::default();

        store.save(&settings).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn settings_roundtrip_restores_identical_state() {
        let dir = tempdir().unwrap();
        let store = FileSettingsStore::new(dir.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings.active_provider = AiProviderKind::OpenAiCompatible;
        settings.compatible.known_models.push("grok-1".to_string());
        settings.openai.transport = ProviderTransport::ChatCompletions;

        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded, settings);
    }
}
