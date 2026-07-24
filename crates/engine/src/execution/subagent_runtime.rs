//! Subagent builtin tool handling and nested AI turn state machine.

use crate::conversation::{
    filter_tool_turn_assistant_message, AgentReasoning, AgentTranscriptItem,
};
use crate::execution::interactive_engine::{
    AUTONOMOUS_CONTINUE_FEEDBACK, MAX_AUTO_CONTINUE_STREAK,
};
use crate::execution::node_invocation::merge_shared_context;
use crate::execution::subagents::CALL_SUBAGENT_TOOL;
use crate::execution::subagents::{
    adhoc_subagent_base_index, build_adhoc_subagent_summaries, merge_subagent_summaries,
};
use crate::execution::telemetry::RunTelemetry;
use crate::execution::tool_results::error_tool_result;
use crate::graph::callable_agent::CallableAgent;
use crate::graph::{
    default_structured_output_schema, effective_output_schema, Node, NodeId, Workflow,
};
use crate::ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentTurnOutcome, AgentTurnSuccess,
};
use crate::tools::{SubagentDeclaration, SubagentStatus, SubagentSummary, ToolCall, ToolResult};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const DECLARE_SUBAGENTS_TOOL: &str = "openflow_declare_subagents";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CallSubagentArgs {
    pub subagent_id: String,
    pub input: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SubagentDeclarationBatch {
    subagents: Vec<SubagentDeclaration>,
}

#[derive(Debug, Clone)]
pub struct DeclareSubagentsOutcome {
    pub summaries: Vec<SubagentSummary>,
    pub tool_result: ToolResult,
}

/// Handle `openflow_declare_subagents` without I/O.
#[must_use]
pub fn handle_declare_subagents(
    node_id: &NodeId,
    tool_call: &ToolCall,
    declared_subagents: &mut BTreeMap<String, SubagentSummary>,
) -> DeclareSubagentsOutcome {
    let declarations = match serde_json::from_value::<SubagentDeclarationBatch>(
        tool_call.arguments.clone(),
    ) {
        Ok(batch) => batch.subagents,
        Err(err) => {
            return DeclareSubagentsOutcome {
                    summaries: Vec::new(),
                    tool_result: error_tool_result(
                        tool_call,
                        format!(
                            "Invalid arguments for {DECLARE_SUBAGENTS_TOOL}: {err}. \
                             Expected {{\"subagents\": [{{\"name\": \"...\", \"purpose\": \"...\"}}]}}."
                        ),
                    ),
                };
        }
    };
    let base_index = adhoc_subagent_base_index(node_id, declared_subagents);
    let summaries = build_adhoc_subagent_summaries(node_id, &declarations, base_index);
    merge_subagent_summaries(declared_subagents, &summaries);
    let declared_json: Vec<Value> = summaries
        .iter()
        .map(|s| serde_json::to_value(s).unwrap_or_default())
        .collect();
    let result_content = serde_json::json!({
        "declared": declared_json,
        "message": "Subagents declared and ready for invocation."
    })
    .to_string();
    DeclareSubagentsOutcome {
        summaries,
        tool_result: ToolResult {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            content: result_content,
            is_error: false,
            artifact_ids: Vec::new(),
            output_meta: None,
        },
    }
}

#[derive(Debug, Clone)]
pub enum SubagentStartOutcome {
    Started(Box<SubagentInvokeSession>, Vec<RunTelemetry>),
    Failed(ToolResult),
}

#[derive(Debug, Clone)]
pub struct SubagentInvokeSession {
    pub subagent: SubagentSummary,
    pub request: AgentRequest,
    pub tool_call_id: String,
    pub parent_node_id: NodeId,
    /// Consecutive text-only turns without tool-call progress.
    text_turn_streak: u8,
}

#[derive(Debug, Clone)]
pub enum SubagentInvokeStep {
    NeedAi(SubagentInvokeSession),
    Done {
        tool_result: ToolResult,
        subagent: SubagentSummary,
        telemetry: Vec<RunTelemetry>,
    },
}

fn resolve_subagent_summary(
    subagent_id: &str,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
    agent_snapshots: &BTreeMap<String, CallableAgent>,
) -> Option<SubagentSummary> {
    declared_subagents.get(subagent_id).cloned().or_else(|| {
        agent_snapshots
            .get(subagent_id)
            .map(CallableAgent::to_subagent_summary)
    })
}

/// Begin a subagent invocation; returns an error tool result when the call cannot start.
pub fn start_subagent_invoke(
    workflow: &Workflow,
    parent_node_id: &NodeId,
    tool_call: &ToolCall,
    declared_subagents: &mut BTreeMap<String, SubagentSummary>,
    agent_snapshots: &BTreeMap<String, CallableAgent>,
    available_tools: Vec<crate::ToolDefinition>,
) -> SubagentStartOutcome {
    let call_args = match serde_json::from_value::<CallSubagentArgs>(tool_call.arguments.clone()) {
        Ok(args) => args,
        Err(err) => {
            return SubagentStartOutcome::Failed(error_tool_result(
                tool_call,
                format!("Invalid arguments for {CALL_SUBAGENT_TOOL}: {err}"),
            ));
        }
    };

    let Some(mut subagent) =
        resolve_subagent_summary(&call_args.subagent_id, declared_subagents, agent_snapshots)
    else {
        return SubagentStartOutcome::Failed(error_tool_result(
            tool_call,
            format!(
                "Subagent '{}' not found. Declare subagents before invoking them.",
                call_args.subagent_id
            ),
        ));
    };

    if subagent.status != SubagentStatus::Declared && subagent.status != SubagentStatus::Completed {
        return SubagentStartOutcome::Failed(error_tool_result(
            tool_call,
            format!(
                "Subagent '{}' is {} and cannot be invoked. Only declared or completed subagents can be called.",
                call_args.subagent_id,
                serde_json::to_value(&subagent.status).unwrap_or_default()
            ),
        ));
    }

    let Some(parent_node) = workflow.nodes.iter().find(|n| n.id == *parent_node_id) else {
        return SubagentStartOutcome::Failed(error_tool_result(
            tool_call,
            format!("Parent node '{parent_node_id}' not found in workflow"),
        ));
    };

    subagent.status = SubagentStatus::Active;
    declared_subagents.insert(subagent.id.clone(), subagent.clone());

    let mut sub_request = if let Some(agent_def) = agent_snapshots.get(&call_args.subagent_id) {
        build_saved_agent_request(
            workflow,
            agent_def,
            &subagent,
            &call_args.input,
            available_tools,
        )
    } else {
        build_adhoc_agent_request(
            workflow,
            parent_node,
            &subagent,
            &call_args.input,
            available_tools,
        )
    };
    if sub_request.model.trim().is_empty() {
        sub_request.model.clone_from(&parent_node.agent.model);
    }

    let telemetry = vec![RunTelemetry::SubagentStarted {
        node_id: parent_node_id.clone(),
        subagent_id: subagent.id.clone(),
    }];

    SubagentStartOutcome::Started(
        Box::new(SubagentInvokeSession {
            subagent,
            request: sub_request,
            tool_call_id: tool_call.id.clone(),
            parent_node_id: parent_node_id.clone(),
            text_turn_streak: 0,
        }),
        telemetry,
    )
}

fn build_saved_agent_request(
    workflow: &Workflow,
    agent: &CallableAgent,
    subagent: &SubagentSummary,
    input: &str,
    available_tools: Vec<crate::ToolDefinition>,
) -> AgentRequest {
    let sub_node_config = agent.tools.clone();
    let system_prompt = merge_shared_context(workflow, &agent.system_prompt);
    let sub_transcript = vec![AgentTranscriptItem::UserMessage {
        content: format!(
            "You are the saved agent \"{}\".\n\nTask: {input}",
            agent.name
        ),
    }];
    AgentRequest {
        workflow_id: workflow.id.clone(),
        node_id: NodeId(subagent.id.clone()),
        node_label: subagent.name.clone(),
        model: agent.model.clone(),
        system_messages: vec![system_prompt],
        task_prompt: input.to_string(),
        input: Value::Null,
        output_schema: effective_output_schema(&agent.output_schema),
        tool_config: sub_node_config,
        available_tools,
        transcript: sub_transcript,
        model_attempt: 1,
        reasoning_effort: None,
        reasoning_budget_tokens: None,
        tool_access_policy: crate::ports::ToolAccessPolicy::Execution,
        allow_user_input: false,
    }
}

fn build_adhoc_agent_request(
    workflow: &Workflow,
    parent_node: &Node,
    subagent: &SubagentSummary,
    input: &str,
    available_tools: Vec<crate::ToolDefinition>,
) -> AgentRequest {
    let sub_node_config = parent_node.agent.tools.clone();
    let sub_transcript = vec![AgentTranscriptItem::UserMessage {
        content: format!(
            "You are a subagent named \"{}\" with the purpose: \"{}\"\n\nTask: {input}",
            subagent.name, subagent.purpose
        ),
    }];
    let system_prompt = merge_shared_context(
        workflow,
        &format!("You are {}. {}", subagent.name, subagent.purpose),
    );
    AgentRequest {
        workflow_id: workflow.id.clone(),
        node_id: NodeId(subagent.id.clone()),
        node_label: subagent.name.clone(),
        model: parent_node.agent.model.clone(),
        system_messages: vec![system_prompt],
        task_prompt: input.to_string(),
        input: Value::Null,
        output_schema: default_structured_output_schema(),
        tool_config: sub_node_config,
        available_tools,
        transcript: sub_transcript,
        model_attempt: 1,
        reasoning_effort: None,
        reasoning_budget_tokens: None,
        tool_access_policy: crate::ports::ToolAccessPolicy::Execution,
        allow_user_input: false,
    }
}

/// Advance a subagent session after the host invokes the model.
#[must_use]
pub fn advance_subagent_invoke(
    mut session: SubagentInvokeSession,
    outcome: Result<AgentTurnOutcome, AgentError>,
    tool_results: Vec<ToolResult>,
) -> SubagentInvokeStep {
    match outcome {
        Ok(AgentTurnOutcome::Completed(success)) => complete_subagent(session, success),
        Ok(AgentTurnOutcome::ToolCalls(batch)) => {
            session.text_turn_streak = 0;
            let mut transcript = session.request.transcript.clone();
            for reasoning in &batch.reasoning {
                transcript.push(AgentTranscriptItem::Reasoning {
                    reasoning: reasoning.clone(),
                });
            }
            if let Some(message) = filter_tool_turn_assistant_message(batch.assistant_message)
                .filter(|message| !message.trim().is_empty())
            {
                transcript.push(AgentTranscriptItem::AssistantMessage { content: message });
            }
            for call in &batch.tool_calls {
                transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
            }
            for result in tool_results {
                transcript.push(AgentTranscriptItem::ToolResult { result });
            }
            session.request.transcript = transcript;
            SubagentInvokeStep::NeedAi(session)
        }
        Ok(AgentTurnOutcome::Message(message)) => {
            continue_autonomous_subagent(session, &message.assistant_message, message.reasoning)
        }
        Ok(AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            assistant_message,
            reasoning,
            ..
        })) => continue_autonomous_subagent(session, &assistant_message, reasoning),
        Err(err) => {
            let name = session.subagent.name.clone();
            complete_subagent_failed(session, format!("Subagent '{name}' failed: {err}"))
        }
    }
}

fn continue_autonomous_subagent(
    mut session: SubagentInvokeSession,
    assistant_message: &str,
    reasoning: Vec<AgentReasoning>,
) -> SubagentInvokeStep {
    if session.text_turn_streak >= MAX_AUTO_CONTINUE_STREAK {
        let name = session.subagent.name.clone();
        return complete_subagent_failed(
            session,
            format!(
                "Subagent '{name}' produced {MAX_AUTO_CONTINUE_STREAK} consecutive turns without a tool call"
            ),
        );
    }

    for reasoning in reasoning {
        session
            .request
            .transcript
            .push(AgentTranscriptItem::Reasoning { reasoning });
    }
    let assistant_message = assistant_message.trim();
    if !assistant_message.is_empty() {
        session
            .request
            .transcript
            .push(AgentTranscriptItem::AssistantMessage {
                content: assistant_message.to_string(),
            });
    }
    session
        .request
        .transcript
        .push(AgentTranscriptItem::UserMessage {
            content: AUTONOMOUS_CONTINUE_FEEDBACK.to_string(),
        });
    session.text_turn_streak += 1;
    session.request.model_attempt = session.request.model_attempt.saturating_add(1);
    SubagentInvokeStep::NeedAi(session)
}

fn complete_subagent(
    session: SubagentInvokeSession,
    AgentTurnSuccess { output, .. }: AgentTurnSuccess,
) -> SubagentInvokeStep {
    let mut subagent = session.subagent;
    subagent.status = SubagentStatus::Completed;
    let subagent_id = subagent.id.clone();
    let content = serde_json::json!({
        "output": output,
        "message": format!("Subagent '{}' completed.", subagent.name)
    })
    .to_string();
    SubagentInvokeStep::Done {
        tool_result: ToolResult {
            tool_call_id: session.tool_call_id,
            tool_name: CALL_SUBAGENT_TOOL.to_string(),
            content,
            is_error: false,
            artifact_ids: Vec::new(),
            output_meta: None,
        },
        subagent,
        telemetry: vec![RunTelemetry::SubagentCompleted {
            node_id: session.parent_node_id,
            subagent_id,
        }],
    }
}

fn complete_subagent_failed(session: SubagentInvokeSession, error: String) -> SubagentInvokeStep {
    let mut subagent = session.subagent;
    subagent.status = SubagentStatus::Failed;
    let subagent_id = subagent.id.clone();
    SubagentInvokeStep::Done {
        tool_result: ToolResult {
            tool_call_id: session.tool_call_id,
            tool_name: CALL_SUBAGENT_TOOL.to_string(),
            content: serde_json::json!({ "error": &error }).to_string(),
            is_error: true,
            artifact_ids: Vec::new(),
            output_meta: None,
        },
        subagent,
        telemetry: vec![RunTelemetry::SubagentFailed {
            node_id: session.parent_node_id,
            subagent_id,
            error,
        }],
    }
}

/// Returns a tool result for runtime builtins that subagents cannot invoke.
#[must_use]
pub fn subagent_runtime_builtin_denied(tool_call: &ToolCall) -> ToolResult {
    error_tool_result(tool_call, "Subagent cannot invoke runtime builtin tools.")
}

/// Whether a tool name is a subagent runtime builtin.
#[must_use]
pub fn is_subagent_runtime_builtin(tool_name: &str) -> bool {
    tool_name == DECLARE_SUBAGENTS_TOOL || tool_name == CALL_SUBAGENT_TOOL
}

#[cfg(test)]
#[allow(
    clippy::panic,
    clippy::unwrap_used,
    reason = "test fixtures use unwrap/panic for brevity"
)]
mod tests {
    use super::*;
    use crate::execution::subagents::CALL_SUBAGENT_TOOL;
    use crate::graph::{NodeId, WorkflowId};
    use crate::ports::AgentMessageTurn;
    use crate::ports::{AgentRequest, AgentToolCallBatch, AgentTurnOutcome};
    use crate::tools::{NodeToolConfig, SubagentStatus, ToolCall};
    use serde_json::json;

    fn sample_session() -> SubagentInvokeSession {
        SubagentInvokeSession {
            subagent: SubagentSummary {
                id: "sub-1".to_string(),
                name: "Researcher".to_string(),
                purpose: "Find facts".to_string(),
                status: SubagentStatus::Active,
            },
            request: AgentRequest {
                workflow_id: WorkflowId("wf-1".to_string()),
                node_id: NodeId("sub-1".to_string()),
                node_label: "Researcher".to_string(),
                model: "test-model".to_string(),
                system_messages: vec!["system".to_string()],
                task_prompt: "task".to_string(),
                input: json!(null),
                output_schema: json!(null),
                tool_config: NodeToolConfig::default(),
                available_tools: Vec::new(),
                transcript: vec![AgentTranscriptItem::UserMessage {
                    content: "Do work".to_string(),
                }],
                model_attempt: 1,
                reasoning_effort: None,
                reasoning_budget_tokens: None,
                tool_access_policy: crate::ports::ToolAccessPolicy::Execution,
                allow_user_input: false,
            },
            tool_call_id: "parent-call".to_string(),
            parent_node_id: NodeId("node-1".to_string()),
            text_turn_streak: 0,
        }
    }

    #[test]
    fn adhoc_subagent_request_uses_default_output_schema() {
        let mut workflow = Workflow::new("Test");
        let mut node = crate::Node::agent("Parent", 0.0, 0.0);
        node.id = NodeId("parent".to_string());
        workflow.nodes.push(node);

        let mut declared = std::collections::BTreeMap::new();
        declared.insert(
            "sub-1".to_string(),
            SubagentSummary {
                id: "sub-1".to_string(),
                name: "backend-impl".to_string(),
                purpose: "Implement backend".to_string(),
                status: SubagentStatus::Declared,
            },
        );

        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: CALL_SUBAGENT_TOOL.to_string(),
            arguments: json!({
                "subagent_id": "sub-1",
                "input": "wire the API"
            }),
        };

        match start_subagent_invoke(
            &workflow,
            &NodeId("parent".to_string()),
            &tool_call,
            &mut declared,
            &std::collections::BTreeMap::new(),
            Vec::new(),
        ) {
            SubagentStartOutcome::Started(session, _) => {
                assert_ne!(session.request.output_schema, Value::Null);
                assert_eq!(session.request.output_schema["type"], "object");
            }
            SubagentStartOutcome::Failed(_) => panic!("expected subagent start"),
        }
    }

    #[test]
    fn failed_start_does_not_mark_subagent_active() {
        let workflow = Workflow::new("Test");
        let mut declared = std::collections::BTreeMap::new();
        declared.insert(
            "sub-1".to_string(),
            SubagentSummary {
                id: "sub-1".to_string(),
                name: "Researcher".to_string(),
                purpose: "Find facts".to_string(),
                status: SubagentStatus::Declared,
            },
        );
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: CALL_SUBAGENT_TOOL.to_string(),
            arguments: json!({ "subagent_id": "sub-1", "input": "go" }),
        };

        match start_subagent_invoke(
            &workflow,
            &NodeId("missing-parent".to_string()),
            &tool_call,
            &mut declared,
            &std::collections::BTreeMap::new(),
            Vec::new(),
        ) {
            SubagentStartOutcome::Failed(result) => assert!(result.is_error),
            SubagentStartOutcome::Started(..) => panic!("expected failure"),
        }
        assert_eq!(declared["sub-1"].status, SubagentStatus::Declared);
    }

    #[test]
    fn declare_subagents_rejects_malformed_arguments() {
        let mut declared = std::collections::BTreeMap::new();
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: DECLARE_SUBAGENTS_TOOL.to_string(),
            arguments: json!({ "subagents": "not-an-array" }),
        };

        let outcome =
            handle_declare_subagents(&NodeId("n1".to_string()), &tool_call, &mut declared);

        assert!(outcome.tool_result.is_error);
        assert!(outcome.summaries.is_empty());
        assert!(declared.is_empty());
    }

    #[test]
    fn saved_agent_without_model_inherits_parent_model() {
        let mut workflow = Workflow::new("Test");
        let mut node = crate::Node::agent("Parent", 0.0, 0.0);
        node.id = NodeId("parent".to_string());
        node.agent.model = "parent-model".to_string();
        workflow.nodes.push(node);

        let mut agent = CallableAgent::new("Helper");
        agent.id = "agent-1".to_string();
        let mut snapshots = std::collections::BTreeMap::new();
        snapshots.insert("agent-1".to_string(), agent);

        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: CALL_SUBAGENT_TOOL.to_string(),
            arguments: json!({ "subagent_id": "agent-1", "input": "go" }),
        };

        match start_subagent_invoke(
            &workflow,
            &NodeId("parent".to_string()),
            &tool_call,
            &mut std::collections::BTreeMap::new(),
            &snapshots,
            Vec::new(),
        ) {
            SubagentStartOutcome::Started(session, _) => {
                assert_eq!(session.request.model, "parent-model");
            }
            SubagentStartOutcome::Failed(result) => {
                panic!("unexpected failure: {}", result.content);
            }
        }
    }

    #[test]
    fn advance_subagent_invoke_records_tool_calls_before_results() {
        let session = sample_session();
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "notes.txt" }),
        };
        let tool_result = ToolResult {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            content: "file contents".to_string(),
            is_error: false,
            artifact_ids: Vec::new(),
            output_meta: None,
        };
        let outcome = Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            raw_text: String::new(),
            assistant_message: Some("Reading notes".to_string()),
            tool_calls: vec![tool_call.clone()],
            reasoning: vec![],
            usage: None,
        }));

        match advance_subagent_invoke(session, outcome, vec![tool_result.clone()]) {
            SubagentInvokeStep::NeedAi(next) => assert_eq!(
                next.request.transcript,
                vec![
                    AgentTranscriptItem::UserMessage {
                        content: "Do work".to_string(),
                    },
                    AgentTranscriptItem::AssistantMessage {
                        content: "Reading notes".to_string(),
                    },
                    AgentTranscriptItem::ToolCall { call: tool_call },
                    AgentTranscriptItem::ToolResult {
                        result: tool_result
                    },
                ]
            ),
            SubagentInvokeStep::Done { .. } => unreachable!("unexpected Done step"),
        }
    }

    #[test]
    fn text_only_subagent_turn_is_repaired_with_a_bounded_nudge() {
        let session = sample_session();
        let outcome = Ok(AgentTurnOutcome::Message(AgentMessageTurn {
            raw_text: "I will inspect the files.".to_string(),
            assistant_message: "I will inspect the files.".to_string(),
            reasoning: Vec::new(),
            usage: None,
        }));

        match advance_subagent_invoke(session, outcome, Vec::new()) {
            SubagentInvokeStep::NeedAi(next) => {
                assert_eq!(next.text_turn_streak, 1);
                assert_eq!(next.request.model_attempt, 2);
                assert!(matches!(
                    next.request.transcript.last(),
                    Some(AgentTranscriptItem::UserMessage { content })
                        if content.contains("No human input is available")
                ));
            }
            SubagentInvokeStep::Done { .. } => unreachable!("unexpected Done step"),
        }
    }
}
