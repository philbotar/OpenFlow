use super::{
    InteractiveEngine, PendingToolBatch, RunError, AUTONOMOUS_CONTINUE_FEEDBACK,
    INTERACTIVE_CONTINUE_FEEDBACK, MALFORMED_REQUEST_INPUT_FEEDBACK, MAX_AUTO_CONTINUE_STREAK,
    MAX_EMPTY_PROVIDER_TURN_RETRIES, MAX_MALFORMED_REQUEST_INPUT_RETRIES,
    MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES, MAX_MIXED_TOOL_TURN_RETRIES,
};
use crate::conversation::{
    filter_tool_turn_assistant_message, is_clarifying_question, AgentReasoning, AgentTranscriptItem,
};
use crate::execution::tool_results::denied_tool_result;
use crate::execution::NodeFailureKind;
use crate::graph::{apply_runtime_patch_to_tool_config, runtime_patch_for, NodeId, RetryPolicy};
use crate::ports::{
    AgentContinueWork, AgentError, AgentMessageTurn, AgentNeedUserInput, AgentToolCallBatch,
    AgentTurnOutcome, AgentTurnSuccess, ToolAccessPolicy, UsageReport,
};
use crate::tools::{
    relativize_tool_call_arguments, tool_access_policy_allows_call, tool_decision_for_call,
    ToolDecision, WRITE_PLAN_ARTIFACT_TOOL,
};
use std::time::Duration;
use tokio::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractionMode {
    Autonomous,
    Conversational,
}

impl InteractiveEngine {
    pub fn on_ai_complete(
        &mut self,
        node_id: &NodeId,
        result: Result<AgentTurnOutcome, AgentError>,
    ) {
        if !self.in_flight_ai.remove(node_id) {
            let message = self.in_flight_ai.iter().next().map_or_else(
                || "no node is awaiting model completion".to_string(),
                |expected| format!("expected model completion for {expected}, got {node_id}"),
            );
            self.reject_misrouted_completion(node_id, message);
            return;
        }

        match result {
            Ok(AgentTurnOutcome::Completed(success)) => {
                self.apply_completion(node_id, success);
            }
            Ok(AgentTurnOutcome::ToolCalls(batch)) => {
                self.apply_tool_calls(node_id, batch);
            }
            Ok(AgentTurnOutcome::ContinueWork(continuation)) => {
                self.apply_continue_work(node_id, continuation);
            }
            Ok(AgentTurnOutcome::Message(message)) => {
                self.work_phase_nodes.remove(node_id);
                if self.interaction_mode(node_id) == InteractionMode::Conversational {
                    self.apply_conversational_message(node_id, message);
                } else {
                    self.handle_autonomous_text_turn(
                        node_id,
                        &message.assistant_message,
                        &message.reasoning,
                        message.usage.as_ref(),
                    );
                }
            }
            Ok(AgentTurnOutcome::NeedsUserInput(input)) => {
                self.work_phase_nodes.remove(node_id);
                if self.interaction_mode(node_id) == InteractionMode::Autonomous {
                    self.handle_autonomous_text_turn(
                        node_id,
                        &input.assistant_message,
                        &input.reasoning,
                        None,
                    );
                    return;
                }
                let retried = self.handle_malformed_request_input_retry(node_id, &input);
                if retried {
                    return;
                }
                self.apply_user_input_request(node_id, input);
            }
            Err(error) => {
                if error.is_interrupted() {
                    self.handle_interrupted(node_id.clone());
                    return;
                }
                if self.handle_malformed_submit_output_retry(node_id, &error) {
                    return;
                }
                if self.handle_mixed_tool_turn_retry(node_id, &error) {
                    return;
                }
                if self.handle_empty_provider_turn_retry(node_id, &error) {
                    return;
                }
                if self.handle_transient_retry(node_id, &error) {
                    return;
                }
                self.fail_node(node_id, &error);
            }
        }
    }

    fn handle_interrupted(&mut self, node_id: NodeId) {
        self.interrupted_nodes.insert(node_id);
    }

    fn interaction_mode(&self, node_id: &NodeId) -> InteractionMode {
        if self.is_plan_mode_source_during_planning(node_id)
            || self
                .find_node(node_id)
                .is_some_and(|node| node.agent.request_user_input)
        {
            InteractionMode::Conversational
        } else {
            InteractionMode::Autonomous
        }
    }

    fn handle_malformed_submit_output_retry(
        &mut self,
        node_id: &NodeId,
        error: &AgentError,
    ) -> bool {
        if !error.is_malformed_submit_output() {
            return false;
        }

        let schema_hint = self.find_node(node_id).map_or_else(
            || "see the node output schema".to_string(),
            |node| {
                serde_json::to_string_pretty(&node.agent.output_schema)
                    .unwrap_or_else(|_| node.agent.output_schema.to_string())
            },
        );

        let retry_count = self
            .submit_output_retries_by_node
            .entry(node_id.clone())
            .or_default();

        if *retry_count >= MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES {
            return false;
        }

        *retry_count += 1;

        self.transcripts
            .entry(node_id.clone())
            .or_default()
            .push(AgentTranscriptItem::UserMessage {
                content: format!(
                    "Your openflow_submit_node_output call was invalid ({error}). \
                     Call openflow_submit_node_output again with arguments shaped as \
                     {{\"output\": <object matching the node output schema>, \"assistant_message\": null}}. \
                     Put schema fields under \"output\", not at the top level. \
                     Node output schema: {schema_hint}"
                ),
            });
        true
    }

    fn handle_malformed_request_input_retry(
        &mut self,
        node_id: &NodeId,
        input: &AgentNeedUserInput,
    ) -> bool {
        if is_clarifying_question(&input.assistant_message) {
            return false;
        }

        let retry_count = self
            .request_input_retries_by_node
            .entry(node_id.clone())
            .or_default();

        if *retry_count >= MAX_MALFORMED_REQUEST_INPUT_RETRIES {
            self.failed_nodes.insert(
                node_id.clone(),
                format!(
                    "node produced more than {MAX_MALFORMED_REQUEST_INPUT_RETRIES} invalid \
                     human-input requests without a direct question"
                ),
            );
            return true;
        }

        *retry_count += 1;
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::UserMessage {
                content: MALFORMED_REQUEST_INPUT_FEEDBACK.to_string(),
            },
        );
        true
    }

    fn handle_mixed_tool_turn_retry(&mut self, node_id: &NodeId, error: &AgentError) -> bool {
        let Some(tool_names) = error.mixed_tool_names() else {
            return false;
        };

        let retry_count = self
            .mixed_tool_turn_retries_by_node
            .entry(node_id.clone())
            .or_default();
        if *retry_count >= MAX_MIXED_TOOL_TURN_RETRIES {
            return false;
        }
        *retry_count += 1;

        let content = if self.work_phase_nodes.contains(node_id) {
            format!(
                "Your last response used control tools during a work turn ({tool_names}) and was rejected; no calls from that response were executed. Stay on this work turn and call only executable tools. After the tool batch finishes, OpenFlow returns to a control turn for openflow_continue_work or openflow_submit_node_output."
            )
        } else {
            format!(
                "Your last response mixed control and executable tools ({tool_names}) and was rejected; no calls from that response were executed. Emit exactly one tool call in this control turn. If more work is required, call openflow_continue_work alone. If the task is complete, call openflow_submit_node_output alone. For large artifacts, submit a repository-relative path and compact metadata instead of the full document."
            )
        };
        self.transcripts
            .entry(node_id.clone())
            .or_default()
            .push(AgentTranscriptItem::UserMessage { content });
        true
    }

    fn handle_empty_provider_turn_retry(&mut self, node_id: &NodeId, error: &AgentError) -> bool {
        if !error.is_empty_provider_turn() {
            return false;
        }

        let continue_feedback = if self
            .find_node(node_id)
            .is_none_or(|node| node.agent.request_user_input)
        {
            INTERACTIVE_CONTINUE_FEEDBACK
        } else {
            AUTONOMOUS_CONTINUE_FEEDBACK
        };

        let retry_count = self
            .empty_turn_retries_by_node
            .entry(node_id.clone())
            .or_default();

        if *retry_count >= MAX_EMPTY_PROVIDER_TURN_RETRIES {
            return false;
        }

        *retry_count += 1;
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::UserMessage {
                content: continue_feedback.to_string(),
            },
        );
        true
    }

    /// A node with user input disabled produced a text-only turn (or an
    /// explicit input request). Nudge it forward instead of pausing; fail the
    /// node after too many consecutive turns without tool-call progress.
    fn handle_autonomous_text_turn(
        &mut self,
        node_id: &NodeId,
        assistant_message: &str,
        reasoning: &[AgentReasoning],
        usage: Option<&UsageReport>,
    ) {
        self.transient_streaks_by_node.remove(node_id);
        self.empty_turn_retries_by_node.remove(node_id);
        self.request_input_retries_by_node.remove(node_id);
        self.submit_output_retries_by_node.remove(node_id);
        if let Some(usage) = usage {
            self.note_usage(usage);
        }
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
        for item in reasoning {
            transcript.push(AgentTranscriptItem::Reasoning {
                reasoning: item.clone(),
            });
        }
        let narration = assistant_message.trim().to_string();
        if !narration.is_empty() {
            transcript.push(AgentTranscriptItem::AssistantMessage { content: narration });
        }
        let streak = self
            .auto_continue_streaks_by_node
            .entry(node_id.clone())
            .or_default();
        if *streak >= MAX_AUTO_CONTINUE_STREAK {
            self.failed_nodes.insert(
                node_id.clone(),
                format!(
                    "node produced {MAX_AUTO_CONTINUE_STREAK} consecutive turns without a \
                     tool call while user input is disabled"
                ),
            );
            return;
        }
        *streak += 1;
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::UserMessage {
                content: AUTONOMOUS_CONTINUE_FEEDBACK.to_string(),
            },
        );
    }

    fn handle_transient_retry(&mut self, node_id: &NodeId, error: &AgentError) -> bool {
        if !error.is_retryable() {
            return false;
        }
        let streak = self
            .transient_streaks_by_node
            .entry(node_id.clone())
            .or_default();
        let Some(delay) = next_retry(&self.workflow.settings.retry_policy, streak) else {
            return false;
        };
        // retries_by_node stays monotonic: model_attempt must only grow so the
        // AI adapter mints a fresh cancellation token per attempt.
        let attempts = self.retries_by_node.entry(node_id.clone()).or_default();
        *attempts = attempts.saturating_add(1);
        self.retry_after_by_node.insert(
            node_id.clone(),
            Instant::now() + delay + retry_jitter(node_id),
        );
        true
    }

    fn fail_node(&mut self, node_id: &NodeId, error: &AgentError) {
        self.retry_after_by_node.remove(node_id);
        self.failed_nodes.insert(node_id.clone(), error.to_string());
    }

    fn apply_completion(&mut self, node_id: &NodeId, success: AgentTurnSuccess) {
        self.work_phase_nodes.remove(node_id);
        self.retry_after_by_node.remove(node_id);
        self.transient_streaks_by_node.remove(node_id);
        self.reset_protocol_recovery(node_id);
        if let Some(usage) = &success.usage {
            self.note_usage(usage);
        }
        if let Some(message) = filter_tool_turn_assistant_message(success.assistant_message)
            .filter(|message| !message.trim().is_empty())
        {
            self.transcripts
                .entry(node_id.clone())
                .or_default()
                .push(AgentTranscriptItem::AssistantMessage { content: message });
        }
        self.outputs.insert(node_id.clone(), success.output.clone());
        if self.plan_mode_source_node_id.as_ref() == Some(node_id)
            && self.frozen_change_evidence_packet.is_none()
        {
            self.frozen_change_evidence_packet = Some(super::FrozenChangeEvidencePacket::new(
                node_id.clone(),
                success.output,
            ));
        }
    }

    fn apply_tool_calls(&mut self, node_id: &NodeId, batch: AgentToolCallBatch) {
        self.work_phase_nodes.remove(node_id);
        self.transient_streaks_by_node.remove(node_id);
        self.reset_protocol_recovery(node_id);
        if let Some(usage) = &batch.usage {
            self.note_usage(usage);
        }
        if self.find_node(node_id).is_none() {
            self.terminal_error = Some(RunError::NodeFailed {
                node_id: node_id.clone(),
                kind: NodeFailureKind::ToolCallNodeNotFound,
            });
            return;
        }

        let mut config = self
            .find_node(node_id)
            .map(|node| node.agent.tools.clone())
            .unwrap_or_default();
        if let Some(store) = &self.runtime_config_store {
            if let Some(patch) = runtime_patch_for(store, node_id) {
                apply_runtime_patch_to_tool_config(&mut config, &patch);
            }
        }
        let policy = if self.is_plan_mode_active() {
            ToolAccessPolicy::Planning
        } else {
            ToolAccessPolicy::Execution
        };
        let root = self.project_repository_root.as_deref();
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
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
        let mut pending_calls = Vec::new();
        let mut requires_approval_for_batch = false;
        for mut call in batch.tool_calls {
            call.arguments = relativize_tool_call_arguments(call.arguments, root);
            transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
            if !tool_access_policy_allows_call(policy, &config, &call) {
                transcript.push(AgentTranscriptItem::ToolResult {
                    result: denied_tool_result(&call, Some("blocked by Plan Mode")),
                });
                continue;
            }
            if matches!(policy, ToolAccessPolicy::Planning) && call.name == WRITE_PLAN_ARTIFACT_TOOL
            {
                pending_calls.push(call);
                continue;
            }
            match tool_decision_for_call(&config, &call) {
                ToolDecision::AutoAllow => pending_calls.push(call),
                ToolDecision::Prompt => {
                    requires_approval_for_batch = true;
                    pending_calls.push(call);
                }
                ToolDecision::Deny => {
                    transcript.push(AgentTranscriptItem::ToolResult {
                        result: denied_tool_result(&call, Some("denied by policy")),
                    });
                }
            }
        }
        if pending_calls.is_empty() {
            return;
        }
        let approval_id = Uuid::new_v4().to_string();
        self.pending_tool_batches.insert(
            approval_id.clone(),
            PendingToolBatch {
                approval_id,
                node_id: node_id.clone(),
                tool_calls: pending_calls,
                requires_approval: requires_approval_for_batch,
                tool_access_policy: policy,
            },
        );
    }

    fn apply_user_input_request(&mut self, node_id: &NodeId, input: AgentNeedUserInput) {
        self.apply_conversational_turn(node_id, input.assistant_message, input.reasoning, None);
    }

    fn apply_continue_work(&mut self, node_id: &NodeId, continuation: AgentContinueWork) {
        self.transient_streaks_by_node.remove(node_id);
        self.reset_protocol_recovery(node_id);
        if let Some(usage) = &continuation.usage {
            self.note_usage(usage);
        }
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
        for reasoning in continuation.reasoning {
            transcript.push(AgentTranscriptItem::Reasoning { reasoning });
        }
        if let Some(content) = continuation
            .assistant_message
            .filter(|content| !content.trim().is_empty())
        {
            transcript.push(AgentTranscriptItem::AssistantMessage { content });
        }
        self.work_phase_nodes.insert(node_id.clone());
    }

    fn apply_conversational_message(&mut self, node_id: &NodeId, message: AgentMessageTurn) {
        self.apply_conversational_turn(
            node_id,
            message.assistant_message,
            message.reasoning,
            message.usage.as_ref(),
        );
    }

    fn apply_conversational_turn(
        &mut self,
        node_id: &NodeId,
        assistant_message: String,
        reasoning: Vec<AgentReasoning>,
        usage: Option<&UsageReport>,
    ) {
        self.transient_streaks_by_node.remove(node_id);
        self.reset_protocol_recovery(node_id);
        if let Some(usage) = usage {
            self.note_usage(usage);
        }
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
        for reasoning in reasoning {
            transcript.push(AgentTranscriptItem::Reasoning { reasoning });
        }
        transcript.push(AgentTranscriptItem::AssistantMessage {
            content: assistant_message,
        });
        self.awaiting_nodes.insert(node_id.clone());
    }

    fn reset_protocol_recovery(&mut self, node_id: &NodeId) {
        self.submit_output_retries_by_node.remove(node_id);
        self.request_input_retries_by_node.remove(node_id);
        self.empty_turn_retries_by_node.remove(node_id);
        self.mixed_tool_turn_retries_by_node.remove(node_id);
        self.auto_continue_streaks_by_node.remove(node_id);
    }
}

fn next_retry(policy: &RetryPolicy, retry_count: &mut u8) -> Option<Duration> {
    if *retry_count >= policy.max_attempts {
        return None;
    }
    *retry_count += 1;
    Some(policy.delay_for_attempt(*retry_count))
}

/// Spread sibling retry times so they do not hit the provider in the same tick.
fn retry_jitter(node_id: &NodeId) -> Duration {
    let hash = node_id.0.bytes().fold(0u64, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u64::from(byte))
    });
    Duration::from_millis(100 + (hash % 400))
}
