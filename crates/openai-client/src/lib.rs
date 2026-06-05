#![allow(clippy::multiple_crate_versions, clippy::too_many_lines)]

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use workflow_core::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTranscriptItem,
    AgentTurnOutcome, AgentTurnSuccess, AiPort, ToolCall, ToolDefinition,
};

const SUBMIT_OUTPUT_TOOL: &str = "openflow_submit_node_output";
const REQUEST_INPUT_TOOL: &str = "openflow_request_user_input";

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
    #[must_use]
    pub fn with_config(config: OpenAiClientConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_config(OpenAiClientConfig::new(api_key))
    }
}

impl OpenAiClientConfig {
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com".to_string(),
            wire_api: OpenAiWireApi::Responses,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        }
    }
}

#[async_trait]
impl AiPort for OpenAiClient {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        match self.config.wire_api {
            OpenAiWireApi::Responses => self.invoke_responses(request).await,
            OpenAiWireApi::ChatCompletions => self.invoke_chat_completions(request).await,
        }
    }
}

#[derive(Debug, Clone)]
struct ToolSpec {
    name: String,
    description: String,
    parameters: Value,
}

fn build_node_context(request: &AgentRequest) -> String {
    let tool_instruction = if request.available_tools.is_empty() {
        "When you are ready to finish, call openflow_submit_node_output exactly once.".to_string()
    } else {
        "Use tools when they materially improve correctness. When you are ready to finish, call openflow_submit_node_output exactly once."
            .to_string()
    };
    format!(
        "Node: {}\nTask:\n{}\n\nUpstream input JSON:\n{}\n\n{}",
        request.node_label, request.task_prompt, request.input, tool_instruction
    )
}

fn should_allow_user_input(request: &AgentRequest) -> bool {
    request.transcript.iter().any(|item| {
        matches!(
            item,
            AgentTranscriptItem::UserMessage { .. } | AgentTranscriptItem::AssistantMessage { .. }
        )
    })
}

fn submit_output_tool(request: &AgentRequest) -> ToolSpec {
    ToolSpec {
        name: SUBMIT_OUTPUT_TOOL.to_string(),
        description: "Submit the final structured node output when the task is complete."
            .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "output": request.output_schema,
                "assistant_message": {
                    "type": ["string", "null"],
                    "description": "Optional human-facing note to show alongside the final result."
                }
            },
            "required": ["output", "assistant_message"]
        }),
    }
}

fn request_input_tool() -> ToolSpec {
    ToolSpec {
        name: REQUEST_INPUT_TOOL.to_string(),
        description: "Ask the human for one specific missing input before the node can continue."
            .to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "assistant_message": {
                    "type": "string",
                    "description": "The exact follow-up question or clarification needed from the human."
                }
            },
            "required": ["assistant_message"]
        }),
    }
}

fn external_tool_spec(tool: &ToolDefinition) -> ToolSpec {
    ToolSpec {
        name: tool.name.clone(),
        description: tool.description.clone(),
        parameters: tool.input_schema.clone(),
    }
}

fn all_tool_specs(request: &AgentRequest) -> Vec<ToolSpec> {
    let mut tools = request
        .available_tools
        .iter()
        .map(external_tool_spec)
        .collect::<Vec<_>>();
    tools.push(submit_output_tool(request));
    if should_allow_user_input(request) {
        tools.push(request_input_tool());
    }
    tools
}

fn tool_payload(tool: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.parameters,
        "strict": true
    })
}

fn transcript_to_responses_input(request: &AgentRequest) -> Vec<Value> {
    let mut input = vec![
        json!({ "role": "system", "content": request.system_prompt }),
        json!({ "role": "user", "content": build_node_context(request) }),
    ];

    for item in &request.transcript {
        match item {
            AgentTranscriptItem::AssistantMessage { content } => {
                input.push(json!({ "role": "assistant", "content": content }));
            }
            AgentTranscriptItem::UserMessage { content } => {
                input.push(json!({ "role": "user", "content": content }));
            }
            AgentTranscriptItem::ToolCall { call } => {
                input.push(json!({
                    "type": "function_call",
                    "call_id": call.id,
                    "name": call.name,
                    "arguments": serde_json::to_string(&call.arguments)
                        .expect("tool arguments serialize")
                }));
            }
            AgentTranscriptItem::ToolResult { result } => {
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": result.tool_call_id,
                    "output": result.content
                }));
            }
        }
    }

    input
}

fn transcript_to_chat_messages(request: &AgentRequest) -> Vec<Value> {
    let mut messages = vec![
        json!({ "role": "system", "content": request.system_prompt }),
        json!({ "role": "user", "content": build_node_context(request) }),
    ];

    for item in &request.transcript {
        match item {
            AgentTranscriptItem::AssistantMessage { content } => {
                messages.push(json!({ "role": "assistant", "content": content }));
            }
            AgentTranscriptItem::UserMessage { content } => {
                messages.push(json!({ "role": "user", "content": content }));
            }
            AgentTranscriptItem::ToolCall { call } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "tool_calls": [{
                        "id": call.id,
                        "type": "function",
                        "function": {
                            "name": call.name,
                            "arguments": serde_json::to_string(&call.arguments)
                                .expect("tool arguments serialize")
                        }
                    }]
                }));
            }
            AgentTranscriptItem::ToolResult { result } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": result.tool_call_id,
                    "content": result.content
                }));
            }
        }
    }

    messages
}

fn parse_internal_tool_outcome(
    tool_name: &str,
    arguments: &str,
    assistant_message: Option<String>,
    label: &str,
) -> Result<AgentTurnOutcome, AgentError> {
    match tool_name {
        SUBMIT_OUTPUT_TOOL => {
            #[derive(Deserialize)]
            struct SubmitOutputArgs {
                output: Value,
                assistant_message: Option<String>,
            }

            let args: SubmitOutputArgs = serde_json::from_str(arguments).map_err(|error| {
                AgentError::Failed(format!(
                    "{label} final output tool arguments were not valid JSON: {error}"
                ))
            })?;
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: args.output,
                raw_text: arguments.to_string(),
                assistant_message: args.assistant_message.or(assistant_message),
            }))
        }
        REQUEST_INPUT_TOOL => {
            #[derive(Deserialize)]
            struct RequestInputArgs {
                assistant_message: String,
            }

            let args: RequestInputArgs = serde_json::from_str(arguments).map_err(|error| {
                AgentError::Failed(format!(
                    "{label} human-input tool arguments were not valid JSON: {error}"
                ))
            })?;
            Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: arguments.to_string(),
                assistant_message: args.assistant_message,
            }))
        }
        _ => Err(AgentError::Failed(format!(
            "{label} attempted unknown internal tool {tool_name}"
        ))),
    }
}

fn parse_plain_json_completion(
    content: Option<&str>,
) -> Result<Option<AgentTurnOutcome>, AgentError> {
    let Some(content) = content.map(str::trim).filter(|content| !content.is_empty()) else {
        return Ok(None);
    };
    let candidate = content
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .or_else(|| {
            content
                .strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
                .map(str::trim)
        })
        .unwrap_or(content);
    let output = match serde_json::from_str::<Value>(candidate) {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    Ok(Some(AgentTurnOutcome::Completed(AgentTurnSuccess {
        output,
        raw_text: content.to_string(),
        assistant_message: None,
    })))
}

fn extract_chat_message_text(content: Option<&Value>) -> Option<String> {
    match content {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(parts)) => {
            let mut text = String::new();
            for part in parts {
                match part {
                    Value::String(value) => text.push_str(value),
                    Value::Object(map) => {
                        if let Some(value) = map.get("text").and_then(Value::as_str) {
                            text.push_str(value);
                        } else if let Some(value) = map.get("refusal").and_then(Value::as_str) {
                            text.push_str(value);
                        }
                    }
                    _ => {}
                }
            }
            (!text.trim().is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn parse_compatible_tool_call(call: &Value) -> Result<ToolCall, AgentError> {
    let call_id = call
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| call.get("call_id").and_then(Value::as_str))
        .unwrap_or("call-legacy");
    let function = call.get("function").unwrap_or(call);
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AgentError::Failed("OpenAI-compatible tool call missing function.name".to_string())
        })?;
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AgentError::Failed("OpenAI-compatible tool call missing function.arguments".to_string())
        })?;
    Ok(ToolCall {
        id: call_id.to_string(),
        name: name.to_string(),
        arguments: serde_json::from_str(arguments).map_err(|error| {
            AgentError::Failed(format!(
                "OpenAI-compatible tool call arguments were not valid JSON: {error}"
            ))
        })?,
        intent: None,
    })
}

fn parse_responses_output(payload: &Value) -> Result<AgentTurnOutcome, AgentError> {
    let output = payload
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::Failed("OpenAI response missing output array".to_string()))?;

    let mut assistant_message = None;
    let mut tool_calls = Vec::new();

    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
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
                                assistant_message = Some(text.to_string());
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
            Some("function_call") => {
                let call_id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AgentError::Failed("OpenAI function_call missing call id".to_string())
                    })?;
                let name = item.get("name").and_then(Value::as_str).ok_or_else(|| {
                    AgentError::Failed("OpenAI function_call missing name".to_string())
                })?;
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        AgentError::Failed("OpenAI function_call missing arguments".to_string())
                    })?;
                tool_calls.push(ToolCall {
                    id: call_id.to_string(),
                    name: name.to_string(),
                    arguments: serde_json::from_str(arguments).map_err(|error| {
                        AgentError::Failed(format!(
                            "OpenAI function_call arguments were not valid JSON: {error}"
                        ))
                    })?,
                    intent: None,
                });
            }
            _ => {}
        }
    }

    if tool_calls.is_empty() {
        return Err(AgentError::Failed(
            "OpenAI response did not contain a function call".to_string(),
        ));
    }

    if let Some(index) = tool_calls
        .iter()
        .position(|call| call.name == SUBMIT_OUTPUT_TOOL || call.name == REQUEST_INPUT_TOOL)
    {
        if tool_calls.len() != 1 {
            return Err(AgentError::Failed(
                "OpenAI response mixed internal and external tool calls".to_string(),
            ));
        }
        let call = &tool_calls[index];
        return parse_internal_tool_outcome(
            &call.name,
            &call.arguments.to_string(),
            assistant_message,
            "OpenAI",
        );
    }

    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls,
    }))
}

fn parse_chat_completion_output(
    payload: &Value,
    allow_plain_text_follow_up: bool,
) -> Result<AgentTurnOutcome, AgentError> {
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

    let assistant_message = extract_chat_message_text(message.get("content"))
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty());

    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| message.get("function_call").cloned().map(|call| vec![call]))
        .unwrap_or_default();

    if tool_calls.is_empty() {
        if let Some(outcome) = parse_plain_json_completion(assistant_message.as_deref())? {
            return Ok(outcome);
        }
        if allow_plain_text_follow_up {
            if let Some(assistant_message) = assistant_message {
                return Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                    raw_text: assistant_message.clone(),
                    assistant_message,
                }));
            }
        }
        return Err(AgentError::Failed(
            "OpenAI-compatible response did not contain a tool call, plain JSON completion, or follow-up prompt"
                .to_string(),
        ));
    }

    let parsed = tool_calls
        .iter()
        .map(parse_compatible_tool_call)
        .collect::<Result<Vec<_>, AgentError>>()?;

    if let Some(index) = parsed
        .iter()
        .position(|call| call.name == SUBMIT_OUTPUT_TOOL || call.name == REQUEST_INPUT_TOOL)
    {
        if parsed.len() != 1 {
            return Err(AgentError::Failed(
                "OpenAI-compatible response mixed internal and external tool calls".to_string(),
            ));
        }
        let call = &parsed[index];
        return parse_internal_tool_outcome(
            &call.name,
            &call.arguments.to_string(),
            assistant_message,
            "OpenAI-compatible",
        );
    }

    Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
        raw_text: assistant_message.clone().unwrap_or_default(),
        assistant_message,
        tool_calls: parsed,
    }))
}

impl OpenAiClient {
    async fn invoke_responses(
        &self,
        request: AgentRequest,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let body = json!({
            "model": request.model,
            "input": transcript_to_responses_input(&request),
            "tools": all_tool_specs(&request)
                .into_iter()
                .map(|tool| tool_payload(&tool))
                .collect::<Vec<_>>()
        });

        let payload = self
            .post_json(&self.config.responses_path, body, "OpenAI")
            .await?;
        parse_responses_output(&payload)
    }

    async fn invoke_chat_completions(
        &self,
        request: AgentRequest,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let body = json!({
            "model": request.model,
            "messages": transcript_to_chat_messages(&request),
            "tools": all_tool_specs(&request)
                .into_iter()
                .map(|tool| json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                        "strict": true
                    }
                }))
                .collect::<Vec<_>>()
        });

        let payload = self
            .post_json(
                &self.config.chat_completions_path,
                body,
                "OpenAI-compatible",
            )
            .await?;
        parse_chat_completion_output(&payload, should_allow_user_input(&request))
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

    fn endpoint(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!(
                "{}{}",
                self.config.base_url.trim_end_matches('/'),
                if path.starts_with('/') {
                    path.to_string()
                } else {
                    format!("/{path}")
                }
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn request() -> AgentRequest {
        AgentRequest {
            workflow_id: workflow_core::WorkflowId("wf-1".to_string()),
            node_id: workflow_core::NodeId("idea".to_string()),
            node_label: "Idea".to_string(),
            model: "gpt-5.5".to_string(),
            system_prompt: "You are precise.".to_string(),
            task_prompt: "Summarize the kickoff.".to_string(),
            input: json!({"entrypoint": {"text": "ORCHID-91"}, "upstream": []}),
            output_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
            tool_config: workflow_core::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
        }
    }

    #[tokio::test]
    async fn responses_request_includes_internal_submit_tool_and_parses_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(body_json(json!({
                "model": "gpt-5.5",
                "input": [
                    { "role": "system", "content": "You are precise." },
                    {
                        "role": "user",
                        "content": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}\n\nWhen you are ready to finish, call openflow_submit_node_output exactly once."
                    }
                ],
                "tools": [{
                    "type": "function",
                    "name": "openflow_submit_node_output",
                    "description": "Submit the final structured node output when the task is complete.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "output": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "summary": { "type": "string" }
                                },
                                "required": ["summary"]
                            },
                            "assistant_message": {
                                "type": ["string", "null"],
                                "description": "Optional human-facing note to show alongside the final result."
                            }
                        },
                        "required": ["output", "assistant_message"]
                    },
                    "strict": true
                }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "function_call",
                    "call_id": "call-1",
                    "name": "openflow_submit_node_output",
                    "arguments": "{\"output\":{\"summary\":\"done\"},\"assistant_message\":null}"
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::Responses,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let outcome = client.invoke(request()).await.unwrap();
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.output, json!({"summary": "done"}));
        assert_eq!(
            serde_json::from_str::<Value>(&success.raw_text).unwrap(),
            json!({"output": {"summary": "done"}, "assistant_message": null})
        );
        assert_eq!(success.assistant_message, None);
    }

    #[tokio::test]
    async fn chat_completions_request_sends_external_tools_and_parses_tool_calls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_json(json!({
                "model": "gpt-5.5",
                "messages": [
                    { "role": "system", "content": "You are precise." },
                    {
                        "role": "user",
                        "content": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}\n\nUse tools when they materially improve correctness. When you are ready to finish, call openflow_submit_node_output exactly once."
                    }
                ],
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": "read",
                            "description": "Read a file or URL.",
                            "parameters": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "path": { "type": "string" }
                                },
                                "required": ["path"]
                            },
                            "strict": true
                        }
                    },
                    {
                        "type": "function",
                        "function": {
                            "name": "openflow_submit_node_output",
                            "description": "Submit the final structured node output when the task is complete.",
                            "parameters": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "output": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "properties": {
                                            "summary": { "type": "string" }
                                        },
                                        "required": ["summary"]
                                    },
                                    "assistant_message": {
                                        "type": ["string", "null"],
                                        "description": "Optional human-facing note to show alongside the final result."
                                    }
                                },
                                "required": ["output", "assistant_message"]
                            },
                            "strict": true
                        }
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": "I need to inspect the README.",
                        "tool_calls": [{
                            "id": "call-7",
                            "type": "function",
                            "function": {
                                "name": "read",
                                "arguments": "{\"path\":\"README.md\"}"
                            }
                        }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });
        let mut request = request();
        request.available_tools = vec![ToolDefinition {
            name: "read".to_string(),
            description: "Read a file or URL.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
            tier: workflow_core::ToolTier::Read,
            concurrency: workflow_core::ToolConcurrency::Shared,
        }];

        let outcome = client.invoke(request).await.unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "I need to inspect the README.".to_string(),
                assistant_message: Some("I need to inspect the README.".to_string()),
                tool_calls: vec![ToolCall {
                    id: "call-7".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                }],
            })
        );
    }

    #[tokio::test]
    async fn chat_completions_plain_json_content_falls_back_to_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": "{\"summary\":\"done without tool call\"}"
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let outcome = client.invoke(request()).await.unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "done without tool call"}),
                raw_text: "{\"summary\":\"done without tool call\"}".to_string(),
                assistant_message: None,
            })
        );
    }

    #[tokio::test]
    async fn chat_completions_content_parts_json_falls_back_to_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": [{
                            "type": "text",
                            "text": "{\"summary\":\"done from content parts\"}"
                        }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let outcome = client.invoke(request()).await.unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::Completed(AgentTurnSuccess {
                output: json!({"summary": "done from content parts"}),
                raw_text: "{\"summary\":\"done from content parts\"}".to_string(),
                assistant_message: None,
            })
        );
    }

    #[tokio::test]
    async fn chat_completions_legacy_function_call_is_treated_as_tool_call() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "function_call": {
                            "name": "openflow_submit_node_output",
                            "arguments": "{\"output\":{\"summary\":\"legacy done\"},\"assistant_message\":null}"
                        }
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let outcome = client.invoke(request()).await.unwrap();
        let AgentTurnOutcome::Completed(success) = outcome else {
            panic!("expected completed outcome");
        };
        assert_eq!(success.output, json!({"summary": "legacy done"}));
        assert_eq!(
            serde_json::from_str::<Value>(&success.raw_text).unwrap(),
            json!({"output": {"summary": "legacy done"}, "assistant_message": null})
        );
        assert_eq!(success.assistant_message, None);
    }

    #[tokio::test]
    async fn chat_completions_plain_text_follow_up_becomes_needs_user_input() {
        let server = MockServer::start().await;
        let mut request = request();
        request.transcript = vec![AgentTranscriptItem::UserMessage {
            content: "hello".to_string(),
        }];

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "content": "You said \"hello\" — but I need a more specific idea to pass forward."
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let outcome = client.invoke(request).await.unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: "You said \"hello\" — but I need a more specific idea to pass forward."
                    .to_string(),
                assistant_message:
                    "You said \"hello\" — but I need a more specific idea to pass forward."
                        .to_string(),
            })
        );
    }

    #[tokio::test]
    async fn chat_completions_transcript_includes_tool_results_and_allows_human_input() {
        let server = MockServer::start().await;
        let mut request = request();
        request.transcript = vec![
            AgentTranscriptItem::UserMessage {
                content: "Need a safer rollout.".to_string(),
            },
            AgentTranscriptItem::ToolCall {
                call: ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: Some("Reading repo overview".to_string()),
                },
            },
            AgentTranscriptItem::ToolResult {
                result: workflow_core::ToolResult {
                    tool_call_id: "call-1".to_string(),
                    tool_name: "read".to_string(),
                    content: "# Overview".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
        ];
        request.available_tools = vec![ToolDefinition {
            name: "read".to_string(),
            description: "Read a file or URL.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
            tier: workflow_core::ToolTier::Read,
            concurrency: workflow_core::ToolConcurrency::Shared,
        }];

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(body_json(json!({
                "model": "gpt-5.5",
                "messages": [
                    { "role": "system", "content": "You are precise." },
                    {
                        "role": "user",
                        "content": "Node: Idea\nTask:\nSummarize the kickoff.\n\nUpstream input JSON:\n{\"entrypoint\":{\"text\":\"ORCHID-91\"},\"upstream\":[]}\n\nUse tools when they materially improve correctness. When you are ready to finish, call openflow_submit_node_output exactly once."
                    },
                    { "role": "user", "content": "Need a safer rollout." },
                    {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call-1",
                            "type": "function",
                            "function": {
                                "name": "read",
                                "arguments": "{\"path\":\"README.md\"}"
                            }
                        }]
                    },
                    {
                        "role": "tool",
                        "tool_call_id": "call-1",
                        "content": "# Overview"
                    }
                ],
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": "read",
                            "description": "Read a file or URL.",
                            "parameters": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "path": { "type": "string" }
                                },
                                "required": ["path"]
                            },
                            "strict": true
                        }
                    },
                    {
                        "type": "function",
                        "function": {
                            "name": "openflow_submit_node_output",
                            "description": "Submit the final structured node output when the task is complete.",
                            "parameters": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "output": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "properties": {
                                            "summary": { "type": "string" }
                                        },
                                        "required": ["summary"]
                                    },
                                    "assistant_message": {
                                        "type": ["string", "null"],
                                        "description": "Optional human-facing note to show alongside the final result."
                                    }
                                },
                                "required": ["output", "assistant_message"]
                            },
                            "strict": true
                        }
                    },
                    {
                        "type": "function",
                        "function": {
                            "name": "openflow_request_user_input",
                            "description": "Ask the human for one specific missing input before the node can continue.",
                            "parameters": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "assistant_message": {
                                        "type": "string",
                                        "description": "The exact follow-up question or clarification needed from the human."
                                    }
                                },
                                "required": ["assistant_message"]
                            },
                            "strict": true
                        }
                    }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "tool_calls": [{
                            "id": "call-9",
                            "type": "function",
                            "function": {
                                "name": "openflow_request_user_input",
                                "arguments": "{\"assistant_message\":\"Which approver is mandatory?\"}"
                            }
                        }]
                    }
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::ChatCompletions,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let outcome = client.invoke(request).await.unwrap();
        assert_eq!(
            outcome,
            AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                raw_text: "{\"assistant_message\":\"Which approver is mandatory?\"}".to_string(),
                assistant_message: "Which approver is mandatory?".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn malformed_tool_arguments_return_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "output": [{
                    "type": "function_call",
                    "call_id": "call-1",
                    "name": "openflow_submit_node_output",
                    "arguments": "not-json"
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: "key".to_string(),
            base_url: server.uri(),
            wire_api: OpenAiWireApi::Responses,
            responses_path: "/v1/responses".to_string(),
            chat_completions_path: "/v1/chat/completions".to_string(),
        });

        let error = client.invoke(request()).await.unwrap_err();
        assert!(error
            .to_string()
            .contains("OpenAI function_call arguments were not valid JSON"));
    }
}
