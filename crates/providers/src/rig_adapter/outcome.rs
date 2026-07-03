//! rig `CompletionResponse` → `AgentTurnOutcome`, reusing the mapping.rs node protocol.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "rig migration: wired when AiClient switches to rig_adapter"
    )
)]

use crate::mapping::{
    attach_usage, parse_internal_tool_outcome, resolve_tool_turn_outcome, NoToolCallsPolicy,
    ResolveToolTurnParams,
};
use engine::{AgentError, AgentTurnOutcome, UsageReport};
use rig_core::completion::Usage;
use rig_core::message::{AssistantContent, Text};

pub fn to_usage_report(usage: &Usage) -> Option<UsageReport> {
    if usage.input_tokens == 0 && usage.output_tokens == 0 && usage.total_tokens == 0 {
        return None;
    }
    Some(UsageReport {
        prompt_tokens: u32::try_from(usage.input_tokens).unwrap_or(u32::MAX),
        completion_tokens: u32::try_from(usage.output_tokens).unwrap_or(u32::MAX),
        total_tokens: u32::try_from(usage.total_tokens).unwrap_or(u32::MAX),
    })
}

pub fn resolve_outcome(
    choice: Vec<AssistantContent>,
    usage: Usage,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    let (text_parts, tool_calls) = partition_choice(choice);
    resolve_collected(
        &text_parts,
        tool_calls,
        usage,
        provider_label,
        output_schema,
    )
}

/// Streaming/raw-wire path where tool arguments are still unparsed strings.
pub fn resolve_outcome_raw_tool_call(
    tool_name: &str,
    arguments: &str,
    usage: Usage,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    parse_internal_tool_outcome(tool_name, arguments, None, provider_label, output_schema)
        .map(|outcome| attach_usage(outcome, to_usage_report(&usage)))
}

fn resolve_collected(
    text_parts: &[String],
    tool_calls: Vec<engine::ToolCall>,
    usage: Usage,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    let assistant_message = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };
    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up: true,
            error: "provider returned neither tool calls nor recoverable output",
        },
        output_schema,
        provider_label,
        usage: to_usage_report(&usage),
        filter_assistant_on_external_batch: true,
    })
}

fn partition_choice(choice: Vec<AssistantContent>) -> (Vec<String>, Vec<engine::ToolCall>) {
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    for item in choice {
        match item {
            AssistantContent::Text(Text { text, .. }) => text_parts.push(text),
            AssistantContent::ToolCall(call) => tool_calls.push(engine::ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
            }),
            AssistantContent::Reasoning(_) | AssistantContent::Image(_) => {}
        }
    }
    (text_parts, tool_calls)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "unit tests assert outcome shapes with expect/panic/unwrap"
)]
mod tests {
    use super::*;
    use crate::mapping::{REQUEST_INPUT_TOOL, SUBMIT_OUTPUT_TOOL};
    use rig_core::message::AssistantContent;
    use serde_json::json;

    fn usage() -> Usage {
        Usage {
            input_tokens: 100,
            output_tokens: 20,
            total_tokens: 120,
            ..Usage::new()
        }
    }

    #[test]
    fn submit_output_tool_call_becomes_completed() {
        let choice = vec![AssistantContent::tool_call(
            "c1",
            SUBMIT_OUTPUT_TOOL,
            json!({"output": {"r": "done"}, "assistant_message": null}),
        )];
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            Some(&json!({"type": "object"})),
        )
        .unwrap();
        match outcome {
            engine::AgentTurnOutcome::Completed(s) => {
                assert_eq!(s.output, json!({"r": "done"}));
                assert_eq!(s.usage.as_ref().map(|u| u.prompt_tokens), Some(100));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn request_input_becomes_needs_user_input() {
        let choice = vec![AssistantContent::tool_call(
            "c1",
            REQUEST_INPUT_TOOL,
            json!({"assistant_message": "Which env?"}),
        )];
        let outcome = resolve_outcome(choice, usage(), "Test provider", None).unwrap();
        assert!(matches!(
            outcome,
            engine::AgentTurnOutcome::NeedsUserInput(n) if n.assistant_message == "Which env?"
        ));
    }

    #[test]
    fn external_tool_calls_become_tool_call_batch() {
        let choice = vec![
            AssistantContent::text("Let me search."),
            AssistantContent::tool_call("c1", "search", json!({"q": "x"})),
        ];
        let outcome = resolve_outcome(choice, usage(), "Test provider", None).unwrap();
        match outcome {
            engine::AgentTurnOutcome::ToolCalls(batch) => {
                assert_eq!(batch.tool_calls.len(), 1);
                assert_eq!(batch.tool_calls[0].name, "search");
            }
            other => panic!("expected ToolCalls, got {other:?}"),
        }
    }

    #[test]
    fn malformed_submit_output_json_recovers_via_jsonrepair() {
        let outcome = resolve_outcome_raw_tool_call(
            SUBMIT_OUTPUT_TOOL,
            r#"{"output": {"r": "done"}, "assistant_message": null,}"#,
            usage(),
            "Test provider",
            None,
        );
        assert!(outcome.is_ok());
    }

    #[test]
    fn no_tool_calls_with_plain_json_text_recovers() {
        let choice = vec![AssistantContent::text(
            r#"{"output": {"r": "v"}, "assistant_message": null}"#,
        )];
        let outcome = resolve_outcome(choice, usage(), "Test provider", None).unwrap();
        assert!(matches!(outcome, engine::AgentTurnOutcome::Completed(_)));
    }
}
