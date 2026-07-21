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
    attach_usage, parse_internal_tool_outcome, resolve_tool_turn_outcome, NoToolCallsPolicy,
    ResolveToolTurnParams,
};
use crate::rig_adapter::reasoning_convert;
use crate::spec::WireApi;
use engine::{AgentError, AgentReasoning, AgentTurnOutcome, UsageReport};
use rig_core::completion::Usage;
use rig_core::message::{AssistantContent, Text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseDiagnostics {
    pub finish_reason: Option<String>,
    pub content_categories: Vec<&'static str>,
    pub usage: Option<UsageReport>,
}

#[must_use]
pub fn response_diagnostics(
    choice: &[AssistantContent],
    usage: &Usage,
    finish_reason: Option<String>,
) -> ResponseDiagnostics {
    let mut content_categories = Vec::new();
    for item in choice {
        let category = match item {
            AssistantContent::Text(_) => "text",
            AssistantContent::Reasoning(_) => "reasoning",
            AssistantContent::ToolCall(_) => "tool_call",
            AssistantContent::Image(_) => "image",
        };
        if !content_categories.contains(&category) {
            content_categories.push(category);
        }
    }
    ResponseDiagnostics {
        finish_reason,
        content_categories,
        usage: to_usage_report(usage),
    }
}

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
    _request: &engine::AgentRequest,
    openai_config: Option<&OpenAiCompatibleConfig>,
) -> NoToolCallsPolicy {
    if matches!(openai_config.map(|c| c.wire_api), Some(WireApi::Responses)) {
        NoToolCallsPolicy::Error("OpenAI response did not contain a function call")
    } else {
        NoToolCallsPolicy::Recover {
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
    let (text_parts, reasoning, tool_calls) = partition_choice(choice);
    let assistant_message = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };
    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        reasoning,
        no_tool_calls,
        output_schema,
        provider_label,
        usage: to_usage_report(&usage),
        filter_assistant_on_external_batch: true,
    })
}

/// Streaming/raw-wire path where tool arguments are still unparsed strings.
pub fn resolve_outcome_raw_tool_call(
    tool_name: &str,
    arguments: &str,
    usage: Usage,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    parse_internal_tool_outcome(
        tool_name,
        arguments,
        None,
        provider_label,
        output_schema,
        Vec::new(),
    )
    .map(|outcome| attach_usage(outcome, to_usage_report(&usage)))
}

pub(super) fn partition_choice(
    choice: Vec<AssistantContent>,
) -> (Vec<String>, Vec<AgentReasoning>, Vec<engine::ToolCall>) {
    let mut text_parts = Vec::new();
    let mut reasoning = Vec::new();
    let mut tool_calls = Vec::new();
    for item in choice {
        match item {
            AssistantContent::Text(Text { text, .. }) => text_parts.push(text),
            AssistantContent::ToolCall(call) => tool_calls.push(engine::ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
            }),
            AssistantContent::Reasoning(block) => {
                reasoning.push(reasoning_convert::rig_to_agent(&block));
            }
            AssistantContent::Image(_) => {}
        }
    }
    (text_parts, reasoning, tool_calls)
}

/// Canonical empty-turn marker. Engine [`AgentError::is_empty_provider_turn`] matches this
/// (and the enriched "no tool calls and no usable text" form).
#[allow(clippy::redundant_pub_crate)] // crate-private module; keep pub(crate) for intentional crate API
pub(crate) const EMPTY_TURN_ERROR: &str =
    "provider returned neither tool calls nor recoverable output";

fn is_empty_turn_message(message: &str) -> bool {
    message.contains(EMPTY_TURN_ERROR) || message.contains("no message or tool call")
}

/// Replace the generic empty-turn message with provider + model context.
#[must_use]
pub fn enrich_empty_turn_error(error: AgentError, provider_label: &str, model: &str) -> AgentError {
    enrich_empty_turn_error_with_response(error, provider_label, model, None)
}

#[must_use]
pub fn enrich_empty_turn_error_with_response(
    error: AgentError,
    provider_label: &str,
    model: &str,
    diagnostics: Option<&ResponseDiagnostics>,
) -> AgentError {
    if let AgentError::Failed(message) = &error {
        if is_empty_turn_message(message) {
            let model = model.trim();
            let mut detail = if model.is_empty() {
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
            if let Some(diagnostics) = diagnostics {
                let finish_reason = diagnostics.finish_reason.as_deref().unwrap_or("missing");
                let categories = if diagnostics.content_categories.is_empty() {
                    "none".to_string()
                } else {
                    diagnostics.content_categories.join(",")
                };
                let usage = diagnostics.usage.as_ref().map_or_else(
                    || "missing".to_string(),
                    |usage| {
                        format!(
                            "prompt={}, completion={}, total={}",
                            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                        )
                    },
                );
                detail.push_str(" Response metadata: finish_reason=");
                detail.push_str(finish_reason);
                detail.push_str("; content=");
                detail.push_str(&categories);
                detail.push_str("; usage=");
                detail.push_str(&usage);
                detail.push('.');
            }
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
    use rig_core::message::{AssistantContent, Reasoning};
    use serde_json::json;

    fn usage() -> Usage {
        Usage {
            input_tokens: 100,
            output_tokens: 20,
            total_tokens: 120,
            ..Usage::new()
        }
    }

    fn recover() -> NoToolCallsPolicy {
        NoToolCallsPolicy::Recover {
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
            recover(),
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
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            recover(),
        )
        .unwrap();
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
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            recover(),
        )
        .unwrap();
        match outcome {
            engine::AgentTurnOutcome::ToolCalls(batch) => {
                assert_eq!(batch.tool_calls.len(), 1);
                assert_eq!(batch.tool_calls[0].name, "search");
            }
            other => panic!("expected ToolCalls, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_preserved_separately_from_assistant_text() {
        let choice = vec![
            AssistantContent::Reasoning(Reasoning::new_with_signature("think", Some("sig".into()))),
            AssistantContent::text("Let me search."),
            AssistantContent::tool_call("c1", "search", json!({"q": "x"})),
        ];
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            recover(),
        )
        .unwrap();
        match outcome {
            engine::AgentTurnOutcome::ToolCalls(batch) => {
                assert_eq!(batch.reasoning.len(), 1);
                assert_eq!(batch.assistant_message.as_deref(), Some("Let me search."));
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
            Some(&json!({"type": "object"})),
        );
        assert!(outcome.is_ok());
    }

    #[test]
    fn no_tool_calls_with_plain_json_text_recovers() {
        let choice = vec![AssistantContent::text(
            r#"{"output": {"r": "v"}, "assistant_message": null}"#,
        )];
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            recover(),
        )
        .unwrap();
        assert!(matches!(outcome, engine::AgentTurnOutcome::Completed(_)));
    }

    #[test]
    fn plain_text_becomes_a_neutral_message_turn() {
        let choice = vec![AssistantContent::text("Hello without tools")];
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            recover(),
        )
        .unwrap();
        let engine::AgentTurnOutcome::Message(message) = outcome else {
            panic!("expected neutral message turn");
        };
        assert_eq!(message.assistant_message, "Hello without tools");
    }

    #[test]
    fn responses_api_plain_text_becomes_a_neutral_message_turn() {
        let choice = vec![AssistantContent::text("plain")];
        let outcome = resolve_outcome(
            choice,
            usage(),
            "Test provider",
            None,
            NoToolCallsPolicy::Error("OpenAI response did not contain a function call"),
        )
        .unwrap();
        assert!(matches!(outcome, AgentTurnOutcome::Message(_)));
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

    #[test]
    fn enrich_rewrites_rig_empty_response_phrase() {
        let err = enrich_empty_turn_error(
            AgentError::Failed(
                "Custom OpenAI-compatible API response error: \
                 Response contained no message or tool call (empty)"
                    .to_string(),
            ),
            "Custom OpenAI-compatible API",
            "mimo",
        );
        let AgentError::Failed(message) = err else {
            panic!("expected Failed");
        };
        assert!(message.contains("no tool calls and no usable text"));
        assert!(message.contains("mimo"));
        assert!(!message.contains("no message or tool call"));
    }

    #[test]
    fn empty_turn_diagnostics_classify_reasoning_finish_and_usage() {
        let choice = vec![AssistantContent::Reasoning(Reasoning::new_with_signature(
            "private work",
            None,
        ))];
        let diagnostics = response_diagnostics(&choice, &usage(), Some("length".to_string()));
        let err = enrich_empty_turn_error_with_response(
            AgentError::Failed(EMPTY_TURN_ERROR.to_string()),
            "Custom OpenAI-compatible API",
            "minimax-m3",
            Some(&diagnostics),
        );
        let AgentError::Failed(message) = err else {
            panic!("expected Failed");
        };
        assert!(message.contains("finish_reason=length"));
        assert!(message.contains("content=reasoning"));
        assert!(message.contains("prompt=100, completion=20, total=120"));
        assert!(!message.contains("private work"));
    }
}
