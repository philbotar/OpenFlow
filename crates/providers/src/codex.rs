//! Refreshable ChatGPT-subscription inference backed by Rig's Codex transport.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use engine::{AgentError, AgentRequest, AgentTurnOutcome, AiPort, AiStreamSink};
use tokio::sync::Mutex;

use crate::auth::CodexOAuthCredentials;
use crate::client::{CodexCredentialSink, OpenAiCodexConfig};
use crate::codex_oauth::{self, CodexOAuthError};
use crate::rig_adapter;
use crate::spec::ProviderId;

const REFRESH_MARGIN: Duration = Duration::from_mins(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct CodexClient {
    provider_id: ProviderId,
    provider_label: String,
    base_url: String,
    credentials: Mutex<CodexOAuthCredentials>,
    credential_sink: Option<Arc<dyn CodexCredentialSink>>,
    http: reqwest::Client,
    #[cfg(test)]
    refresh_endpoint: Option<String>,
}

impl CodexClient {
    pub(crate) fn new(
        provider_id: ProviderId,
        provider_label: String,
        config: OpenAiCodexConfig,
    ) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(config.request_timeout)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            provider_id,
            provider_label,
            base_url: config.base_url,
            credentials: Mutex::new(config.credentials),
            credential_sink: config.credential_sink,
            http,
            #[cfg(test)]
            refresh_endpoint: None,
        }
    }

    async fn credentials_for_request(
        &self,
        rejected_access_token: Option<&str>,
    ) -> Result<CodexOAuthCredentials, AgentError> {
        // Holding this one mutex across refresh + persistence is intentional:
        // refresh tokens may rotate, so concurrent nodes must share one flight.
        let mut current = self.credentials.lock().await;
        let needs_refresh = rejected_access_token.map_or_else(
            || expires_within(&current, REFRESH_MARGIN),
            |rejected| current.access_token == rejected,
        );
        if !needs_refresh {
            return Ok(current.clone());
        }

        let refreshed = self.refresh(&current).await?;
        if let Some(sink) = &self.credential_sink {
            sink.save(&refreshed).map_err(|error| {
                AgentError::Failed(format!(
                    "could not persist refreshed {} credentials: {error}",
                    self.provider_label
                ))
            })?;
        }
        *current = refreshed.clone();
        Ok(refreshed)
    }

    async fn refresh(
        &self,
        credentials: &CodexOAuthCredentials,
    ) -> Result<CodexOAuthCredentials, AgentError> {
        #[cfg(test)]
        let result = if let Some(endpoint) = &self.refresh_endpoint {
            codex_oauth::refresh_with_endpoint(&self.http, endpoint, credentials).await
        } else {
            codex_oauth::refresh_codex_credentials(&self.http, credentials).await
        };
        #[cfg(not(test))]
        let result = codex_oauth::refresh_codex_credentials(&self.http, credentials).await;

        result.map_err(|error| map_refresh_error(error, &self.provider_label))
    }

    async fn invoke_once(
        &self,
        credentials: &CodexOAuthCredentials,
        request: &AgentRequest,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let model = rig_adapter::build_codex_model(
            &self.provider_label,
            &self.base_url,
            &request.model,
            credentials,
            self.http.clone(),
        )?;
        rig_adapter::invoke_codex_model(&model, request, &self.provider_label, &self.provider_id)
            .await
    }

    async fn invoke_stream_once(
        &self,
        credentials: &CodexOAuthCredentials,
        request: &AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let model = rig_adapter::build_codex_model(
            &self.provider_label,
            &self.base_url,
            &request.model,
            credentials,
            self.http.clone(),
        )?;
        rig_adapter::invoke_codex_model_stream(
            &model,
            request,
            sink,
            &self.provider_label,
            &self.provider_id,
        )
        .await
    }
}

#[async_trait]
impl AiPort for CodexClient {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        let credentials = self.credentials_for_request(None).await?;
        match self.invoke_once(&credentials, &request).await {
            Err(error) if rig_adapter::is_codex_unauthorized(&error, &self.provider_label) => {
                let refreshed = self
                    .credentials_for_request(Some(&credentials.access_token))
                    .await?;
                self.invoke_once(&refreshed, &request).await
            }
            result => result,
        }
    }

    async fn invoke_stream(
        &self,
        request: AgentRequest,
        sink: &dyn AiStreamSink,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let credentials = self.credentials_for_request(None).await?;
        match self.invoke_stream_once(&credentials, &request, sink).await {
            Err(error) if rig_adapter::is_codex_unauthorized(&error, &self.provider_label) => {
                let refreshed = self
                    .credentials_for_request(Some(&credentials.access_token))
                    .await?;
                self.invoke_stream_once(&refreshed, &request, sink).await
            }
            result => result,
        }
    }
}

fn expires_within(credentials: &CodexOAuthCredentials, margin: Duration) -> bool {
    credentials.expires_at <= now_unix_seconds().saturating_add(seconds_i64(margin))
}

fn seconds_i64(duration: Duration) -> i64 {
    i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| seconds_i64(duration))
}

fn map_refresh_error(error: CodexOAuthError, provider_label: &str) -> AgentError {
    match error {
        CodexOAuthError::Transport { .. } => {
            AgentError::Transient(format!("{provider_label} token refresh failed: {error}"))
        }
        CodexOAuthError::Http { status, .. } if matches!(status, 408 | 409 | 429 | 500..=599) => {
            AgentError::Transient(format!("{provider_label} token refresh failed: {error}"))
        }
        CodexOAuthError::Http { .. } | CodexOAuthError::MissingCredential(_) => {
            AgentError::Permanent(format!("{provider_label} token refresh failed: {error}"))
        }
        _ => AgentError::Failed(format!("{provider_label} token refresh failed: {error}")),
    }
}

#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "provider lifecycle tests use unwrap/panic for brevity"
)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::client::CodexCredentialSink;

    #[derive(Default)]
    struct RecordingSink {
        saved: StdMutex<Vec<CodexOAuthCredentials>>,
    }

    impl CodexCredentialSink for RecordingSink {
        fn save(&self, credentials: &CodexOAuthCredentials) -> Result<(), String> {
            self.saved.lock().unwrap().push(credentials.clone());
            Ok(())
        }
    }

    fn credentials(access_token: &str, expires_at: i64) -> CodexOAuthCredentials {
        CodexOAuthCredentials {
            access_token: access_token.into(),
            refresh_token: "refresh-old".into(),
            id_token: Some("id-old".into()),
            expires_at,
            account_id: "account-123".into(),
            email: Some("person@example.com".into()),
        }
    }

    fn client(
        server: &MockServer,
        initial: CodexOAuthCredentials,
        sink: Arc<RecordingSink>,
    ) -> CodexClient {
        let mut client = CodexClient::new(
            ProviderId::from("openai-codex"),
            "OpenAI Codex".into(),
            OpenAiCodexConfig {
                base_url: server.uri(),
                request_timeout: Duration::from_secs(5),
                credentials: initial,
                credential_sink: Some(sink),
            },
        );
        client.refresh_endpoint = Some(format!("{}/oauth/token", server.uri()));
        client
    }

    fn test_request() -> AgentRequest {
        AgentRequest {
            workflow_id: engine::WorkflowId("wf-1".into()),
            node_id: engine::NodeId("node-1".into()),
            node_label: "Node".into(),
            model: "gpt-5.3-codex".into(),
            system_messages: vec!["Be precise.".into()],
            task_prompt: "Return the result.".into(),
            input: json!({}),
            output_schema: json!({
                "type": "object",
                "properties": {"summary": {"type": "string"}},
                "required": ["summary"]
            }),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: Some("high".into()),
            reasoning_budget_tokens: None,
            turn_phase: engine::AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: false,
        }
    }

    fn completed_submit_sse() -> String {
        let response = json!({
            "type": "response.completed",
            "response": {
                "id": "resp_1",
                "object": "response",
                "created_at": 1,
                "status": "completed",
                "error": null,
                "incomplete_details": null,
                "instructions": null,
                "max_output_tokens": null,
                "model": "gpt-5.3-codex",
                "usage": {
                    "input_tokens": 1,
                    "input_tokens_details": {"cached_tokens": 0},
                    "output_tokens": 1,
                    "output_tokens_details": {"reasoning_tokens": 0},
                    "total_tokens": 2
                },
                "output": [{
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call-submit",
                    "name": "openflow_submit_node_output",
                    "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}",
                    "status": "completed"
                }],
                "tools": []
            }
        });
        format!("data: {response}\n\ndata: [DONE]\n\n")
    }

    async fn mount_refresh(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "access-new",
                "refresh_token": "refresh-new",
                "expires_in": 3600
            })))
            .expect(1)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn proactive_refresh_is_single_flight_and_persisted() {
        let server = MockServer::start().await;
        mount_refresh(&server).await;
        let sink = Arc::new(RecordingSink::default());
        let client = client(&server, credentials("access-old", 0), sink.clone());

        let (first, second) = tokio::join!(
            client.credentials_for_request(None),
            client.credentials_for_request(None)
        );

        assert_eq!(first.unwrap().access_token, "access-new");
        assert_eq!(second.unwrap().access_token, "access-new");
        let saved = sink.saved.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].refresh_token, "refresh-new");
    }

    #[tokio::test]
    async fn unauthorized_request_refreshes_persists_and_retries_once() {
        let server = MockServer::start().await;
        mount_refresh(&server).await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer access-old"))
            .respond_with(ResponseTemplate::new(401).set_body_string("expired"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("authorization", "Bearer access-new"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(completed_submit_sse()),
            )
            .expect(1)
            .mount(&server)
            .await;
        let sink = Arc::new(RecordingSink::default());
        let client = client(&server, credentials("access-old", 4_000_000_000), sink.clone());

        let outcome = client.invoke(test_request()).await.unwrap();

        assert!(matches!(outcome, AgentTurnOutcome::Completed(_)));
        assert_eq!(sink.saved.lock().unwrap().len(), 1);
        let requests = server.received_requests().await.unwrap();
        let paths = requests
            .iter()
            .map(|request| request.url.path().to_string())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["/responses", "/oauth/token", "/responses"]);
    }

    #[test]
    fn refresh_statuses_have_bounded_retry_classification() {
        let transient = map_refresh_error(
            CodexOAuthError::Http {
                operation: "token refresh",
                status: 503,
                code: None,
            },
            "OpenAI Codex",
        );
        let permanent = map_refresh_error(
            CodexOAuthError::Http {
                operation: "token refresh",
                status: 401,
                code: Some("invalid_grant".into()),
            },
            "OpenAI Codex",
        );

        assert!(transient.is_retryable());
        assert!(matches!(permanent, AgentError::Permanent(_)));
    }
}
