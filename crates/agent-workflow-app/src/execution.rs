use crate::state::{RunTraceEntry, TraceStatus};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use workflow_core::{
    AiPort, ChatMessage, ChatRole, EnginePollResult, InteractiveEngine, NodeId, RunReport, Workflow,
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
    NodeThinking {
        node_id: NodeId,
        message: String,
    },
    NodeAwaitingInput {
        node_id: NodeId,
        label: String,
        context: String,
    },
    NodeCompleted {
        node_id: NodeId,
        output: Value,
    },
    NodeFailed {
        node_id: NodeId,
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
                send_ai_start_events(&event_tx, &node_id, &request);
                let result = ai.invoke(request).await;
                let invoke_error = result.as_ref().err().map(ToString::to_string);
                engine.on_ai_complete(&node_id, result);
                if let Some(output) = engine.node_output(&node_id) {
                    let _ = event_tx.send(ExecutionEvent::NodeCompleted { node_id, output });
                } else {
                    let error =
                        invoke_error.unwrap_or_else(|| "node invocation failed".to_string());
                    let _ = event_tx.send(ExecutionEvent::NodeFailed { node_id, error });
                    break;
                }
            }
            EnginePollResult::AwaitInput {
                node_id,
                label,
                context,
            } => {
                let _ = event_tx.send(ExecutionEvent::NodeQueued {
                    node_id: node_id.clone(),
                    label: label.clone(),
                });
                let _ = event_tx.send(ExecutionEvent::NodeAwaitingInput {
                    node_id: node_id.clone(),
                    label,
                    context,
                });
                if let Some(ExecutionAction::ProvideInput(text)) = action_rx.recv().await {
                    let _ = engine.on_human_input(&node_id, &text);
                } else {
                    break;
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

fn send_ai_start_events(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &str,
    request: &workflow_core::AgentRequest,
) {
    let _ = event_tx.send(ExecutionEvent::NodeQueued {
        node_id: NodeId(node_id.to_string()),
        label: request.node_label.clone(),
    });
    let _ = event_tx.send(ExecutionEvent::NodeStarted {
        node_id: NodeId(node_id.to_string()),
        label: request.node_label.clone(),
    });
    let system_preview = request.system_prompt.chars().take(120).collect::<String>();
    let _ = event_tx.send(ExecutionEvent::NodeThinking {
        node_id: NodeId(node_id.to_string()),
        message: format!("System prompt: {system_preview}..."),
    });
    let upstream_json = request.input.to_string();
    let upstream_preview = upstream_json.chars().take(200).collect::<String>();
    let _ = event_tx.send(ExecutionEvent::NodeThinking {
        node_id: NodeId(node_id.to_string()),
        message: format!("Upstream input: {upstream_preview}"),
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
                snapshot.push_chat(node_id, ChatRole::System, format!("Node '{label}' started"));
            }
            ExecutionEvent::NodeThinking { node_id, message } => {
                snapshot.push_chat(node_id, ChatRole::Thinking, message);
            }
            ExecutionEvent::NodeAwaitingInput {
                node_id,
                label,
                context,
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
            ExecutionEvent::NodeCompleted { node_id, output } => {
                let label = snapshot.node_label_or_id(&node_id);
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
            ExecutionEvent::NodeFailed { node_id, error } => {
                let label = snapshot.node_label_or_id(&node_id);
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

    fn node_label_or_id(&self, node_id: &str) -> String {
        self.run_trace
            .iter()
            .find(|entry| entry.node_id == node_id)
            .map_or_else(|| node_id.to_string(), |entry| entry.node_label.clone())
    }
}
