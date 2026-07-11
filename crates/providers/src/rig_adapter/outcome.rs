//! rig `CompletionResponse` → `AgentTurnOutcome`, reusing the mapping.rs node protocol.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "rig migration: wired when AiClient switches to rig_adapter"
    )
)]

use crate::client::OpenAiCompatibleConfig;
use crate::mapping::{
    attach_usage, parse_internal_tool_outcome, resolve_tool_turn_outcome, should_allow_user_input,
    NoToolCallsPolicy, ResolveToolTurnParams,
};
use crate::spec::WireApi;
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

pub fn no_tool_calls_policy(
    request: &engine::AgentRequest,
    openai_config: Option<&OpenAiCompatibleConfig>,
) -> NoToolCallsPolicy {
    if matches!(openai_config.map(|c| c.wire_api), Some(WireApi::Responses)) {
        NoToolCallsPolicy::Error("OpenAI response did not contain a function call")
    } else {
        NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up: should_allow_user_input(request),
            error: "provider returned neither tool calls nor recoverable output",
        }
    }
}

pub fn resolve_outcome(
    choice: Vec<AssistantContent>,
    usage: Usage,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
    no_tool_calls: NoToolCallsPolicy,
) -> Result<AgentTurnOutcome, AgentError> {
    let (text_parts, tool_calls) = partition_choice(choice);
    resolve_collected(
        &text_parts,
        tool_calls,
        usage,
        provider_label,
        output_schema,
        no_tool_calls,
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
    no_tool_calls: NoToolCallsPolicy,
) -> Result<AgentTurnOutcome, AgentError> {
    let assistant_message = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };
    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls,
        output_schema,
        provider_label,
        usage: to_usage_report(&usage),
        filter_assistant_on_external_batch: true,
    })
}

pub(crate) fn partition_choice(
    choice: Vec<AssistantContent>,
) -> (Vec<String>, Vec<engine::ToolCall>) {
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
            AssistantContent::Reasoning(reasoning) => {
                let text = reasoning.display_text();
                if !text.is_empty() {
                    text_parts.push(text);
                }
            }
            AssistantContent::Image(_) => {}
        }
    }
    (text_parts, tool_calls)
}

const EMPTY_TURN_ERROR: &str = "provider returned neither tool calls nor recoverable output";

/// Replace the generic empty-turn message with provider + model context.
#[must_use]
pub fn enrich_empty_turn_error(error: AgentError, provider_label: &str, model: &str) -> AgentError {
    if let AgentError::Failed(message) = &error {
        if message.contains(EMPTY_TURN_ERROR) {
            let model = model.trim();
            let detail = if model.is_empty() {
                format!(
                    "{provider_label} returned no tool calls and no usable text. \
                     Workflow nodes require a tool call (web_search, submit_output, etc.). \
                     Try another model or provider."
                )
            } else {
                format!(
                    "{provider_label} model `{model}` returned no tool calls and no usable text. \
                     Workflow nodes require a tool call (web_search, submit_output, etc.). \
                     Try another model or provider."
                )
            };
            return AgentError::Failed(detail);
        }
    }
    error
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

    fn recover(allow_plain_text_follow_up: bool) -> NoToolCallsPolicy {
        NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up,
            error: "provider returned neither tool calls nor recoverable output",
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
            recover(true),
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
        let outcome =
            resolve_outcome(choice, usage(), "Test provider", None, recover(true)).unwrap();
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
        let outcome =
            resolve_outcome(choice, usage(), "Test provider", None, recover(true)).unwrap();
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
        let outcome =
            resolve_outcome(choice, usage(), "Test provider", None, recover(true)).unwrap();
        assert!(matches!(outcome, engine::AgentTurnOutcome::Completed(_)));
    }

    #[test]
    fn plain_text_without_user_input_tool_fails() {
        let choice = vec![AssistantContent::text("Hello without tools")];
        let err =
            resolve_outcome(choice, usage(), "Test provider", None, recover(false)).unwrap_err();
        assert!(matches!(err, AgentError::Failed(_)));
    }

    #[test]
    fn responses_api_no_tool_calls_errors() {
        let choice = vec![AssistantContent::text("plain")];
        let err = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            NoToolCallsPolicy::Error("OpenAI response did not contain a function call"),
        )
        .unwrap_err();
        assert!(matches!(err, AgentError::Failed(_)));
    }

    #[test]
    fn enrich_empty_turn_error_adds_provider_and_model() {
        let err = enrich_empty_turn_error(
            AgentError::Failed(EMPTY_TURN_ERROR.to_string()),
            "Custom OpenAI-compatible API",
            "mimo-v2.5",
        );
        let AgentError::Failed(message) = err else {
            panic!("expected Failed");
        };
        assert!(message.contains("Custom OpenAI-compatible API"));
        assert!(message.contains("mimo-v2.5"));
        assert!(!message.contains(EMPTY_TURN_ERROR));
    }
}
