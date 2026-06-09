//! LSP writethrough settings (env + persisted app settings).

use crate::settings::model::LspSettings as PersistedLspSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSettings {
    pub enabled: bool,
    pub format_on_write: bool,
    pub diagnostics_on_write: bool,
    pub timeout_ms: u64,
}

impl Default for LspSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            format_on_write: false,
            diagnostics_on_write: false,
            timeout_ms: 5_000,
        }
    }
}

impl LspSettings {
    fn apply_env_overrides(&mut self) {
        if matches!(
            std::env::var("PI_LSP_ENABLED").as_deref(),
            Ok("0") | Ok("false") | Ok("off")
        ) {
            self.enabled = false;
        }
        if matches!(
            std::env::var("PI_LSP_FORMAT_ON_WRITE").as_deref(),
            Ok("1") | Ok("true") | Ok("on")
        ) {
            self.format_on_write = true;
        }
        if matches!(
            std::env::var("PI_LSP_DIAGNOSTICS_ON_WRITE").as_deref(),
            Ok("1") | Ok("true") | Ok("on")
        ) {
            self.diagnostics_on_write = true;
        }
        if let Ok(value) = std::env::var("PI_LSP_TIMEOUT_MS") {
            if let Ok(timeout) = value.parse() {
                self.timeout_ms = timeout;
            }
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        let mut settings = Self::default();
        settings.apply_env_overrides();
        settings
    }

    #[must_use]
    pub fn from_persisted(persisted: &PersistedLspSettings) -> Self {
        let mut settings = Self {
            enabled: persisted.enabled,
            format_on_write: persisted.format_on_write,
            diagnostics_on_write: persisted.diagnostics_on_write,
            ..Self::default()
        };
        settings.apply_env_overrides();
        settings
    }

    #[must_use]
    pub fn writethrough_active(&self) -> bool {
        self.enabled && (self.format_on_write || self.diagnostics_on_write)
    }
}
