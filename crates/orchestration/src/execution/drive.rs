use crate::agent_store::AgentDefinition;
use crate::state::ToolArtifactSummary;
use crate::tools::{ArtifactStore, ToolRegistry, ToolRunner, ToolRunnerError};
use domain::{
    filter_tool_turn_assistant_message, AgentNeedUserInput, AgentRequest, AgentToolCallBatch,
    AgentTranscriptItem, AgentTurnOutcome, AiPort, ChatRole, EnginePollResult, InteractiveEngine,
    NodeId, SubagentDeclaration, SubagentStatus, SubagentSummary, Workflow,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use super::subagents::{
    adhoc_subagent_base_index, append_shared_context, augment_call_subagent_tool_description,
    agent_purpose, build_adhoc_subagent_summaries, build_predefined_subagent_summaries,
    merge_subagent_summaries_into_map,
};
use super::{ExecutionAction, ExecutionEvent};

#[derive(serde::Deserialize)]
struct SubagentDeclarationBatch {
    subagents: Vec<SubagentDeclaration>,
}

#[derive(serde::Deserialize)]
struct CallSubagentArgs {
    subagent_id: String,
    input: String,
}
pub(super) async fn drive_interactive_workflow<A>(
    workflow: Workflow,
    entrypoint: Option<String>,
    execution_cwd: PathBuf,
    ai: A,
    event_tx: UnboundedSender<ExecutionEvent>,
    mut action_rx: UnboundedReceiver<ExecutionAction>,
    agent_snapshots: BTreeMap<String, AgentDefinition>,
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
    let artifact_root = std::env::temp_dir().join(format!("openflow-run-{}", Uuid::new_v4()));
    let artifacts = match ArtifactStore::new(artifact_root) {
        Ok(store) => store,
        Err(error) => {
            let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
            return;
        }
    };
    let tool_runner = ToolRunner::new(tool_registry, execution_cwd, artifacts);
    let mut declared_subagents: BTreeMap<String, SubagentSummary> = BTreeMap::new();
    let mut predefined_registered: HashSet<NodeId> = HashSet::new();
    let mut proposed_tool_calls: HashSet<String> = HashSet::new();

    loop {
        match engine.poll() {
            EnginePollResult::CallAi {
                node_id,
                mut request,
            } => {
                if !predefined_registered.contains(&node_id) {
                    if let Some(node) = workflow.nodes.iter().find(|node| node.id == node_id) {
                        let summaries = build_predefined_subagent_summaries(node, &agent_snapshots);
                        if !summaries.is_empty() {
                            merge_subagent_summaries_into_map(&mut declared_subagents, &summaries);
                            let _ = event_tx.send(ExecutionEvent::SubagentsDeclared {
                                node_id: node_id.clone(),
                                summaries,
                            });
                        }
                    }
                    predefined_registered.insert(node_id.clone());
                }
                request.available_tools =
                    tool_runner.registry().definitions_for(&request.tool_config);
                if let Some(node) = workflow.nodes.iter().find(|node| node.id == node_id) {
                    augment_call_subagent_tool_description(
                        &mut request.available_tools,
                        node,
                        &declared_subagents,
                        &agent_snapshots,
                    );
                }
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
                                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
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
                approval_id,
                node_id,
                label,
                tool_calls,
            } => {
                let mut approval_request = None;
                for tool_call in &tool_calls {
                    if proposed_tool_calls.insert(tool_call.id.clone()) {
                        let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                            node_id: node_id.clone(),
                            label: label.clone(),
                            tool_call: tool_call.clone(),
                        });
                    }
                    let tier = tool_runner
                        .registry()
                        .get(&tool_call.name)
                        .map(|registered| registered.definition.tier)
                        .unwrap_or_else(|_| {
                            workflow
                                .nodes
                                .iter()
                                .find(|node| node.id == node_id)
                                .map(|node| {
                                    domain::tool_tier_for_call(&node.agent.tools, &tool_call.name)
                                })
                                .unwrap_or(domain::ToolTier::Write)
                        });
                    if approval_request.is_none() {
                        approval_request = Some(domain::PendingToolApproval {
                            approval_id: approval_id.clone(),
                            node_id: node_id.to_string(),
                            node_label: label.clone(),
                            tool_call: tool_call.clone(),
                            tier,
                        });
                    }
                }
                if let Some(request) = approval_request {
                    let _ = event_tx.send(ExecutionEvent::ToolApprovalRequested { request });
                }
                let approved = wait_for_approval(&mut action_rx, &approval_id).await;
                if let Err(error) = engine.on_tool_decision(&approval_id, approved) {
                    let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                    return;
                }
                for tool_call in &tool_calls {
                    if approved {
                        let _ = event_tx.send(ExecutionEvent::ToolApproved {
                            approval_id: approval_id.clone(),
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                        });
                    } else {
                        let _ = event_tx.send(ExecutionEvent::ToolDenied {
                            approval_id: approval_id.clone(),
                            node_id: node_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            reason: "denied by user".to_string(),
                        });
                    }
                }
            }
            EnginePollResult::RunTools {
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
                        let base_index = adhoc_subagent_base_index(&node_id, &declared_subagents);
                        let summaries =
                            build_adhoc_subagent_summaries(&node_id, &declarations, base_index);
                        merge_subagent_summaries_into_map(&mut declared_subagents, &summaries);
                        let _ = event_tx.send(ExecutionEvent::SubagentsDeclared {
                            node_id: node_id.clone(),
                            summaries: summaries.clone(),
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
                        let subagent =
                            if let Some(summary) = declared_subagents.get(&call_args.subagent_id) {
                                Some(summary.clone())
                            } else {
                                agent_snapshots.get(&call_args.subagent_id).map(|agent| {
                                    SubagentSummary {
                                        id: agent.id.clone(),
                                        name: agent.name.clone(),
                                        purpose: agent_purpose(agent),
                                        status: SubagentStatus::Declared,
                                    }
                                })
                            };
                        let Some(subagent) = subagent else {
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
                        // Build subagent request from saved agent or ad-hoc declaration
                        let Some(parent_node) = workflow.nodes.iter().find(|n| n.id == node_id) else {
                            let result_content = serde_json::json!({
                                "error": format!("Parent node '{node_id}' not found in workflow")
                            })
                            .to_string();
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
                        };
                        let (sub_node_config, sub_request) = if let Some(agent_def) =
                            agent_snapshots.get(&call_args.subagent_id)
                        {
                            let sub_node_config = agent_def.tools.clone();
                            let sub_available_tools = tool_runner
                                .registry()
                                .definitions_for_subagent(&sub_node_config);
                            let system_prompt =
                                append_shared_context(&workflow, &agent_def.system_prompt);
                            let sub_transcript = vec![AgentTranscriptItem::UserMessage {
                                content: format!(
                                    "You are the saved agent \"{}\".\n\nTask: {}",
                                    agent_def.name, call_args.input
                                ),
                            }];
                            let sub_request = AgentRequest {
                                workflow_id: workflow.id.clone(),
                                node_id: NodeId(subagent.id.clone()),
                                node_label: subagent.name.clone(),
                                model: agent_def.model.clone(),
                                system_prompt,
                                task_prompt: call_args.input.clone(),
                                input: serde_json::json!(null),
                                output_schema: agent_def.output_schema.clone(),
                                tool_config: sub_node_config.clone(),
                                available_tools: sub_available_tools,
                                transcript: sub_transcript,
                            };
                            (sub_node_config, sub_request)
                        } else {
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
                            let system_prompt = append_shared_context(
                                &workflow,
                                &format!("You are {}. {}", subagent.name, subagent.purpose),
                            );
                            let sub_request = AgentRequest {
                                workflow_id: workflow.id.clone(),
                                node_id: NodeId(subagent.id.clone()),
                                node_label: subagent.name.clone(),
                                model: parent_node.agent.model.clone(),
                                system_prompt,
                                task_prompt: call_args.input.clone(),
                                input: serde_json::json!(null),
                                output_schema: Value::Null,
                                tool_config: sub_node_config.clone(),
                                available_tools: sub_available_tools,
                                transcript: sub_transcript,
                            };
                            (sub_node_config, sub_request)
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
                    if proposed_tool_calls.insert(tool_call.id.clone()) {
                        let _ = event_tx.send(ExecutionEvent::ToolCallProposed {
                            node_id: node_id.clone(),
                            label: label.clone(),
                            tool_call: tool_call.clone(),
                        });
                    }
                    if let Err(error) = tool_runner.registry().get(&tool_call.name) {
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
                    let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
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
