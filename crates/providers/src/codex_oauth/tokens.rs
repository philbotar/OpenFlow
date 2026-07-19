use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::auth::CodexOAuthCredentials;

use super::{CODEX_OAUTH_CLIENT_ID, CodexOAuthEndpoints, CodexOAuthError};

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
}

pub async fn refresh_codex_credentials(
    http: &Client,
    credentials: &CodexOAuthCredentials,
) -> Result<CodexOAuthCredentials, CodexOAuthError> {
    refresh_with_endpoint(http, &CodexOAuthEndpoints::default().token, credentials).await
}

pub(crate) async fn refresh_with_endpoint(
    http: &Client,
    token_endpoint: &str,
    credentials: &CodexOAuthCredentials,
) -> Result<CodexOAuthCredentials, CodexOAuthError> {
    let response = http
        .post(token_endpoint)
        .json(&serde_json::json!({
            "client_id": CODEX_OAUTH_CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": credentials.refresh_token,
        }))
        .send()
        .await
        .map_err(|_| CodexOAuthError::Transport {
            operation: "token refresh",
        })?;

    let status = response.status();
    if !status.is_success() {
        let code = response
            .json::<Value>()
            .await
            .ok()
            .and_then(|body| oauth_error_code(&body));
        return Err(CodexOAuthError::Http {
            operation: "token refresh",
            status: status.as_u16(),
            code,
        });
    }

    let response =
        response
            .json::<TokenResponse>()
            .await
            .map_err(|_| CodexOAuthError::InvalidResponse {
                operation: "token refresh",
            })?;
    merge_refresh_response(credentials, response)
}

pub(super) async fn exchange_authorization_code(
    http: &Client,
    token_endpoint: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<CodexOAuthCredentials, CodexOAuthError> {
    let response = http
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", CODEX_OAUTH_CLIENT_ID),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|_| CodexOAuthError::Transport {
            operation: "token exchange",
        })?;

    let status = response.status();
    if !status.is_success() {
        let code = response
            .json::<Value>()
            .await
            .ok()
            .and_then(|body| oauth_error_code(&body));
        return Err(CodexOAuthError::Http {
            operation: "token exchange",
            status: status.as_u16(),
            code,
        });
    }

    let response =
        response
            .json::<TokenResponse>()
            .await
            .map_err(|_| CodexOAuthError::InvalidResponse {
                operation: "token exchange",
            })?;
    credentials_from_exchange(response)
}

fn credentials_from_exchange(
    response: TokenResponse,
) -> Result<CodexOAuthCredentials, CodexOAuthError> {
    let access_token = response
        .access_token
        .ok_or(CodexOAuthError::MissingCredential("an access token"))?;
    let refresh_token = response
        .refresh_token
        .ok_or(CodexOAuthError::MissingCredential("a refresh token"))?;
    let id_token = response
        .id_token
        .ok_or(CodexOAuthError::MissingCredential("an ID token"))?;

    let access_claims = jwt_claims(&access_token);
    let id_claims = jwt_claims(&id_token);
    let account_id = id_claims
        .as_ref()
        .and_then(account_id_from_claims)
        .or_else(|| access_claims.as_ref().and_then(account_id_from_claims))
        .ok_or(CodexOAuthError::MissingCredential("a ChatGPT account ID"))?;
    let email = id_claims
        .as_ref()
        .and_then(email_from_claims)
        .or_else(|| access_claims.as_ref().and_then(email_from_claims));
    let expires_at = response
        .expires_in
        .map(|seconds| now_unix_seconds().saturating_add(seconds))
        .or_else(|| access_claims.as_ref().and_then(expiry_from_claims))
        .or_else(|| id_claims.as_ref().and_then(expiry_from_claims))
        .ok_or(CodexOAuthError::MissingCredential("token expiry"))?;

    Ok(CodexOAuthCredentials {
        access_token,
        refresh_token,
        id_token: Some(id_token),
        expires_at,
        account_id,
        email,
    })
}

fn merge_refresh_response(
    previous: &CodexOAuthCredentials,
    response: TokenResponse,
) -> Result<CodexOAuthCredentials, CodexOAuthError> {
    let access_token = response
        .access_token
        .unwrap_or_else(|| previous.access_token.clone());
    let refresh_token = response
        .refresh_token
        .unwrap_or_else(|| previous.refresh_token.clone());
    let id_token = response.id_token.or_else(|| previous.id_token.clone());
    let access_claims = jwt_claims(&access_token);
    let id_claims = id_token.as_deref().and_then(jwt_claims);
    let account_id = id_claims
        .as_ref()
        .and_then(account_id_from_claims)
        .or_else(|| access_claims.as_ref().and_then(account_id_from_claims))
        .unwrap_or_else(|| previous.account_id.clone());
    let email = id_claims
        .as_ref()
        .and_then(email_from_claims)
        .or_else(|| access_claims.as_ref().and_then(email_from_claims))
        .or_else(|| previous.email.clone());
    let expires_at = response
        .expires_in
        .map(|seconds| now_unix_seconds().saturating_add(seconds))
        .or_else(|| access_claims.as_ref().and_then(expiry_from_claims))
        .or_else(|| id_claims.as_ref().and_then(expiry_from_claims))
        .unwrap_or(previous.expires_at);

    Ok(CodexOAuthCredentials {
        access_token,
        refresh_token,
        id_token,
        expires_at,
        account_id,
        email,
    })
}

fn oauth_error_code(body: &Value) -> Option<String> {
    body.get("error")
        .and_then(Value::as_str)
        .or_else(|| {
            body.get("error")
                .and_then(Value::as_object)
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str)
        })
        .filter(|code| !code.is_empty())
        .map(str::to_string)
}

fn jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn account_id_from_claims(claims: &Value) -> Option<String> {
    claims
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object)
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .or_else(|| {
            claims
                .get("https://api.openai.com/auth.chatgpt_account_id")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn email_from_claims(claims: &Value) -> Option<String> {
    claims
        .get("email")
        .and_then(Value::as_str)
        .or_else(|| {
            claims
                .get("https://api.openai.com/profile.email")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn expiry_from_claims(claims: &Value) -> Option<i64> {
    claims.get("exp").and_then(Value::as_i64)
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

    fn jwt(claims: Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap_or_default());
        format!("{header}.{payload}.signature")
    }

    #[test]
    fn exchange_extracts_account_email_and_expiry_from_jwt_claims() {
        let id_token = jwt(serde_json::json!({
            "email": "person@example.com",
            "https://api.openai.com/auth": { "chatgpt_account_id": "acct-secret" }
        }));
        let access_token = jwt(serde_json::json!({ "exp": 4_000_000_000_i64 }));

        let credentials = credentials_from_exchange(TokenResponse {
            access_token: Some(access_token),
            refresh_token: Some("refresh-secret".to_string()),
            id_token: Some(id_token),
            expires_in: None,
        })
        .unwrap_or_else(|error| panic!("unexpected error: {error}"));

        assert_eq!(credentials.account_id, "acct-secret");
        assert_eq!(credentials.email.as_deref(), Some("person@example.com"));
        assert_eq!(credentials.expires_at, 4_000_000_000_i64);
    }

    #[test]
    fn refresh_preserves_fields_omitted_by_current_token_endpoint() {
        let previous = CodexOAuthCredentials {
            access_token: "access-old".to_string(),
            refresh_token: "refresh-old".to_string(),
            id_token: Some("id-old".to_string()),
            expires_at: 123,
            account_id: "acct-old".to_string(),
            email: Some("old@example.com".to_string()),
        };

        let merged = merge_refresh_response(
            &previous,
            TokenResponse {
                access_token: None,
                refresh_token: None,
                id_token: None,
                expires_in: None,
            },
        )
        .unwrap_or_else(|error| panic!("unexpected error: {error}"));

        assert_eq!(merged.access_token, "access-old");
        assert_eq!(merged.refresh_token, "refresh-old");
        assert_eq!(merged.account_id, "acct-old");
        assert_eq!(merged.email.as_deref(), Some("old@example.com"));
        assert_eq!(merged.expires_at, 123);
    }

    #[test]
    fn oauth_error_debug_and_display_do_not_include_response_body() {
        let error = CodexOAuthError::Http {
            operation: "token refresh",
            status: 401,
            code: Some("refresh_token_invalidated".to_string()),
        };

        let rendered = format!("{error:?} {error}");
        assert!(rendered.contains("refresh_token_invalidated"));
        assert!(!rendered.contains("access-old"));
    }
}
