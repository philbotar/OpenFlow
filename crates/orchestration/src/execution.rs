#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::missing_panics_doc,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::redundant_clone,
    clippy::significant_drop_tightening,
    clippy::too_many_lines
)]

use crate::state::{
    AgentStatus, RunTraceEntry, ToolArtifactSummary, ToolCallSummary, TraceStatus, WorkflowRunState,
};
use crate::tools::{
    resolve_tool_policy, ApprovalDecision, ArtifactStore, ToolApprovalRequest, ToolRegistry,
    ToolRunner, ToolRunnerError,
};
use domain::{
    filter_tool_turn_assistant_message, AgentNeedUserInput, AgentRequest, AgentToolCallBatch,
    AgentTranscriptItem, AgentTurnOutcome, AiPort, ChatMessage, ChatRole, EnginePollResult,
    InteractiveEngine, NodeId, RunReport, SubagentDeclaration, SubagentStatus, SubagentSummary,
    ToolCall, ToolCallStatus, ToolOutputMeta, Workflow,
};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct SubagentDeclarationBatch {
    subagents: Vec<SubagentDeclaration>,
}

#[derive(serde::Deserialize)]
struct CallSubagentArgs {
    subagent_id: String,
    input: String,
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    NodeQueued {
        node_id: NodeId,
        label: String,
    },
    NodeStarted {
        node_id: NodeId,
        label: String,
    },
    ChatMessage {
        node_id: NodeId,
        role: ChatRole,
        content: String,
    },
    NodeAwaitingInput {
        node_id: NodeId,
        label: String,
        context: String,
        is_initial: bool,
    },
    ToolCallProposed {
        node_id: NodeId,
        label: String,
        tool_call: ToolCall,
    },
    ToolApprovalRequested {
        request: domain::PendingToolApproval,
    },
    ToolApproved {
        approval_id: String,
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
    },
    ToolDenied {
        approval_id: String,
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        reason: String,
    },
    ToolStarted {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        arguments: Value,
    },
    ToolCompleted {
        node_id: NodeId,
        tool_call_id: String,
        tool_name: String,
        content: String,
        is_error: bool,
        output_meta: Option<ToolOutputMeta>,
        artifact_ids: Vec<String>,
    },
    ToolArtifactCreated {
        node_id: NodeId,
        artifact: ToolArtifactSummary,
    },
    NodeCompleted {
        node_id: NodeId,
        label: String,
        output: Value,
    },
    NodeFailed {
        node_id: NodeId,
        label: String,
        error: String,
    },
    Finished(RunReport),
    Error(String),
    SubagentsDeclared {
        node_id: NodeId,
        declarations: Vec<SubagentDeclaration>,
    },
    SubagentStarted {
        node_id: NodeId,
        subagent_id: String,
    },
    SubagentCompleted {
        node_id: NodeId,
        subagent_id: String,
    },
    SubagentFailed {
        node_id: NodeId,
        subagent_id: String,
        error: String,
    },
}

pub enum ExecutionAction {
    ProvideInput(String),
    ResolveApproval { approval_id: String, allow: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualInput {
    pub node_id: NodeId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalResponse {
    pub approval_id: String,
    pub allow: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunSnapshot {
    pub report: RunReport,
    pub run_trace: Vec<RunTraceEntry>,
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub outputs: BTreeMap<NodeId, Value>,
    pub pending_approvals: Vec<domain::PendingToolApproval>,
    pub tool_calls_by_node: BTreeMap<NodeId, Vec<ToolCallSummary>>,
    pub tool_artifacts: BTreeMap<String, ToolArtifactSummary>,
}

#[derive(Debug, Error)]
pub enum WorkflowExecutionError {
    #[error("{0}")]
    Execution(String),
    #[error("node {node_id} failed: {message}")]
    NodeFailed { node_id: NodeId, message: String },
    #[error("node {0} requested manual input but no scripted input was provided")]
    MissingManualInput(NodeId),
    #[error("tool approval {0} was requested but no scripted approval was provided")]
    MissingApproval(String),
}

pub fn spawn_interactive_workflow_run<A>(
    runtime: &tokio::runtime::Runtime,
    workflow: Workflow,
    entrypoint: Option<String>,
    ai: A,
) -> (
    tokio::task::JoinHandle<()>,
    UnboundedReceiver<ExecutionEvent>,
    UnboundedSender<ExecutionAction>,
)
where
    A: AiPort + Send + Sync + 'static,
{
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = runtime.spawn(async move {
        drive_interactive_workflow(workflow, entrypoint, ai, event_tx, action_rx).await;
    });
    (handle, event_rx, action_tx)
}

async fn drive_interactive_workflow<A>(
    workflow: Workflow,
    entrypoint: Option<String>,
    ai: A,
    event_tx: UnboundedSender<ExecutionEvent>,
    mut action_rx: UnboundedReceiver<ExecutionAction>,
) where
    A: AiPort,
{
    let mut engine = match InteractiveEngine::new(workflow.clone(), entrypoint) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
            return;
        }
    };

    let tool_registry = ToolRegistry::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let artifact_root = std::env::temp_dir().join(format!("openflow-run-{}", Uuid::new_v4()));
    let artifacts = match ArtifactStore::new(artifact_root) {
        Ok(store) => store,
        Err(error) => {
            let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
            return;
        }
    };
    let tool_runner = ToolRunner::new(tool_registry, cwd, artifacts);
    let mut exec_approval_granted = false;
    let mut declared_subagents: BTreeMap<String, SubagentSummary> = BTreeMap::new();

    loop {
        match engine.poll() {
            EnginePollResult::CallAi {
                node_id,
                mut request,
            } => {
                request.available_tools =
                    tool_runner.registry().definitions_for(&request.tool_config);
                send_node_start_events(&event_tx, &request);
                let result = ai.invoke((*request).clone()).await;
                if let Ok(outcome) = &result {
                    emit_assistant_message(&event_tx, &node_id, outcome);
                }
                let invoke_error = result.as_ref().err().map(ToString::to_string);
                let label = request.node_label.clone();
                engine.on_ai_complete(&node_id, result);
                if let Some(output) = engine.node_output(&node_id) {
                    let _ = event_tx.send(ExecutionEvent::NodeCompleted {
                        node_id: NodeId(node_id.to_string()),
                        label,
                        output,
                    });
                } else if let Some(error) = invoke_error {
                    let _ = event_tx.send(ExecutionEvent::NodeFailed {
                        node_id: NodeId(node_id.to_string()),
                        label,
                        error,
                    });
                    return;
                }
            }
            EnginePollResult::AwaitInput {
                node_id,
                label,
                context,
                is_initial,
            } => {
                let awaiting_node_id = node_id.clone();
                if is_initial {
                    let _ = event_tx.send(ExecutionEvent::NodeQueued {
                        node_id: node_id.clone(),
                        label: label.clone(),
                    });
                }
                let _ = event_tx.send(ExecutionEvent::NodeAwaitingInput {
                    node_id,
                    label,
                    context,
                    is_initial,
                });
                loop {
                    match action_rx.recv().await {
                        Some(ExecutionAction::ProvideInput(text)) => {
                            if let Err(error) = engine.on_human_input(&awaiting_node_id, &text) {
                                let _ = event_tx.send(ExecutionEvent::Error(error));
                                return;
                            }
                            break;
                        }
                        Some(ExecutionAction::ResolveApproval { .. }) => continue,
                        None => return,
                    }
                }
            }
            EnginePollResult::AwaitToolApproval {
                node_id,
                label,
                tool_calls,
            } => {
                let node_config = workflow
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .map(|node| node.agent.tools.clone())
                    .unwrap_or_default();
                let mut results = Vec::new();
                for tool_call in tool_calls {
                    // Handle runtime builtin: openflow_declare_subagents
                    if tool_call.name == "openflow_declare_subagents" {
                        let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                            node_id: node_id.clone(),
                            label: label.clone(),
                            tool_call: tool_call.clone(),
                        });
                        let declarations =
                            match serde_json::value::from_value::<SubagentDeclarationBatch>(
                                tool_call.arguments.clone(),
                            ) {
                                Ok(batch) => batch.subagents,
                                Err(_) => Vec::new(),
                            };
                        let summaries: Vec<SubagentSummary> = declarations
                            .iter()
                            .enumerate()
                            .map(|(i, dec)| SubagentSummary {
                                id: format!("{}-subagent-{}", node_id, i + 1),
                                name: dec.name.clone(),
                                purpose: dec.purpose.clone(),
                                status: SubagentStatus::Declared,
                            })
                            .collect();
                        for summary in &summaries {
                            declared_subagents.insert(summary.id.clone(), summary.clone());
                        }
                        let _ = event_tx.send(ExecutionEvent::SubagentsDeclared {
                            node_id: node_id.clone(),
                            declarations,
                        });
                        let declared_json: Vec<Value> = summaries
                            .iter()
                            .map(|s| serde_json::to_value(s).unwrap_or_default())
                            .collect();
                        let result_content = serde_json::json!({
                            "declared": declared_json,
                            "message": "Subagents declared and ready for invocation."
                        })
                        .to_string();
                        let tool_result = domain::ToolResult {
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            content: result_content.clone(),
                            is_error: false,
                            artifact_ids: Vec::new(),
                            output_meta: None,
                        };
                        let _ = event_tx.send(ExecutionEvent::ToolStarted {
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            arguments: tool_call.arguments.clone(),
                        });
                        let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            content: result_content.clone(),
                            is_error: false,
                            output_meta: None,
                            artifact_ids: Vec::new(),
                        });
                        results.push(tool_result);
                        continue;
                    }
                    // Handle runtime builtin: openflow_call_subagent
                    if tool_call.name == "openflow_call_subagent" {
                        let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                            node_id: node_id.clone(),
                            label: label.clone(),
                            tool_call: tool_call.clone(),
                        });
                        let call_args = match serde_json::value::from_value::<CallSubagentArgs>(
                            tool_call.arguments.clone(),
                        ) {
                            Ok(args) => args,
                            Err(err) => {
                                let result_content = serde_json::json!({
                                    "error": format!("Invalid arguments for openflow_call_subagent: {err}")
                                }).to_string();
                                let tool_result = domain::ToolResult {
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    content: result_content.clone(),
                                    is_error: true,
                                    artifact_ids: Vec::new(),
                                    output_meta: None,
                                };
                                let _ = event_tx.send(ExecutionEvent::ToolStarted {
                                    node_id: node_id.clone(),
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    arguments: tool_call.arguments.clone(),
                                });
                                let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                    node_id: node_id.clone(),
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    content: result_content,
                                    is_error: true,
                                    output_meta: None,
                                    artifact_ids: Vec::new(),
                                });
                                results.push(tool_result);
                                continue;
                            }
                        };
                        let subagent = match declared_subagents.get(&call_args.subagent_id) {
                            Some(s) => s.clone(),
                            None => {
                                let result_content = serde_json::json!({
                                    "error": format!("Subagent '{}' not found. Declare subagents before invoking them.", call_args.subagent_id)
                                }).to_string();
                                let tool_result = domain::ToolResult {
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    content: result_content.clone(),
                                    is_error: true,
                                    artifact_ids: Vec::new(),
                                    output_meta: None,
                                };
                                let _ = event_tx.send(ExecutionEvent::ToolStarted {
                                    node_id: node_id.clone(),
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    arguments: tool_call.arguments.clone(),
                                });
                                let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                    node_id: node_id.clone(),
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    content: result_content,
                                    is_error: true,
                                    output_meta: None,
                                    artifact_ids: Vec::new(),
                                });
                                results.push(tool_result);
                                continue;
                            }
                        };
                        if subagent.status != SubagentStatus::Declared
                            && subagent.status != SubagentStatus::Completed
                        {
                            let result_content = serde_json::json!({
                                "error": format!("Subagent '{}' is {} and cannot be invoked. Only declared or completed subagents can be called.", call_args.subagent_id, serde_json::to_value(&subagent.status).unwrap_or_default())
                            }).to_string();
                            let tool_result = domain::ToolResult {
                                tool_call_id: tool_call.id.clone(),
                                tool_name: tool_call.name.clone(),
                                content: result_content.clone(),
                                is_error: true,
                                artifact_ids: Vec::new(),
                                output_meta: None,
                            };
                            let _ = event_tx.send(ExecutionEvent::ToolStarted {
                                node_id: node_id.clone(),
                                tool_call_id: tool_call.id.clone(),
                                tool_name: tool_call.name.clone(),
                                arguments: tool_call.arguments.clone(),
                            });
                            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                node_id: node_id.clone(),
                                tool_call_id: tool_call.id.clone(),
                                tool_name: tool_call.name.clone(),
                                content: result_content,
                                is_error: true,
                                output_meta: None,
                                artifact_ids: Vec::new(),
                            });
                            results.push(tool_result);
                            continue;
                        }
                        // Transition subagent to Active
                        declared_subagents.insert(
                            subagent.id.clone(),
                            SubagentSummary {
                                status: SubagentStatus::Active,
                                ..subagent.clone()
                            },
                        );
                        let _ = event_tx.send(ExecutionEvent::SubagentStarted {
                            node_id: node_id.clone(),
                            subagent_id: subagent.id.clone(),
                        });
                        let _ = event_tx.send(ExecutionEvent::ToolStarted {
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            arguments: tool_call.arguments.clone(),
                        });
                        // Build subagent request from node data
                        let parent_node = workflow
                            .nodes
                            .iter()
                            .find(|n| n.id == node_id)
                            .expect("node exists in workflow");
                        let sub_node_config = node_config.clone();
                        let sub_available_tools = tool_runner
                            .registry()
                            .definitions_for_subagent(&sub_node_config);
                        let sub_transcript = vec![AgentTranscriptItem::UserMessage {
                            content: format!(
                                "You are a subagent named \"{}\" with the purpose: \"{}\"\n\nTask: {}",
                                subagent.name, subagent.purpose, call_args.input
                            ),
                        }];
                        let sub_request = AgentRequest {
                            workflow_id: workflow.id.clone(),
                            node_id: NodeId(subagent.id.clone()),
                            node_label: subagent.name.clone(),
                            model: parent_node.agent.model.clone(),
                            system_prompt: format!(
                                "You are {}. {}",
                                subagent.name, subagent.purpose
                            ),
                            task_prompt: call_args.input.clone(),
                            input: serde_json::json!(null),
                            output_schema: Value::Null,
                            tool_config: sub_node_config.clone(),
                            available_tools: sub_available_tools,
                            transcript: sub_transcript,
                        };
                        // Execute subagent in a mini-loop
                        let max_rounds = sub_node_config.max_tool_rounds;
                        let mut sub_transcript = sub_request.transcript.clone();
                        let mut sub_outcome = ai.invoke(sub_request.clone()).await;
                        let mut sub_round = 0u8;
                        let sub_result_content = loop {
                            match sub_outcome {
                                Ok(AgentTurnOutcome::Completed(success)) => {
                                    declared_subagents.insert(
                                        subagent.id.clone(),
                                        SubagentSummary {
                                            status: SubagentStatus::Completed,
                                            ..subagent.clone()
                                        },
                                    );
                                    let _ = event_tx.send(ExecutionEvent::SubagentCompleted {
                                        node_id: node_id.clone(),
                                        subagent_id: subagent.id.clone(),
                                    });
                                    break serde_json::json!({
                                        "output": success.output,
                                        "message": format!("Subagent '{}' completed.", subagent.name)
                                    }).to_string();
                                }
                                Ok(AgentTurnOutcome::ToolCalls(batch)) => {
                                    if sub_round >= max_rounds {
                                        declared_subagents.insert(
                                            subagent.id.clone(),
                                            SubagentSummary {
                                                status: SubagentStatus::Failed,
                                                ..subagent.clone()
                                            },
                                        );
                                        let _ = event_tx.send(ExecutionEvent::SubagentFailed {
                                            node_id: node_id.clone(),
                                            subagent_id: subagent.id.clone(),
                                            error: "Max tool rounds exceeded".to_string(),
                                        });
                                        break serde_json::json!({
                                            "error": format!("Subagent '{}' exceeded maximum tool rounds ({})", subagent.name, max_rounds)
                                        }).to_string();
                                    }
                                    sub_round += 1;
                                    if let Some(msg) = &batch.assistant_message {
                                        sub_transcript.push(
                                            AgentTranscriptItem::AssistantMessage {
                                                content: msg.clone(),
                                            },
                                        );
                                    }
                                    for tc in &batch.tool_calls {
                                        let is_runtime_builtin = tc.name
                                            == "openflow_declare_subagents"
                                            || tc.name == "openflow_call_subagent";
                                        if is_runtime_builtin {
                                            sub_transcript.push(AgentTranscriptItem::ToolResult {
                                                result: domain::ToolResult {
                                                    tool_call_id: tc.id.clone(),
                                                    tool_name: tc.name.clone(),
                                                    content: serde_json::json!({
                                                        "error": "Subagent cannot invoke runtime builtin tools."
                                                    }).to_string(),
                                                    is_error: true,
                                                    artifact_ids: Vec::new(),
                                                    output_meta: None,
                                                },
                                            });
                                            continue;
                                        }
                                        // Auto-approve and execute
                                        match tool_runner.execute(tc.clone()).await {
                                            Ok(record) => {
                                                sub_transcript.push(
                                                    AgentTranscriptItem::ToolResult {
                                                        result: record.result.clone(),
                                                    },
                                                );
                                            }
                                            Err(err) => {
                                                let error_content = err.to_string();
                                                sub_transcript.push(
                                                    AgentTranscriptItem::ToolResult {
                                                        result: domain::ToolResult {
                                                            tool_call_id: tc.id.clone(),
                                                            tool_name: tc.name.clone(),
                                                            content: error_content,
                                                            is_error: true,
                                                            artifact_ids: Vec::new(),
                                                            output_meta: None,
                                                        },
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    let next_request = AgentRequest {
                                        transcript: sub_transcript.clone(),
                                        ..sub_request.clone()
                                    };
                                    sub_outcome = ai.invoke(next_request).await;
                                    continue;
                                }
                                Ok(AgentTurnOutcome::NeedsUserInput(_)) => {
                                    declared_subagents.insert(
                                        subagent.id.clone(),
                                        SubagentSummary {
                                            status: SubagentStatus::Failed,
                                            ..subagent.clone()
                                        },
                                    );
                                    let _ = event_tx.send(ExecutionEvent::SubagentFailed {
                                        node_id: node_id.clone(),
                                        subagent_id: subagent.id.clone(),
                                        error: "Subagent requires user input (not supported)"
                                            .to_string(),
                                    });
                                    break serde_json::json!({
                                        "error": format!("Subagent '{}' requires user input, which is not supported in subagent context.", subagent.name)
                                    }).to_string();
                                }
                                Err(err) => {
                                    declared_subagents.insert(
                                        subagent.id.clone(),
                                        SubagentSummary {
                                            status: SubagentStatus::Failed,
                                            ..subagent.clone()
                                        },
                                    );
                                    let _ = event_tx.send(ExecutionEvent::SubagentFailed {
                                        node_id: node_id.clone(),
                                        subagent_id: subagent.id.clone(),
                                        error: err.to_string(),
                                    });
                                    break serde_json::json!({
                                        "error": format!("Subagent '{}' failed: {}", subagent.name, err)
                                    }).to_string();
                                }
                            }
                        };
                        let is_subagent_error = sub_result_content.contains("\"error\"");
                        let tool_result = domain::ToolResult {
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            content: sub_result_content.clone(),
                            is_error: is_subagent_error,
                            artifact_ids: Vec::new(),
                            output_meta: None,
                        };
                        let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            content: sub_result_content,
                            is_error: is_subagent_error,
                            output_meta: None,
                            artifact_ids: Vec::new(),
                        });
                        results.push(tool_result);
                        continue;
                    }
                    let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                        node_id: node_id.clone(),
                        label: label.clone(),
                        tool_call: tool_call.clone(),
                    });
                    let registered = match tool_runner.registry().get(&tool_call.name) {
                        Ok(registered) => registered,
                        Err(error) => {
                            let record = tool_runner
                                .denied(tool_call.clone(), format!("Tool unavailable: {error}"));
                            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                node_id: node_id.clone(),
                                tool_call_id: record.result.tool_call_id.clone(),
                                tool_name: record.result.tool_name.clone(),
                                content: record.result.content.clone(),
                                is_error: true,
                                output_meta: None,
                                artifact_ids: Vec::new(),
                            });
                            results.push(record.result);
                            continue;
                        }
                    };
                    let decision = resolve_tool_policy(
                        &node_config,
                        &tool_call.name,
                        registered.definition.tier,
                        exec_approval_granted,
                    );
                    let approved = match decision {
                        ApprovalDecision::Allow => true,
                        ApprovalDecision::Deny => false,
                        ApprovalDecision::Prompt => {
                            let request = ToolApprovalRequest {
                                approval_id: Uuid::new_v4().to_string(),
                                node_id: node_id.clone(),
                                node_label: label.clone(),
                                tool_call: tool_call.clone(),
                                tier: registered.definition.tier,
                            };
                            let approval_id = request.approval_id.clone();
                            let _ = event_tx.send(ExecutionEvent::ToolApprovalRequested {
                                request: request.to_pending(),
                            });
                            wait_for_approval(&mut action_rx, &approval_id).await
                        }
                    };

                    if !approved {
                        let reason = format!("Tool call denied for {}", tool_call.name);
                        let record = tool_runner.denied(tool_call.clone(), reason.clone());
                        let _ = event_tx.send(ExecutionEvent::ToolDenied {
                            approval_id: String::new(),
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            reason,
                        });
                        let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                            node_id: node_id.clone(),
                            tool_call_id: record.result.tool_call_id.clone(),
                            tool_name: record.result.tool_name.clone(),
                            content: record.result.content.clone(),
                            is_error: true,
                            output_meta: None,
                            artifact_ids: Vec::new(),
                        });
                        results.push(record.result);
                        continue;
                    }

                    if registered.definition.tier == domain::ToolTier::Exec {
                        exec_approval_granted = true;
                    }
                    let _ = event_tx.send(ExecutionEvent::ToolStarted {
                        node_id: node_id.clone(),
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        arguments: tool_call.arguments.clone(),
                    });
                    match tool_runner.execute(tool_call.clone()).await {
                        Ok(record) => {
                            if let Some(artifact) = record.artifact.clone() {
                                let _ = event_tx.send(ExecutionEvent::ToolArtifactCreated {
                                    node_id: node_id.clone(),
                                    artifact: ToolArtifactSummary {
                                        artifact_id: artifact.artifact_id.clone(),
                                        tool_name: artifact.tool_name.clone(),
                                        path: artifact.path.clone(),
                                        size_bytes: artifact.size_bytes,
                                    },
                                });
                            }
                            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                node_id: node_id.clone(),
                                tool_call_id: record.result.tool_call_id.clone(),
                                tool_name: record.result.tool_name.clone(),
                                content: record.result.content.clone(),
                                is_error: false,
                                output_meta: record.result.output_meta.clone(),
                                artifact_ids: record.result.artifact_ids.clone(),
                            });
                            results.push(record.result);
                        }
                        Err(error) => {
                            let record =
                                tool_runner.denied(tool_call.clone(), render_tool_error(error));
                            let _ = event_tx.send(ExecutionEvent::ToolCompleted {
                                node_id: node_id.clone(),
                                tool_call_id: record.result.tool_call_id.clone(),
                                tool_name: record.result.tool_name.clone(),
                                content: record.result.content.clone(),
                                is_error: true,
                                output_meta: None,
                                artifact_ids: Vec::new(),
                            });
                            results.push(record.result);
                        }
                    }
                }
                if let Err(error) = engine.on_tool_results(&node_id, results) {
                    let _ = event_tx.send(ExecutionEvent::Error(error));
                    return;
                }
            }
            EnginePollResult::Completed(report) => {
                let _ = event_tx.send(ExecutionEvent::Finished(report));
                return;
            }
            EnginePollResult::Failed(error) => {
                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                return;
            }
        }
    }
}

async fn wait_for_approval(
    action_rx: &mut UnboundedReceiver<ExecutionAction>,
    approval_id: &str,
) -> bool {
    loop {
        match action_rx.recv().await {
            Some(ExecutionAction::ResolveApproval {
                approval_id: received,
                allow,
            }) if received == approval_id => return allow,
            Some(ExecutionAction::ProvideInput(_)) => continue,
            Some(ExecutionAction::ResolveApproval { .. }) => continue,
            None => return false,
        }
    }
}

fn render_tool_error(error: ToolRunnerError) -> String {
    error.to_string()
}

fn send_node_start_events(event_tx: &UnboundedSender<ExecutionEvent>, request: &AgentRequest) {
    let _ = event_tx.send(ExecutionEvent::NodeQueued {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::NodeStarted {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
}

fn emit_assistant_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &str,
    outcome: &AgentTurnOutcome,
) {
    let message = match outcome {
        AgentTurnOutcome::Completed(success) => success.assistant_message.clone(),
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            assistant_message, ..
        }) => filter_tool_turn_assistant_message(assistant_message.clone()),
        AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            assistant_message, ..
        }) => Some(assistant_message.clone()),
    };
    if let Some(content) = message.filter(|value| !value.trim().is_empty()) {
        let _ = event_tx.send(ExecutionEvent::ChatMessage {
            node_id: NodeId(node_id.to_string()),
            role: ChatRole::Assistant,
            content,
        });
    }
}

/// # Errors
/// Returns an error if the workflow execution fails.
pub async fn run_workflow_headless<A>(
    workflow: Workflow,
    entrypoint: Option<String>,
    ai: A,
    manual_inputs: Vec<ManualInput>,
    approvals: Vec<ApprovalResponse>,
) -> Result<WorkflowRunSnapshot, WorkflowExecutionError>
where
    A: AiPort + Send + Sync + 'static,
{
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = tokio::spawn(drive_interactive_workflow(
        workflow.clone(),
        entrypoint,
        ai,
        event_tx,
        action_rx,
    ));
    let mut manual_inputs = VecDeque::from(manual_inputs);
    let mut approvals = VecDeque::from(approvals);
    let mut state = WorkflowRunState::running_for_workflow(&workflow);

    while let Some(event) = event_rx.recv().await {
        let awaiting_input = matches!(
            &event,
            ExecutionEvent::NodeAwaitingInput { node_id, .. }
                if manual_inputs
                    .front()
                    .map(|next| next.node_id == *node_id)
                    .unwrap_or(false)
        );
        let awaiting_approval = matches!(
            &event,
            ExecutionEvent::ToolApprovalRequested { request }
                if approvals
                    .front()
                    .map(|next| next.approval_id.is_empty() || next.approval_id == request.approval_id)
                    .unwrap_or(false)
        );

        apply_event_to_run_state(&workflow, &mut state, event);

        if awaiting_input {
            let input = manual_inputs.pop_front().unwrap();
            action_tx
                .send(ExecutionAction::ProvideInput(input.text.clone()))
                .map_err(|_| WorkflowExecutionError::Execution("run channel closed".to_string()))?;
            record_user_input(&mut state, &input.node_id, input.text);
        }
        if awaiting_approval {
            let approval = approvals.pop_front().unwrap();
            let approval_id = if approval.approval_id.is_empty() {
                state
                    .pending_approvals
                    .first()
                    .map(|item| item.approval_id.clone())
                    .unwrap_or_default()
            } else {
                approval.approval_id
            };
            action_tx
                .send(ExecutionAction::ResolveApproval {
                    approval_id,
                    allow: approval.allow,
                })
                .map_err(|_| WorkflowExecutionError::Execution("run channel closed".to_string()))?;
            state.pending_approvals.clear();
        }

        if !state.active {
            break;
        }

        if let Some(node_id) = state.awaiting_node_id.clone() {
            if manual_inputs.front().map(|item| item.node_id.clone()) != Some(node_id.clone()) {
                return Err(WorkflowExecutionError::MissingManualInput(node_id));
            }
        }
        if let Some(approval) = state.pending_approvals.first() {
            let matches_next = approvals.front().is_some_and(|item| {
                item.approval_id.is_empty() || item.approval_id == approval.approval_id
            });
            if !matches_next {
                return Err(WorkflowExecutionError::MissingApproval(
                    approval.approval_id.clone(),
                ));
            }
        }
    }

    handle.abort();
    if let Some(error) = state.last_error.clone() {
        return Err(WorkflowExecutionError::Execution(error));
    }
    let report = state
        .last_report
        .clone()
        .ok_or_else(|| WorkflowExecutionError::Execution("run did not finish".to_string()))?;
    Ok(WorkflowRunSnapshot {
        report,
        run_trace: state.run_trace,
        chat_logs: state.chat_logs,
        outputs: state.outputs,
        pending_approvals: state.pending_approvals,
        tool_calls_by_node: state.tool_calls_by_node,
        tool_artifacts: state.tool_artifacts,
    })
}

pub fn apply_event_to_run_state(
    _workflow: &Workflow,
    state: &mut WorkflowRunState,
    event: ExecutionEvent,
) {
    match event {
        ExecutionEvent::NodeQueued { node_id, label } => {
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Queued);
            state.run_trace.push(RunTraceEntry {
                node_id,
                node_label: label,
                status: TraceStatus::Queued,
                message: "queued".to_string(),
                output: None,
            });
        }
        ExecutionEvent::NodeStarted { node_id, label } => {
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Started);
            state.run_trace.push(RunTraceEntry {
                node_id,
                node_label: label,
                status: TraceStatus::Running,
                message: "started OpenAI node call".to_string(),
                output: None,
            });
        }
        ExecutionEvent::ChatMessage {
            node_id,
            role,
            content,
        } => {
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(role, content));
        }
        ExecutionEvent::NodeAwaitingInput {
            node_id,
            label,
            context,
            ..
        } => {
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::AwaitingInput);
            state.awaiting_node_id = Some(node_id.clone());
            state.active_manual_node_id = None;
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label.clone(),
                status: TraceStatus::Paused,
                message: "paused for human input".to_string(),
                output: None,
            });
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Node '{label}' is awaiting human input."),
                ));
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::Thinking,
                    format!("Context:\n{context}"),
                ));
        }
        ExecutionEvent::ToolCallProposed {
            node_id, tool_call, ..
        } => {
            let calls = state.tool_calls_by_node.entry(node_id.clone()).or_default();
            calls.push(ToolCallSummary {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                status: ToolCallStatus::Proposed,
                arguments: tool_call.arguments.clone(),
                last_output: None,
                is_error: false,
            });
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::tool_marker(tool_call.id.clone()));
        }
        ExecutionEvent::ToolApprovalRequested { request } => {
            state.awaiting_node_id = None;
            state.active_tool_call_id = Some(request.tool_call.id.clone());
            state.pending_approvals = vec![request.clone()];
            state.status_by_node.insert(
                NodeId(request.node_id.clone()),
                AgentStatus::AwaitingToolApproval,
            );
            state.run_trace.push(RunTraceEntry {
                node_id: NodeId(request.node_id.clone()),
                node_label: request.node_label.clone(),
                status: TraceStatus::Paused,
                message: format!("awaiting approval for {}", request.tool_call.name),
                output: None,
            });
            state
                .chat_logs
                .entry(NodeId(request.node_id.clone()))
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Approval required for tool '{}'.", request.tool_call.name),
                ));
            update_tool_status(
                state,
                &NodeId(request.node_id),
                &request.tool_call.id,
                ToolCallStatus::AwaitingApproval,
                None,
                false,
            );
        }
        ExecutionEvent::ToolApproved {
            node_id,
            tool_call_id,
            ..
        } => {
            state.pending_approvals.clear();
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Running,
                None,
                false,
            );
        }
        ExecutionEvent::ToolDenied {
            node_id,
            tool_call_id,
            reason,
            ..
        } => {
            state.pending_approvals.clear();
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Blocked,
                Some(reason),
                true,
            );
        }
        ExecutionEvent::ToolStarted {
            node_id,
            tool_call_id,
            tool_name,
            ..
        } => {
            state.active_tool_call_id = Some(tool_call_id.clone());
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::RunningTool);
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: tool_name.clone(),
                status: TraceStatus::Running,
                message: format!("running tool {tool_name}"),
                output: None,
            });
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                ToolCallStatus::Running,
                None,
                false,
            );
        }
        ExecutionEvent::ToolCompleted {
            node_id,
            tool_call_id,
            tool_name: _,
            content,
            is_error,
            artifact_ids: _,
            ..
        } => {
            state.active_tool_call_id = None;
            update_tool_status(
                state,
                &node_id,
                &tool_call_id,
                if is_error {
                    ToolCallStatus::Failed
                } else {
                    ToolCallStatus::Completed
                },
                Some(content),
                is_error,
            );
        }
        ExecutionEvent::ToolArtifactCreated { artifact, .. } => {
            state
                .tool_artifacts
                .insert(artifact.artifact_id.clone(), artifact);
        }
        ExecutionEvent::NodeCompleted {
            node_id,
            label,
            output,
        } => {
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Completed);
            state.outputs.insert(node_id.clone(), output.clone());
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label,
                status: TraceStatus::Completed,
                message: "completed".to_string(),
                output: Some(output.clone()),
            });
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(ChatRole::Assistant, output.to_string()));
        }
        ExecutionEvent::NodeFailed {
            node_id,
            label,
            error,
        } => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state
                .status_by_node
                .insert(node_id.clone(), AgentStatus::Failed);
            state.run_trace.push(RunTraceEntry {
                node_id: node_id.clone(),
                node_label: label,
                status: TraceStatus::Failed,
                message: error.clone(),
                output: None,
            });
            state.last_error = Some(error.clone());
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Failed: {error}"),
                ));
        }
        ExecutionEvent::Finished(report) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state.last_report = Some(report);
        }
        ExecutionEvent::Error(error) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.active_tool_call_id = None;
            state.pending_approvals.clear();
            state.last_error = Some(error);
        }
        ExecutionEvent::SubagentsDeclared {
            node_id,
            declarations,
        } => {
            let summaries: Vec<SubagentSummary> = declarations
                .iter()
                .enumerate()
                .map(|(i, dec)| SubagentSummary {
                    id: format!("{}-subagent-{}", node_id, i + 1),
                    name: dec.name.clone(),
                    purpose: dec.purpose.clone(),
                    status: SubagentStatus::Declared,
                })
                .collect();
            state.subagents_by_node.insert(node_id.clone(), summaries);
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Declared {} subagent(s).", declarations.len()),
                ));
        }
        ExecutionEvent::SubagentStarted {
            node_id,
            subagent_id,
        } => {
            if let Some(subs) = state.subagents_by_node.get_mut(&node_id) {
                if let Some(sub) = subs.iter_mut().find(|s| s.id == *subagent_id) {
                    sub.status = SubagentStatus::Active;
                }
            }
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Subagent {} started.", subagent_id),
                ));
        }
        ExecutionEvent::SubagentCompleted {
            node_id,
            subagent_id,
        } => {
            if let Some(subs) = state.subagents_by_node.get_mut(&node_id) {
                if let Some(sub) = subs.iter_mut().find(|s| s.id == *subagent_id) {
                    sub.status = SubagentStatus::Completed;
                }
            }
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Subagent {} completed.", subagent_id),
                ));
        }
        ExecutionEvent::SubagentFailed {
            node_id,
            subagent_id,
            error,
        } => {
            if let Some(subs) = state.subagents_by_node.get_mut(&node_id) {
                if let Some(sub) = subs.iter_mut().find(|s| s.id == *subagent_id) {
                    sub.status = SubagentStatus::Failed;
                }
            }
            state
                .chat_logs
                .entry(node_id.clone())
                .or_default()
                .push(ChatMessage::text(
                    ChatRole::System,
                    format!("Subagent {} failed: {}", subagent_id, error),
                ));
        }
    }
}

fn update_tool_status(
    state: &mut WorkflowRunState,
    node_id: &NodeId,
    tool_call_id: &str,
    status: ToolCallStatus,
    content: Option<String>,
    is_error: bool,
) {
    if let Some(call) = state
        .tool_calls_by_node
        .entry(node_id.clone())
        .or_default()
        .iter_mut()
        .find(|call| call.tool_call_id == tool_call_id)
    {
        call.status = status;
        call.is_error = is_error;
        if let Some(content) = content {
            call.last_output = Some(content);
        }
    }
}

pub fn record_user_input(state: &mut WorkflowRunState, node_id: &str, text: String) {
    state
        .chat_logs
        .entry(NodeId(node_id.to_string()))
        .or_default()
        .push(ChatMessage::text(ChatRole::User, text));
    state.awaiting_node_id = None;
    state.active_manual_node_id = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::{AgentTurnSuccess, NodeToolConfig, ToolRef, ToolTier};
    use parking_lot::Mutex;
    use serde_json::json;
    use std::sync::Arc;

    fn workflow() -> Workflow {
        let mut workflow = Workflow::new("trace");
        let mut first = domain::Node::agent("First", 0.0, 0.0);
        first.id = NodeId("first".to_string());
        first.agent.model = "test-model".to_string();
        workflow.nodes = vec![first];
        workflow
    }
    #[test]
    fn reducer_tracks_tool_approval_and_completion() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::ToolCallProposed {
                node_id: NodeId("first".to_string()),
                label: "First".to_string(),
                tool_call: ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: json!({"path": "README.md"}),
                    intent: None,
                },
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::ToolApprovalRequested {
                request: domain::PendingToolApproval {
                    approval_id: "approval-1".to_string(),
                    node_id: "first".to_string(),
                    node_label: "First".to_string(),
                    tool_call: ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                        intent: None,
                    },
                    tier: ToolTier::Read,
                },
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::ToolCompleted {
                node_id: NodeId("first".to_string()),
                tool_call_id: "call-1".to_string(),
                tool_name: "read".to_string(),
                content: "done".to_string(),
                is_error: false,
                output_meta: None,
                artifact_ids: Vec::new(),
            },
        );

        assert_eq!(state.pending_approvals.len(), 1);
        assert_eq!(
            state.tool_calls_by_node[&NodeId("first".to_string())][0].tool_name,
            "read"
        );
        let chat = &state.chat_logs[&NodeId("first".to_string())];
        assert_eq!(chat[0].tool_call_id.as_deref(), Some("call-1"));
        assert!(chat[1]
            .content
            .contains("Approval required for tool 'read'."));
        assert_eq!(
            state.tool_calls_by_node[&NodeId("first".to_string())][0].last_output.as_deref(),
            Some("done")
        );
    }

    #[tokio::test]
    async fn headless_run_auto_approves_read_tool_and_reenters_model_loop() {
        #[derive(Clone, Default)]
        struct ScriptedAi {
            calls: Arc<Mutex<usize>>,
        }

        #[async_trait]
        impl AiPort for ScriptedAi {
            async fn invoke(
                &self,
                request: AgentRequest,
            ) -> Result<AgentTurnOutcome, domain::AgentError> {
                let mut calls = self.calls.lock();
                *calls += 1;
                if *calls == 1 {
                    assert_eq!(request.available_tools.len(), 3);
                    return Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                        raw_text: String::new(),
                        assistant_message: Some("Inspecting docs".to_string()),
                        tool_calls: vec![ToolCall {
                            id: "call-1".to_string(),
                            name: "read".to_string(),
                            arguments: json!({"path": "README.md"}),
                            intent: None,
                        }],
                    }));
                }
                Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                    output: json!({"summary": "done"}),
                    raw_text: "{}".to_string(),
                    assistant_message: None,
                }))
            }
        }

        let mut workflow = workflow();
        workflow.nodes[0].agent.tools.catalog.tools = vec![ToolRef {
            name: "read".to_string(),
        }];
        let snapshot = run_workflow_headless(
            workflow,
            None,
            ScriptedAi::default(),
            Vec::new(),
            Vec::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            snapshot.outputs[&NodeId("first".to_string())],
            json!({"summary": "done"})
        );
        assert!(!snapshot.tool_calls_by_node[&NodeId("first".to_string())].is_empty());
    }

    #[tokio::test]
    async fn headless_run_requires_scripted_approval_for_prompted_tool() {
        #[derive(Clone)]
        struct PromptingAi;

        #[async_trait]
        impl AiPort for PromptingAi {
            async fn invoke(
                &self,
                _request: AgentRequest,
            ) -> Result<AgentTurnOutcome, domain::AgentError> {
                Ok(AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
                    raw_text: String::new(),
                    assistant_message: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({"path": "README.md"}),
                        intent: None,
                    }],
                }))
            }
        }

        let mut workflow = workflow();
        workflow.nodes[0].agent.tools.catalog.tools = vec![ToolRef {
            name: "read".to_string(),
        }];
        workflow.nodes[0].agent.tools.approval_mode = Some(domain::ApprovalMode::AlwaysAsk);
        let error = run_workflow_headless(workflow, None, PromptingAi, Vec::new(), Vec::new())
            .await
            .unwrap_err();
        assert!(matches!(error, WorkflowExecutionError::MissingApproval(_)));
    }

    #[test]
    fn reducer_marks_failure_terminal_with_label() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::NodeFailed {
                node_id: NodeId("first".to_string()),
                label: "First".to_string(),
                error: "boom".to_string(),
            },
        );

        assert!(!state.active);
        assert_eq!(state.last_error.as_deref(), Some("boom"));
        assert_eq!(state.run_trace[0].node_label, "First");
        assert_eq!(state.run_trace[0].status, TraceStatus::Failed);
    }

    #[test]
    fn subagents_declared_event_updates_run_state() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentsDeclared {
                node_id: NodeId("first".to_string()),
                declarations: vec![
                    SubagentDeclaration {
                        name: "Researcher".to_string(),
                        purpose: "Investigate API behavior".to_string(),
                    },
                    SubagentDeclaration {
                        name: "Writer".to_string(),
                        purpose: "Summarize findings".to_string(),
                    },
                ],
            },
        );

        let subs = &state.subagents_by_node[&NodeId("first".to_string())];
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].name, "Researcher");
        assert_eq!(subs[0].purpose, "Investigate API behavior");
        assert_eq!(subs[0].status, SubagentStatus::Declared);
        assert_eq!(subs[0].id, "first-subagent-1");
        assert_eq!(subs[1].name, "Writer");
        assert_eq!(subs[1].id, "first-subagent-2");

        assert!(state.chat_logs[&NodeId("first".to_string())]
            .iter()
            .any(|m| m.content.contains("Declared 2 subagent")));
    }

    #[test]
    fn subagents_are_scoped_to_declaring_node() {
        let mut second = domain::Node::agent("Second", 100.0, 0.0);
        second.id = NodeId("second".to_string());
        second.agent.model = "test-model".to_string();
        let mut workflow = workflow();
        workflow.nodes.push(second);
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentsDeclared {
                node_id: NodeId("first".to_string()),
                declarations: vec![SubagentDeclaration {
                    name: "Researcher".to_string(),
                    purpose: "Investigate".to_string(),
                }],
            },
        );

        assert!(state
            .subagents_by_node
            .contains_key(&NodeId("first".to_string())));
        assert!(!state
            .subagents_by_node
            .contains_key(&NodeId("second".to_string())));
    }

    #[test]
    fn fresh_run_state_has_empty_subagents() {
        let workflow = workflow();
        let state = WorkflowRunState::running_for_workflow(&workflow);
        assert!(state.subagents_by_node.is_empty());
    }

    #[test]
    fn subagent_started_event_transitions_status() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        // First declare the subagent
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentsDeclared {
                node_id: NodeId("first".to_string()),
                declarations: vec![SubagentDeclaration {
                    name: "Worker".to_string(),
                    purpose: "Do work".to_string(),
                }],
            },
        );

        // Then start it
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentStarted {
                node_id: NodeId("first".to_string()),
                subagent_id: "first-subagent-1".to_string(),
            },
        );

        let sub = &state.subagents_by_node[&NodeId("first".to_string())][0];
        assert_eq!(sub.status, SubagentStatus::Active);
        assert!(state.chat_logs[&NodeId("first".to_string())]
            .iter()
            .any(|m| m.content.contains("Subagent first-subagent-1 started")));
    }

    #[test]
    fn subagent_completed_event_transitions_status() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentsDeclared {
                node_id: NodeId("first".to_string()),
                declarations: vec![SubagentDeclaration {
                    name: "Worker".to_string(),
                    purpose: "Do work".to_string(),
                }],
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentStarted {
                node_id: NodeId("first".to_string()),
                subagent_id: "first-subagent-1".to_string(),
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentCompleted {
                node_id: NodeId("first".to_string()),
                subagent_id: "first-subagent-1".to_string(),
            },
        );

        let sub = &state.subagents_by_node[&NodeId("first".to_string())][0];
        assert_eq!(sub.status, SubagentStatus::Completed);
    }

    #[test]
    fn subagent_failed_event_transitions_status() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentsDeclared {
                node_id: NodeId("first".to_string()),
                declarations: vec![SubagentDeclaration {
                    name: "Worker".to_string(),
                    purpose: "Do work".to_string(),
                }],
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentStarted {
                node_id: NodeId("first".to_string()),
                subagent_id: "first-subagent-1".to_string(),
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::SubagentFailed {
                node_id: NodeId("first".to_string()),
                subagent_id: "first-subagent-1".to_string(),
                error: "API error".to_string(),
            },
        );

        let sub = &state.subagents_by_node[&NodeId("first".to_string())][0];
        assert_eq!(sub.status, SubagentStatus::Failed);
        assert!(state.chat_logs[&NodeId("first".to_string())]
            .iter()
            .any(|m| m
                .content
                .contains("Subagent first-subagent-1 failed: API error")));
    }

    #[test]
    fn declare_subagents_tool_is_always_in_definitions() {
        let registry = ToolRegistry::new();
        let definitions = registry.definitions_for(&NodeToolConfig::default());
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"openflow_declare_subagents"));
        assert!(names.contains(&"openflow_call_subagent"));
    }
}
