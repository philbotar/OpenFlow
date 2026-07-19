use crate::error::BackendError;
use crate::settings::ports::SettingsStore;
use parking_lot::Mutex;
use providers::codex_oauth::{
    CodexLoginCancellation, CodexLoginPrompt, CodexOAuthClient, CodexOAuthError,
};
use providers::{CodexOAuthCredentials, ProviderId};
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CodexLoginStatus {
    Disconnected,
    Starting,
    AwaitingBrowser,
    AwaitingDevice {
        verification_url: String,
        user_code: String,
        expires_at: i64,
    },
    Connected {
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

impl CodexLoginStatus {
    fn is_in_progress(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::AwaitingBrowser | Self::AwaitingDevice { .. }
        )
    }
}

struct CodexLoginSession {
    status: CodexLoginStatus,
    cancellation: Option<CodexLoginCancellation>,
}

pub struct CodexLoginCoordinator {
    store: Arc<dyn SettingsStore>,
    client: CodexOAuthClient,
    session: Arc<Mutex<CodexLoginSession>>,
}

impl CodexLoginCoordinator {
    #[must_use]
    pub fn new(store: Arc<dyn SettingsStore>) -> Self {
        let status = connected_status(&*store).unwrap_or(CodexLoginStatus::Disconnected);
        Self {
            store,
            client: CodexOAuthClient::default(),
            session: Arc::new(Mutex::new(CodexLoginSession {
                status,
                cancellation: None,
            })),
        }
    }

    #[must_use]
    pub fn status(&self) -> CodexLoginStatus {
        self.session.lock().status.clone()
    }

    /// Starts one browser-first login and publishes progress for polling IPC clients.
    pub async fn start<F>(&self, open_browser: F) -> Result<CodexLoginStatus, BackendError>
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync,
    {
        let cancellation = CodexLoginCancellation::default();
        {
            let mut session = self.session.lock();
            if session.status.is_in_progress() || session.cancellation.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "ChatGPT sign-in is already in progress",
                )
                .into());
            }
            session.status = CodexLoginStatus::Starting;
            session.cancellation = Some(cancellation.clone());
        }

        let session = Arc::clone(&self.session);
        let result = self
            .client
            .login(&cancellation, |prompt| {
                let status = match prompt {
                    CodexLoginPrompt::Browser { authorization_url } => {
                        session.lock().status = CodexLoginStatus::AwaitingBrowser;
                        open_browser(&authorization_url)?;
                        return Ok(());
                    }
                    CodexLoginPrompt::Device {
                        verification_url,
                        user_code,
                        expires_at,
                    } => CodexLoginStatus::AwaitingDevice {
                        verification_url,
                        user_code,
                        expires_at,
                    },
                };
                session.lock().status = status;
                Ok(())
            })
            .await;

        let status = match result {
            Ok(credentials) if cancellation.is_cancelled() => CodexLoginStatus::Cancelled,
            Ok(credentials) => {
                persist_credentials(&*self.store, Some(credentials.clone()))?;
                CodexLoginStatus::Connected {
                    email: credentials.email,
                }
            }
            Err(CodexOAuthError::Cancelled) => CodexLoginStatus::Cancelled,
            Err(error) => CodexLoginStatus::Failed {
                message: error.to_string(),
            },
        };
        let mut session = self.session.lock();
        session.cancellation = None;
        session.status = status.clone();
        Ok(status)
    }

    #[must_use]
    pub fn cancel(&self) -> CodexLoginStatus {
        let mut session = self.session.lock();
        if let Some(cancellation) = &session.cancellation {
            cancellation.cancel();
            session.status = CodexLoginStatus::Cancelled;
        }
        session.status.clone()
    }

    pub fn disconnect(&self) -> Result<CodexLoginStatus, BackendError> {
        {
            let mut session = self.session.lock();
            if let Some(cancellation) = session.cancellation.take() {
                cancellation.cancel();
            }
        }
        persist_credentials(&*self.store, None)?;
        let status = CodexLoginStatus::Disconnected;
        self.session.lock().status = status.clone();
        Ok(status)
    }
}

fn connected_status(store: &dyn SettingsStore) -> Option<CodexLoginStatus> {
    let settings = store.load().ok()?;
    let credentials = settings
        .providers
        .get(&ProviderId::from("openai-codex"))?
        .codex_oauth
        .as_ref()?;
    Some(CodexLoginStatus::Connected {
        email: credentials.email.clone(),
    })
}

fn persist_credentials(
    store: &dyn SettingsStore,
    credentials: Option<CodexOAuthCredentials>,
) -> io::Result<()> {
    let mut settings = store.load()?;
    let profile = settings
        .providers
        .get_mut(&ProviderId::from("openai-codex"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "OpenAI Codex profile missing"))?;
    profile.codex_oauth = credentials;
    store.save_raw(&settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_status_is_camel_case_and_never_contains_credentials() {
        let status = CodexLoginStatus::AwaitingDevice {
            verification_url: "https://auth.example/device".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            expires_at: 1_800_000_000,
        };
        let json = serde_json::to_string(&status).expect("serialize status");
        assert_eq!(
            json,
            r#"{"state":"awaitingDevice","verificationUrl":"https://auth.example/device","userCode":"ABCD-EFGH","expiresAt":1800000000}"#
        );
        for name in [
            "accessToken",
            "refreshToken",
            "idToken",
            "accountId",
            "access_token",
            "refresh_token",
            "account_id",
        ] {
            assert!(!json.contains(name));
        }

        assert_eq!(
            serde_json::to_string(&CodexLoginStatus::AwaitingBrowser).unwrap(),
            r#"{"state":"awaitingBrowser"}"#
        );
    }
}
