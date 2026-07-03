//! `AgentRequest` → rig `CompletionRequest` translation.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "rig migration: wired when AiClient switches to rig_adapter"
    )
)]

use crate::mapping::{all_tool_specs, build_node_context, ToolSpec};
use engine::{AgentRequest, AgentTranscriptItem};
use rig_core::completion::CompletionRequest;
use rig_core::message::{
    AssistantContent, Message, ToolCall as RigToolCall, ToolChoice, ToolFunction,
};
use rig_core::OneOrMany;
use serde_json::json;

pub fn to_completion_request(request: &AgentRequest) -> CompletionRequest {
    let mut history: Vec<Message> = vec![Message::user(build_node_context(request))];
    for item in &request.transcript {
        match item {
            AgentTranscriptItem::UserMessage { content } => {
                history.push(Message::user(content.clone()));
            }
            AgentTranscriptItem::AssistantMessage { content } => {
                history.push(Message::assistant(content.clone()));
            }
            AgentTranscriptItem::ToolCall { call } => {
                history.push(assistant_tool_call_message(call));
            }
            AgentTranscriptItem::ToolResult { result } => {
                history.push(user_tool_result_message(result));
            }
        }
    }
    CompletionRequest {
        model: Some(request.model.clone()),
        preamble: Some(request.system_content()),
        chat_history: OneOrMany::many(history)
            .unwrap_or_else(|_| OneOrMany::one(Message::user(build_node_context(request)))),
        documents: Vec::new(),
        tools: all_tool_specs(request).into_iter().map(rig_tool).collect(),
        temperature: None,
        max_tokens: None,
        tool_choice: Some(ToolChoice::Required),
        additional_params: additional_params(request),
        output_schema: None,
    }
}

fn rig_tool(spec: ToolSpec) -> rig_core::completion::ToolDefinition {
    rig_core::completion::ToolDefinition {
        name: spec.name,
        description: spec.description,
        parameters: spec.parameters,
    }
}

fn additional_params(request: &AgentRequest) -> Option<serde_json::Value> {
    let mut params = serde_json::Map::new();
    if let Some(effort) = &request.reasoning_effort {
        params.insert("reasoning_effort".into(), json!(effort));
    }
    if let Some(budget) = request.reasoning_budget_tokens {
        params.insert("reasoning_budget_tokens".into(), json!(budget));
    }
    if params.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(params))
    }
}

fn assistant_tool_call_message(call: &engine::ToolCall) -> Message {
    Message::Assistant {
        id: None,
        content: OneOrMany::one(AssistantContent::ToolCall(RigToolCall::new(
            call.id.clone(),
            ToolFunction {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        ))),
    }
}

fn user_tool_result_message(result: &engine::ToolResult) -> Message {
    Message::tool_result(result.tool_call_id.clone(), result.content.clone())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "unit tests assert message shapes with expect/panic"
)]
mod tests {
    use super::*;
    use crate::mapping::SUBMIT_OUTPUT_TOOL;
    use engine::{NodeId, ToolCall as EngineToolCall, WorkflowId};
    use rig_core::message::{Text, ToolResultContent, UserContent};
    use serde_json::json;

    fn request_with_transcript(transcript: Vec<AgentTranscriptItem>) -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId("wf-1".into()),
            node_id: NodeId("n1".into()),
            node_label: "Node".into(),
            model: "claude-sonnet-4-6".into(),
            system_messages: vec!["sys-a".into(), "sys-b".into()],
            task_prompt: "do the thing".into(),
            input: json!({"k": "v"}),
            output_schema: json!({"type": "object", "properties": {"r": {"type": "string"}}}),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript,
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
        }
    }

    #[test]
    fn maps_system_messages_and_task_prompt() {
        let req = to_completion_request(&request_with_transcript(Vec::new()));
        assert_eq!(req.preamble.as_deref(), Some("sys-a\n\nsys-b"));
        let first = req.chat_history.first();
        assert!(matches!(first, Message::User { .. }));
        let Message::User { content } = first else {
            panic!("expected user message");
        };
        let UserContent::Text(Text { text, .. }) = content.first() else {
            panic!("expected text user content");
        };
        assert!(text.contains("do the thing"));
    }

    #[test]
    fn always_includes_submit_output_tool_and_requires_tool_choice() {
        let req = to_completion_request(&request_with_transcript(Vec::new()));
        assert!(req.tools.iter().any(|t| t.name == SUBMIT_OUTPUT_TOOL));
        assert_eq!(req.tool_choice, Some(ToolChoice::Required));
    }

    #[test]
    fn transcript_tool_call_and_result_stay_paired() {
        let transcript = vec![
            AgentTranscriptItem::ToolCall {
                call: EngineToolCall {
                    id: "c1".into(),
                    name: "search".into(),
                    arguments: json!({"q": "x"}),
                },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "c1".into(),
                    tool_name: "search".into(),
                    content: "found".into(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
        ];
        let req = to_completion_request(&request_with_transcript(transcript));
        let msgs: Vec<_> = req.chat_history.iter().collect();
        assert_eq!(msgs.len(), 3);
        assert!(matches!(msgs[0], Message::User { .. }));
        assert!(matches!(msgs[1], Message::Assistant { .. }));
        assert!(matches!(msgs[2], Message::User { .. }));
        let Message::Assistant { content, .. } = msgs[1] else {
            panic!("expected assistant tool-call message");
        };
        assert!(matches!(
            content.first(),
            AssistantContent::ToolCall(call) if call.function.name == "search"
        ));
        let Message::User { content } = msgs[2] else {
            panic!("expected user tool-result message");
        };
        assert!(matches!(
            content.first(),
            UserContent::ToolResult(result) if result.id == "c1"
                && result.content.first() == ToolResultContent::text("found")
        ));
    }

    #[test]
    fn reasoning_params_flow_into_additional_params() {
        let mut request = request_with_transcript(Vec::new());
        request.reasoning_effort = Some("high".into());
        request.reasoning_budget_tokens = Some(2048);
        let req = to_completion_request(&request);
        let params = req.additional_params.expect("params");
        assert_eq!(params["reasoning_effort"], "high");
        assert_eq!(params["reasoning_budget_tokens"], 2048);
    }
}
