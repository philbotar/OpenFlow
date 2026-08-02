//! Addressable registry of independent top-level run sessions.

use crate::api::{FileEditPreview, UserMessageInput};
use crate::error::BackendError;
use crate::mcp::client_capabilities::McpClientRequestDecision;
use crate::run::coordinator::{DurableResumeParams, RunCoordinator, RunStartParams};
#[cfg(test)]
use crate::run::execution::ExecutionAction;
use crate::run::execution::ExecutionEvent;
use crate::run::persistence::{RunStoreRoot, RunSummary};
use crate::run::ports::{RunAttachmentStore, RunCheckpointStore};
use crate::run::resources::SharedRunResources;
use crate::run::state::WorkflowRunState;
use engine::NodeRuntimeConfigPatch;
#[cfg(test)]
use engine::Workflow;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
#[cfg(test)]
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;

#[derive(Default)]
struct RegistryState {
    sessions: BTreeMap<String, Arc<RunCoordinator>>,
    latest_run_id: Option<String>,
}

/// Owns every in-process top-level run. Each coordinator still hosts exactly one session.
pub struct RunRegistry {
    runtime_handle: tokio::runtime::Handle,
    attachment_store: Arc<dyn RunAttachmentStore>,
    resources: Arc<SharedRunResources>,
    state: RwLock<RegistryState>,
}

impl RunRegistry {
    #[must_use]
    pub fn new(
        runtime_handle: tokio::runtime::Handle,
        attachment_store: Arc<dyn RunAttachmentStore>,
    ) -> Self {
        Self {
            runtime_handle,
            attachment_store,
            resources: Arc::new(SharedRunResources::default()),
            state: RwLock::new(RegistryState::default()),
        }
    }

    fn new_coordinator(&self) -> Arc<RunCoordinator> {
        Arc::new(RunCoordinator::with_shared_resources(
            self.runtime_handle.clone(),
            Arc::clone(&self.attachment_store),
            Arc::clone(&self.resources),
        ))
    }

    async fn register(
        &self,
        coordinator: Arc<RunCoordinator>,
        run_state: &WorkflowRunState,
    ) -> Result<(), BackendError> {
        let run_id = run_state.run_id.clone().ok_or(BackendError::RunMissingId)?;
        let mut state = self.state.write().await;
        state.sessions.insert(run_id.clone(), coordinator);
        state.latest_run_id = Some(run_id);
        Ok(())
    }

    pub async fn coordinator_for(&self, run_id: &str) -> Result<Arc<RunCoordinator>, BackendError> {
        self.state
            .read()
            .await
            .sessions
            .get(run_id)
            .cloned()
            .ok_or_else(|| BackendError::RunNotFound(run_id.to_string()))
    }

    async fn latest_coordinator(&self) -> Result<Arc<RunCoordinator>, BackendError> {
        let state = self.state.read().await;
        let run_id = state
            .latest_run_id
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        state
            .sessions
            .get(run_id)
            .cloned()
            .ok_or_else(|| BackendError::RunNotFound(run_id.clone()))
    }

    /// Start and register a fresh independent session.
    pub async fn start_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let coordinator = self.new_coordinator();
        let (run_state, events) = coordinator.start_run(params).await?;
        self.register(coordinator, &run_state).await?;
        Ok((run_state, events))
    }

    /// Restore a durable run into its existing in-process session, or register a new one.
    pub async fn resume_durable_run_with_continuation(
        &self,
        params: DurableResumeParams<'_>,
        continuation: Option<crate::api::DurableRunContinuationInput>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let existing = self.state.read().await.sessions.get(params.run_id).cloned();
        if let Some(coordinator) = existing.as_ref() {
            if coordinator.is_run_active().await {
                return Err(BackendError::RunAlreadyActive(params.run_id.to_string()));
            }
        }
        let coordinator = existing.unwrap_or_else(|| self.new_coordinator());
        let (run_state, events) = coordinator
            .resume_durable_run_with_continuation(params, continuation)
            .await?;
        self.register(coordinator, &run_state).await?;
        Ok((run_state, events))
    }

    pub fn list_runs(
        &self,
        run_store: &dyn RunCheckpointStore,
        roots: &[RunStoreRoot],
        workflow_id: Option<&str>,
    ) -> Result<Vec<RunSummary>, BackendError> {
        Ok(run_store.list_runs(roots, workflow_id)?)
    }

    pub async fn get_run_state_for(&self, run_id: &str) -> Option<WorkflowRunState> {
        let coordinator = self.state.read().await.sessions.get(run_id).cloned()?;
        coordinator.get_run_state().await
    }

    pub async fn active_run_states(&self) -> Vec<WorkflowRunState> {
        let coordinators = self
            .state
            .read()
            .await
            .sessions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut active = Vec::new();
        for coordinator in coordinators {
            if let Some(run_state) = coordinator
                .get_run_state()
                .await
                .filter(|state| state.active)
            {
                active.push(run_state);
            }
        }
        active
    }

    pub async fn stop_run_for(
        &self,
        run_id: &str,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .stop_run_and_persist(run_store)
            .await
    }

    pub async fn apply_execution_event_for(
        &self,
        run_id: &str,
        event: ExecutionEvent,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .apply_execution_event(event, run_store)
            .await
    }

    #[must_use]
    pub async fn is_run_active_for(&self, run_id: &str) -> bool {
        match self.coordinator_for(run_id).await {
            Ok(coordinator) => coordinator.is_run_active().await,
            Err(_) => false,
        }
    }

    pub async fn stop_run_and_persist(
        &self,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .stop_run_and_persist(run_store)
            .await
    }

    pub async fn continue_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let coordinator = self.latest_coordinator().await?;
        let result = coordinator.continue_run(params).await?;
        self.register(coordinator, &result.0).await?;
        Ok(result)
    }

    pub async fn continue_run_for(
        &self,
        run_id: &str,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let coordinator = self.coordinator_for(run_id).await?;
        let result = coordinator.continue_run(params).await?;
        self.register(coordinator, &result.0).await?;
        Ok(result)
    }

    #[must_use]
    pub async fn is_run_continuable(&self) -> bool {
        match self.latest_coordinator().await {
            Ok(coordinator) => coordinator.is_run_continuable().await,
            Err(_) => false,
        }
    }

    #[must_use]
    pub async fn is_run_continuable_for(&self, run_id: &str) -> bool {
        match self.coordinator_for(run_id).await {
            Ok(coordinator) => coordinator.is_run_continuable().await,
            Err(_) => false,
        }
    }

    pub async fn interrupt_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .interrupt_node(node_id)
            .await
    }

    pub async fn interrupt_node_for(
        &self,
        run_id: &str,
        node_id: &str,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .interrupt_node(node_id)
            .await
    }

    pub async fn retry_node(&self, node_id: &str) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator().await?.retry_node(node_id).await
    }

    pub async fn retry_node_for(
        &self,
        run_id: &str,
        node_id: &str,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .retry_node(node_id)
            .await
    }

    pub async fn update_node_runtime_config(
        &self,
        node_id: &str,
        patch: NodeRuntimeConfigPatch,
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .update_node_runtime_config(node_id, patch)
            .await
    }

    pub async fn update_node_runtime_config_for(
        &self,
        run_id: &str,
        node_id: &str,
        patch: NodeRuntimeConfigPatch,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .update_node_runtime_config(node_id, patch)
            .await
    }

    #[must_use]
    pub async fn is_run_active(&self) -> bool {
        !self.active_run_states().await.is_empty()
    }

    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .apply_execution_event(event, run_store)
            .await
    }

    pub async fn submit_user_message_with_skill_ids(
        &self,
        node_id: &str,
        message: UserMessageInput,
        skill_ids: &[String],
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .submit_user_message_with_skill_ids(node_id, message, skill_ids)
            .await
    }

    pub async fn submit_user_message_with_skill_ids_for(
        &self,
        run_id: &str,
        node_id: &str,
        message: UserMessageInput,
        skill_ids: &[String],
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .submit_user_message_with_skill_ids(node_id, message, skill_ids)
            .await
    }

    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
        reason: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .submit_tool_approval(approval_id, allow, reason)
            .await
    }

    pub async fn submit_tool_approval_for(
        &self,
        run_id: &str,
        approval_id: &str,
        allow: bool,
        reason: Option<String>,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .submit_tool_approval(approval_id, allow, reason)
            .await
    }

    pub async fn resolve_mcp_client_request(
        &self,
        request_id: &str,
        decision: McpClientRequestDecision,
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .resolve_mcp_client_request(request_id, decision)
            .await
    }

    pub async fn resolve_mcp_client_request_for(
        &self,
        run_id: &str,
        request_id: &str,
        decision: McpClientRequestDecision,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .resolve_mcp_client_request(request_id, decision)
            .await
    }

    #[must_use]
    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        let coordinator = self.latest_coordinator().await.ok()?;
        coordinator.get_run_state().await
    }

    #[must_use]
    pub async fn current_run_id(&self) -> Option<String> {
        self.state.read().await.latest_run_id.clone()
    }

    pub async fn preview_file_edit(
        &self,
        approval_id: &str,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        self.latest_coordinator()
            .await?
            .preview_file_edit(approval_id, tool_name, arguments)
            .await
    }

    pub async fn preview_file_edit_for(
        &self,
        run_id: &str,
        approval_id: &str,
        tool_name: String,
        arguments: serde_json::Value,
    ) -> Result<FileEditPreview, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .preview_file_edit(approval_id, tool_name, arguments)
            .await
    }

    pub async fn git_diff_file(&self, path: String) -> Result<String, BackendError> {
        self.latest_coordinator().await?.git_diff_file(path).await
    }

    pub async fn git_diff_file_for(
        &self,
        run_id: &str,
        path: String,
    ) -> Result<String, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .git_diff_file(path)
            .await
    }

    pub async fn revert_edit_batch(
        &self,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.latest_coordinator()
            .await?
            .revert_edit_batch(batch_id)
            .await
    }

    pub async fn revert_edit_batch_for(
        &self,
        run_id: &str,
        batch_id: String,
    ) -> Result<WorkflowRunState, BackendError> {
        self.coordinator_for(run_id)
            .await?
            .revert_edit_batch(batch_id)
            .await
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        self.latest_coordinator().await?.clear_run_trace().await
    }

    pub async fn clear_run_trace_for(
        &self,
        run_id: &str,
    ) -> Result<Option<WorkflowRunState>, BackendError> {
        self.coordinator_for(run_id).await?.clear_run_trace().await
    }

    pub async fn stop_all_and_persist(&self, run_store: &dyn RunCheckpointStore) {
        let coordinators = self
            .state
            .read()
            .await
            .sessions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for coordinator in coordinators {
            if coordinator.is_run_active().await {
                let _ = coordinator.stop_run_and_persist(run_store).await;
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn test_seed_session(
        &self,
        workflow: Workflow,
        mut run_state: WorkflowRunState,
        action_tx: UnboundedSender<ExecutionAction>,
    ) {
        let run_id = run_state
            .run_id
            .get_or_insert_with(|| format!("test-seed-{}", uuid::Uuid::new_v4()))
            .clone();
        run_state
            .workflow_id
            .get_or_insert_with(|| workflow.id.to_string());
        let coordinator = self.new_coordinator();
        coordinator
            .test_seed_session(workflow, run_state, action_tx)
            .await;
        let mut state = self.state.write().await;
        state.sessions.insert(run_id.clone(), coordinator);
        state.latest_run_id = Some(run_id);
    }
}

#[cfg(test)]
mod tests {
    use crate::run::resources::SharedRunResources;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn shared_resource_budgets_queue_excess_work_without_limiting_sessions() {
        let resources = Arc::new(SharedRunResources::with_limits(1, 1));
        let first_ai = resources.acquire_ai().await;
        let ai_waiter_resources = Arc::clone(&resources);
        let ai_waiter = tokio::spawn(async move {
            let _permit = ai_waiter_resources.acquire_ai().await;
        });
        tokio::task::yield_now().await;
        assert!(!ai_waiter.is_finished());
        drop(first_ai);
        tokio::time::timeout(Duration::from_secs(1), ai_waiter)
            .await
            .expect("AI waiter released")
            .expect("AI waiter task");

        let first_tool = resources.acquire_tool().await;
        let tool_waiter_resources = Arc::clone(&resources);
        let tool_waiter = tokio::spawn(async move {
            let _permit = tool_waiter_resources.acquire_tool().await;
        });
        tokio::task::yield_now().await;
        assert!(!tool_waiter.is_finished());
        drop(first_tool);
        tokio::time::timeout(Duration::from_secs(1), tool_waiter)
            .await
            .expect("tool waiter released")
            .expect("tool waiter task");
    }

    #[test]
    fn mutation_gates_are_shared_only_by_canonical_cwd() {
        let resources = SharedRunResources::with_limits(1, 1);
        let first = tempfile::tempdir().expect("first cwd");
        let second = tempfile::tempdir().expect("second cwd");
        let first_path = first.path().canonicalize().expect("canonical first cwd");
        let second_path = second.path().canonicalize().expect("canonical second cwd");

        let first_gate = resources.mutation_gate_for(&first_path);
        let same_gate = resources.mutation_gate_for(&first_path);
        let other_gate = resources.mutation_gate_for(&second_path);

        assert!(Arc::ptr_eq(&first_gate, &same_gate));
        assert!(!Arc::ptr_eq(&first_gate, &other_gate));
    }
}
