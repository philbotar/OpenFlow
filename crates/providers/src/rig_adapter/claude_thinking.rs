//! Map `OpenFlow` reasoning settings to `Claude` `thinking` / `output_config` wire fields.

use engine::{AgentError, AgentRequest};
use rig_core::completion::CompletionRequest;
use rig_core::message::ToolChoice;
use serde_json::{Value, json};

const MIN_MANUAL_BUDGET: u32 = 1_024;
const MAX_MANUAL_BUDGET: u32 = 59_000;
const ADAPTIVE_MIN_MAX_TOKENS: u64 = 16_000;
const MANUAL_ANSWER_HEADROOM: u64 = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudePlatform {
    Anthropic,
    Bedrock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilityMode {
    NonClaude,
    Unsupported,
    Manual,
    AdaptiveOptional,
    AdaptiveDefaultOn,
    AdaptiveAlwaysOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffortInput<'a> {
    Unset,
    ExplicitNone,
    Value(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WireThinking {
    emit_disabled: bool,
    adaptive: bool,
    manual_budget: Option<u32>,
    effort: Option<&'static str>,
}

pub fn apply(
    platform: ClaudePlatform,
    request: &mut CompletionRequest,
    agent_request: &AgentRequest,
) -> Result<(), AgentError> {
    strip_openflow_reasoning_keys(request);
    let mode = classify_mode(platform, &agent_request.model);
    if matches!(mode, CapabilityMode::NonClaude) {
        return Ok(());
    }

    let effort = parse_effort(agent_request);
    let wire = map_wire(mode, effort, agent_request.reasoning_budget_tokens)?;
    if !wire_is_active(wire) {
        return Ok(());
    }

    merge_claude_params(request, wire);
    if !wire.emit_disabled {
        request.tool_choice = Some(ToolChoice::Auto);
        apply_max_tokens_floor(request, wire);
    }
    Ok(())
}

const fn wire_is_active(wire: WireThinking) -> bool {
    wire.emit_disabled || wire.adaptive || wire.manual_budget.is_some()
}

fn strip_openflow_reasoning_keys(request: &mut CompletionRequest) {
    let Some(Value::Object(mut map)) = request.additional_params.take() else {
        return;
    };
    map.remove("reasoning_effort");
    map.remove("reasoning_budget_tokens");
    request.additional_params = if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    };
}

fn parse_effort(agent: &AgentRequest) -> EffortInput<'_> {
    match agent
        .reasoning_effort
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => EffortInput::Unset,
        Some("none") => EffortInput::ExplicitNone,
        Some(value) => EffortInput::Value(value),
    }
}

fn map_wire(
    mode: CapabilityMode,
    effort: EffortInput<'_>,
    budget: Option<u32>,
) -> Result<WireThinking, AgentError> {
    match mode {
        CapabilityMode::NonClaude | CapabilityMode::Unsupported => Ok(inactive_wire()),
        CapabilityMode::AdaptiveAlwaysOn => Ok(adaptive_wire(effort, true)),
        CapabilityMode::AdaptiveDefaultOn => match effort {
            EffortInput::Unset => Ok(adaptive_wire(EffortInput::Unset, true)),
            EffortInput::ExplicitNone => Ok(WireThinking {
                emit_disabled: true,
                adaptive: false,
                manual_budget: None,
                effort: None,
            }),
            EffortInput::Value(_) => Ok(adaptive_wire(effort, false)),
        },
        CapabilityMode::AdaptiveOptional => match effort {
            EffortInput::Unset | EffortInput::ExplicitNone => Ok(inactive_wire()),
            EffortInput::Value(_) => Ok(adaptive_wire(effort, false)),
        },
        CapabilityMode::Manual => manual_wire(effort, budget),
    }
}

fn adaptive_wire(effort: EffortInput<'_>, default_on: bool) -> WireThinking {
    let effort_level = match effort {
        EffortInput::Unset | EffortInput::ExplicitNone if default_on => None,
        EffortInput::Value("adaptive") => None,
        EffortInput::Value("low") => Some("low"),
        EffortInput::Value("medium") => Some("medium"),
        EffortInput::Value("high") => Some("high"),
        EffortInput::Unset | EffortInput::ExplicitNone | EffortInput::Value(_) => {
            return inactive_wire();
        }
    };
    WireThinking {
        emit_disabled: false,
        adaptive: true,
        manual_budget: None,
        effort: effort_level,
    }
}

fn manual_wire(effort: EffortInput<'_>, budget: Option<u32>) -> Result<WireThinking, AgentError> {
    let tier = match effort {
        EffortInput::Value("adaptive" | "medium") => Some(40_960),
        EffortInput::Value("low") => Some(10_240),
        EffortInput::Value("high") => Some(59_000),
        EffortInput::Unset | EffortInput::ExplicitNone | EffortInput::Value(_) => {
            return Ok(inactive_wire());
        }
    };
    let budget = budget.unwrap_or_else(|| tier.unwrap_or(40_960));
    validate_manual_budget(budget)?;
    Ok(WireThinking {
        emit_disabled: false,
        adaptive: false,
        manual_budget: Some(budget),
        effort: None,
    })
}

const fn inactive_wire() -> WireThinking {
    WireThinking {
        emit_disabled: false,
        adaptive: false,
        manual_budget: None,
        effort: None,
    }
}

fn validate_manual_budget(budget: u32) -> Result<(), AgentError> {
    if !(MIN_MANUAL_BUDGET..=MAX_MANUAL_BUDGET).contains(&budget) {
        return Err(AgentError::Permanent(format!(
            "Claude manual thinking budget must be between {MIN_MANUAL_BUDGET} and {MAX_MANUAL_BUDGET} tokens (got {budget})"
        )));
    }
    Ok(())
}

fn merge_claude_params(request: &mut CompletionRequest, wire: WireThinking) {
    let mut map = request
        .additional_params
        .take()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();

    if wire.emit_disabled {
        map.insert("thinking".into(), json!({ "type": "disabled" }));
    } else if wire.adaptive {
        map.insert(
            "thinking".into(),
            json!({ "type": "adaptive", "display": "summarized" }),
        );
        if let Some(effort) = wire.effort {
            map.insert("output_config".into(), json!({ "effort": effort }));
        }
    } else if let Some(budget) = wire.manual_budget {
        map.insert(
            "thinking".into(),
            json!({ "type": "enabled", "budget_tokens": budget }),
        );
    }

    request.additional_params = Some(Value::Object(map));
}

fn apply_max_tokens_floor(request: &mut CompletionRequest, wire: WireThinking) {
    let floor = if wire.adaptive {
        ADAPTIVE_MIN_MAX_TOKENS
    } else if let Some(budget) = wire.manual_budget {
        u64::from(budget) + MANUAL_ANSWER_HEADROOM
    } else {
        return;
    };
    let current = request.max_tokens.unwrap_or(0);
    if current < floor {
        request.max_tokens = Some(floor);
    }
}

fn classify_mode(platform: ClaudePlatform, model: &str) -> CapabilityMode {
    let normalized = normalize_model_id(platform, model);
    if !normalized.contains("claude") {
        return CapabilityMode::NonClaude;
    }
    if contains_any(
        &normalized,
        &["fable", "mythos-5", "mythos-preview", "mythos_preview"],
    ) {
        return CapabilityMode::AdaptiveAlwaysOn;
    }
    if contains_any(&normalized, &["sonnet-5", "sonnet_5"]) {
        return CapabilityMode::AdaptiveDefaultOn;
    }
    if contains_any(
        &normalized,
        &["opus-4-8", "opus-4-7", "opus-4-6", "sonnet-4-6"],
    ) {
        return CapabilityMode::AdaptiveOptional;
    }
    if contains_any(&normalized, &["claude-3-7", "claude-3.7"]) {
        return CapabilityMode::Manual;
    }
    if contains_any(
        &normalized,
        &[
            "haiku-4-5",
            "sonnet-4-5",
            "opus-4-5",
            "sonnet-4-2025",
            "opus-4-2025",
            "claude-sonnet-4",
            "claude-opus-4",
        ],
    ) {
        return CapabilityMode::Manual;
    }
    CapabilityMode::Unsupported
}

fn normalize_model_id(platform: ClaudePlatform, model: &str) -> String {
    let mut id = model.trim().to_ascii_lowercase();
    for prefix in ["us.", "eu.", "apac.", "global."] {
        if let Some(rest) = id.strip_prefix(prefix) {
            id = rest.to_string();
        }
    }
    if matches!(platform, ClaudePlatform::Bedrock) {
        id = id.strip_prefix("anthropic.").unwrap_or(&id).to_string();
    }
    id
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{AgentRequest, NodeId, WorkflowId};
    use rig_core::OneOrMany;
    use rig_core::message::Message;
    use serde_json::json;

    fn empty_request() -> CompletionRequest {
        CompletionRequest {
            model: None,
            preamble: None,
            chat_history: OneOrMany::one(Message::user("ctx")),
            documents: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            tool_choice: None,
            additional_params: None,
            output_schema: None,
        }
    }

    fn minimal_agent(model: &str) -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId("wf".into()),
            node_id: NodeId("n1".into()),
            node_label: "Node".into(),
            model: model.into(),
            system_messages: vec!["sys".into()],
            task_prompt: "task".into(),
            input: json!({}),
            output_schema: json!({}),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript: Vec::new(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            turn_phase: engine::AgentTurnPhase::Control,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: false,
        }
    }

    #[test]
    fn adaptive_high_maps_thinking_and_effort() {
        let mut req = empty_request();
        let mut agent = minimal_agent("claude-sonnet-4-6");
        agent.reasoning_effort = Some("high".into());
        assert!(apply(ClaudePlatform::Anthropic, &mut req, &agent).is_ok());
        assert_eq!(
            req.additional_params
                .as_ref()
                .and_then(|params| params["thinking"]["type"].as_str()),
            Some("adaptive")
        );
        assert_eq!(
            req.additional_params
                .as_ref()
                .and_then(|params| params["thinking"]["display"].as_str()),
            Some("summarized")
        );
        assert_eq!(
            req.additional_params
                .as_ref()
                .and_then(|params| params["output_config"]["effort"].as_str()),
            Some("high")
        );
        assert_eq!(req.tool_choice, Some(ToolChoice::Auto));
    }

    #[test]
    fn manual_low_sets_budget_and_max_tokens() {
        let mut req = empty_request();
        let mut agent = minimal_agent("anthropic.claude-sonnet-4-20250514-v1:0");
        agent.reasoning_effort = Some("low".into());
        assert!(apply(ClaudePlatform::Bedrock, &mut req, &agent).is_ok());
        assert_eq!(
            req.additional_params
                .as_ref()
                .and_then(|params| params["thinking"]["type"].as_str()),
            Some("enabled")
        );
        assert_eq!(
            req.additional_params
                .as_ref()
                .and_then(|params| params["thinking"]["budget_tokens"].as_u64()),
            Some(10_240)
        );
        assert!(req.max_tokens.is_some_and(|tokens| tokens > 10_240));
    }

    #[test]
    fn unsupported_claude_strips_raw_keys() {
        let mut req = CompletionRequest {
            additional_params: Some(json!({
                "reasoning_effort": "high",
                "reasoning_budget_tokens": 1024
            })),
            chat_history: OneOrMany::one(Message::user("ctx")),
            ..empty_request()
        };
        let agent = minimal_agent("claude-3-5-sonnet-latest");
        assert!(apply(ClaudePlatform::Anthropic, &mut req, &agent).is_ok());
        assert!(req.additional_params.is_none());
        assert!(req.tool_choice.is_none());
    }

    #[test]
    fn sonnet_5_none_emits_disabled() {
        let mut req = empty_request();
        let mut agent = minimal_agent("claude-sonnet-5");
        agent.reasoning_effort = Some("none".into());
        assert!(apply(ClaudePlatform::Anthropic, &mut req, &agent).is_ok());
        assert_eq!(
            req.additional_params
                .as_ref()
                .and_then(|params| params["thinking"]["type"].as_str()),
            Some("disabled")
        );
        assert!(req.tool_choice.is_none());
    }

    #[test]
    fn invalid_manual_budget_errors() {
        let mut req = empty_request();
        let mut agent = minimal_agent("anthropic.claude-sonnet-4-20250514-v1:0");
        agent.reasoning_effort = Some("high".into());
        agent.reasoning_budget_tokens = Some(128_000);
        assert!(matches!(
            apply(ClaudePlatform::Bedrock, &mut req, &agent),
            Err(AgentError::Permanent(_))
        ));
    }
}
