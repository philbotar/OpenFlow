use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use workflow_core::{AgentError, AgentRequest, AgentResponse, AiPort};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiWireApi {
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiClientConfig {
    pub api_key: String,
    pub base_url: String,
    pub wire_api: OpenAiWireApi,
    pub responses_path: String,
    pub chat_completions_path: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiClient {
    http: Client,
    config: OpenAiClientConfig,
}

impl OpenAiClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_config(OpenAiClientConfig::openai_default(api_key))
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let mut config = OpenAiClientConfig::openai_default(api_key);
        config.base_url = base_url.into();
        Self::with_config(config)
    }

    #[must_use]
    pub fn with_config(config: OpenAiClientConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        let base = self.config.base_url.trim().trim_end_matches('/');
        let mut normalized_path = path.trim().trim_start_matches('/').to_string();

        // Avoid duplicated API prefixes like base_url=/v1 and path=v1/chat/completions.
        if let Ok(parsed) = reqwest::Url::parse(base) {
            let base_path = parsed.path().trim_matches('/');
            if !base_path.is_empty() {
                let prefix = format!("{base_path}/");
                if normalized_path == base_path {
                    normalized_path.clear();
                } else if normalized_path.starts_with(&prefix) {
                    normalized_path = normalized_path[prefix.len()..].to_string();
                }
            }
        }

        if normalized_path.is_empty() {
            base.to_string()
        } else {
            format!("{base}/{normalized_path}")
        }
    }
}

impl OpenAiClientConfig {
    pub fn openai_default(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com".to_string(),
            wire_api: OpenAiWireApi::Responses,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
        }
    }
}

pub type OpenAiResponsesClient = OpenAiClient;

#[async_trait]
impl AiPort for OpenAiClient {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        match self.config.wire_api {
            OpenAiWireApi::Responses => self.invoke_responses(request).await,
            OpenAiWireApi::ChatCompletions => self.invoke_chat_completions(request).await,
        }
    }
}

fn extract_responses_output_text(payload: &Value) -> Result<String, AgentError> {
    let output = payload
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::Failed("OpenAI response missing output array".to_string()))?;

    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }

        let content = item
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                AgentError::Failed("OpenAI message missing content array".to_string())
            })?;

        for content_item in content {
            match content_item.get("type").and_then(Value::as_str) {
                Some("output_text") => {
                    if let Some(text) = content_item.get("text").and_then(Value::as_str) {
                        return Ok(text.to_string());
                    }
                }
                Some("refusal") => {
                    let refusal = content_item
                        .get("refusal")
                        .and_then(Value::as_str)
                        .unwrap_or("model refused the request");
                    return Err(AgentError::Failed(format!("OpenAI refusal: {refusal}")));
                }
                _ => {}
            }
        }
    }

    Err(AgentError::Failed(
        "OpenAI response did not contain output_text".to_string(),
    ))
}

fn build_user_content(request: &AgentRequest) -> String {
    format!(
        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}",
        request.node_label, request.task_prompt, request.input
    )
}

fn extract_chat_completion_content(payload: &Value) -> Result<String, AgentError> {
    let message = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| {
            AgentError::Failed("OpenAI-compatible response missing choices[0].message".to_string())
        })?;

    if let Some(refusal) = message.get("refusal").and_then(Value::as_str) {
        return Err(AgentError::Failed(format!(
            "OpenAI-compatible refusal: {refusal}"
        )));
    }

    message
        .get("content")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            AgentError::Failed(
                "OpenAI-compatible response did not contain assistant content".to_string(),
            )
        })
}

fn parse_structured_output(label: &str, text: String) -> Result<AgentResponse, AgentError> {
    let output: Value = serde_json::from_str(&text).map_err(|error| {
        AgentError::Failed(format!(
            "{label} structured output was not valid JSON: {error}"
        ))
    })?;

    Ok(AgentResponse {
        output,
        raw_text: text,
    })
}

impl OpenAiClient {
    async fn invoke_responses(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let body = json!({
            "model": request.model,
            "input": [
                {
                    "role": "system",
                    "content": request.system_prompt
                },
                {
                    "role": "user",
                    "content": build_user_content(&request)
                }
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "node_output",
                    "strict": true,
                    "schema": request.output_schema
                }
            }
        });

        let payload = self
            .post_json(&self.config.responses_path, body, "OpenAI")
            .await?;
        let text = extract_responses_output_text(&payload)?;
        parse_structured_output("OpenAI", text)
    }

    async fn invoke_chat_completions(
        &self,
        request: AgentRequest,
    ) -> Result<AgentResponse, AgentError> {
        let body = json!({
            "model": request.model,
            "messages": [
                {
                    "role": "system",
                    "content": request.system_prompt
                },
                {
                    "role": "user",
                    "content": build_user_content(&request)
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "node_output",
                    "strict": true,
                    "schema": request.output_schema
                }
            }
        });

        let payload = self
            .post_json(
                &self.config.chat_completions_path,
                body,
                "OpenAI-compatible",
            )
            .await?;
        let text = extract_chat_completion_content(&payload)?;
        parse_structured_output("OpenAI-compatible", text)
    }

    async fn post_json(&self, path: &str, body: Value, label: &str) -> Result<Value, AgentError> {
        let response = self
            .http
            .post(self.endpoint(path))
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| AgentError::Failed(format!("{label} request failed: {error}")))?;

        let status = response.status();
        let payload: Value = response.json().await.map_err(|error| {
            AgentError::Failed(format!("{label} response JSON failed: {error}"))
        })?;

        if !status.is_success() {
            return Err(AgentError::Failed(format!(
                "{label} returned HTTP {status}: {payload}"
            )));
        }

        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use workflow_core::{AgentRequest, NodeId, WorkflowId};

    fn request() -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId("wf".to_string()),
            node_id: NodeId("node-1".to_string()),
            node_label: "Planner".to_string(),
            model: "gpt-5.5".to_string(),
            system_prompt: "You plan features.".to_string(),
            task_prompt: "Create a plan summary.".to_string(),
            input: json!({"upstream": []}),
            output_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
        }
    }

    #[test]
    fn endpoint_deduplicates_matching_base_path_prefix() {
        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "test-key".to_string(),
            base_url: "https://api.deepinfra.com/v1".to_string(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/openai/chat/completions".to_string(),
        });

        assert_eq!(
            client.endpoint("v1/openai/chat/completions"),
            "https://api.deepinfra.com/v1/openai/chat/completions"
        );
    }

    #[test]
    fn endpoint_keeps_path_when_base_has_no_path_prefix() {
        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "test-key".to_string(),
            base_url: "https://api.deepinfra.com".to_string(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/openai/chat/completions".to_string(),
        });

        assert_eq!(
            client.endpoint("v1/openai/chat/completions"),
            "https://api.deepinfra.com/v1/openai/chat/completions"
        );
    }

    #[tokio::test]
    async fn sends_responses_request_with_json_schema_format() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(json!({
                "model": "gpt-5.5",
                "input": [
                    {
                        "role": "system",
                        "content": "You plan features."
                    },
                    {
                        "role": "user",
                        "content": "Node: Planner\nTask:\nCreate a plan summary.\n\nUpstream input JSON:\n{\"upstream\":[]}"
                    }
                ],
                "text": {
                    "format": {
                        "type": "json_schema",
                        "name": "node_output",
                        "strict": true,
                        "schema": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "summary": { "type": "string" }
                            },
                            "required": ["summary"]
                        }
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "{\"summary\":\"done\"}"
                    }]
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_base_url("test-key", server.uri());

        let response = client.invoke(request()).await.unwrap();

        assert_eq!(response.output, json!({"summary": "done"}));
    }

    #[tokio::test]
    async fn maps_refusal_to_agent_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "refusal",
                        "refusal": "cannot help"
                    }]
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_base_url("test-key", server.uri());

        let error = client.invoke(request()).await.unwrap_err();

        assert_eq!(error.to_string(), "OpenAI refusal: cannot help");
    }

    #[tokio::test]
    async fn maps_http_error_status_to_agent_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {
                    "message": "bad key"
                }
            })))
            .mount(&server)
            .await;
        let client = OpenAiClient::with_base_url("test-key", server.uri());

        let error = client.invoke(request()).await.unwrap_err();

        assert!(error.to_string().contains("OpenAI returned HTTP 401"));
        assert!(error.to_string().contains("bad key"));
    }

    #[tokio::test]
    async fn missing_output_text_returns_agent_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "input_text",
                        "text": "not an output"
                    }]
                }]
            })))
            .mount(&server)
            .await;
        let client = OpenAiClient::with_base_url("test-key", server.uri());

        let error = client.invoke(request()).await.unwrap_err();

        assert_eq!(
            error.to_string(),
            "OpenAI response did not contain output_text"
        );
    }

    #[tokio::test]
    async fn invalid_structured_output_json_returns_agent_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "not-json"
                    }]
                }]
            })))
            .mount(&server)
            .await;
        let client = OpenAiClient::with_base_url("test-key", server.uri());

        let error = client.invoke(request()).await.unwrap_err();

        assert!(error
            .to_string()
            .contains("OpenAI structured output was not valid JSON"));
    }

    #[tokio::test]
    async fn sends_chat_completions_request_with_json_schema_response_format() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(json!({
                "model": "vendor-model",
                "messages": [
                    {
                        "role": "system",
                        "content": "You plan features."
                    },
                    {
                        "role": "user",
                        "content": "Node: Planner\nTask:\nCreate a plan summary.\n\nUpstream input JSON:\n{\"upstream\":[]}"
                    }
                ],
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "node_output",
                        "strict": true,
                        "schema": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "summary": { "type": "string" }
                            },
                            "required": ["summary"]
                        }
                    }
                }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "{\"summary\":\"done\"}"
                    }
                }]
            })))
            .mount(&server)
            .await;

        let mut request = request();
        request.model = "vendor-model".to_string();
        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "test-key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
        });

        let response = client.invoke(request).await.unwrap();

        assert_eq!(response.output, json!({"summary": "done"}));
    }

    #[tokio::test]
    async fn chat_completions_refusal_maps_to_agent_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "refusal": "cannot help"
                    }
                }]
            })))
            .mount(&server)
            .await;
        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "test-key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
        });

        let error = client.invoke(request()).await.unwrap_err();

        assert_eq!(error.to_string(), "OpenAI-compatible refusal: cannot help");
    }

    #[tokio::test]
    async fn chat_completions_missing_content_maps_to_agent_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant"
                    }
                }]
            })))
            .mount(&server)
            .await;
        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "test-key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
        });

        let error = client.invoke(request()).await.unwrap_err();

        assert_eq!(
            error.to_string(),
            "OpenAI-compatible response did not contain assistant content"
        );
    }

    #[tokio::test]
    async fn chat_completions_uses_configured_endpoint_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/custom/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "{\"summary\":\"done\"}"
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "test-key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "custom/chat/completions".to_string(),
        });

        let response = client.invoke(request()).await.unwrap();

        assert_eq!(response.output, json!({"summary": "done"}));
    }
}
