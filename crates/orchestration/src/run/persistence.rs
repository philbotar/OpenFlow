use crate::run::state::{ProjectedChatMessage, WorkflowRunState};
use engine::{AgentTranscriptItem, ChatMessage, ChatRole, InteractiveEngineCheckpoint, Workflow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Paused,
    Stopped,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunCheckpointReason {
    Started,
    AwaitingInput,
    AwaitingToolApproval,
    AwaitingRetry,
    UserStopped,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub workflow_id: String,
    pub workflow_name: String,
    pub workflow_hash: String,
    /// Exact prepared workflow used to start the run.
    pub workflow_snapshot: Workflow,
    pub project_id: Option<String>,
    pub execution_cwd: String,
    pub artifact_root: String,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: RunStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCheckpointPayload {
    pub seq: u32,
    pub created_at_ms: i64,
    pub reason: RunCheckpointReason,
    pub engine: InteractiveEngineCheckpoint,
    pub projection: WorkflowRunState,
}

impl RunCheckpointPayload {
    /// Drop a pre-dedicated-tool choice card while preserving the underlying pause.
    pub(crate) fn discard_structured_user_input(&mut self) {
        self.engine.structured_input_by_node.clear();
        self.projection.structured_input_by_node.clear();
    }

    /// Repair a direct-chat projection from the canonical engine transcript.
    ///
    /// Older checkpoints could capture engine state after a model turn while their projection
    /// still lagged behind. Merge missing visible messages at their transcript position.
    pub(crate) fn repair_direct_chat_projection(&mut self) -> bool {
        let Some(node_id) = self.engine.transcripts.keys().next().cloned() else {
            return false;
        };
        let transcript = self
            .engine
            .transcripts
            .get(&node_id)
            .cloned()
            .unwrap_or_default();
        let entrypoint = self.engine.entrypoint_text.clone().map(|content| {
            let mut message = ChatMessage::text(ChatRole::User, content);
            message
                .attachments
                .clone_from(&self.engine.entrypoint_attachments);
            message
        });
        let transcript_messages = transcript
            .into_iter()
            .filter_map(|item| match item {
                AgentTranscriptItem::AssistantMessage { content } => {
                    Some(ChatMessage::text(ChatRole::Assistant, content))
                }
                AgentTranscriptItem::UserMessage {
                    content,
                    attachments,
                } => {
                    let mut message = ChatMessage::text(ChatRole::User, content);
                    message.attachments = attachments;
                    Some(message)
                }
                AgentTranscriptItem::Reasoning { .. }
                | AgentTranscriptItem::ToolCall { .. }
                | AgentTranscriptItem::ToolResult { .. } => None,
            })
            .collect::<Vec<_>>();
        let messages = self.projection.chat_logs.entry(node_id).or_default();
        let mut changed = false;
        let mut cursor = 0;

        if let Some(entrypoint) = entrypoint {
            if let Some(index) = messages
                .iter()
                .position(|message| visible_message_matches(message, &entrypoint))
            {
                cursor = index + 1;
            } else {
                messages.insert(0, ProjectedChatMessage::from(entrypoint));
                cursor = 1;
                changed = true;
            }
        }

        for transcript_message in transcript_messages {
            if let Some(offset) = messages[cursor..]
                .iter()
                .position(|message| visible_message_matches(message, &transcript_message))
            {
                let index = cursor + offset;
                if messages[index].streaming {
                    messages[index].streaming = false;
                    changed = true;
                }
                cursor = index + 1;
            } else {
                messages.insert(cursor, ProjectedChatMessage::from(transcript_message));
                cursor += 1;
                changed = true;
            }
        }
        changed
    }
}

fn visible_message_matches(actual: &ProjectedChatMessage, expected: &ChatMessage) -> bool {
    actual.role == expected.role
        && actual.content == expected.content
        && actual.attachments == expected.attachments
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingRunCheckpoint {
    pub reason: RunCheckpointReason,
    pub engine: InteractiveEngineCheckpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStoreRoot {
    pub project_id: Option<String>,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run_id: String,
    pub name: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub project_id: Option<String>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: RunStatus,
}

#[must_use]
pub(crate) fn run_name(workflow_name: &str, entrypoint_text: Option<&str>) -> String {
    let normalized_entrypoint = entrypoint_text
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !normalized_entrypoint.is_empty() {
        let mut chars = normalized_entrypoint.chars();
        let name = chars.by_ref().take(60).collect::<String>();
        return if chars.next().is_some() {
            format!("{name}…")
        } else {
            name
        };
    }

    let workflow_name = workflow_name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if workflow_name.is_empty() {
        "Workflow run".to_string()
    } else {
        format!("{workflow_name} run")
    }
}

#[must_use]
pub fn workflow_hash(workflow: &Workflow) -> String {
    let bytes = serde_json::to_vec(workflow).expect("workflow must serialize for run hash");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

impl RunRecord {
    #[must_use]
    pub fn summary(&self) -> RunSummary {
        RunSummary {
            run_id: self.run_id.clone(),
            name: self
                .name
                .clone()
                .unwrap_or_else(|| run_name(&self.workflow_name, None)),
            workflow_id: self.workflow_id.clone(),
            workflow_name: self.workflow_name.clone(),
            project_id: self.project_id.clone(),
            started_at_ms: self.started_at_ms,
            updated_at_ms: self.updated_at_ms,
            status: self.status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Workflow;

    #[test]
    fn run_record_serializes_camel_case_fields() {
        let record = RunRecord {
            run_id: "run-1".to_string(),
            name: Some("Review provider retries".to_string()),
            workflow_id: "wf-1".to_string(),
            workflow_name: "Demo".to_string(),
            workflow_hash: "abc".to_string(),
            workflow_snapshot: Workflow::new("Demo"),
            project_id: Some("project-1".to_string()),
            execution_cwd: "/tmp/demo".to_string(),
            artifact_root: "/tmp/demo/.flow/runs/run-1/artifacts".to_string(),
            started_at_ms: 1,
            updated_at_ms: 2,
            status: RunStatus::Paused,
        };

        let json = serde_json::to_string(&record).expect("serialize run record");

        assert!(json.contains("runId"));
        assert!(json.contains("workflowId"));
        assert!(json.contains("workflowSnapshot"));
        assert!(json.contains("artifactRoot"));
        assert!(json.contains("\"paused\""));
    }

    #[test]
    fn workflow_hash_changes_when_workflow_changes() {
        let mut first = Workflow::new("first");
        let second = first.clone();
        first.name = "changed".to_string();

        assert_ne!(workflow_hash(&first), workflow_hash(&second));
    }
}
