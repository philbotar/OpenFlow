use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;

use crate::auth::CodexOAuthCredentials;

use super::browser::wait_until_cancelled;
use super::tokens::exchange_authorization_code;
use super::{
    CodexLoginCancellation, CodexLoginPrompt, CodexOAuthClient, CodexOAuthError,
    CODEX_OAUTH_CLIENT_ID,
};

const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_auth_id: String,
    user_code: String,
    #[serde(default = "default_interval")]
    interval: u64,
    #[serde(default = "default_expires_in")]
    expires_in: u64,
}

#[derive(Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    code_verifier: String,
}

pub(super) async fn login<F>(
    client: &CodexOAuthClient,
    cancellation: &CodexLoginCancellation,
    publish: &F,
) -> Result<CodexOAuthCredentials, CodexOAuthError>
where
    F: Fn(CodexLoginPrompt) -> Result<(), String> + Send + Sync,
{
    let device = request_device_code(client).await?;
    let expires_at =
        now_unix_seconds().saturating_add(i64::try_from(device.expires_in).unwrap_or(i64::MAX));
    publish(CodexLoginPrompt::Device {
        verification_url: client.endpoints.device_verification.clone(),
        user_code: device.user_code.clone(),
        expires_at,
    })
    .map_err(CodexOAuthError::Prompt)?;

    let authorized = tokio::time::timeout(
        client.device_timeout,
        poll_for_authorization(client, cancellation, &device),
    )
    .await
    .map_err(|_| CodexOAuthError::TimedOut)??;

    let exchange = exchange_authorization_code(
        &client.http,
        &client.endpoints.token,
        &authorized.authorization_code,
        DEVICE_REDIRECT_URI,
        &authorized.code_verifier,
    );
    tokio::select! {
        () = wait_until_cancelled(cancellation) => Err(CodexOAuthError::Cancelled),
        result = exchange => result,
    }
}

async fn request_device_code(
    client: &CodexOAuthClient,
) -> Result<DeviceCodeResponse, CodexOAuthError> {
    let response = client
        .http
        .post(&client.endpoints.device_user_code)
        .json(&serde_json::json!({ "client_id": CODEX_OAUTH_CLIENT_ID }))
        .send()
        .await
        .map_err(|_| CodexOAuthError::Transport {
            operation: "device authorization",
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(http_error(response, "device authorization").await);
    }
    response
        .json()
        .await
        .map_err(|_| CodexOAuthError::InvalidResponse {
            operation: "device authorization",
        })
}

async fn poll_for_authorization(
    client: &CodexOAuthClient,
    cancellation: &CodexLoginCancellation,
    device: &DeviceCodeResponse,
) -> Result<DeviceTokenResponse, CodexOAuthError> {
    let interval = std::time::Duration::from_secs(device.interval.max(1));
    loop {
        tokio::select! {
            () = wait_until_cancelled(cancellation) => return Err(CodexOAuthError::Cancelled),
            () = tokio::time::sleep(interval) => {}
        }

        let response = client
            .http
            .post(&client.endpoints.device_token)
            .json(&serde_json::json!({
                "device_auth_id": device.device_auth_id,
                "user_code": device.user_code,
            }))
            .send()
            .await
            .map_err(|_| CodexOAuthError::Transport {
                operation: "device authorization polling",
            })?;
        match response.status() {
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {}
            status if status.is_success() => {
                return response
                    .json()
                    .await
                    .map_err(|_| CodexOAuthError::InvalidResponse {
                        operation: "device authorization polling",
                    });
            }
            _ => return Err(http_error(response, "device authorization polling").await),
        }
    }
}

async fn http_error(response: reqwest::Response, operation: &'static str) -> CodexOAuthError {
    let status = response.status().as_u16();
    let code = response
        .json::<Value>()
        .await
        .ok()
        .and_then(|body| body.get("error")?.as_str().map(str::to_string));
    CodexOAuthError::Http {
        operation,
        status,
        code,
    }
}

const fn default_interval() -> u64 {
    5
}

const fn default_expires_in() -> u64 {
    15 * 60
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_callback_matches_current_codex_contract() {
        assert_eq!(
            DEVICE_REDIRECT_URI,
            format!("{}/deviceauth/callback", super::super::CODEX_OAUTH_ISSUER)
        );
    }
}
