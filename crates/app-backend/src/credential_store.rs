#[cfg(test)]
use parking_lot::Mutex;
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::Arc;
use thiserror::Error;

const DEFAULT_SERVICE: &str = "step-through-agentic-workflow";

#[derive(Debug, Error)]
pub enum CredentialStoreError {
    #[error("credential store entry failed: {0}")]
    Entry(String),
    #[error("credential store read failed: {0}")]
    Read(String),
    #[error("credential store write failed: {0}")]
    Write(String),
    #[error("credential store delete failed: {0}")]
    Delete(String),
}

#[derive(Debug, Clone)]
pub struct CredentialStore {
    backend: CredentialStoreBackend,
}

#[derive(Debug, Clone)]
enum CredentialStoreBackend {
    System {
        service: String,
    },
    #[cfg(test)]
    Memory(Arc<Mutex<BTreeMap<String, String>>>),
}

impl CredentialStore {
    #[must_use]
    pub fn system() -> Self {
        Self {
            backend: CredentialStoreBackend::System {
                service: DEFAULT_SERVICE.to_string(),
            },
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            backend: CredentialStoreBackend::Memory(Arc::new(Mutex::new(BTreeMap::new()))),
        }
    }

    pub fn get(&self, key_ref: &str) -> Result<Option<String>, CredentialStoreError> {
        match &self.backend {
            CredentialStoreBackend::System { service } => {
                let entry = system_entry(service, key_ref)?;
                match entry.get_password() {
                    Ok(secret) => Ok(Some(secret)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(error) => Err(CredentialStoreError::Read(error.to_string())),
                }
            }
            #[cfg(test)]
            CredentialStoreBackend::Memory(entries) => Ok(entries.lock().get(key_ref).cloned()),
        }
    }

    pub fn set(&self, key_ref: &str, secret: &str) -> Result<(), CredentialStoreError> {
        match &self.backend {
            CredentialStoreBackend::System { service } => system_entry(service, key_ref)?
                .set_password(secret)
                .map_err(|error| CredentialStoreError::Write(error.to_string())),
            #[cfg(test)]
            CredentialStoreBackend::Memory(entries) => {
                entries
                    .lock()
                    .insert(key_ref.to_string(), secret.to_string());
                Ok(())
            }
        }
    }

    pub fn delete(&self, key_ref: &str) -> Result<(), CredentialStoreError> {
        match &self.backend {
            CredentialStoreBackend::System { service } => {
                let entry = system_entry(service, key_ref)?;
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(CredentialStoreError::Delete(error.to_string())),
                }
            }
            #[cfg(test)]
            CredentialStoreBackend::Memory(entries) => {
                entries.lock().remove(key_ref);
                Ok(())
            }
        }
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::system()
    }
}

fn system_entry(service: &str, key_ref: &str) -> Result<keyring::Entry, CredentialStoreError> {
    keyring::Entry::new(service, key_ref)
        .map_err(|error| CredentialStoreError::Entry(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_round_trips_and_deletes_secret() {
        let store = CredentialStore::in_memory();

        assert_eq!(store.get("provider:openai:api-key").unwrap(), None);
        store
            .set("provider:openai:api-key", "sk-test")
            .expect("set key");
        assert_eq!(
            store.get("provider:openai:api-key").unwrap().as_deref(),
            Some("sk-test")
        );
        store.delete("provider:openai:api-key").expect("delete key");
        assert_eq!(store.get("provider:openai:api-key").unwrap(), None);
    }
}
