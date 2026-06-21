use super::{
    InteractiveEngine, PendingToolBatch, RunError, RunEvent, RunEventKind,
    MALFORMED_REQUEST_INPUT_FEEDBACK, MAX_MALFORMED_REQUEST_INPUT_RETRIES,
    MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES,
};
use crate::conversation::{
    filter_tool_turn_assistant_message, is_clarifying_question, AgentTranscriptItem,
};
use crate::execution::retry::{next_retry, retrying_event};
use crate::execution::tool_results::denied_tool_result;
use crate::execution::NodeFailureKind;
use crate::graph::NodeId;
use crate::ports::{
    AgentError, AgentNeedUserInput, AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess,
};
use crate::tools::{tool_decision_for_call, ToolDecision};
use uuid::Uuid;

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
            Ok(AgentTurnOutcome::NeedsUserInput(input)) => {
                if self.handle_malformed_request_input_retry(node_id, &input) {
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
                if self.handle_transient_retry(node_id, &error) {
                    return;
                }
                self.fail_node(node_id, &error);
            }
        }
    }

    fn handle_interrupted(&mut self, node_id: NodeId) {
        self.interrupted_nodes.insert(node_id.clone());
        self.events.push(RunEvent {
            node_id,
            kind: RunEventKind::Failed,
            message: "interrupted by user".to_string(),
            output: None,
        });
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
            |node| node.agent.output_schema.to_string(),
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
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Retrying,
            message: format!(
                "retrying after malformed submit-output tool call ({}/{MAX_MALFORMED_SUBMIT_OUTPUT_RETRIES})",
                *retry_count
            ),
            output: None,
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
            return false;
        }
        *retry_count += 1;
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::UserMessage {
                content: MALFORMED_REQUEST_INPUT_FEEDBACK.to_string(),
            },
        );
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Retrying,
            message: format!(
                "retrying after non-question request-user-input ({}/{MAX_MALFORMED_REQUEST_INPUT_RETRIES})",
                *retry_count
            ),
            output: None,
        });
        true
    }

    fn handle_transient_retry(&mut self, node_id: &NodeId, error: &AgentError) -> bool {
        if !error.is_retryable() {
            return false;
        }
        let retry_count = self.retries_by_node.entry(node_id.clone()).or_default();
        let Some(delay) = next_retry(&self.workflow.settings.retry_policy, retry_count) else {
            return false;
        };
        self.pending_retry_delay = Some(
            self.pending_retry_delay
                .map_or(delay, |existing| existing.max(delay)),
        );
        self.events.push(retrying_event(node_id.clone(), delay));
        true
    }

    fn fail_node(&mut self, node_id: &NodeId, error: &AgentError) {
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Failed,
            message: error.to_string(),
            output: None,
        });
        self.failed_nodes.insert(node_id.clone(), error.to_string());
    }

    fn apply_completion(&mut self, node_id: &NodeId, success: AgentTurnSuccess) {
        if let Some(message) = filter_tool_turn_assistant_message(success.assistant_message)
            .filter(|message| !message.trim().is_empty())
        {
            self.transcripts
                .entry(node_id.clone())
                .or_default()
                .push(AgentTranscriptItem::AssistantMessage { content: message });
        }
        self.outputs.insert(node_id.clone(), success.output.clone());
        self.events.push(RunEvent {
            node_id: node_id.clone(),
            kind: RunEventKind::Completed,
            message: "completed".to_string(),
            output: Some(success.output),
        });
    }

    fn apply_tool_calls(&mut self, node_id: &NodeId, batch: AgentToolCallBatch) {
        if self.find_node(node_id).is_none() {
            self.terminal_error = Some(RunError::NodeFailed {
                node_id: node_id.clone(),
                kind: NodeFailureKind::ToolCallNodeNotFound,
            });
            return;
        }

        let config = self
            .find_node(node_id)
            .map(|node| node.agent.tools.clone())
            .unwrap_or_default();
        let transcript = self.transcripts.entry(node_id.clone()).or_default();
        if let Some(message) = filter_tool_turn_assistant_message(batch.assistant_message)
            .filter(|message| !message.trim().is_empty())
        {
            transcript.push(AgentTranscriptItem::AssistantMessage { content: message });
        }
        let mut pending_calls = Vec::new();
        let mut requires_approval_for_batch = false;
        for call in batch.tool_calls {
            transcript.push(AgentTranscriptItem::ToolCall { call: call.clone() });
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
            },
        );
    }

    fn apply_user_input_request(&mut self, node_id: &NodeId, input: AgentNeedUserInput) {
        self.transcripts.entry(node_id.clone()).or_default().push(
            AgentTranscriptItem::AssistantMessage {
                content: input.assistant_message,
            },
        );
        self.awaiting_nodes.insert(node_id.clone());
    }
}
