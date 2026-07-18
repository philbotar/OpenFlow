use crate::run::state::WorkflowRunState;
use engine::{InteractiveEngineCheckpoint, Workflow};
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
    pub workflow_id: String,
    pub workflow_name: String,
    pub project_id: Option<String>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub status: RunStatus,
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
