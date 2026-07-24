use super::{
    looks_like_narrated_file_mutation, InteractiveEngine, PendingToolBatch, RunError,
    AUTONOMOUS_CONTINUE_FEEDBACK, EMPTY_AFTER_NARRATED_WRITE_FEEDBACK,
    EMPTY_AFTER_WRITE_EXPAND_FEEDBACK, EMPTY_TURN_HUMAN_HANDOFF, EXPAND_VIA_EDIT_FEEDBACK,
    INCOMPLETE_WRITE_FEEDBACK, INTERACTIVE_CONTINUE_FEEDBACK, MALFORMED_REQUEST_INPUT_FEEDBACK,
    MAX_AUTO_CONTINUE_STREAK, MAX_EMPTY_PROVIDER_TURN_RETRIES, MAX_MALFORMED_REQUEST_INPUT_RETRIES,
    MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES, MAX_MIXED_TOOL_TURN_RETRIES, NARRATED_WRITE_FEEDBACK,
    PLAN_EXPAND_VIA_EDIT_FEEDBACK, PLAN_NARRATED_WRITE_FEEDBACK, TEXT_STREAK_HUMAN_HANDOFF,
};
use crate::conversation::{
    filter_tool_turn_assistant_message, is_clarifying_question, AgentReasoning, AgentTranscriptItem,
};
use crate::execution::tool_results::{denied_tool_result, error_tool_result};
use crate::execution::NodeFailureKind;
use crate::graph::{apply_runtime_patch_to_tool_config, runtime_patch_for, NodeId, RetryPolicy};
use crate::ports::{
    AgentError, AgentNeedUserInput, AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess,
    ToolAccessPolicy, UsageReport,
};
use crate::tools::{
    is_plan_draft_mutation_call, relativize_tool_call_arguments, tool_access_policy_allows_call,
    tool_decision_for_call, NodeToolConfig, ToolCall, ToolDecision, WRITE_PLAN_ARTIFACT_TOOL,
};
use serde_json::Value;
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
                if self.is_plan_mode_source_during_planning(node_id)
                    && !self.has_committed_plan_artifact(node_id)
                {
                    self.transcripts.entry(node_id.clone()).or_default().push(
                        AgentTranscriptItem::UserMessage {
                            content: "Plan Mode cannot freeze this node yet. Build or revise \
                                      run://PLAN.md, then call openflow_write_plan_artifact. The \
                                      seal request must receive explicit human approval before \
                                      openflow_submit_node_output."
                                .to_string(),
                        },
                    );
                    return;
                }
                self.apply_completion(node_id, success);
            }
            Ok(AgentTurnOutcome::ToolCalls(batch)) => {
                self.apply_tool_calls(node_id, batch);
            }
            Ok(AgentTurnOutcome::Message(message)) => {
                // Plain text never pauses — only openflow_request_user_input does.
                // Conversational nodes get the interactive nudge; autonomous get the
                // no-human-available nudge.
                self.handle_text_only_turn(
                    node_id,
                    &message.assistant_message,
                    &message.reasoning,
                    message.usage.as_ref(),
                );
            }
            Ok(AgentTurnOutcome::NeedsUserInput(input)) => {
                if self.interaction_mode(node_id) == InteractionMode::Autonomous {
                    self.handle_text_only_turn(
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

        let content = format!(
            "Your last response mixed harness and executable tools ({tool_names}) and was rejected; no calls from that response were executed. Call either exactly one harness tool by itself (openflow_submit_node_output when complete, or openflow_request_user_input with one direct question), or one or more executable tools with no harness tools in the same batch."
        );
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

        let conversational = self.interaction_mode(node_id) == InteractionMode::Conversational;
        let planning = self.is_plan_mode_source_during_planning(node_id);
        let write_nudge = self.should_nudge_write(node_id);
        let continue_feedback = if planning && self.recent_successful_write(node_id) {
            PLAN_EXPAND_VIA_EDIT_FEEDBACK
        } else if planning && write_nudge {
            PLAN_NARRATED_WRITE_FEEDBACK
        } else if self.recent_successful_write(node_id) {
            EMPTY_AFTER_WRITE_EXPAND_FEEDBACK
        } else if write_nudge {
            EMPTY_AFTER_NARRATED_WRITE_FEEDBACK
        } else if conversational {
            INTERACTIVE_CONTINUE_FEEDBACK
        } else {
            AUTONOMOUS_CONTINUE_FEEDBACK
        };

        let retry_count = self
            .empty_turn_retries_by_node
            .entry(node_id.clone())
            .or_default();

        if *retry_count >= MAX_EMPTY_PROVIDER_TURN_RETRIES {
            // Conversational nodes: hand off to the human instead of hard-failing a flaky model.
            if conversational {
                self.apply_conversational_turn(
                    node_id,
                    EMPTY_TURN_HUMAN_HANDOFF.to_string(),
                    Vec::new(),
                    None,
                );
                return true;
            }
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

    /// True when recent transcript shows the model intended to write a file.
    fn should_nudge_write(&self, node_id: &NodeId) -> bool {
        let Some(transcript) = self.transcripts.get(node_id) else {
            return false;
        };
        for item in transcript.iter().rev().take(8) {
            match item {
                AgentTranscriptItem::AssistantMessage { content }
                    if looks_like_narrated_file_mutation(content) =>
                {
                    return true;
                }
                AgentTranscriptItem::UserMessage { content }
                    if content.contains("narrated creating or updating a file")
                        || content.contains("already intended to write a file")
                        || content.contains("A write already succeeded") =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// True when a successful `write` tool result appears in the recent transcript.
    fn recent_successful_write(&self, node_id: &NodeId) -> bool {
        let Some(transcript) = self.transcripts.get(node_id) else {
            return false;
        };
        for item in transcript.iter().rev().take(40) {
            if let AgentTranscriptItem::ToolResult { result } = item {
                if result.tool_name == "write" && !result.is_error {
                    return true;
                }
            }
        }
        false
    }

    fn has_committed_plan_artifact(&self, node_id: &NodeId) -> bool {
        self.transcripts.get(node_id).is_some_and(|transcript| {
            transcript.iter().any(|item| {
                matches!(
                    item,
                    AgentTranscriptItem::ToolResult { result }
                        if result.tool_name == WRITE_PLAN_ARTIFACT_TOOL
                            && !result.is_error
                            && !result.artifact_ids.is_empty()
                )
            })
        })
    }

    /// Text-only turn (or an autonomous explicit input request). Nudge forward
    /// instead of pausing; fail the node after too many consecutive turns
    /// without tool-call progress. Human pauses require
    /// `openflow_request_user_input` on conversational nodes.
    fn handle_text_only_turn(
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
        let conversational = self.interaction_mode(node_id) == InteractionMode::Conversational;
        let planning = self.is_plan_mode_source_during_planning(node_id);
        let expand_via_edit = self.recent_successful_write(node_id);
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
            .get(node_id)
            .copied()
            .unwrap_or(0);
        if streak >= MAX_AUTO_CONTINUE_STREAK {
            if conversational {
                self.apply_conversational_turn(
                    node_id,
                    TEXT_STREAK_HUMAN_HANDOFF.to_string(),
                    Vec::new(),
                    None,
                );
                return;
            }
            self.failed_nodes.insert(
                node_id.clone(),
                format!(
                    "node produced {MAX_AUTO_CONTINUE_STREAK} consecutive turns without a \
                     tool call while user input is disabled"
                ),
            );
            return;
        }
        *self
            .auto_continue_streaks_by_node
            .entry(node_id.clone())
            .or_default() += 1;
        let feedback = if looks_like_narrated_file_mutation(assistant_message) {
            if planning && expand_via_edit {
                PLAN_EXPAND_VIA_EDIT_FEEDBACK
            } else if planning {
                PLAN_NARRATED_WRITE_FEEDBACK
            } else if expand_via_edit {
                EXPAND_VIA_EDIT_FEEDBACK
            } else {
                NARRATED_WRITE_FEEDBACK
            }
        } else if conversational {
            INTERACTIVE_CONTINUE_FEEDBACK
        } else {
            AUTONOMOUS_CONTINUE_FEEDBACK
        };
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::UserMessage {
                content: feedback.to_string(),
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

        let config = self.effective_tool_config(node_id);
        let policy = if self.is_plan_mode_active() {
            ToolAccessPolicy::Planning
        } else {
            ToolAccessPolicy::Execution
        };
        let can_manage_plan = self.is_plan_mode_source_during_planning(node_id);
        let root = self.project_repository_root.as_deref();
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
        record_tool_turn_context(transcript, &batch.reasoning, batch.assistant_message);
        let mut pending_calls = Vec::new();
        let mut requires_approval_for_batch = false;
        let mut incomplete_write = false;
        for mut call in batch.tool_calls {
            call.arguments = relativize_tool_call_arguments(call.arguments, root);
            transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
            if write_call_missing_content(&call) {
                incomplete_write = true;
                transcript.push(AgentTranscriptItem::ToolResult {
                    result: error_tool_result(
                        &call,
                        "[invalid_args] write: missing field `content` — required fields: path (string), content (string). For large docs, write a small stub then edit in chunks.",
                    ),
                });
                continue;
            }
            let is_plan_draft_mutation = is_plan_draft_mutation_call(&call);
            if matches!(policy, ToolAccessPolicy::Planning)
                && (call.name == WRITE_PLAN_ARTIFACT_TOOL || is_plan_draft_mutation)
                && !can_manage_plan
            {
                transcript.push(AgentTranscriptItem::ToolResult {
                    result: denied_tool_result(
                        &call,
                        Some(
                            "only the selected Plan Mode evidence source node may mutate or seal \
                             run://PLAN.md",
                        ),
                    ),
                });
                continue;
            }
            if !tool_access_policy_allows_call(policy, &config, &call) {
                let deny_reason = if matches!(policy, ToolAccessPolicy::Planning) {
                    planning_unavailable_tool_reason(&call)
                } else {
                    "blocked by Plan Mode"
                };
                transcript.push(AgentTranscriptItem::ToolResult {
                    result: denied_tool_result(&call, Some(deny_reason)),
                });
                continue;
            }
            if matches!(policy, ToolAccessPolicy::Planning) && is_plan_draft_mutation {
                pending_calls.push(call);
                continue;
            }
            if matches!(policy, ToolAccessPolicy::Planning) && call.name == WRITE_PLAN_ARTIFACT_TOOL
            {
                requires_approval_for_batch = true;
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
        self.finish_tool_call_batch(
            node_id,
            pending_calls,
            requires_approval_for_batch,
            incomplete_write,
            policy,
        );
    }

    fn effective_tool_config(&self, node_id: &NodeId) -> NodeToolConfig {
        let mut config = self
            .find_node(node_id)
            .map(|node| node.agent.tools.clone())
            .unwrap_or_default();
        if let Some(patch) = self
            .runtime_config_store
            .as_ref()
            .and_then(|store| runtime_patch_for(store, node_id))
        {
            apply_runtime_patch_to_tool_config(&mut config, &patch);
        }
        config
    }

    fn finish_tool_call_batch(
        &mut self,
        node_id: &NodeId,
        tool_calls: Vec<ToolCall>,
        requires_approval: bool,
        incomplete_write: bool,
        tool_access_policy: ToolAccessPolicy,
    ) {
        if incomplete_write {
            self.transcripts.entry(node_id.clone()).or_default().push(
                AgentTranscriptItem::UserMessage {
                    content: INCOMPLETE_WRITE_FEEDBACK.to_string(),
                },
            );
        }
        if tool_calls.is_empty() {
            return;
        }
        let approval_id = Uuid::new_v4().to_string();
        self.pending_tool_batches.insert(
            approval_id.clone(),
            PendingToolBatch {
                approval_id,
                node_id: node_id.clone(),
                tool_calls,
                requires_approval,
                tool_access_policy,
            },
        );
    }

    fn apply_user_input_request(&mut self, node_id: &NodeId, input: AgentNeedUserInput) {
        self.apply_conversational_turn(node_id, input.assistant_message, input.reasoning, None);
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

/// `write` requires a non-null `content` field (empty string is allowed).
fn write_call_missing_content(call: &ToolCall) -> bool {
    call.name == "write" && call.arguments.get("content").is_none_or(Value::is_null)
}

fn record_tool_turn_context(
    transcript: &mut Vec<AgentTranscriptItem>,
    reasoning: &[AgentReasoning],
    assistant_message: Option<String>,
) {
    transcript.extend(
        reasoning
            .iter()
            .cloned()
            .map(|reasoning| AgentTranscriptItem::Reasoning { reasoning }),
    );
    if let Some(message) = filter_tool_turn_assistant_message(assistant_message)
        .filter(|message| !message.trim().is_empty())
    {
        transcript.push(AgentTranscriptItem::AssistantMessage { content: message });
    }
}

/// Planning deny copy: bash is not offered, but models still invent it from the preamble.
fn planning_unavailable_tool_reason(call: &ToolCall) -> &'static str {
    if call.name == "bash" {
        "bash is not available during Plan Mode planning. Do not retry bash. Call write with both \
         path and content at run://PLAN.md for the run-local plan draft."
    } else if matches!(call.name.as_str(), "write" | "edit") {
        "blocked by Plan Mode — use write/edit on run://PLAN.md for the run-local plan draft; \
         repository docs/**/*.md writes require a write-enabled node"
    } else {
        "blocked by Plan Mode — use run://PLAN.md for the run-local plan draft; bash, MCP, \
         subagents, and non-docs repository writes are denied"
    }
}

/// Spread sibling retry times so they do not hit the provider in the same tick.
fn retry_jitter(node_id: &NodeId) -> Duration {
    let hash = node_id.0.bytes().fold(0u64, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u64::from(byte))
    });
    Duration::from_millis(100 + (hash % 400))
}
