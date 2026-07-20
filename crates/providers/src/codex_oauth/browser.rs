use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use reqwest::Url;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::auth::CodexOAuthCredentials;

use super::tokens::exchange_authorization_code;
use super::{
    CodexLoginCancellation, CodexLoginPrompt, CodexOAuthClient, CodexOAuthError,
    CODEX_OAUTH_CLIENT_ID,
};

const OAUTH_SCOPE: &str = "openid profile email offline_access";
const MAX_CALLBACK_BYTES: usize = 16 * 1024;

pub(super) async fn login<F>(
    client: &CodexOAuthClient,
    cancellation: &CodexLoginCancellation,
    publish: &F,
) -> Result<CodexOAuthCredentials, CodexOAuthError>
where
    F: Fn(CodexLoginPrompt) -> Result<(), String> + Send + Sync,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, client.callback_port))
        .await
        .map_err(|error| {
            if error.kind() == ErrorKind::AddrInUse {
                CodexOAuthError::CallbackPortBusy
            } else {
                CodexOAuthError::Callback(error.kind().to_string())
            }
        })?;
    let verifier = random_base64::<64>()?;
    let challenge = pkce_challenge(&verifier);
    let state = random_base64::<32>()?;
    let redirect_uri = format!("http://localhost:{}/auth/callback", client.callback_port);
    let authorization_url = build_authorization_url(
        &client.endpoints.authorize,
        &redirect_uri,
        &challenge,
        &state,
    )?;

    publish(CodexLoginPrompt::Browser { authorization_url }).map_err(CodexOAuthError::Prompt)?;

    let callback = tokio::time::timeout(
        client.browser_timeout,
        wait_for_callback(&listener, cancellation, &state),
    )
    .await
    .map_err(|_| CodexOAuthError::TimedOut)??;

    let exchange = exchange_authorization_code(
        &client.http,
        &client.endpoints.token,
        &callback.code,
        &redirect_uri,
        &verifier,
    );
    let credentials = tokio::select! {
        () = wait_until_cancelled(cancellation) => Err(CodexOAuthError::Cancelled),
        result = exchange => result,
    };

    let mut stream = callback.stream;
    let (status, body) = if credentials.is_ok() {
        (
            "200 OK",
            "ChatGPT sign-in completed. You can return to OpenFlow.",
        )
    } else {
        (
            "400 Bad Request",
            "ChatGPT sign-in could not be completed. Return to OpenFlow for details.",
        )
    };
    let _ = write_response(&mut stream, status, body).await;
    credentials
}

struct CallbackRequest {
    code: String,
    stream: TcpStream,
}

async fn wait_for_callback(
    listener: &TcpListener,
    cancellation: &CodexLoginCancellation,
    expected_state: &str,
) -> Result<CallbackRequest, CodexOAuthError> {
    loop {
        let (mut stream, _address) = tokio::select! {
            () = wait_until_cancelled(cancellation) => return Err(CodexOAuthError::Cancelled),
            accepted = listener.accept() => accepted.map_err(|error| {
                CodexOAuthError::Callback(error.kind().to_string())
            })?,
        };

        let request_target = match read_request_target(&mut stream).await {
            Ok(target) => target,
            Err(error) => {
                let _ = write_response(&mut stream, "400 Bad Request", "Invalid request.").await;
                if matches!(error, CodexOAuthError::MissingAuthorizationCode) {
                    continue;
                }
                return Err(error);
            }
        };
        let url = Url::parse(&format!("http://localhost{request_target}"))
            .map_err(|_| CodexOAuthError::MissingAuthorizationCode)?;
        if url.path() != "/auth/callback" {
            let _ = write_response(&mut stream, "404 Not Found", "Not found.").await;
            continue;
        }

        let parameters = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        if parameters.get("state").map(|value| value.as_ref()) != Some(expected_state) {
            let _ = write_response(&mut stream, "400 Bad Request", "State mismatch.").await;
            return Err(CodexOAuthError::StateMismatch);
        }
        let Some(code) = parameters
            .get("code")
            .map(|value| value.to_string())
            .filter(|value| !value.is_empty())
        else {
            let _ = write_response(
                &mut stream,
                "400 Bad Request",
                "Missing authorization code.",
            )
            .await;
            return Err(CodexOAuthError::MissingAuthorizationCode);
        };

        return Ok(CallbackRequest { code, stream });
    }
}

async fn read_request_target(stream: &mut TcpStream) -> Result<String, CodexOAuthError> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|error| CodexOAuthError::Callback(error.kind().to_string()))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > MAX_CALLBACK_BYTES {
            return Err(CodexOAuthError::MissingAuthorizationCode);
        }
    }

    let request =
        std::str::from_utf8(&request).map_err(|_| CodexOAuthError::MissingAuthorizationCode)?;
    let mut parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    if parts.next() != Some("GET") {
        return Err(CodexOAuthError::MissingAuthorizationCode);
    }
    parts
        .next()
        .map(str::to_string)
        .ok_or(CodexOAuthError::MissingAuthorizationCode)
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    body: &str,
) -> Result<(), CodexOAuthError> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|error| CodexOAuthError::Callback(error.kind().to_string()))?;
    stream
        .shutdown()
        .await
        .map_err(|error| CodexOAuthError::Callback(error.kind().to_string()))
}

fn build_authorization_url(
    endpoint: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> Result<String, CodexOAuthError> {
    let mut url = Url::parse(endpoint).map_err(|_| CodexOAuthError::InvalidResponse {
        operation: "authorization URL",
    })?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", CODEX_OAUTH_CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", OAUTH_SCOPE)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("state", state)
        .append_pair("originator", "openflow");
    Ok(url.into())
}

fn random_base64<const SIZE: usize>() -> Result<String, CodexOAuthError> {
    let mut bytes = [0_u8; SIZE];
    getrandom::fill(&mut bytes).map_err(|_| CodexOAuthError::InvalidResponse {
        operation: "secure random generation",
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(super) async fn wait_until_cancelled(cancellation: &CodexLoginCancellation) {
    while !cancellation.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256_matches_rfc_7636_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorization_url_uses_current_codex_scope_and_security_parameters() {
        let url = build_authorization_url(
            "https://auth.example/oauth/authorize",
            "http://localhost:1455/auth/callback",
            "challenge",
            "state-secret",
        )
        .unwrap_or_else(|error| panic!("unexpected error: {error}"));
        let url = Url::parse(&url).unwrap_or_else(|error| panic!("invalid URL: {error}"));
        let query = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(
            query.get("scope").map(|value| value.as_ref()),
            Some(OAUTH_SCOPE)
        );
        assert_eq!(
            query
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
        assert_eq!(
            query
                .get("id_token_add_organizations")
                .map(|value| value.as_ref()),
            Some("true")
        );
        assert_eq!(
            query
                .get("codex_cli_simplified_flow")
                .map(|value| value.as_ref()),
            Some("true")
        );
        assert_eq!(
            query.get("originator").map(|value| value.as_ref()),
            Some("openflow")
        );
    }
}
