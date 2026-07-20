use crate::error::BackendError;
use crate::settings::ports::SettingsStore;
use parking_lot::Mutex;
use providers::{
    login_codex, CodexLoginCancellation, CodexLoginPrompt, CodexOAuthCredentials, ProviderId,
};
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
    generation: u64,
}

pub struct CodexLoginCoordinator {
    store: Arc<dyn SettingsStore>,
    session: Arc<Mutex<CodexLoginSession>>,
}

impl CodexLoginCoordinator {
    #[must_use]
    pub fn new(store: Arc<dyn SettingsStore>) -> Self {
        let status = connected_status(&*store).unwrap_or(CodexLoginStatus::Disconnected);
        Self {
            store,
            session: Arc::new(Mutex::new(CodexLoginSession {
                status,
                cancellation: None,
                generation: 0,
            })),
        }
    }

    #[must_use]
    pub fn status(&self) -> CodexLoginStatus {
        self.session.lock().status.clone()
    }

    /// Starts one browser-first login and immediately returns a pollable status.
    pub fn start<F>(&self, open_browser: F) -> Result<CodexLoginStatus, BackendError>
    where
        F: Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    {
        let cancellation = CodexLoginCancellation::default();
        let generation = {
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
            session.generation = session.generation.wrapping_add(1);
            session.generation
        };

        let session = Arc::clone(&self.session);
        let store = Arc::clone(&self.store);
        tokio::spawn(async move {
            let result = login_codex(&cancellation, |prompt| {
                let status = match prompt {
                    CodexLoginPrompt::Browser { authorization_url } => {
                        let mut current = session.lock();
                        if current.generation != generation || cancellation.is_cancelled() {
                            return Err("ChatGPT sign-in is no longer active".to_string());
                        }
                        current.status = CodexLoginStatus::AwaitingBrowser;
                        drop(current);
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
                let mut current = session.lock();
                if current.generation != generation || cancellation.is_cancelled() {
                    return Err("ChatGPT sign-in is no longer active".to_string());
                }
                current.status = status;
                Ok(())
            })
            .await;

            let status = match result {
                Ok(_) if cancellation.is_cancelled() => CodexLoginStatus::Cancelled,
                Ok(credentials) => match persist_credentials(&*store, Some(credentials.clone())) {
                    Ok(()) => CodexLoginStatus::Connected {
                        email: credentials.email,
                    },
                    Err(error) => CodexLoginStatus::Failed {
                        message: format!("could not save ChatGPT credentials: {error}"),
                    },
                },
                Err(_) if cancellation.is_cancelled() => CodexLoginStatus::Cancelled,
                Err(error) => CodexLoginStatus::Failed {
                    message: error.to_string(),
                },
            };
            finish_login(&session, generation, status);
        });

        Ok(CodexLoginStatus::Starting)
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
            session.generation = session.generation.wrapping_add(1);
        }
        persist_credentials(&*self.store, None)?;
        let status = CodexLoginStatus::Disconnected;
        self.session.lock().status = status.clone();
        Ok(status)
    }
}

fn finish_login(session: &Mutex<CodexLoginSession>, generation: u64, status: CodexLoginStatus) {
    let mut session = session.lock();
    if session.generation == generation {
        session.cancellation = None;
        session.status = status;
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

    #[test]
    fn stale_login_completion_cannot_overwrite_a_disconnect() {
        let session = Mutex::new(CodexLoginSession {
            status: CodexLoginStatus::Disconnected,
            cancellation: None,
            generation: 2,
        });

        finish_login(
            &session,
            1,
            CodexLoginStatus::Connected {
                email: Some("person@example.com".to_string()),
            },
        );

        assert_eq!(session.lock().status, CodexLoginStatus::Disconnected);
    }
}
