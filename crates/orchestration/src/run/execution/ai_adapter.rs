use super::timing::emit_phase_timed;
use super::ExecutionEvent;
use async_trait::async_trait;
use engine::{
    filter_tool_turn_assistant_message, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, AiPort, AiStreamEvent, AiStreamSink,
    ChatRole, NodeId,
};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

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
}

impl AiStreamSink for StreamSink {
    fn on_stream_event(&self, event: AiStreamEvent) {
        let AiStreamEvent::AssistantDelta { content } = event;
        if content.is_empty() {
            return;
        }
        self.streamed.store(true, Ordering::Relaxed);
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
        let sink = StreamSink {
            event_tx: self.event_tx.clone(),
            node_id: node_id.clone(),
            message_id: message_id.clone(),
            streamed: Arc::clone(&streamed),
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
        if streamed.load(Ordering::Relaxed) {
            finalize_stream_message(&self.event_tx, &node_id, &message_id);
        }
        if let Ok(outcome) = &result {
            if !streamed.load(Ordering::Relaxed) {
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

fn emit_assistant_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &str,
    outcome: &AgentTurnOutcome,
) {
    let message = match outcome {
        AgentTurnOutcome::Completed(success) => success.assistant_message.clone(),
        AgentTurnOutcome::ToolCalls(AgentToolCallBatch {
            assistant_message, ..
        }) => assistant_message.clone(),
        AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
            assistant_message, ..
        }) => Some(assistant_message.clone()),
    };
    let message = filter_tool_turn_assistant_message(message);
    if let Some(content) = message.filter(|value| !value.trim().is_empty()) {
        let _ = event_tx.send(ExecutionEvent::ChatMessage {
            node_id: NodeId(node_id.to_string()),
            role: ChatRole::Assistant,
            content,
        });
    }
}
