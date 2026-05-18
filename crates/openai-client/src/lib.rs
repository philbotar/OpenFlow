use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use workflow_core::{AgentError, AgentRequest, AgentResponse, AiPort};

#[derive(Debug, Clone)]
pub struct OpenAiResponsesClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl OpenAiResponsesClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.openai.com".to_string(),
        }
    }

    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }
}

#[async_trait]
impl AiPort for OpenAiResponsesClient {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let body = json!({
            "model": request.model,
            "input": [
                {
                    "role": "system",
                    "content": request.system_prompt
                },
                {
                    "role": "user",
                    "content": format!(
                        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}",
                        request.node_label,
                        request.task_prompt,
                        request.input
                    )
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

        let response = self
            .http
            .post(format!(
                "{}/v1/responses",
                self.base_url.trim_end_matches('/')
            ))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| AgentError::Failed(format!("OpenAI request failed: {error}")))?;

        let status = response.status();
        let payload: Value = response
            .json()
            .await
            .map_err(|error| AgentError::Failed(format!("OpenAI response JSON failed: {error}")))?;

        if !status.is_success() {
            return Err(AgentError::Failed(format!(
                "OpenAI returned HTTP {status}: {payload}"
            )));
        }

        let text = extract_output_text(&payload)?;
        let output: Value = serde_json::from_str(&text).map_err(|error| {
            AgentError::Failed(format!(
                "OpenAI structured output was not valid JSON: {error}"
            ))
        })?;

        Ok(AgentResponse {
            output,
            raw_text: text,
        })
    }
}

fn extract_output_text(payload: &Value) -> Result<String, AgentError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use workflow_core::AgentRequest;

    fn request() -> AgentRequest {
        AgentRequest {
            workflow_id: "wf".to_string(),
            node_id: "node-1".to_string(),
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

        let client = OpenAiResponsesClient::with_base_url("test-key", server.uri());

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

        let client = OpenAiResponsesClient::with_base_url("test-key", server.uri());

        let error = client.invoke(request()).await.unwrap_err();

        assert_eq!(error.to_string(), "OpenAI refusal: cannot help");
    }
}
