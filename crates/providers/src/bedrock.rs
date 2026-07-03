use crate::auth::AuthConfig;
use crate::aws_runtime::load_aws_sdk_config;
use crate::bedrock_errors::{map_bedrock_runtime_error, map_bedrock_stream_error};
use crate::client::BedrockConfig;
use crate::mapping::{
    all_tool_specs, build_node_context, resolve_tool_turn_outcome, should_allow_user_input,
    NoToolCallsPolicy, ResolveToolTurnParams, ToolSpec,
};
use aws_sdk_bedrockruntime::types::{
    AutoToolChoice, ContentBlock, ConversationRole, ConverseStreamOutput, InferenceConfiguration,
    Message, SystemContentBlock, TokenUsage, Tool, ToolChoice, ToolConfiguration, ToolInputSchema,
    ToolResultBlock, ToolResultContentBlock, ToolResultStatus, ToolSpecification, ToolUseBlock,
};
use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;
use aws_smithy_types::{Document, Number};
use engine::{
    AgentError, AgentRequest, AgentTranscriptItem, AgentTurnOutcome, AiStreamEvent, AiStreamSink,
    ToolCall,
};
use serde_json::Value;
use std::collections::BTreeMap;

const DEFAULT_MAX_TOKENS: i32 = 4096;

struct ConverseInput {
    system: Vec<SystemContentBlock>,
    messages: Vec<Message>,
    inference_config: InferenceConfiguration,
    tool_config: Option<ToolConfiguration>,
    tool_names: BedrockToolNames,
}

/// Bedrock Converse tool names must match `[a-zA-Z0-9_-]+`; MCP tools use `/`.
struct BedrockToolNames {
    wire_to_original: BTreeMap<String, String>,
}

impl BedrockToolNames {
    fn from_specs(specs: &[ToolSpec]) -> Result<Self, AgentError> {
        let mut wire_to_original = BTreeMap::new();
        for tool in specs {
            let wire = bedrock_wire_tool_name(&tool.name);
            if wire.is_empty() {
                return Err(AgentError::Failed(format!(
                    "Bedrock tool name {name} is empty after sanitization",
                    name = tool.name
                )));
            }
            if let Some(existing) = wire_to_original.get(&wire) {
                if existing != &tool.name {
                    return Err(AgentError::Failed(format!(
                        "Bedrock tool names {existing} and {name} both sanitize to {wire}",
                        name = tool.name
                    )));
                }
                continue;
            }
            wire_to_original.insert(wire, tool.name.clone());
        }
        Ok(Self { wire_to_original })
    }

    fn original_name(&self, wire: &str) -> String {
        self.wire_to_original
            .get(wire)
            .cloned()
            .unwrap_or_else(|| wire.to_string())
    }
}

fn bedrock_wire_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub async fn invoke(
    config: &BedrockConfig,
    auth: &AuthConfig,
    request: AgentRequest,
) -> Result<AgentTurnOutcome, AgentError> {
    let client = bedrock_runtime_client(config, auth).await?;
    let input = build_converse_input(&request)?;
    let allow_follow_up = should_allow_user_input(&request);
    let output_schema = request.output_schema.clone();
    let model_id = request.model.clone();
    let response = client
        .converse()
        .model_id(model_id)
        .set_system(Some(input.system))
        .set_messages(Some(input.messages))
        .set_inference_config(Some(input.inference_config))
        .set_tool_config(input.tool_config)
        .send()
        .await
        .map_err(|error| map_bedrock_runtime_error(&error))?;
    let usage = usage_report_from_bedrock(response.usage());
    let message = response
        .output
        .ok_or_else(|| AgentError::Failed("Bedrock Converse response missing output".into()))?
        .as_message()
        .map_err(|_| AgentError::Failed("Bedrock Converse output was not a message".into()))?
        .clone();
    parse_converse_message(
        &message,
        &input.tool_names,
        allow_follow_up,
        Some(&output_schema),
        usage,
    )
}

pub async fn invoke_stream(
    config: &BedrockConfig,
    auth: &AuthConfig,
    request: AgentRequest,
    sink: &dyn AiStreamSink,
) -> Result<AgentTurnOutcome, AgentError> {
    let client = bedrock_runtime_client(config, auth).await?;
    let input = build_converse_input(&request)?;
    let allow_follow_up = should_allow_user_input(&request);
    let output_schema = request.output_schema.clone();
    let model_id = request.model.clone();
    let mut stream = client
        .converse_stream()
        .model_id(model_id)
        .set_system(Some(input.system))
        .set_messages(Some(input.messages))
        .set_inference_config(Some(input.inference_config))
        .set_tool_config(input.tool_config)
        .send()
        .await
        .map_err(|error| map_bedrock_runtime_error(&error))?
        .stream;
    let mut aggregator = ConverseStreamAggregator::default();
    while let Some(event) = stream
        .recv()
        .await
        .map_err(|error| map_bedrock_stream_error(&error))?
    {
        if let Some(delta) = aggregator.apply_event(&event) {
            sink.on_stream_event(AiStreamEvent::AssistantDelta { content: delta });
        }
    }
    let usage = aggregator.usage();
    let message = aggregator.into_message();
    parse_converse_message(
        &message,
        &input.tool_names,
        allow_follow_up,
        Some(&output_schema),
        usage,
    )
}

fn build_converse_input(request: &AgentRequest) -> Result<ConverseInput, AgentError> {
    let specs = all_tool_specs(request);
    let tool_names = BedrockToolNames::from_specs(&specs)?;
    let system = vec![SystemContentBlock::Text(request.system_content())];
    let mut messages = vec![Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::Text(build_node_context(request)))
        .build()
        .map_err(|_| AgentError::Failed("Bedrock failed to build node context message".into()))?];
    messages.extend(transcript_to_messages(request)?);
    let tool_config = build_tool_config(&specs)?;
    Ok(ConverseInput {
        system,
        messages,
        inference_config: InferenceConfiguration::builder()
            .max_tokens(DEFAULT_MAX_TOKENS)
            .build(),
        tool_config,
        tool_names,
    })
}

fn build_tool_config(specs: &[ToolSpec]) -> Result<Option<ToolConfiguration>, AgentError> {
    if specs.is_empty() {
        return Ok(None);
    }
    let tools = specs
        .iter()
        .map(|tool| bedrock_tool_from_spec(tool, &bedrock_wire_tool_name(&tool.name)))
        .collect::<Result<Vec<_>, _>>()?;
    ToolConfiguration::builder()
        .set_tools(Some(tools))
        .tool_choice(ToolChoice::Auto(AutoToolChoice::builder().build()))
        .build()
        .map(Some)
        .map_err(|_| AgentError::Failed("Bedrock failed to build tool config".into()))
}

fn bedrock_tool_from_spec(tool: &ToolSpec, wire_name: &str) -> Result<Tool, AgentError> {
    let schema = document_from_json(&tool.parameters)?;
    let spec = ToolSpecification::builder()
        .name(wire_name.to_string())
        .description(tool.description.clone())
        .input_schema(ToolInputSchema::Json(schema))
        .build()
        .map_err(|_| AgentError::Failed(format!("Bedrock failed to build tool {}", tool.name)))?;
    Ok(Tool::ToolSpec(spec))
}

fn tool_use_content_block(call: &ToolCall) -> Result<ContentBlock, AgentError> {
    let input = document_from_json(&call.arguments)?;
    ToolUseBlock::builder()
        .tool_use_id(call.id.clone())
        .name(bedrock_wire_tool_name(&call.name))
        .input(input)
        .build()
        .map(ContentBlock::ToolUse)
        .map_err(|_| AgentError::Failed("Bedrock failed to build tool use block".into()))
}

fn tool_result_content_block(result: &engine::ToolResult) -> Result<ContentBlock, AgentError> {
    let status = if result.is_error {
        ToolResultStatus::Error
    } else {
        ToolResultStatus::Success
    };
    ToolResultBlock::builder()
        .tool_use_id(result.tool_call_id.clone())
        .status(status)
        .content(ToolResultContentBlock::Text(result.content.clone()))
        .build()
        .map(ContentBlock::ToolResult)
        .map_err(|_| AgentError::Failed("Bedrock failed to build tool result block".into()))
}

fn assistant_message(blocks: Vec<ContentBlock>) -> Result<Message, AgentError> {
    Message::builder()
        .role(ConversationRole::Assistant)
        .set_content(Some(blocks))
        .build()
        .map_err(|_| AgentError::Failed("Bedrock failed to build assistant message".into()))
}

fn user_message(blocks: Vec<ContentBlock>) -> Result<Message, AgentError> {
    Message::builder()
        .role(ConversationRole::User)
        .set_content(Some(blocks))
        .build()
        .map_err(|_| AgentError::Failed("Bedrock failed to build user message".into()))
}

fn transcript_to_messages(request: &AgentRequest) -> Result<Vec<Message>, AgentError> {
    let mut messages = Vec::new();
    let items = &request.transcript;
    let mut index = 0;
    while index < items.len() {
        match &items[index] {
            AgentTranscriptItem::AssistantMessage { content } => {
                let mut blocks = vec![ContentBlock::Text(content.clone())];
                index += 1;
                while let Some(AgentTranscriptItem::ToolCall { call }) = items.get(index) {
                    blocks.push(tool_use_content_block(call)?);
                    index += 1;
                }
                messages.push(assistant_message(blocks)?);
            }
            AgentTranscriptItem::UserMessage { content } => {
                messages.push(user_message(vec![ContentBlock::Text(content.clone())])?);
                index += 1;
            }
            AgentTranscriptItem::ToolCall { call } => {
                let mut blocks = vec![tool_use_content_block(call)?];
                index += 1;
                while let Some(AgentTranscriptItem::ToolCall { call }) = items.get(index) {
                    blocks.push(tool_use_content_block(call)?);
                    index += 1;
                }
                messages.push(assistant_message(blocks)?);
            }
            AgentTranscriptItem::ToolResult { result } => {
                let mut blocks = vec![tool_result_content_block(result)?];
                index += 1;
                while let Some(AgentTranscriptItem::ToolResult { result }) = items.get(index) {
                    blocks.push(tool_result_content_block(result)?);
                    index += 1;
                }
                messages.push(user_message(blocks)?);
            }
        }
    }
    Ok(messages)
}

fn document_from_json(value: &Value) -> Result<Document, AgentError> {
    Ok(match value {
        Value::Null => Document::Null,
        Value::Bool(value) => Document::Bool(*value),
        Value::Number(number) => Document::Number(json_number_to_smithy(number)),
        Value::String(value) => Document::String(value.clone()),
        Value::Array(values) => Document::Array(
            values
                .iter()
                .map(document_from_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Object(values) => Document::Object(
            values
                .iter()
                .map(|(key, value)| document_from_json(value).map(|doc| (key.clone(), doc)))
                .collect::<Result<_, _>>()?,
        ),
    })
}

fn json_number_to_smithy(number: &serde_json::Number) -> Number {
    match (number.as_u64(), number.as_i64(), number.as_f64()) {
        (Some(value), _, _) => Number::PosInt(value),
        (_, Some(value), _) if value >= 0 => Number::PosInt(value.cast_unsigned()),
        (_, Some(value), _) => Number::NegInt(value),
        (_, _, Some(value)) => Number::Float(value),
        _ => Number::PosInt(0),
    }
}

fn json_from_document(document: &Document) -> Result<Value, AgentError> {
    Ok(match document {
        Document::Null => Value::Null,
        Document::Bool(value) => Value::Bool(*value),
        Document::String(value) => Value::String(value.clone()),
        Document::Number(value) => Value::Number(match value {
            Number::PosInt(value) => serde_json::Number::from(*value),
            Number::NegInt(value) => serde_json::Number::from(*value),
            Number::Float(value) => {
                serde_json::Number::from_f64(*value).unwrap_or_else(|| 0.into())
            }
        }),
        Document::Array(values) => Value::Array(
            values
                .iter()
                .map(json_from_document)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Document::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| json_from_document(value).map(|json| (key.clone(), json)))
                .collect::<Result<_, _>>()?,
        ),
    })
}

fn parse_converse_message(
    message: &Message,
    tool_names: &BedrockToolNames,
    allow_plain_text_follow_up: bool,
    output_schema: Option<&Value>,
    usage: Option<engine::UsageReport>,
) -> Result<AgentTurnOutcome, AgentError> {
    let mut assistant_text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in message.content() {
        if let Ok(text) = block.as_text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                assistant_text_parts.push(trimmed.to_string());
            }
            continue;
        }
        if let Ok(tool_use) = block.as_tool_use() {
            tool_calls.push(parse_tool_use_block(tool_use, tool_names)?);
        }
    }

    let assistant_message =
        (!assistant_text_parts.is_empty()).then(|| assistant_text_parts.join("\n"));

    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up,
            error: "Bedrock response did not contain a tool call, plain JSON completion, or follow-up prompt",
        },
        output_schema,
        provider_label: "Bedrock",
        usage,
        filter_assistant_on_external_batch: false,
    })
}

fn usage_report_from_bedrock(usage: Option<&TokenUsage>) -> Option<engine::UsageReport> {
    let usage = usage?;
    Some(engine::UsageReport {
        prompt_tokens: u32::try_from(usage.input_tokens()).ok()?,
        completion_tokens: u32::try_from(usage.output_tokens()).ok()?,
        total_tokens: u32::try_from(usage.total_tokens()).ok()?,
    })
}

fn parse_tool_use_block(
    tool_use: &ToolUseBlock,
    tool_names: &BedrockToolNames,
) -> Result<ToolCall, AgentError> {
    let wire_name = tool_use.name();
    let original_name = tool_names.original_name(wire_name);
    Ok(ToolCall {
        id: tool_use.tool_use_id().to_string(),
        name: original_name,
        arguments: json_from_document(tool_use.input())?,
    })
}

async fn bedrock_runtime_client(
    config: &BedrockConfig,
    auth: &AuthConfig,
) -> Result<BedrockRuntimeClient, AgentError> {
    let AuthConfig::AwsCredentials { profile, region } = auth else {
        return Err(AgentError::Permanent(
            "Bedrock requires AWS credentials config".into(),
        ));
    };
    let effective_region = config.region.trim();
    if effective_region.is_empty() && region.trim().is_empty() {
        return Err(AgentError::Permanent(
            "Amazon Bedrock AWS region missing".into(),
        ));
    }
    let region_value = if effective_region.is_empty() {
        region.clone()
    } else {
        effective_region.to_string()
    };
    let profile_name = config
        .aws_profile
        .as_deref()
        .or(profile.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let shared = load_aws_sdk_config(&region_value, profile_name).await;
    Ok(BedrockRuntimeClient::new(&shared))
}

struct PendingToolUse {
    tool_use_id: String,
    name: String,
    input_json: String,
}

#[derive(Default)]
struct ConverseStreamAggregator {
    text_by_block: BTreeMap<i32, String>,
    pending_tools: BTreeMap<i32, PendingToolUse>,
    tool_uses: Vec<ToolUseBlock>,
    usage: Option<engine::UsageReport>,
}

impl ConverseStreamAggregator {
    fn apply_event(&mut self, event: &ConverseStreamOutput) -> Option<String> {
        match event {
            ConverseStreamOutput::ContentBlockDelta(delta_event) => {
                let index = delta_event.content_block_index;
                let delta = delta_event.delta()?;
                if let Ok(text) = delta.as_text() {
                    self.text_by_block.entry(index).or_default().push_str(text);
                    return Some(text.clone());
                }
                if let Ok(tool_delta) = delta.as_tool_use() {
                    if let Some(pending) = self.pending_tools.get_mut(&index) {
                        pending.input_json.push_str(tool_delta.input());
                    }
                }
            }
            ConverseStreamOutput::ContentBlockStart(start_event) => {
                let index = start_event.content_block_index;
                if let Some(start) = start_event.start() {
                    if let Ok(tool_start) = start.as_tool_use() {
                        self.pending_tools.insert(
                            index,
                            PendingToolUse {
                                tool_use_id: tool_start.tool_use_id().to_string(),
                                name: tool_start.name().to_string(),
                                input_json: String::new(),
                            },
                        );
                    }
                }
            }
            ConverseStreamOutput::ContentBlockStop(stop_event) => {
                if let Some(pending) = self.pending_tools.remove(&stop_event.content_block_index) {
                    let arguments = serde_json::from_str(&pending.input_json)
                        .unwrap_or_else(|_| Value::Object(serde_json::Map::default()));
                    let input = document_from_json(&arguments)
                        .unwrap_or_else(|_| Document::Object(std::collections::HashMap::new()));
                    if let Ok(block) = ToolUseBlock::builder()
                        .tool_use_id(pending.tool_use_id)
                        .name(pending.name)
                        .input(input)
                        .build()
                    {
                        self.tool_uses.push(block);
                    }
                }
            }
            ConverseStreamOutput::Metadata(metadata) => {
                self.usage = usage_report_from_bedrock(metadata.usage());
            }
            ConverseStreamOutput::MessageStop(_) | ConverseStreamOutput::MessageStart(_) | _ => {}
        }
        None
    }

    fn usage(&self) -> Option<engine::UsageReport> {
        self.usage.clone()
    }

    fn into_message(self) -> Message {
        let mut content = Vec::new();
        for (_, text) in self.text_by_block {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                content.push(ContentBlock::Text(trimmed.to_string()));
            }
        }
        for tool_use in self.tool_uses {
            content.push(ContentBlock::ToolUse(tool_use));
        }
        Message::builder()
            .role(ConversationRole::Assistant)
            .set_content(Some(content))
            .build()
            .unwrap_or_else(|_| empty_assistant_message())
    }
}

#[allow(clippy::expect_used)]
fn empty_assistant_message() -> Message {
    Message::builder()
        .role(ConversationRole::Assistant)
        .content(ContentBlock::Text(String::new()))
        .build()
        .expect("empty assistant message")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::{
        ContentBlockDelta, ConverseStreamMetadataEvent, TokenUsage,
    };
    use engine::{
        AgentToolCallBatch, NodeId, NodeToolConfig, ToolDefinition, ToolTier, UsageReport,
        WorkflowId,
    };

    fn sample_agent_request() -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId("wf-1".to_string()),
            node_id: NodeId("node-1".to_string()),
            node_label: "Summarize".to_string(),
            model: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            system_messages: vec!["You are precise.".to_string()],
            task_prompt: "Summarize the kickoff.".to_string(),
            input: serde_json::json!({"entrypoint": {"text": "ORCHID-91"}, "upstream": []}),
            output_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": { "summary": { "type": "string" } },
                "required": ["summary"]
            }),
            tool_config: NodeToolConfig::default(),
            available_tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"]
                }),
                tier: ToolTier::Read,
                concurrency: engine::ToolConcurrency::Shared,
            }],
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
        }
    }

    #[test]
    fn maps_agent_request_to_converse_shape() {
        let request = sample_agent_request();
        let input = build_converse_input(&request).expect("converse input");
        assert_eq!(input.system.len(), 1);
        assert_eq!(input.messages.len(), 1);
        assert!(input.tool_config.is_some());
    }

    #[test]
    fn maps_tool_result_transcript_blocks() {
        let mut request = sample_agent_request();
        request.transcript = vec![
            AgentTranscriptItem::ToolCall {
                call: ToolCall {
                    id: "toolu_1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "README.md"}),
                },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "toolu_1".to_string(),
                    tool_name: "read".to_string(),
                    content: "file contents".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
        ];
        let input = build_converse_input(&request).expect("converse input");
        assert_eq!(input.messages.len(), 3);
        assert!(message_contains_tool_use(&input.messages));
        assert!(message_contains_tool_result(&input.messages));
    }

    #[test]
    fn groups_parallel_tool_calls_and_results_into_single_messages() {
        let mut request = sample_agent_request();
        request.transcript = vec![
            AgentTranscriptItem::ToolCall {
                call: ToolCall {
                    id: "tooluse_a".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "a.md"}),
                },
            },
            AgentTranscriptItem::ToolCall {
                call: ToolCall {
                    id: "tooluse_b".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "b.md"}),
                },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "tooluse_a".to_string(),
                    tool_name: "read".to_string(),
                    content: "a".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "tooluse_b".to_string(),
                    tool_name: "read".to_string(),
                    content: "b".to_string(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
        ];
        let input = build_converse_input(&request).expect("converse input");
        assert_eq!(input.messages.len(), 3);
        let assistant = &input.messages[1];
        assert_eq!(assistant.role(), &ConversationRole::Assistant);
        assert_eq!(assistant.content().len(), 2);
        let user = &input.messages[2];
        assert_eq!(user.role(), &ConversationRole::User);
        assert_eq!(user.content().len(), 2);
        assert_eq!(
            user.content()[0]
                .as_tool_result()
                .ok()
                .map(aws_sdk_bedrockruntime::types::ToolResultBlock::tool_use_id),
            Some("tooluse_a")
        );
        assert_eq!(
            user.content()[1]
                .as_tool_result()
                .ok()
                .map(aws_sdk_bedrockruntime::types::ToolResultBlock::tool_use_id),
            Some("tooluse_b")
        );
    }

    fn message_contains_tool_use(messages: &[Message]) -> bool {
        messages.iter().any(|message| {
            message
                .content()
                .iter()
                .any(|block| block.as_tool_use().is_ok())
        })
    }

    fn message_contains_tool_result(messages: &[Message]) -> bool {
        messages.iter().any(|message| {
            message
                .content()
                .iter()
                .any(|block| block.as_tool_use().is_err() && block.as_tool_result().is_ok())
        })
    }

    #[test]
    fn stream_aggregator_collects_text_deltas() {
        let mut agg = ConverseStreamAggregator::default();
        let delta = aws_sdk_bedrockruntime::types::ContentBlockDeltaEvent::builder()
            .content_block_index(0)
            .delta(ContentBlockDelta::Text("Hello".to_string()))
            .build()
            .expect("delta");
        let event = ConverseStreamOutput::ContentBlockDelta(delta);
        assert_eq!(agg.apply_event(&event).as_deref(), Some("Hello"));
        let delta = aws_sdk_bedrockruntime::types::ContentBlockDeltaEvent::builder()
            .content_block_index(0)
            .delta(ContentBlockDelta::Text(" world".to_string()))
            .build()
            .expect("delta");
        let event = ConverseStreamOutput::ContentBlockDelta(delta);
        assert_eq!(agg.apply_event(&event).as_deref(), Some(" world"));
        let message = agg.into_message();
        assert_eq!(
            message.content()[0].as_text().map(String::as_str),
            Ok("Hello world")
        );
    }

    #[test]
    fn stream_aggregator_collects_usage_from_metadata() {
        let mut agg = ConverseStreamAggregator::default();
        let metadata = ConverseStreamMetadataEvent::builder()
            .usage(
                TokenUsage::builder()
                    .input_tokens(23)
                    .output_tokens(7)
                    .total_tokens(30)
                    .build()
                    .expect("usage"),
            )
            .build();
        let event = ConverseStreamOutput::Metadata(metadata);
        assert_eq!(agg.apply_event(&event), None);
        assert_eq!(
            agg.usage(),
            Some(UsageReport {
                prompt_tokens: 23,
                completion_tokens: 7,
                total_tokens: 30,
            })
        );
    }

    #[test]
    fn parse_converse_message_attaches_usage_to_tool_calls() {
        let tool_names = BedrockToolNames::from_specs(&[ToolSpec {
            name: "read".to_string(),
            description: "Read a file.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }])
        .expect("tool names");
        let message = Message::builder()
            .role(ConversationRole::Assistant)
            .content(ContentBlock::Text("Read that file.".to_string()))
            .content(ContentBlock::ToolUse(
                ToolUseBlock::builder()
                    .tool_use_id("toolu_1")
                    .name("read")
                    .input(Document::Object(std::collections::HashMap::from([(
                        "path".to_string(),
                        Document::String("README.md".to_string()),
                    )])))
                    .build()
                    .expect("tool use"),
            ))
            .build()
            .expect("message");

        let outcome = parse_converse_message(
            &message,
            &tool_names,
            false,
            None,
            Some(UsageReport {
                prompt_tokens: 9,
                completion_tokens: 4,
                total_tokens: 13,
            }),
        )
        .expect("outcome");

        assert_eq!(
            outcome,
            AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                raw_text: "Read that file.".to_string(),
                assistant_message: Some("Read that file.".to_string()),
                tool_calls: vec![ToolCall {
                    id: "toolu_1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "README.md"}),
                }],
                usage: Some(UsageReport {
                    prompt_tokens: 9,
                    completion_tokens: 4,
                    total_tokens: 13,
                }),
            })
        );
    }

    #[test]
    fn bedrock_wire_tool_name_replaces_slashes_for_mcp_tools() {
        assert_eq!(
            bedrock_wire_tool_name("mcp/playwright/browser_click"),
            "mcp_playwright_browser_click"
        );
    }

    #[test]
    fn converse_tool_config_uses_sanitized_mcp_tool_names() {
        let mut request = sample_agent_request();
        request.available_tools.push(ToolDefinition {
            name: "mcp/playwright/browser_click".to_string(),
            description: "Click a browser element.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "selector": { "type": "string" } }
            }),
            tier: ToolTier::Write,
            concurrency: engine::ToolConcurrency::Shared,
        });
        let input = build_converse_input(&request).expect("converse input");
        let tools = input
            .tool_config
            .as_ref()
            .map(aws_sdk_bedrockruntime::types::ToolConfiguration::tools)
            .expect("tool config");
        assert!(tools.iter().any(|tool| tool
            .as_tool_spec()
            .is_ok_and(|spec| { spec.name() == "mcp_playwright_browser_click" })));
        assert_eq!(
            input
                .tool_names
                .original_name("mcp_playwright_browser_click"),
            "mcp/playwright/browser_click"
        );
    }
}
