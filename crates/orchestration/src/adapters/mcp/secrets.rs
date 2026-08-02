use crate::mcp::ports::{McpSecretRef, SecretStore, SecretStoreError, SecretStoreOperation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

pub const MCP_SECRET_FILE_NAME: &str = "mcp-secrets.json";
const MCP_SECRET_FILE_VERSION: u32 = 1;
const MAX_MCP_SECRET_FILE_BYTES: u64 = 16 * 1024 * 1024;
static MCP_SECRET_FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone)]
pub struct FileSecretStore {
    path: PathBuf,
}

impl std::fmt::Debug for FileSecretStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileSecretStore")
            .field("path", &self.path)
            .finish()
    }
}

impl FileSecretStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(crate::adapters::storage::json_file_store::OPENFLOW_DATA_DIR_SLUG)
            .join(MCP_SECRET_FILE_NAME)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn legacy_keychain_migration_complete(&self) -> Result<bool, SecretStoreError> {
        let _guard = self.lock(SecretStoreOperation::Get)?;
        Ok(self
            .load(SecretStoreOperation::Get)?
            .legacy_keychain_migration_complete)
    }

    pub(crate) fn mark_legacy_keychain_migration_complete(&self) -> Result<(), SecretStoreError> {
        let _guard = self.lock(SecretStoreOperation::Set)?;
        let mut document = self.load(SecretStoreOperation::Set)?;
        document.legacy_keychain_migration_complete = true;
        self.save(&document, SecretStoreOperation::Set)
    }

    fn lock(
        &self,
        operation: SecretStoreOperation,
    ) -> Result<MutexGuard<'static, ()>, SecretStoreError> {
        MCP_SECRET_FILE_LOCK
            .lock()
            .map_err(|_| SecretStoreError::Backend { operation })
    }

    fn load(&self, operation: SecretStoreOperation) -> Result<SecretFile, SecretStoreError> {
        reject_symlink(&self.path, operation)?;
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SecretFile::default());
            }
            Err(error) => return Err(map_io_error(&error, operation)),
        };
        if metadata.len() > MAX_MCP_SECRET_FILE_BYTES {
            return Err(SecretStoreError::InvalidRequest { operation });
        }
        if let Some(parent) = self.path.parent() {
            set_private_directory_permissions(parent, operation)?;
        }
        set_private_file_permissions(&self.path, operation)?;
        let text =
            fs::read_to_string(&self.path).map_err(|error| map_io_error(&error, operation))?;
        let document: SecretFile = serde_json::from_str(&text)
            .map_err(|_| SecretStoreError::InvalidRequest { operation })?;
        if document.version != MCP_SECRET_FILE_VERSION {
            return Err(SecretStoreError::InvalidRequest { operation });
        }
        Ok(document)
    }

    fn save(
        &self,
        document: &SecretFile,
        operation: SecretStoreOperation,
    ) -> Result<(), SecretStoreError> {
        let parent = self
            .path
            .parent()
            .ok_or(SecretStoreError::InvalidRequest { operation })?;
        reject_symlink(parent, operation)?;
        fs::create_dir_all(parent).map_err(|error| map_io_error(&error, operation))?;
        set_private_directory_permissions(parent, operation)?;
        reject_symlink(&self.path, operation)?;

        let text = serde_json::to_string_pretty(document)
            .map_err(|_| SecretStoreError::Backend { operation })?;
        if text.len() as u64 > MAX_MCP_SECRET_FILE_BYTES {
            return Err(SecretStoreError::InvalidRequest { operation });
        }
        let temporary_path = self.path.with_extension(format!("tmp-{}", Uuid::new_v4()));
        let mut pending = PendingSecretFile::new(temporary_path.clone());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        set_private_file_mode(&mut options);
        {
            let mut file = options
                .open(&temporary_path)
                .map_err(|error| map_io_error(&error, operation))?;
            file.write_all(text.as_bytes())
                .map_err(|error| map_io_error(&error, operation))?;
            file.sync_all()
                .map_err(|error| map_io_error(&error, operation))?;
        }
        reject_symlink(&self.path, operation)?;
        fs::rename(&temporary_path, &self.path).map_err(|error| map_io_error(&error, operation))?;
        pending.disarm();
        set_private_file_permissions(&self.path, operation)?;
        if let Ok(directory) = fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    }
}

impl SecretStore for FileSecretStore {
    fn get(&self, secret_ref: &McpSecretRef) -> Result<Option<String>, SecretStoreError> {
        let _guard = self.lock(SecretStoreOperation::Get)?;
        Ok(self
            .load(SecretStoreOperation::Get)?
            .secrets
            .get(secret_ref.as_str())
            .cloned())
    }

    fn set(&self, secret_ref: &McpSecretRef, secret_value: &str) -> Result<(), SecretStoreError> {
        let _guard = self.lock(SecretStoreOperation::Set)?;
        let mut document = self.load(SecretStoreOperation::Set)?;
        document
            .secrets
            .insert(secret_ref.as_str().to_string(), secret_value.to_string());
        self.save(&document, SecretStoreOperation::Set)
    }

    fn delete(&self, secret_ref: &McpSecretRef) -> Result<(), SecretStoreError> {
        let _guard = self.lock(SecretStoreOperation::Delete)?;
        let mut document = self.load(SecretStoreOperation::Delete)?;
        if document.secrets.remove(secret_ref.as_str()).is_some() {
            self.save(&document, SecretStoreOperation::Delete)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretFile {
    version: u32,
    #[serde(default)]
    legacy_keychain_migration_complete: bool,
    #[serde(default)]
    secrets: BTreeMap<String, String>,
}

impl Default for SecretFile {
    fn default() -> Self {
        Self {
            version: MCP_SECRET_FILE_VERSION,
            legacy_keychain_migration_complete: false,
            secrets: BTreeMap::new(),
        }
    }
}

struct PendingSecretFile {
    path: PathBuf,
    armed: bool,
}

impl PendingSecretFile {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingSecretFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn reject_symlink(path: &Path, operation: SecretStoreOperation) -> Result<(), SecretStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(SecretStoreError::AccessDenied { operation })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(map_io_error(&error, operation)),
    }
}

fn map_io_error(error: &std::io::Error, operation: SecretStoreOperation) -> SecretStoreError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => SecretStoreError::AccessDenied { operation },
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            SecretStoreError::InvalidRequest { operation }
        }
        _ => SecretStoreError::Backend { operation },
    }
}

#[cfg(unix)]
fn set_private_file_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_private_file_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_private_directory_permissions(
    path: &Path,
    operation: SecretStoreOperation,
) -> Result<(), SecretStoreError> {
    use std::os::unix::fs::PermissionsExt;
    if path.file_name().and_then(|name| name.to_str()) != Some("openflow") {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| map_io_error(&error, operation))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(
    _path: &Path,
    _operation: SecretStoreOperation,
) -> Result<(), SecretStoreError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(
    path: &Path,
    operation: SecretStoreOperation,
) -> Result<(), SecretStoreError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| map_io_error(&error, operation))
}

#[cfg(not(unix))]
fn set_private_file_permissions(
    _path: &Path,
    _operation: SecretStoreOperation,
) -> Result<(), SecretStoreError> {
    Ok(())
}

const MCP_KEYRING_SERVICE: &str = "io.openflow.mcp";

/// Read/delete adapter used only for one-time migration into [`FileSecretStore`].
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct LegacyKeyringSecretStore;

impl LegacyKeyringSecretStore {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl LegacyKeyringSecretStore {
    fn entry(
        secret_ref: &McpSecretRef,
        operation: SecretStoreOperation,
    ) -> Result<keyring::Entry, SecretStoreError> {
        keyring::Entry::new(MCP_KEYRING_SERVICE, secret_ref.as_str())
            .map_err(|error| map_keyring_error(error, operation))
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl SecretStore for LegacyKeyringSecretStore {
    fn get(&self, secret_ref: &McpSecretRef) -> Result<Option<String>, SecretStoreError> {
        let entry = Self::entry(secret_ref, SecretStoreOperation::Get)?;
        map_get_result(entry.get_password())
    }

    fn set(&self, secret_ref: &McpSecretRef, secret_value: &str) -> Result<(), SecretStoreError> {
        let entry = Self::entry(secret_ref, SecretStoreOperation::Set)?;
        entry
            .set_password(secret_value)
            .map_err(|error| map_keyring_error(error, SecretStoreOperation::Set))
    }

    fn delete(&self, secret_ref: &McpSecretRef) -> Result<(), SecretStoreError> {
        let entry = Self::entry(secret_ref, SecretStoreOperation::Delete)?;
        map_delete_result(entry.delete_credential())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
impl SecretStore for LegacyKeyringSecretStore {
    fn get(&self, _secret_ref: &McpSecretRef) -> Result<Option<String>, SecretStoreError> {
        Err(SecretStoreError::UnsupportedPlatform)
    }

    fn set(&self, _secret_ref: &McpSecretRef, _secret_value: &str) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::UnsupportedPlatform)
    }

    fn delete(&self, _secret_ref: &McpSecretRef) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::UnsupportedPlatform)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn map_get_result(result: keyring::Result<String>) -> Result<Option<String>, SecretStoreError> {
    match result {
        Ok(secret_value) => Ok(Some(secret_value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(map_keyring_error(error, SecretStoreOperation::Get)),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn map_delete_result(result: keyring::Result<()>) -> Result<(), SecretStoreError> {
    match result {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(map_keyring_error(error, SecretStoreOperation::Delete)),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn map_keyring_error(error: keyring::Error, operation: SecretStoreOperation) -> SecretStoreError {
    match error {
        keyring::Error::NoStorageAccess(_) => SecretStoreError::AccessDenied { operation },
        keyring::Error::PlatformFailure(_) => SecretStoreError::Unavailable { operation },
        keyring::Error::TooLong(_, _) | keyring::Error::Invalid(_, _) => {
            SecretStoreError::InvalidRequest { operation }
        }
        keyring::Error::NoEntry | keyring::Error::BadEncoding(_) | keyring::Error::Ambiguous(_) => {
            SecretStoreError::Backend { operation }
        }
        _ => SecretStoreError::Backend { operation },
    }
}

#[cfg(all(
    test,
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
mod tests {
    use super::{map_delete_result, map_get_result, map_keyring_error, FileSecretStore};
    use crate::mcp::ports::{mcp_secret_ref, SecretStore, SecretStoreError, SecretStoreOperation};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn mcp_secret_keyring_no_entry_maps_to_absence_or_idempotent_delete() {
        assert_eq!(map_get_result(Err(keyring::Error::NoEntry)).unwrap(), None);
        map_delete_result(Err(keyring::Error::NoEntry)).unwrap();
    }

    #[test]
    fn mcp_secret_keyring_errors_are_typed_and_sanitized() {
        let error = map_keyring_error(
            keyring::Error::BadEncoding(b"do-not-leak-this-secret".to_vec()),
            SecretStoreOperation::Get,
        );

        assert_eq!(
            error,
            SecretStoreError::Backend {
                operation: SecretStoreOperation::Get,
            }
        );
        assert!(!format!("{error:?}").contains("do-not-leak-this-secret"));
        assert!(!error.to_string().contains("do-not-leak-this-secret"));

        assert_eq!(
            map_keyring_error(
                keyring::Error::Invalid("password".to_string(), "secret-value".to_string()),
                SecretStoreOperation::Set,
            ),
            SecretStoreError::InvalidRequest {
                operation: SecretStoreOperation::Set,
            }
        );
    }

    #[test]
    fn mcp_secret_file_round_trips_and_deletes_values() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("openflow").join("mcp-secrets.json");
        let store = FileSecretStore::new(&path);
        let secret_ref = mcp_secret_ref("massive", "env.MASSIVE_API_KEY").unwrap();

        assert_eq!(store.get(&secret_ref).unwrap(), None);
        store.set(&secret_ref, "local-secret").unwrap();
        assert_eq!(
            store.get(&secret_ref).unwrap().as_deref(),
            Some("local-secret")
        );

        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(persisted["version"], 1);
        assert_eq!(persisted["secrets"][secret_ref.as_str()], "local-secret");

        store.delete(&secret_ref).unwrap();
        store.delete(&secret_ref).unwrap();
        assert_eq!(store.get(&secret_ref).unwrap(), None);
    }

    #[test]
    fn mcp_secret_file_serializes_updates_across_store_instances() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("openflow").join("mcp-secrets.json");

        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for index in 0..8 {
                let path = path.clone();
                handles.push(scope.spawn(move || {
                    let store = FileSecretStore::new(path);
                    let secret_ref =
                        mcp_secret_ref(&format!("server-{index}"), "env.API_KEY").unwrap();
                    store.set(&secret_ref, &format!("secret-{index}"))
                }));
            }
            for handle in handles {
                handle.join().unwrap().unwrap();
            }
        });

        let store = FileSecretStore::new(&path);
        for index in 0..8 {
            let secret_ref = mcp_secret_ref(&format!("server-{index}"), "env.API_KEY").unwrap();
            assert_eq!(
                store.get(&secret_ref).unwrap().as_deref(),
                Some(format!("secret-{index}").as_str())
            );
        }
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn mcp_secret_file_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let parent = dir.path().join("openflow");
        let path = parent.join("mcp-secrets.json");
        let store = FileSecretStore::new(&path);
        let secret_ref = mcp_secret_ref("massive", "env.MASSIVE_API_KEY").unwrap();

        store.set(&secret_ref, "local-secret").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&parent).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[cfg(unix)]
    #[test]
    fn mcp_secret_file_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.json");
        fs::write(&target, r#"{"version":1,"secrets":{}}"#).unwrap();
        let path = dir.path().join("mcp-secrets.json");
        symlink(&target, &path).unwrap();
        let store = FileSecretStore::new(&path);
        let secret_ref = mcp_secret_ref("massive", "env.MASSIVE_API_KEY").unwrap();

        assert!(matches!(
            store.get(&secret_ref),
            Err(SecretStoreError::AccessDenied {
                operation: SecretStoreOperation::Get
            })
        ));
        assert!(matches!(
            store.set(&secret_ref, "must-not-write"),
            Err(SecretStoreError::AccessDenied {
                operation: SecretStoreOperation::Set
            })
        ));
        assert!(!fs::read_to_string(target)
            .unwrap()
            .contains("must-not-write"));
    }
}
