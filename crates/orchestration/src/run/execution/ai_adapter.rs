use super::timing::emit_phase_timed;
use super::ExecutionEvent;
use async_trait::async_trait;
use engine::{
    filter_tool_turn_assistant_message, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, AiPort, AiStreamEvent, AiStreamSink,
    ChatRole, NodeId,
};
use parking_lot::Mutex;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

// #region agent log
fn agent_debug_log(hypothesis_id: &str, location: &str, message: &str, data: serde_json::Value) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/Users/philipbotar/Developer/Step-through-agentic-workflow/.cursor/debug-74ef61.log")
    {
        let _ = writeln!(
            file,
            "{}",
            json!({
                "sessionId": "74ef61",
                "hypothesisId": hypothesis_id,
                "location": location,
                "message": message,
                "data": data,
                "timestamp": timestamp,
            })
        );
    }
}
// #endregion

pub struct AiInvocationAdapter<A> {
    inner: Arc<A>,
    event_tx: UnboundedSender<ExecutionEvent>,
    lifecycle_by_node: Mutex<BTreeMap<NodeId, u8>>,
}

impl<A> AiInvocationAdapter<A>
where
    A: AiPort + Send + Sync + 'static,
{
    pub fn new(inner: Arc<A>, event_tx: UnboundedSender<ExecutionEvent>) -> Self {
        Self {
            inner,
            event_tx,
            lifecycle_by_node: Mutex::new(BTreeMap::new()),
        }
    }
}

struct StreamSink {
    event_tx: UnboundedSender<ExecutionEvent>,
    node_id: NodeId,
    message_id: String,
    streamed: Arc<AtomicBool>,
    streamed_content: Arc<Mutex<String>>,
}

impl AiStreamSink for StreamSink {
    fn on_stream_event(&self, event: AiStreamEvent) {
        let AiStreamEvent::AssistantDelta { content } = event;
        if content.is_empty() {
            return;
        }
        self.streamed.store(true, Ordering::Relaxed);
        self.streamed_content.lock().push_str(&content);
        let _ = self.event_tx.send(ExecutionEvent::ChatMessageDelta {
            node_id: self.node_id.clone(),
            message_id: self.message_id.clone(),
            delta: content,
            finalize: false,
        });
    }
}

#[async_trait]
impl<A> AiPort for AiInvocationAdapter<A>
where
    A: AiPort + Send + Sync,
{
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        maybe_send_node_start_events(self, &request);
        let node_id = request.node_id.clone();
        let label = request.node_label.clone();
        let message_id = Uuid::new_v4().to_string();
        let streamed = Arc::new(AtomicBool::new(false));
        let streamed_content = Arc::new(Mutex::new(String::new()));
        let sink = StreamSink {
            event_tx: self.event_tx.clone(),
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            streamed: Arc::clone(&streamed),
            streamed_content: Arc::clone(&streamed_content),
        };
        let started = Instant::now();
        let result = self.inner.invoke_stream(request, &sink).await;
        emit_phase_timed(
            &self.event_tx,
            "ai_invoke",
            &label,
            Some(node_id.clone()),
            started,
        );
        let did_stream = streamed.load(Ordering::Relaxed);
        if did_stream {
            finalize_stream_message(&self.event_tx, &node_id, &message_id);
        }
        if let Ok(outcome) = &result {
            // #region agent log
            let (outcome_kind, assistant_message) = match outcome {
                AgentTurnOutcome::Completed(success) => {
                    ("completed", success.assistant_message.clone())
                }
                AgentTurnOutcome::ToolCalls(batch) => {
                    ("tool_calls", batch.assistant_message.clone())
                }
                AgentTurnOutcome::NeedsUserInput(input) => {
                    ("needs_user_input", Some(input.assistant_message.clone()))
                }
            };
            let streamed_text = streamed_content.lock().clone();
            let will_emit = should_emit_assistant_message(did_stream, outcome, &streamed_text);
            agent_debug_log(
                "H1",
                "ai_adapter.rs:invoke",
                "ai invoke outcome",
                json!({
                    "nodeId": node_id.0,
                    "outcomeKind": outcome_kind,
                    "didStream": did_stream,
                    "willEmitAssistantMessage": will_emit,
                    "assistantMessage": assistant_message,
                    "streamedTextPreview": streamed_text.chars().take(200).collect::<String>(),
                }),
            );
            // #endregion
            if will_emit {
                emit_assistant_message(&self.event_tx, &node_id, outcome);
            }
            if let AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. }) = outcome {
                let _ = self.event_tx.send(ExecutionEvent::NodeCompleted {
                    node_id,
                    label,
                    output: output.clone(),
                });
            }
        }
        result
    }
}

fn maybe_send_node_start_events<A>(adapter: &AiInvocationAdapter<A>, request: &AgentRequest) {
    let mut lifecycle = adapter.lifecycle_by_node.lock();
    if lifecycle.get(&request.node_id).copied().unwrap_or(0) >= request.model_attempt {
        return;
    }
    lifecycle.insert(request.node_id.clone(), request.model_attempt);
    let _ = adapter.event_tx.send(ExecutionEvent::NodeQueued {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
    let _ = adapter.event_tx.send(ExecutionEvent::NodeStarted {
        node_id: request.node_id.clone(),
        label: request.node_label.clone(),
    });
}

fn finalize_stream_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    message_id: &str,
) {
    let _ = event_tx.send(ExecutionEvent::ChatMessageDelta {
        node_id: node_id.clone(),
        message_id: message_id.to_string(),
        delta: String::new(),
        finalize: true,
    });
}

fn assistant_message_for_outcome(outcome: &AgentTurnOutcome) -> Option<String> {
    match outcome {
        AgentTurnOutcome::Completed(success) => success.assistant_message.clone(),
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            assistant_message, ..
        }) => assistant_message.clone(),
        AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            assistant_message, ..
        }) => Some(assistant_message.clone()),
    }
}

fn should_emit_assistant_message(
    did_stream: bool,
    outcome: &AgentTurnOutcome,
    streamed_text: &str,
) -> bool {
    if !did_stream {
        return true;
    }
    let Some(content) = filter_tool_turn_assistant_message(assistant_message_for_outcome(outcome))
    else {
        return false;
    };
    let trimmed = content.trim();
    !trimmed.is_empty() && !streamed_text.contains(trimmed)
}

fn emit_assistant_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &str,
    outcome: &AgentTurnOutcome,
) {
    let message = assistant_message_for_outcome(outcome);
    let filtered = filter_tool_turn_assistant_message(message.clone());
    // #region agent log
    agent_debug_log(
        "H2",
        "ai_adapter.rs:emit_assistant_message",
        "assistant message emit decision",
        json!({
            "nodeId": node_id,
            "rawMessage": message,
            "filteredMessage": filtered,
            "willEmit": filtered.as_ref().is_some_and(|value| !value.trim().is_empty()),
        }),
    );
    // #endregion
    if let Some(content) = filtered.filter(|value| !value.trim().is_empty()) {
        let _ = event_tx.send(ExecutionEvent::ChatMessage {
            node_id: NodeId(node_id.to_string()),
            role: ChatRole::Assistant,
            content,
        });
    }
}
