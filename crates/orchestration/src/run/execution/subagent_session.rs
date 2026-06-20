//! Subagent AI-invocation loop extracted from [`super::tool_port::ToolPortImpl`].

use std::sync::Arc;
use std::time::Instant;

use engine::{
    advance_subagent_invoke, is_subagent_runtime_builtin, start_subagent_invoke,
    subagent_runtime_builtin_denied, AgentToolCallBatch, AgentTurnOutcome, InteractiveEngine,
    NodeId, NodeToolConfig, SubagentInvokeStep, SubagentStartOutcome, ToolCall, ToolResult,
};

use crate::run::execution::timing::emit_phase_timed;

use super::{abort_run, send_run_telemetry, ToolPortImpl};

impl<A> ToolPortImpl<A>
where
    A: engine::AiPort + Send + Sync + 'static,
{
    pub(super) async fn run_call_subagent(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        label: &str,
        tool_call: &ToolCall,
        node_config: &NodeToolConfig,
    ) -> Option<ToolResult> {
        self.propose_tool_call(node_id, label, tool_call);
        let available_tools = self
            .tool_runner
            .registry()
            .definitions_for_subagent(node_config);
        let (mut session, startup_telemetry) = {
            let mut declared = self.declared_subagents.lock();
            match start_subagent_invoke(
                &self.workflow,
                node_id,
                tool_call,
                &mut declared,
                &self.agent_snapshots,
                available_tools,
            ) {
                SubagentStartOutcome::Started(session, telemetry) => (*session, telemetry),
                SubagentStartOutcome::Failed(tool_result) => {
                    self.emit_tool_started(node_id, tool_call);
                    self.emit_tool_completed(node_id, tool_call, &tool_result);
                    return Some(tool_result);
                }
            }
        };
        send_run_telemetry(&self.event_tx, startup_telemetry);
        self.emit_tool_started(node_id, tool_call);

        let conversation_id = format!("subagent:{}", uuid::Uuid::new_v4());
        let mut outcome = self.invoke_ai_or_cancel(session.request.clone()).await?;
        loop {
            let tool_results = if let Ok(AgentTurnOutcome::ToolCalls(batch)) = &outcome {
                self.execute_subagent_tool_batch(engine, node_id, &conversation_id, batch)
                    .await?
            } else {
                Vec::new()
            };
            match advance_subagent_invoke(session, outcome, tool_results) {
                SubagentInvokeStep::NeedAi(next_session) => {
                    session = next_session;
                    outcome = self.invoke_ai_or_cancel(session.request.clone()).await?;
                }
                SubagentInvokeStep::Done {
                    tool_result,
                    subagent,
                    telemetry,
                } => {
                    self.declared_subagents
                        .lock()
                        .insert(subagent.id.clone(), subagent);
                    send_run_telemetry(&self.event_tx, telemetry);
                    self.emit_tool_completed(node_id, tool_call, &tool_result);
                    return Some(tool_result);
                }
            }
        }
    }

    pub(super) async fn execute_subagent_tool_batch(
        &self,
        engine: &mut InteractiveEngine,
        node_id: &NodeId,
        conversation_id: &str,
        batch: &AgentToolCallBatch,
    ) -> Option<Vec<ToolResult>> {
        let mut results = Vec::new();
        for tool_call in &batch.tool_calls {
            if is_subagent_runtime_builtin(&tool_call.name) {
                results.push(subagent_runtime_builtin_denied(tool_call));
                continue;
            }
            match self
                .execute_tool_or_cancel(engine, tool_call.clone(), node_id, conversation_id)
                .await
            {
                Some(Ok(record)) => {
                    self.record_tool_file_changes(engine, node_id, &record);
                    results.push(record.result);
                }
                Some(Err(err)) => results.push(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    content: err.to_string(),
                    is_error: true,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                }),
                None => return None,
            }
        }
        Some(results)
    }

    pub(super) async fn invoke_ai_or_cancel(
        &self,
        mut request: engine::AgentRequest,
    ) -> Option<Result<AgentTurnOutcome, engine::AgentError>> {
        let ai = Arc::clone(&self.ai);
        let label = format!("subagent · {}", request.node_label);
        let node_id = request.node_id.clone();
        let policy = self.workflow.settings.retry_policy.clone();
        let cancel_token = self.cancel_token.clone();
        let mut retry_count: u8 = 0;

        loop {
            if cancel_token.is_cancelled() {
                abort_run(&self.event_tx, &self.aborted_emitted);
                return None;
            }

            request.model_attempt = retry_count.saturating_add(1);
            let started = Instant::now();
            let result = tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    abort_run(&self.event_tx, &self.aborted_emitted);
                    None
                }
                result = ai.invoke(request.clone()) => Some(result),
            };

            let result = result?;

            match result {
                Ok(outcome) => {
                    emit_phase_timed(&self.event_tx, "ai_invoke", &label, Some(node_id), started);
                    return Some(Ok(outcome));
                }
                Err(err) if err.is_retryable() && retry_count < policy.max_attempts => {
                    retry_count += 1;
                    let delay = policy.delay_for_attempt(retry_count);
                    tokio::select! {
                        biased;
                        () = cancel_token.cancelled() => {
                            abort_run(&self.event_tx, &self.aborted_emitted);
                            return None;
                        }
                        () = tokio::time::sleep(delay) => {}
                    }
                }
                Err(err) => {
                    emit_phase_timed(&self.event_tx, "ai_invoke", &label, Some(node_id), started);
                    return Some(Err(err));
                }
            }
        }
    }
}
