//! `ChatGPT Codex` OAuth protocol support.
//!
//! The protocol is compatible with `OpenAI Codex`'s browser and device login
//! flows. The implementation is original Rust code; the device fallback policy
//! is OpenFlow-specific.
//!
//! OAuth endpoint, PKCE, and token-body details are derived from oh-my-pi
//! (`packages/ai/src/auth/oauth/openai-codex.ts`, MIT).

mod browser;
mod device;
mod tokens;

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use thiserror::Error;

use crate::auth::CodexOAuthCredentials;

pub use tokens::refresh_codex_credentials;
#[cfg(test)]
pub(crate) use tokens::refresh_with_endpoint;

pub const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CODEX_OAUTH_ISSUER: &str = "https://auth.openai.com";
pub const CODEX_CALLBACK_PORT: u16 = 1455;

const BROWSER_TIMEOUT: Duration = Duration::from_mins(10);
const DEVICE_TIMEOUT: Duration = Duration::from_mins(15);

#[derive(Clone)]
pub struct CodexOAuthClient {
    http: Client,
    endpoints: CodexOAuthEndpoints,
    callback_port: u16,
    browser_timeout: Duration,
    device_timeout: Duration,
}

impl Default for CodexOAuthClient {
    fn default() -> Self {
        Self {
            http: Client::new(),
            endpoints: CodexOAuthEndpoints::default(),
            callback_port: CODEX_CALLBACK_PORT,
            browser_timeout: BROWSER_TIMEOUT,
            device_timeout: DEVICE_TIMEOUT,
        }
    }
}

impl CodexOAuthClient {
    #[must_use]
    pub fn new(http: Client) -> Self {
        Self {
            http,
            ..Self::default()
        }
    }

    /// Starts browser login, falling back to device authorization only when
    /// the fixed loopback callback port is already occupied.
    ///
    /// # Errors
    /// Returns an error when browser or device login fails or is cancelled.
    pub async fn login<F>(
        &self,
        cancellation: &CodexLoginCancellation,
        publish: F,
    ) -> Result<CodexOAuthCredentials, CodexOAuthError>
    where
        F: Fn(CodexLoginPrompt) -> Result<(), String> + Send + Sync,
    {
        // ponytail: one cancellation boundary drops any pending browser or device I/O.
        let login = async {
            match browser::login(self, &publish).await {
                Err(CodexOAuthError::CallbackPortBusy) => device::login(self, &publish).await,
                result => result,
            }
        };
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(CodexOAuthError::Cancelled),
            result = login => result,
        }
    }

    #[cfg(test)]
    const fn for_test(
        http: Client,
        endpoints: CodexOAuthEndpoints,
        callback_port: u16,
        browser_timeout: Duration,
        device_timeout: Duration,
    ) -> Self {
        Self {
            http,
            endpoints,
            callback_port,
            browser_timeout,
            device_timeout,
        }
    }
}

/// Starts the built-in `ChatGPT OAuth` flow without exposing a concrete client
/// outside this provider adapter crate.
///
/// # Errors
/// Returns an error when browser or device login fails or is cancelled.
pub async fn login_codex<F>(
    cancellation: &CodexLoginCancellation,
    publish: F,
) -> Result<CodexOAuthCredentials, CodexOAuthError>
where
    F: Fn(CodexLoginPrompt) -> Result<(), String> + Send + Sync,
{
    CodexOAuthClient::default()
        .login(cancellation, publish)
        .await
}

#[derive(Clone, Default)]
pub struct CodexLoginCancellation {
    cancelled: Arc<AtomicBool>,
}

impl CodexLoginCancellation {
    async fn cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub enum CodexLoginPrompt {
    Browser {
        authorization_url: String,
    },
    Device {
        verification_url: String,
        user_code: String,
        expires_at: i64,
    },
}

impl fmt::Debug for CodexLoginPrompt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Browser { .. } => formatter
                .debug_struct("Browser")
                .field("authorization_url", &"<redacted>")
                .finish(),
            Self::Device { expires_at, .. } => formatter
                .debug_struct("Device")
                .field("verification_url", &"<redacted>")
                .field("user_code", &"<redacted>")
                .field("expires_at", expires_at)
                .finish(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CodexOAuthError {
    #[error("the ChatGPT sign-in callback port is already in use")]
    CallbackPortBusy,
    #[error("ChatGPT sign-in was cancelled")]
    Cancelled,
    #[error("ChatGPT sign-in timed out")]
    TimedOut,
    #[error("ChatGPT sign-in callback state did not match")]
    StateMismatch,
    #[error("ChatGPT sign-in callback did not include an authorization code")]
    MissingAuthorizationCode,
    #[error("could not publish ChatGPT sign-in instructions: {0}")]
    Prompt(String),
    #[error("could not start the ChatGPT sign-in callback: {0}")]
    Callback(String),
    #[error("ChatGPT {operation} request failed")]
    Transport { operation: &'static str },
    #[error("ChatGPT {operation} returned HTTP {status}{code_suffix}", code_suffix = format_code(.code.as_deref()))]
    Http {
        operation: &'static str,
        status: u16,
        code: Option<String>,
    },
    #[error("ChatGPT {operation} returned an invalid response")]
    InvalidResponse { operation: &'static str },
    #[error("ChatGPT credentials did not include {0}")]
    MissingCredential(&'static str),
}

fn format_code(code: Option<&str>) -> String {
    code.map_or_else(String::new, |code| format!(" ({code})"))
}

#[derive(Clone)]
struct CodexOAuthEndpoints {
    authorize: String,
    token: String,
    device_user_code: String,
    device_token: String,
    device_verification: String,
}

impl Default for CodexOAuthEndpoints {
    fn default() -> Self {
        Self {
            authorize: format!("{CODEX_OAUTH_ISSUER}/oauth/authorize"),
            token: format!("{CODEX_OAUTH_ISSUER}/oauth/token"),
            device_user_code: format!("{CODEX_OAUTH_ISSUER}/api/accounts/deviceauth/usercode"),
            device_token: format!("{CODEX_OAUTH_ISSUER}/api/accounts/deviceauth/token"),
            device_verification: format!("{CODEX_OAUTH_ISSUER}/codex/device"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use std::sync::Mutex;
    use wiremock::matchers::{body_json, body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn login_prompt_debug_redacts_browser_state_and_device_code() {
        let browser = CodexLoginPrompt::Browser {
            authorization_url: "https://auth.example/authorize?state=secret".to_string(),
        };
        let device = CodexLoginPrompt::Device {
            verification_url: "https://auth.example/device".to_string(),
            user_code: "SECRET-CODE".to_string(),
            expires_at: 123,
        };

        let rendered = format!("{browser:?} {device:?}");
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("SECRET-CODE"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn cancellation_is_shared_between_clones() {
        let cancellation = CodexLoginCancellation::default();
        let clone = cancellation.clone();

        clone.cancel();

        assert!(cancellation.is_cancelled());
    }

    #[tokio::test]
    #[allow(clippy::unwrap_used, reason = "loopback cancellation regression")]
    async fn cancellation_drops_stalled_browser_callback() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::{TcpListener, TcpStream};

        let reserved = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let client = CodexOAuthClient {
            callback_port: address.port(),
            ..CodexOAuthClient::default()
        };
        let cancellation = CodexLoginCancellation::default();
        let (published, mut prompts) = tokio::sync::mpsc::unbounded_channel();
        let stalled_callback = async {
            prompts.recv().await.unwrap();
            let mut socket = TcpStream::connect(address).await.unwrap();
            socket
                .write_all(b"GET /auth/callback HTTP/1.1\r\n")
                .await
                .unwrap();
            // Let the callback reader consume the incomplete header before cancellation.
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancellation.cancel();
            std::future::pending::<()>().await;
            drop(socket);
        };
        let result = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(2), client.login(&cancellation, |prompt| {
                published.send(prompt).map_err(|error| error.to_string())
            })) => result,
            () = stalled_callback => unreachable!(),
        };
        assert!(matches!(result, Ok(Err(CodexOAuthError::Cancelled))));
        assert!(
            TcpListener::bind(address).await.is_ok(),
            "callback port must be released"
        );
    }

    #[tokio::test]
    #[allow(clippy::unwrap_used, reason = "mock HTTP cancellation regression")]
    async fn cancellation_drops_stalled_device_requests() {
        for stalled_path in ["/device/code", "/device/token", "/oauth/token"] {
            let server = MockServer::start().await;
            let received = Arc::new(tokio::sync::Notify::new());
            let signal = received.clone();
            Mock::given(method("POST"))
                .and(path(stalled_path))
                .respond_with(move |_: &wiremock::Request| {
                    signal.notify_one();
                    ResponseTemplate::new(200).set_delay(Duration::from_secs(10))
                })
                .with_priority(1)
                .mount(&server)
                .await;
            for (path, body) in [
                (
                    "/device/code",
                    serde_json::json!({
                        "device_auth_id": "device-id", "user_code": "code", "interval": 1
                    }),
                ),
                (
                    "/device/token",
                    serde_json::json!({
                        "authorization_code": "code", "code_verifier": "verifier"
                    }),
                ),
            ] {
                Mock::given(method("POST"))
                    .and(wiremock::matchers::path(path))
                    .respond_with(ResponseTemplate::new(200).set_body_json(body))
                    .with_priority(2)
                    .mount(&server)
                    .await;
            }
            let occupied = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            let client = CodexOAuthClient {
                callback_port: occupied.local_addr().unwrap().port(),
                endpoints: CodexOAuthEndpoints {
                    device_user_code: format!("{}/device/code", server.uri()),
                    device_token: format!("{}/device/token", server.uri()),
                    token: format!("{}/oauth/token", server.uri()),
                    ..CodexOAuthEndpoints::default()
                },
                ..CodexOAuthClient::default()
            };
            let cancellation = CodexLoginCancellation::default();
            let cancel_pending_request = async {
                received.notified().await;
                cancellation.cancel();
                std::future::pending::<()>().await;
            };
            let result = tokio::select! {
                result = tokio::time::timeout(Duration::from_secs(3), client.login(&cancellation, |_| Ok(()))) => result,
                () = cancel_pending_request => unreachable!(),
            };
            assert!(
                matches!(result, Ok(Err(CodexOAuthError::Cancelled))),
                "stalled {stalled_path}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn occupied_callback_port_falls_back_to_device_flow() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/usercode"))
            .and(body_json(serde_json::json!({
                "client_id": CODEX_OAUTH_CLIENT_ID
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_auth_id": "device-id",
                "user_code": "ABCD-EFGH",
                "interval": 0,
                "expires_in": 60
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/token"))
            .and(body_json(serde_json::json!({
                "device_auth_id": "device-id",
                "user_code": "ABCD-EFGH"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authorization_code": "authorization-code",
                "code_verifier": "device-verifier"
            })))
            .mount(&server)
            .await;

        let id_token = test_jwt(&serde_json::json!({
            "email": "person@example.com",
            "https://api.openai.com/auth": { "chatgpt_account_id": "account-id" }
        }));
        let access_token = test_jwt(&serde_json::json!({ "exp": 4_000_000_000_i64 }));
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code=authorization-code"))
            .and(body_string_contains("code_verifier=device-verifier"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": access_token,
                "refresh_token": "refresh-token",
                "id_token": id_token
            })))
            .mount(&server)
            .await;

        let occupied = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0));
        assert!(occupied.is_ok());
        let Ok(occupied) = occupied else {
            return;
        };
        let address = occupied.local_addr();
        assert!(address.is_ok());
        let Ok(address) = address else {
            return;
        };
        let base = server.uri();
        let client = CodexOAuthClient::for_test(
            Client::new(),
            CodexOAuthEndpoints {
                authorize: format!("{base}/oauth/authorize"),
                token: format!("{base}/oauth/token"),
                device_user_code: format!("{base}/api/accounts/deviceauth/usercode"),
                device_token: format!("{base}/api/accounts/deviceauth/token"),
                device_verification: format!("{base}/codex/device"),
            },
            address.port(),
            Duration::from_secs(2),
            Duration::from_secs(4),
        );
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let recorded = prompts.clone();

        let credentials = client
            .login(&CodexLoginCancellation::default(), move |prompt| {
                recorded
                    .lock()
                    .map_err(|_| "prompt lock poisoned".to_string())?
                    .push(prompt);
                Ok(())
            })
            .await;

        assert!(credentials.is_ok());
        let Ok(credentials) = credentials else {
            return;
        };
        assert_eq!(credentials.account_id, "account-id");
        assert_eq!(credentials.email.as_deref(), Some("person@example.com"));
        assert!(matches!(
            prompts.lock().as_deref(),
            Ok(recorded) if matches!(recorded.as_slice(), [CodexLoginPrompt::Device { .. }])
        ));
    }

    fn test_jwt(claims: &serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap_or_default());
        format!("{header}.{payload}.signature")
    }
}
