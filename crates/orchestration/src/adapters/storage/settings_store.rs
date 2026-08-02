use crate::adapters::storage::json_file_store::{write_json_file, OPENFLOW_DATA_DIR_SLUG};
use crate::mcp::model::{McpAuth, McpConnection, PersistedValue};
use crate::mcp::ports::{mcp_secret_ref, McpSecretRef, SecretStore};
use crate::settings::model::{merge_preserved_secrets, AppSettings};
use crate::settings::ports::SettingsStore;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse_settings_json(text: &str) -> Result<AppSettings, serde_json::Error> {
    serde_json::from_str::<AppSettings>(text).map(AppSettings::normalized)
}

#[derive(Clone)]
pub struct FileSettingsStore {
    path: PathBuf,
    secret_store: Arc<dyn SecretStore>,
    legacy_secret_migration: Option<LegacySecretMigration>,
}

#[derive(Clone)]
struct LegacySecretMigration {
    target: Arc<crate::adapters::mcp::FileSecretStore>,
    source: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for FileSettingsStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileSettingsStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl FileSettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let target = Arc::new(crate::adapters::mcp::FileSecretStore::new(
            path.with_file_name(crate::adapters::mcp::MCP_SECRET_FILE_NAME),
        ));
        Self::new_with_secret_migration(
            path,
            target,
            Arc::new(crate::adapters::mcp::LegacyKeyringSecretStore::new()),
        )
    }

    pub fn new_with_secret_store(
        path: impl Into<PathBuf>,
        secret_store: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            path: path.into(),
            secret_store,
            legacy_secret_migration: None,
        }
    }

    fn new_with_secret_migration(
        path: impl Into<PathBuf>,
        target: Arc<crate::adapters::mcp::FileSecretStore>,
        source: Arc<dyn SecretStore>,
    ) -> Self {
        let secret_store: Arc<dyn SecretStore> = target.clone();
        Self {
            path: path.into(),
            secret_store,
            legacy_secret_migration: Some(LegacySecretMigration { target, source }),
        }
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
            Ok(mut settings) => {
                if let Some(migration) = self.legacy_secret_migration.as_ref() {
                    migrate_legacy_mcp_secrets(&settings, migration)?;
                }
                let mutations = secure_mcp_values(&mut settings, self.secret_store.as_ref())?;
                if !mutations.is_empty() {
                    if let Err(error) = write_json_file(&self.path, &settings, "settings") {
                        rollback_secret_mutations(self.secret_store.as_ref(), &mutations);
                        return Err(error);
                    }
                }
                Ok(settings)
            }
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
        let mut secured = settings.clone();
        if let Some(migration) = self.legacy_secret_migration.as_ref() {
            migrate_legacy_mcp_secrets(&secured, migration)?;
        }
        let mutations = secure_mcp_values(&mut secured, self.secret_store.as_ref())?;
        if let Err(error) = write_json_file(&self.path, &secured, "settings") {
            rollback_secret_mutations(self.secret_store.as_ref(), &mutations);
            return Err(error);
        }
        Ok(())
    }

    /// # Errors
    /// Returns an error if the settings cannot be serialized or written to disk.
    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        let mut to_save = settings.clone();
        let mut existing_settings = None;
        if self.path.exists() {
            let existing = self.load()?;
            merge_preserved_secrets(&mut to_save, &existing);
            existing_settings = Some(existing);
        }
        let removed = if let Some(existing) = existing_settings.as_ref() {
            delete_removed_mcp_secrets(self.secret_store.as_ref(), existing, &to_save)?
        } else {
            Vec::new()
        };
        if let Err(error) = self.save_raw(&to_save) {
            restore_deleted_mcp_secrets(self.secret_store.as_ref(), &removed);
            return Err(error);
        }
        Ok(())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn migrate_legacy_mcp_secrets(
    settings: &AppSettings,
    migration: &LegacySecretMigration,
) -> io::Result<()> {
    if migration
        .target
        .legacy_keychain_migration_complete()
        .map_err(io::Error::other)?
    {
        return Ok(());
    }

    let refs = collect_mcp_secret_refs(settings)?;
    let mut legacy_refs = Vec::new();
    for secret_ref in refs.keys() {
        let legacy_value = match migration.source.get(secret_ref) {
            Ok(value) => value,
            Err(crate::mcp::ports::SecretStoreError::UnsupportedPlatform) => None,
            Err(error) => return Err(io::Error::other(error)),
        };
        let Some(legacy_value) = legacy_value else {
            continue;
        };
        if migration
            .target
            .get(secret_ref)
            .map_err(io::Error::other)?
            .is_none()
        {
            migration
                .target
                .set(secret_ref, &legacy_value)
                .map_err(io::Error::other)?;
            let verified = migration.target.get(secret_ref).map_err(io::Error::other)?;
            if verified.as_deref() != Some(legacy_value.as_str()) {
                return Err(io::Error::other(
                    "MCP secret storage verification failed during keychain migration",
                ));
            }
        }
        legacy_refs.push(secret_ref.clone());
    }

    for secret_ref in legacy_refs {
        migration
            .source
            .delete(&secret_ref)
            .map_err(io::Error::other)?;
    }
    migration
        .target
        .mark_legacy_keychain_migration_complete()
        .map_err(io::Error::other)
}

#[derive(Debug)]
struct SecretMutation {
    secret_ref: McpSecretRef,
    previous: Option<String>,
}

fn secure_mcp_values(
    settings: &mut AppSettings,
    secret_store: &dyn SecretStore,
) -> io::Result<Vec<SecretMutation>> {
    let mut mutations = Vec::new();
    for server in &mut settings.mcp.servers {
        let (prefix, values) = match &mut server.connection {
            McpConnection::Stdio { environment, .. } => ("env", environment),
            McpConnection::StreamableHttp { headers, .. }
            | McpConnection::LegacySse { headers, .. } => ("header", headers),
        };
        for (key, value) in values {
            match value {
                PersistedValue::Literal { value: literal } if !literal.is_empty() => {
                    let secret_ref = match mcp_secret_ref(&server.id, &format!("{prefix}.{key}")) {
                        Ok(secret_ref) => secret_ref,
                        Err(error) => {
                            rollback_secret_mutations(secret_store, &mutations);
                            return Err(io::Error::other(error));
                        }
                    };
                    let previous = match secret_store.get(&secret_ref) {
                        Ok(previous) => previous,
                        Err(error) => {
                            rollback_secret_mutations(secret_store, &mutations);
                            return Err(io::Error::other(error));
                        }
                    };
                    if let Err(error) = secret_store.set(&secret_ref, literal) {
                        mutations.push(SecretMutation {
                            secret_ref,
                            previous,
                        });
                        rollback_secret_mutations(secret_store, &mutations);
                        return Err(io::Error::other(error));
                    }
                    let verified = match secret_store.get(&secret_ref) {
                        Ok(verified) => verified,
                        Err(error) => {
                            mutations.push(SecretMutation {
                                secret_ref,
                                previous,
                            });
                            rollback_secret_mutations(secret_store, &mutations);
                            return Err(io::Error::other(error));
                        }
                    };
                    if verified.as_deref() != Some(literal.as_str()) {
                        let current = SecretMutation {
                            secret_ref,
                            previous,
                        };
                        mutations.push(current);
                        rollback_secret_mutations(secret_store, &mutations);
                        return Err(io::Error::other(
                            "MCP secret storage verification failed during write",
                        ));
                    }
                    let resolved_value = Some(literal.clone());
                    let wire_ref = secret_ref.to_string();
                    mutations.push(SecretMutation {
                        secret_ref,
                        previous,
                    });
                    *value = PersistedValue::Secret {
                        secret_ref: wire_ref,
                        resolved_value,
                    };
                }
                PersistedValue::Literal { .. } => {}
                PersistedValue::Secret {
                    secret_ref,
                    resolved_value,
                } => {
                    let parsed = match secret_ref.parse::<McpSecretRef>() {
                        Ok(parsed) => parsed,
                        Err(error) => {
                            rollback_secret_mutations(secret_store, &mutations);
                            return Err(io::Error::other(error));
                        }
                    };
                    *resolved_value = match secret_store.get(&parsed) {
                        Ok(value) => value,
                        Err(error) => {
                            rollback_secret_mutations(secret_store, &mutations);
                            return Err(io::Error::other(error));
                        }
                    };
                }
            }
        }
        let auth = match &mut server.connection {
            McpConnection::StreamableHttp { auth, .. } | McpConnection::LegacySse { auth, .. } => {
                auth
            }
            McpConnection::Stdio { .. } => continue,
        };
        if let McpAuth::Static {
            secret_ref,
            resolved_value,
            ..
        } = auth
        {
            let parsed = match secret_ref.parse::<McpSecretRef>() {
                Ok(parsed) => parsed,
                Err(error) => {
                    rollback_secret_mutations(secret_store, &mutations);
                    return Err(io::Error::other(error));
                }
            };
            *resolved_value = match secret_store.get(&parsed) {
                Ok(value) => value,
                Err(error) => {
                    rollback_secret_mutations(secret_store, &mutations);
                    return Err(io::Error::other(error));
                }
            };
        }
    }
    Ok(mutations)
}

fn rollback_secret_mutations(secret_store: &dyn SecretStore, mutations: &[SecretMutation]) {
    for mutation in mutations.iter().rev() {
        if let Some(previous) = mutation.previous.as_deref() {
            let _ = secret_store.set(&mutation.secret_ref, previous);
        } else {
            let _ = secret_store.delete(&mutation.secret_ref);
        }
    }
}

fn delete_removed_mcp_secrets(
    secret_store: &dyn SecretStore,
    existing: &AppSettings,
    incoming: &AppSettings,
) -> io::Result<Vec<(McpSecretRef, Option<String>)>> {
    let existing_refs = collect_mcp_secret_refs(existing)?;
    let incoming_refs = collect_mcp_secret_refs(incoming)?;
    let mut deleted = Vec::new();
    for (secret_ref, _resolved_value) in existing_refs {
        if incoming_refs.contains_key(&secret_ref) {
            continue;
        }
        let previous = secret_store.get(&secret_ref).map_err(io::Error::other)?;
        if let Err(error) = secret_store.delete(&secret_ref) {
            restore_deleted_mcp_secrets(secret_store, &deleted);
            return Err(io::Error::other(error));
        }
        deleted.push((secret_ref, previous));
    }
    Ok(deleted)
}

fn restore_deleted_mcp_secrets(
    secret_store: &dyn SecretStore,
    deleted: &[(McpSecretRef, Option<String>)],
) {
    for (secret_ref, value) in deleted {
        if let Some(value) = value {
            let _ = secret_store.set(secret_ref, value);
        }
    }
}

fn collect_mcp_secret_refs(
    settings: &AppSettings,
) -> io::Result<std::collections::BTreeMap<McpSecretRef, Option<String>>> {
    let mut refs = std::collections::BTreeMap::new();
    for server in &settings.mcp.servers {
        let values = match &server.connection {
            McpConnection::Stdio { environment, .. } => environment,
            McpConnection::StreamableHttp { headers, .. }
            | McpConnection::LegacySse { headers, .. } => headers,
        };
        for value in values.values() {
            if let PersistedValue::Secret {
                secret_ref,
                resolved_value,
            } = value
            {
                refs.insert(
                    secret_ref
                        .parse::<McpSecretRef>()
                        .map_err(io::Error::other)?,
                    resolved_value.clone(),
                );
            }
        }
        let auth = match &server.connection {
            McpConnection::StreamableHttp { auth, .. } | McpConnection::LegacySse { auth, .. } => {
                auth
            }
            McpConnection::Stdio { .. } => continue,
        };
        match auth {
            McpAuth::Static {
                secret_ref,
                resolved_value,
                ..
            } => {
                refs.insert(
                    secret_ref
                        .parse::<McpSecretRef>()
                        .map_err(io::Error::other)?,
                    resolved_value.clone(),
                );
            }
            McpAuth::OAuth {
                credential_ref: Some(credential_ref),
                ..
            } => {
                refs.insert(
                    credential_ref
                        .parse::<McpSecretRef>()
                        .map_err(io::Error::other)?,
                    None,
                );
            }
            McpAuth::None | McpAuth::OAuth { .. } => {}
        }
    }
    Ok(refs)
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

    fn set_mcp_secret(&self, secret_ref: &McpSecretRef, value: &str) -> io::Result<()> {
        self.secret_store
            .set(secret_ref, value)
            .map_err(io::Error::other)?;
        let verified = self
            .secret_store
            .get(secret_ref)
            .map_err(io::Error::other)?;
        if verified.as_deref() != Some(value) {
            return Err(io::Error::other(
                "MCP secret storage verification failed during write",
            ));
        }
        Ok(())
    }

    fn get_mcp_secret(&self, secret_ref: &McpSecretRef) -> io::Result<Option<String>> {
        self.secret_store.get(secret_ref).map_err(io::Error::other)
    }

    fn delete_mcp_secret(&self, secret_ref: &McpSecretRef) -> io::Result<()> {
        self.secret_store
            .delete(secret_ref)
            .map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::mcp::FileSecretStore;
    use crate::mcp::model::{
        McpConnection, McpInstall, McpServerRecord, McpServerSource, PersistedValue,
    };
    use crate::mcp::ports::{McpSecretRef, SecretStore, SecretStoreError};
    use crate::settings::model::LspSettings;
    use crate::settings::ports::ProviderTransport;
    use providers::ProviderId;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<HashMap<McpSecretRef, String>>,
        fail_after_sets: Mutex<Option<usize>>,
        set_count: Mutex<usize>,
    }

    impl MemorySecretStore {
        fn fail_after_sets(count: usize) -> Self {
            Self {
                fail_after_sets: Mutex::new(Some(count)),
                ..Self::default()
            }
        }
    }

    impl SecretStore for MemorySecretStore {
        fn get(&self, secret_ref: &McpSecretRef) -> Result<Option<String>, SecretStoreError> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(
            &self,
            secret_ref: &McpSecretRef,
            secret_value: &str,
        ) -> Result<(), SecretStoreError> {
            let mut count = self.set_count.lock().unwrap();
            if self
                .fail_after_sets
                .lock()
                .unwrap()
                .is_some_and(|limit| *count >= limit)
            {
                return Err(SecretStoreError::Unavailable {
                    operation: crate::mcp::ports::SecretStoreOperation::Set,
                });
            }
            *count += 1;
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.clone(), secret_value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &McpSecretRef) -> Result<(), SecretStoreError> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    fn store_with_memory_secrets(
        path: impl Into<PathBuf>,
    ) -> (FileSettingsStore, Arc<MemorySecretStore>) {
        let secrets = Arc::new(MemorySecretStore::default());
        (
            FileSettingsStore::new_with_secret_store(path, secrets.clone()),
            secrets,
        )
    }

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
    fn legacy_mcp_plaintext_migrates_to_verified_secret_refs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value["mcp"] = serde_json::json!({
            "servers": [{
                "id": "massive",
                "displayName": "Massive",
                "command": "mcp_massive",
                "args": [],
                "env": {
                    "MASSIVE_API_KEY": "must-leave-json",
                    "MCP_MODE": "stdio"
                },
                "enabled": true
            }]
        });
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let (store, secrets) = store_with_memory_secrets(&path);

        let loaded = store.load().unwrap();

        let McpConnection::Stdio { environment, .. } = &loaded.mcp.servers[0].connection else {
            panic!("stdio server");
        };
        for (key, expected) in [
            ("MASSIVE_API_KEY", "must-leave-json"),
            ("MCP_MODE", "stdio"),
        ] {
            let PersistedValue::Secret {
                secret_ref,
                resolved_value,
            } = &environment[key]
            else {
                panic!("legacy value must become a secret ref");
            };
            assert_eq!(resolved_value.as_deref(), Some(expected));
            let parsed = secret_ref.parse::<McpSecretRef>().unwrap();
            assert_eq!(secrets.get(&parsed).unwrap().as_deref(), Some(expected));
        }
        assert!(!loaded.mcp.servers[0].enabled);
        assert!(!crate::mcp::trust::is_trusted(&loaded.mcp.servers[0]));
        let persisted = fs::read_to_string(&path).unwrap();
        assert!(!persisted.contains("must-leave-json"));
        assert!(persisted.contains("mcp-secret:v1:"));
        let persisted: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        let environment = &persisted["mcp"]["servers"][0]["connection"]["environment"];
        assert_eq!(environment["MCP_MODE"]["type"], "secret");
        assert!(environment["MCP_MODE"].get("value").is_none());
    }

    #[test]
    fn failed_secret_migration_rolls_back_and_leaves_settings_unchanged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value["mcp"] = serde_json::json!({
            "servers": [{
                "id": "massive",
                "displayName": "Massive",
                "command": "mcp_massive",
                "env": {"FIRST": "secret-one", "SECOND": "secret-two"}
            }]
        });
        let original = serde_json::to_string_pretty(&value).unwrap();
        fs::write(&path, &original).unwrap();
        let secrets = Arc::new(MemorySecretStore::fail_after_sets(1));
        let store = FileSettingsStore::new_with_secret_store(&path, secrets.clone());

        let error = store.load().expect_err("second secret write must fail");

        assert!(error.to_string().contains("MCP secret storage"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(secrets.values.lock().unwrap().is_empty());
    }

    #[test]
    fn opaque_static_and_oauth_refs_migrate_from_legacy_store_once() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let secret_path = dir.path().join("mcp-secrets.json");
        let target = Arc::new(FileSecretStore::new(&secret_path));
        let legacy = Arc::new(MemorySecretStore::default());
        let static_ref = mcp_secret_ref("massive", "env.MASSIVE_API_KEY").unwrap();
        let oauth_ref = mcp_secret_ref("hosted", "oauth.credentials").unwrap();
        legacy.set(&static_ref, "massive-secret").unwrap();
        legacy.set(&oauth_ref, "oauth-json").unwrap();

        let mut settings = AppSettings::default();
        settings.mcp.servers.push(McpServerRecord::new(
            "massive",
            "Massive",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "mcp_massive".into(),
                args: Vec::new(),
                environment: std::collections::BTreeMap::from([(
                    "MASSIVE_API_KEY".into(),
                    PersistedValue::Secret {
                        secret_ref: static_ref.to_string(),
                        resolved_value: None,
                    },
                )]),
            },
        ));
        settings.mcp.servers.push(McpServerRecord::new(
            "hosted",
            "Hosted",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::StreamableHttp {
                url: "https://mcp.example.test/mcp".into(),
                allow_localhost: false,
                headers: Default::default(),
                auth: McpAuth::OAuth {
                    client_id: "openflow".into(),
                    scopes: Vec::new(),
                    issuer: None,
                    credential_ref: Some(oauth_ref.to_string()),
                },
            },
        ));
        write_json_file(&settings_path, &settings, "settings").unwrap();
        let store = FileSettingsStore::new_with_secret_migration(
            &settings_path,
            target.clone(),
            legacy.clone(),
        );

        let loaded = store.load().unwrap();

        assert_eq!(
            target.get(&static_ref).unwrap().as_deref(),
            Some("massive-secret")
        );
        assert_eq!(
            target.get(&oauth_ref).unwrap().as_deref(),
            Some("oauth-json")
        );
        assert_eq!(legacy.get(&static_ref).unwrap(), None);
        assert_eq!(legacy.get(&oauth_ref).unwrap(), None);
        assert!(target.legacy_keychain_migration_complete().unwrap());
        let McpConnection::Stdio { environment, .. } = &loaded.mcp.servers[0].connection else {
            panic!("stdio server");
        };
        assert_eq!(
            environment["MASSIVE_API_KEY"].runtime_value(),
            Some("massive-secret")
        );

        legacy.set(&static_ref, "must-not-be-read").unwrap();
        let loaded_again = store.load().unwrap();
        let McpConnection::Stdio { environment, .. } = &loaded_again.mcp.servers[0].connection
        else {
            panic!("stdio server");
        };
        assert_eq!(
            environment["MASSIVE_API_KEY"].runtime_value(),
            Some("massive-secret")
        );
        assert_eq!(
            legacy.get(&static_ref).unwrap().as_deref(),
            Some("must-not-be-read")
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_keychain_migration_keeps_legacy_value() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let target_path = dir.path().join("mcp-secrets.json");
        let symlink_target = dir.path().join("outside.json");
        fs::write(&symlink_target, r#"{"version":1,"secrets":{}}"#).unwrap();
        symlink(&symlink_target, &target_path).unwrap();
        let target = Arc::new(FileSecretStore::new(&target_path));
        let legacy = Arc::new(MemorySecretStore::default());
        let secret_ref = mcp_secret_ref("massive", "env.MASSIVE_API_KEY").unwrap();
        legacy.set(&secret_ref, "legacy-secret").unwrap();
        let mut settings = AppSettings::default();
        settings.mcp.servers.push(McpServerRecord::new(
            "massive",
            "Massive",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "mcp_massive".into(),
                args: Vec::new(),
                environment: std::collections::BTreeMap::from([(
                    "MASSIVE_API_KEY".into(),
                    PersistedValue::Secret {
                        secret_ref: secret_ref.to_string(),
                        resolved_value: None,
                    },
                )]),
            },
        ));
        write_json_file(&settings_path, &settings, "settings").unwrap();
        let store =
            FileSettingsStore::new_with_secret_migration(&settings_path, target, legacy.clone());

        assert!(store.load().is_err());
        assert_eq!(
            legacy.get(&secret_ref).unwrap().as_deref(),
            Some("legacy-secret")
        );
        assert!(!fs::read_to_string(symlink_target)
            .unwrap()
            .contains("legacy-secret"));
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
    fn save_from_redacted_snapshot_preserves_existing_mcp_environment() {
        let dir = tempdir().unwrap();
        let (store, _secrets) = store_with_memory_secrets(dir.path().join("settings.json"));
        let mut settings = AppSettings::default();
        let mut server = McpServerRecord::new(
            "massive",
            "Massive",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "mcp_massive".into(),
                args: Vec::new(),
                environment: std::collections::BTreeMap::from([(
                    "MASSIVE_API_KEY".into(),
                    PersistedValue::Literal {
                        value: "secret".into(),
                    },
                )]),
            },
        );
        server.enabled = true;
        settings.mcp.servers.push(server);
        store.save(&settings).unwrap();

        let mut redacted = store.load().unwrap().redacted();
        redacted.mcp.servers[0].display_name = "Massive MCP".into();
        store.save(&redacted).unwrap();

        let loaded = store.load().unwrap();
        let McpConnection::Stdio { environment, .. } = &loaded.mcp.servers[0].connection else {
            panic!("stdio server");
        };
        assert_eq!(
            environment["MASSIVE_API_KEY"].runtime_value(),
            Some("secret")
        );
        assert!(matches!(
            environment.get("MASSIVE_API_KEY"),
            Some(PersistedValue::Secret { .. })
        ));
        assert_eq!(loaded.mcp.servers[0].display_name, "Massive MCP");
    }

    #[test]
    fn removing_mcp_secret_ref_deletes_secret_store_entry() {
        let dir = tempdir().unwrap();
        let (store, secrets) = store_with_memory_secrets(dir.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings.mcp.servers.push(McpServerRecord::new(
            "massive",
            "Massive",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::Stdio {
                command: "mcp_massive".into(),
                args: Vec::new(),
                environment: std::collections::BTreeMap::from([(
                    "MASSIVE_API_KEY".into(),
                    PersistedValue::Literal {
                        value: "secret".into(),
                    },
                )]),
            },
        ));
        store.save(&settings).unwrap();
        assert_eq!(secrets.values.lock().unwrap().len(), 1);

        store.save(&AppSettings::default()).unwrap();

        assert!(secrets.values.lock().unwrap().is_empty());
    }

    #[test]
    fn removing_oauth_credential_ref_deletes_secret_store_entry() {
        let dir = tempdir().unwrap();
        let (store, secrets) = store_with_memory_secrets(dir.path().join("settings.json"));
        let credential_ref = mcp_secret_ref("hosted", "oauth.credentials").unwrap();
        secrets
            .set(&credential_ref, "oauth-credential-json")
            .unwrap();
        let mut settings = AppSettings::default();
        settings.mcp.servers.push(McpServerRecord::new(
            "hosted",
            "Hosted",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::StreamableHttp {
                url: "https://mcp.example.test/mcp".to_string(),
                allow_localhost: false,
                headers: Default::default(),
                auth: McpAuth::OAuth {
                    client_id: "openflow".to_string(),
                    scopes: vec!["tools.read".to_string()],
                    issuer: Some("https://auth.example.test".to_string()),
                    credential_ref: Some(credential_ref.to_string()),
                },
            },
        ));
        store.save(&settings).unwrap();

        store.save(&AppSettings::default()).unwrap();

        assert_eq!(secrets.get(&credential_ref).unwrap(), None);
    }

    #[test]
    #[cfg(unix)]
    fn failed_settings_save_restores_deleted_oauth_credentials() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let (store, secrets) = store_with_memory_secrets(&path);
        let credential_ref = mcp_secret_ref("hosted", "oauth.credentials").unwrap();
        secrets
            .set(&credential_ref, "oauth-credential-json")
            .unwrap();
        let mut settings = AppSettings::default();
        settings.mcp.servers.push(McpServerRecord::new(
            "hosted",
            "Hosted",
            McpServerSource::Manual,
            McpInstall::External,
            McpConnection::StreamableHttp {
                url: "https://mcp.example.test/mcp".to_string(),
                allow_localhost: false,
                headers: Default::default(),
                auth: McpAuth::OAuth {
                    client_id: "openflow".to_string(),
                    scopes: Vec::new(),
                    issuer: Some("https://auth.example.test".to_string()),
                    credential_ref: Some(credential_ref.to_string()),
                },
            },
        ));
        store.save(&settings).unwrap();
        let mut permissions = fs::metadata(dir.path()).unwrap().permissions();
        permissions.set_mode(0o555);
        fs::set_permissions(dir.path(), permissions).unwrap();

        let result = store.save(&AppSettings::default());

        let mut restore = fs::metadata(dir.path()).unwrap().permissions();
        restore.set_mode(0o755);
        fs::set_permissions(dir.path(), restore).unwrap();
        assert!(result.is_err());
        assert_eq!(
            secrets.get(&credential_ref).unwrap().as_deref(),
            Some("oauth-credential-json")
        );
        assert!(path.exists());
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

        assert!(store
            .load()
            .unwrap()
            .providers
            .get(&ProviderId::from("openai"))
            .expect("openai profile")
            .api_key
            .is_empty());
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
