use super::{AppBackend, BackendError};

impl AppBackend {
    pub(super) fn authoring_provider(
        &self,
        settings: &crate::settings::model::AppSettings,
        transient_api_key: Option<&str>,
    ) -> Result<(crate::settings::model::AppSettings, Box<dyn engine::AiPort>), BackendError> {
        let mut merged = settings.clone();
        crate::settings::model::merge_preserved_secrets(
            &mut merged,
            &self.settings.store().load()?,
        );
        let mut provider_config = crate::settings::provider::resolve_provider_config(
            &merged,
            transient_api_key,
            self.settings.env(),
        )?;
        crate::settings::provider::attach_codex_credential_sink(
            &mut provider_config,
            self.settings.store_arc(),
        );
        let ai = providers::create_provider(provider_config);
        Ok((merged, ai))
    }

    pub fn backend_err(&self, error: BackendError) -> BackendError {
        log::warn!("backend error: {error}");
        error
    }

    pub(super) fn persistence_err(&self, code: &str, error: BackendError) -> BackendError {
        log::warn!("{code}: {error}");
        error
    }
}
