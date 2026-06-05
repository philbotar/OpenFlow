use crate::state::{AgentStatus, RunTraceEntry, TraceStatus, WorkflowRunState};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use workflow_core::{
    AiPort, ChatMessage, ChatRole, ConversationAgentRequest, EnginePollResult, InteractiveEngine,
    NodeId, RunReport, Workflow,
};

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
}

pub enum ExecutionAction {
    ProvideInput(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualInput {
    pub node_id: NodeId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunSnapshot {
    pub report: RunReport,
    pub run_trace: Vec<RunTraceEntry>,
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub outputs: BTreeMap<NodeId, Value>,
}

#[derive(Debug, Error)]
pub enum WorkflowExecutionError {
    #[error("{0}")]
    Execution(String),
    #[error("node {node_id} failed: {message}")]
    NodeFailed { node_id: NodeId, message: String },
    #[error("node {0} requested manual input but no scripted input was provided")]
    MissingManualInput(NodeId),
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
    let mut engine = match InteractiveEngine::new(workflow, entrypoint) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
            return;
        }
    };

    loop {
        match engine.poll() {
            EnginePollResult::CallAi { node_id, request } => {
                send_auto_start_events(&event_tx, &request);
                let result = ai.invoke(request.clone()).await;
                let invoke_error = result.as_ref().err().map(ToString::to_string);
                let label = request.node_label.clone();
                engine.on_ai_complete(&node_id, result);
                if let Some(output) = engine.node_output(&node_id) {
                    let _ = event_tx.send(ExecutionEvent::NodeCompleted {
                        node_id,
                        label,
                        output,
                    });
                } else {
                    let error =
                        invoke_error.unwrap_or_else(|| "node invocation failed".to_string());
                    let _ = event_tx.send(ExecutionEvent::NodeFailed {
                        node_id,
                        label,
                        error,
                    });
                    break;
                }
            }
            EnginePollResult::CallConversationAi { node_id, request } => {
                send_conversation_start_events(&event_tx, &request);
                let result = ai.invoke_conversation(request.clone()).await;
                let invoke_error = result.as_ref().err().map(ToString::to_string);
                if let Ok(response) = &result {
                    if let Some(message) = &response.assistant_message {
                        let _ = event_tx.send(ExecutionEvent::ChatMessage {
                            node_id: node_id.clone(),
                            role: ChatRole::Assistant,
                            content: message.clone(),
                        });
                    }
                }
                let label = request.node_label.clone();
                engine.on_conversation_ai_complete(&node_id, result);
                if let Some(output) = engine.node_output(&node_id) {
                    let _ = event_tx.send(ExecutionEvent::NodeCompleted {
                        node_id,
                        label,
                        output,
                    });
                } else if let Some(error) = invoke_error {
                    let _ = event_tx.send(ExecutionEvent::NodeFailed {
                        node_id,
                        label,
                        error,
                    });
                    break;
                }
            }
            EnginePollResult::AwaitInput {
                node_id,
                label,
                context,
                is_initial,
            } => {
                if is_initial {
                    let _ = event_tx.send(ExecutionEvent::NodeQueued {
                        node_id: node_id.clone(),
                        label: label.clone(),
                    });
                }
                let _ = event_tx.send(ExecutionEvent::NodeAwaitingInput {
                    node_id: node_id.clone(),
                    label: label.clone(),
                    context,
                    is_initial,
                });
                match action_rx.recv().await {
                    Some(ExecutionAction::ProvideInput(text)) => {
                        if let Err(error) = engine.on_human_input(&node_id, &text) {
                            let _ = event_tx.send(ExecutionEvent::Error(error));
                            break;
                        }
                    }
                    None => break,
                }
            }
            EnginePollResult::Completed(report) => {
                let _ = event_tx.send(ExecutionEvent::Finished(report));
                break;
            }
            EnginePollResult::Failed(error) => {
                let _ = event_tx.send(ExecutionEvent::Error(error.to_string()));
                break;
            }
        }
    }
}

fn send_auto_start_events(
    event_tx: &UnboundedSender<ExecutionEvent>,
    request: &workflow_core::AgentRequest,
) {
    let _ = event_tx.send(ExecutionEvent::NodeQueued {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::NodeStarted {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::ChatMessage {
        node_id: request.node_id.clone(),
        role: ChatRole::System,
        content: format!("Node '{}' started", request.node_label),
    });
    let system_preview = request.system_prompt.chars().take(120).collect::<String>();
    let _ = event_tx.send(ExecutionEvent::ChatMessage {
        node_id: request.node_id.clone(),
        role: ChatRole::Thinking,
        content: format!("System prompt: {system_preview}..."),
    });
    let upstream_json = request.input.to_string();
    let upstream_preview = upstream_json.chars().take(200).collect::<String>();
    let _ = event_tx.send(ExecutionEvent::ChatMessage {
        node_id: request.node_id.clone(),
        role: ChatRole::Thinking,
        content: format!("Upstream input: {upstream_preview}"),
    });
}

fn send_conversation_start_events(
    event_tx: &UnboundedSender<ExecutionEvent>,
    request: &ConversationAgentRequest,
) {
    let _ = event_tx.send(ExecutionEvent::NodeStarted {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::ChatMessage {
        node_id: request.node_id.clone(),
        role: ChatRole::Thinking,
        content: format!(
            "Continuing paused node with {} conversation message(s).",
            request.conversation.len()
        ),
    });
}

/// # Errors
/// Returns an error if the workflow execution fails.
#[allow(clippy::too_many_lines)]
pub async fn run_workflow_headless<A>(
    workflow: Workflow,
    entrypoint: Option<String>,
    ai: A,
    manual_inputs: Vec<ManualInput>,
) -> Result<WorkflowRunSnapshot, WorkflowExecutionError>
where
    A: AiPort + 'static,
{
    let mut manual_inputs = manual_inputs.into_iter().fold(
        BTreeMap::<NodeId, VecDeque<String>>::new(),
        |mut acc, input| {
            acc.entry(input.node_id).or_default().push_back(input.text);
            acc
        },
    );

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    let driver = tokio::spawn(drive_interactive_workflow(
        workflow, entrypoint, ai, event_tx, action_rx,
    ));

    let mut snapshot = PartialSnapshot::default();

    while let Some(event) = event_rx.recv().await {
        match event {
            ExecutionEvent::NodeQueued { node_id, label } => {
                snapshot.push_trace(node_id, label, TraceStatus::Queued, "queued", None);
            }
            ExecutionEvent::NodeStarted { node_id, label } => {
                snapshot.push_trace(
                    node_id.clone(),
                    label.clone(),
                    TraceStatus::Running,
                    "started OpenAI node call",
                    None,
                );
            }
            ExecutionEvent::ChatMessage {
                node_id,
                role,
                content,
            } => {
                snapshot.push_chat(node_id, role, content);
            }
            ExecutionEvent::NodeAwaitingInput {
                node_id,
                label,
                context,
                ..
            } => {
                snapshot.push_trace(
                    node_id.clone(),
                    label.clone(),
                    TraceStatus::Paused,
                    "paused for human input",
                    None,
                );
                snapshot.push_chat(
                    node_id.clone(),
                    ChatRole::System,
                    format!("Node '{label}' is awaiting human input."),
                );
                snapshot.push_chat(
                    node_id.clone(),
                    ChatRole::Thinking,
                    format!("Context:\n{context}"),
                );
                let Some(text) = manual_inputs
                    .get_mut(&node_id)
                    .and_then(VecDeque::pop_front)
                else {
                    driver.abort();
                    return Err(WorkflowExecutionError::MissingManualInput(node_id));
                };
                snapshot.push_chat(node_id, ChatRole::User, text.clone());
                let _ = action_tx.send(ExecutionAction::ProvideInput(text));
            }
            ExecutionEvent::NodeCompleted {
                node_id,
                label,
                output,
            } => {
                snapshot.push_trace(
                    node_id.clone(),
                    label,
                    TraceStatus::Completed,
                    "completed",
                    Some(output.clone()),
                );
                snapshot.outputs.insert(node_id.clone(), output.clone());
                snapshot.push_chat(node_id, ChatRole::Assistant, output.to_string());
            }
            ExecutionEvent::NodeFailed {
                node_id,
                label,
                error,
            } => {
                snapshot.push_trace(node_id.clone(), label, TraceStatus::Failed, &error, None);
                snapshot.push_chat(
                    node_id.clone(),
                    ChatRole::System,
                    format!("Failed: {error}"),
                );
                driver.abort();
                return Err(WorkflowExecutionError::NodeFailed {
                    node_id,
                    message: error,
                });
            }
            ExecutionEvent::Finished(report) => {
                let _ = driver.await;
                return Ok(WorkflowRunSnapshot {
                    outputs: report
                        .outputs
                        .iter()
                        .map(|output| (output.node_id.clone(), output.output.clone()))
                        .collect(),
                    report,
                    run_trace: snapshot.run_trace,
                    chat_logs: snapshot.chat_logs,
                });
            }
            ExecutionEvent::Error(message) => {
                driver.abort();
                return Err(WorkflowExecutionError::Execution(message));
            }
        }
    }

    let _ = driver.await;
    Err(WorkflowExecutionError::Execution(
        "workflow execution ended without a final report".to_string(),
    ))
}

#[derive(Default)]
struct PartialSnapshot {
    run_trace: Vec<RunTraceEntry>,
    chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    outputs: BTreeMap<NodeId, Value>,
}

impl PartialSnapshot {
    fn push_trace(
        &mut self,
        node_id: NodeId,
        node_label: String,
        status: TraceStatus,
        message: &str,
        output: Option<Value>,
    ) {
        self.run_trace.push(RunTraceEntry {
            node_id,
            node_label,
            status,
            message: message.to_string(),
            output,
        });
    }

    fn push_chat(&mut self, node_id: NodeId, role: ChatRole, content: String) {
        self.chat_logs
            .entry(node_id)
            .or_default()
            .push(ChatMessage { role, content });
    }
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
                .push(ChatMessage { role, content });
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
                .push(ChatMessage {
                    role: ChatRole::System,
                    content: format!("Node '{label}' is awaiting human input."),
                });
            state
                .chat_logs
                .entry(node_id)
                .or_default()
                .push(ChatMessage {
                    role: ChatRole::Thinking,
                    content: format!("Context:\n{context}"),
                });
        }
        ExecutionEvent::NodeCompleted {
            node_id,
            label,
            output,
        } => {
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
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
                .push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: output.to_string(),
                });
        }
        ExecutionEvent::NodeFailed {
            node_id,
            label,
            error,
        } => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
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
                .push(ChatMessage {
                    role: ChatRole::System,
                    content: format!("Failed: {error}"),
                });
        }
        ExecutionEvent::Finished(report) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.last_report = Some(report);
        }
        ExecutionEvent::Error(error) => {
            state.active = false;
            state.awaiting_node_id = None;
            state.active_manual_node_id = None;
            state.last_error = Some(error);
        }
    }
}

pub fn record_user_input(state: &mut WorkflowRunState, node_id: &str, text: String) {
    state
        .chat_logs
        .entry(NodeId(node_id.to_string()))
        .or_default()
        .push(ChatMessage {
            role: ChatRole::User,
            content: text,
        });
    state.awaiting_node_id = None;
    state.active_manual_node_id = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn workflow() -> Workflow {
        let mut workflow = Workflow::new("trace");
        let mut first = workflow_core::Node::agent("First", 0.0, 0.0);
        first.id = NodeId("first".to_string());
        workflow.nodes = vec![first];
        workflow
    }

    #[test]
    fn reducer_preserves_labels_and_transition_order() {
        let workflow = workflow();
        let mut state = WorkflowRunState::running_for_workflow(&workflow);

        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::NodeQueued {
                node_id: NodeId("first".to_string()),
                label: "First".to_string(),
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::NodeAwaitingInput {
                node_id: NodeId("first".to_string()),
                label: "First".to_string(),
                context: "Entrypoint: kickoff".to_string(),
                is_initial: true,
            },
        );
        record_user_input(&mut state, "first", "Need approvals".to_string());
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::NodeStarted {
                node_id: NodeId("first".to_string()),
                label: "First".to_string(),
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::ChatMessage {
                node_id: NodeId("first".to_string()),
                role: ChatRole::Assistant,
                content: "Locked.".to_string(),
            },
        );
        apply_event_to_run_state(
            &workflow,
            &mut state,
            ExecutionEvent::NodeCompleted {
                node_id: NodeId("first".to_string()),
                label: "First".to_string(),
                output: json!({"summary": "approved"}),
            },
        );

        assert_eq!(
            state
                .run_trace
                .iter()
                .map(|entry| (&*entry.node_id, entry.node_label.as_str(), entry.status))
                .collect::<Vec<_>>(),
            vec![
                ("first", "First", TraceStatus::Queued),
                ("first", "First", TraceStatus::Paused),
                ("first", "First", TraceStatus::Running),
                ("first", "First", TraceStatus::Completed),
            ]
        );
        let chat = state.chat_logs.get(&NodeId("first".to_string())).unwrap();
        assert_eq!(chat[2].role, ChatRole::User);
        assert_eq!(chat[3].role, ChatRole::Assistant);
        assert_eq!(
            state.outputs[&NodeId("first".to_string())],
            json!({"summary": "approved"})
        );
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
}
