use super::{emit_phase_timed, send_or_log, ExecutionEvent, NodeInterrupts};
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
use tokio::sync::{mpsc::UnboundedSender, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct AiInvocationAdapter<A> {
    inner: Arc<A>,
    event_tx: UnboundedSender<ExecutionEvent>,
    lifecycle_by_node: Mutex<BTreeMap<NodeId, u8>>,
    node_interrupts: NodeInterrupts,
    run_cancel_token: CancellationToken,
    context_window_sizes: BTreeMap<String, u32>,
    /// Limits in-flight provider HTTP calls per run (`None` = unlimited).
    invoke_slots: Option<Arc<Semaphore>>,
}

impl<A> AiInvocationAdapter<A>
where
    A: AiPort + Send + Sync + 'static,
{
    pub fn new(
        inner: Arc<A>,
        event_tx: UnboundedSender<ExecutionEvent>,
        node_interrupts: NodeInterrupts,
        run_cancel_token: CancellationToken,
        context_window_sizes: BTreeMap<String, u32>,
        max_concurrent_ai_calls: u8,
    ) -> Self {
        let invoke_slots = if max_concurrent_ai_calls == 0 {
            None
        } else {
            Some(Arc::new(Semaphore::new(usize::from(
                max_concurrent_ai_calls,
            ))))
        };
        Self {
            inner,
            event_tx,
            lifecycle_by_node: Mutex::new(BTreeMap::new()),
            node_interrupts,
            run_cancel_token,
            context_window_sizes,
            invoke_slots,
        }
    }

    fn node_token_for(&self, node_id: &NodeId, attempt: u8) -> CancellationToken {
        let mut registry = self.node_interrupts.lock();
        let needs_new = match registry.get(node_id) {
            Some((stored_attempt, _)) => *stored_attempt < attempt,
            None => true,
        };
        if needs_new {
            let token = self.run_cancel_token.child_token();
            registry.insert(node_id.clone(), (attempt, token.clone()));
            token
        } else {
            registry
                .get(node_id)
                .expect("token just inserted")
                .1
                .clone()
        }
    }
}

struct StreamSink {
    event_tx: UnboundedSender<ExecutionEvent>,
    node_id: NodeId,
    assistant_message_id: String,
    thinking_message_id: String,
    streamed: Arc<AtomicBool>,
    streamed_content: Arc<Mutex<String>>,
    thinking_streamed: Arc<AtomicBool>,
}

impl AiStreamSink for StreamSink {
    fn on_stream_event(&self, event: AiStreamEvent) {
        match event {
            AiStreamEvent::AssistantDelta { content } => {
                if content.is_empty() {
                    return;
                }
                self.streamed.store(true, Ordering::Relaxed);
                self.streamed_content.lock().push_str(&content);
                send_or_log(
                    &self.event_tx,
                    ExecutionEvent::ChatMessageDelta {
                        node_id: self.node_id.clone(),
                        message_id: self.assistant_message_id.clone(),
                        role: ChatRole::Assistant,
                        delta: content,
                        finalize: false,
                    },
                );
            }
            AiStreamEvent::ThinkingDelta { content } => {
                if content.is_empty() {
                    return;
                }
                self.thinking_streamed.store(true, Ordering::Relaxed);
                send_or_log(
                    &self.event_tx,
                    ExecutionEvent::ChatMessageDelta {
                        node_id: self.node_id.clone(),
                        message_id: self.thinking_message_id.clone(),
                        role: ChatRole::Thinking,
                        delta: content,
                        finalize: false,
                    },
                );
            }
        }
    }
}

#[async_trait]
impl<A> AiPort for AiInvocationAdapter<A>
where
    A: AiPort + Send + Sync + 'static,
{
    async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        maybe_send_node_start_events(self, &request);
        let node_id = request.node_id.clone();
        let label = request.node_label.clone();
        let model = request.model.clone();
        let attempt = request.model_attempt;
        let assistant_message_id = Uuid::new_v4().to_string();
        let thinking_message_id = Uuid::new_v4().to_string();
        let streamed = Arc::new(AtomicBool::new(false));
        let streamed_content = Arc::new(Mutex::new(String::new()));
        let thinking_streamed = Arc::new(AtomicBool::new(false));
        let sink = StreamSink {
            event_tx: self.event_tx.clone(),
            node_id: node_id.clone(),
            assistant_message_id: assistant_message_id.clone(),
            thinking_message_id: thinking_message_id.clone(),
            streamed: Arc::clone(&streamed),
            streamed_content: Arc::clone(&streamed_content),
            thinking_streamed: Arc::clone(&thinking_streamed),
        };
        let node_token = self.node_token_for(&node_id, attempt);
        let _invoke_slot = if let Some(sem) = &self.invoke_slots {
            Some(tokio::select! {
                biased;
                () = self.run_cancel_token.cancelled() => return Err(AgentError::Interrupted),
                () = node_token.cancelled() => return Err(AgentError::Interrupted),
                permit = sem.clone().acquire_owned() => permit.ok(),
            })
        } else {
            None
        };
        let started = Instant::now();
        let result = tokio::select! {
            biased;
            () = node_token.cancelled() => Err(AgentError::Interrupted),
            result = self.inner.invoke_stream(request, &sink) => result,
        };
        emit_phase_timed(
            &self.event_tx,
            "ai_invoke",
            &label,
            Some(node_id.clone()),
            started,
        );
        if let Err(ref error) = result {
            send_or_log(
                &self.event_tx,
                ExecutionEvent::AiInvokeFailed {
                    node_id: node_id.clone(),
                    label: label.clone(),
                    error: error.to_string(),
                },
            );
        }
        let did_stream = streamed.load(Ordering::Relaxed);
        if did_stream {
            finalize_stream_message(
                &self.event_tx,
                &node_id,
                &assistant_message_id,
                ChatRole::Assistant,
            );
        }
        if thinking_streamed.load(Ordering::Relaxed) {
            finalize_stream_message(
                &self.event_tx,
                &node_id,
                &thinking_message_id,
                ChatRole::Thinking,
            );
        }
        if let Ok(outcome) = &result {
            if should_emit_assistant_message(did_stream, outcome, &streamed_content.lock()) {
                emit_assistant_message(&self.event_tx, &node_id, outcome);
            }
            // Emit usage report if available
            let usage = match outcome {
                AgentTurnOutcome::Completed(s) => s.usage.clone(),
                AgentTurnOutcome::ToolCalls(b) => b.usage.clone(),
                AgentTurnOutcome::NeedsUserInput(_) => None,
            };
            if let Some(usage) = usage {
                let max_context_tokens =
                    crate::settings::lookup_context_window_size(&self.context_window_sizes, &model);
                send_or_log(
                    &self.event_tx,
                    ExecutionEvent::UsageReported {
                        node_id: node_id.clone(),
                        usage,
                        model,
                        max_context_tokens,
                    },
                );
            }
            if let AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. }) = outcome {
                send_or_log(
                    &self.event_tx,
                    ExecutionEvent::NodeCompleted {
                        node_id,
                        label,
                        output: output.clone(),
                    },
                );
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
    send_or_log(
        &adapter.event_tx,
        ExecutionEvent::NodeQueued {
            node_id: request.node_id.clone(),
            label: request.node_label.clone(),
        },
    );
    send_or_log(
        &adapter.event_tx,
        ExecutionEvent::NodeStarted {
            node_id: request.node_id.clone(),
            label: request.node_label.clone(),
        },
    );
}

fn finalize_stream_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    message_id: &str,
    role: ChatRole,
) {
    send_or_log(
        event_tx,
        ExecutionEvent::ChatMessageDelta {
            node_id: node_id.clone(),
            message_id: message_id.to_string(),
            role,
            delta: String::new(),
            finalize: true,
        },
    );
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
    let stripped_streamed =
        filter_tool_turn_assistant_message(Some(streamed_text.to_string())).unwrap_or_default();
    !trimmed.is_empty() && !stripped_streamed.contains(trimmed)
}

fn emit_assistant_message(
    event_tx: &UnboundedSender<ExecutionEvent>,
    node_id: &NodeId,
    outcome: &AgentTurnOutcome,
) {
    if let Some(content) =
        filter_tool_turn_assistant_message(assistant_message_for_outcome(outcome))
            .filter(|value| !value.trim().is_empty())
    {
        send_or_log(
            event_tx,
            ExecutionEvent::ChatMessage {
                node_id: node_id.clone(),
                role: ChatRole::Assistant,
                content,
            },
        );
    }
}
