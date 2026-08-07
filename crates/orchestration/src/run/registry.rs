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
use parking_lot::Mutex as ParkingMutex;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Weak};
use tokio::sync::mpsc::UnboundedReceiver;
#[cfg(test)]
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, RwLock};

const MAX_RETAINED_RUN_SESSIONS: usize = 64;

#[derive(Default)]
struct RegistryState {
    sessions: BTreeMap<String, RegistryEntry>,
    session_order: VecDeque<String>,
    latest_run_id: Option<String>,
}

struct RegistryEntry {
    coordinator: Arc<RunCoordinator>,
    active: bool,
}

impl RegistryState {
    fn register(&mut self, run_id: String, coordinator: Arc<RunCoordinator>, active: bool) {
        self.session_order.retain(|candidate| candidate != &run_id);
        self.session_order.push_back(run_id.clone());
        self.sessions.insert(
            run_id.clone(),
            RegistryEntry {
                coordinator,
                active,
            },
        );
        self.latest_run_id = Some(run_id.clone());
        self.prune_inactive_sessions();
    }

    fn set_active(&mut self, run_id: &str, active: bool) {
        if let Some(entry) = self.sessions.get_mut(run_id) {
            entry.active = active;
        }
        self.prune_inactive_sessions();
    }

    fn prune_inactive_sessions(&mut self) {
        while self.sessions.len() > MAX_RETAINED_RUN_SESSIONS {
            let Some(index) = self
                .session_order
                .iter()
                .position(|run_id| self.sessions.get(run_id).is_some_and(|entry| !entry.active))
            else {
                break;
            };
            let run_id = self
                .session_order
                .remove(index)
                .expect("inactive run index must exist");
            self.sessions.remove(&run_id);
            if self.latest_run_id.as_deref() == Some(run_id.as_str()) {
                self.latest_run_id = self.session_order.back().cloned();
            }
        }
    }
}

/// Owns every in-process top-level run. Each coordinator still hosts exactly one session.
pub struct RunRegistry {
    runtime_handle: tokio::runtime::Handle,
    attachment_store: Arc<dyn RunAttachmentStore>,
    resources: Arc<SharedRunResources>,
    state: RwLock<RegistryState>,
    workflow_start_gates: ParkingMutex<BTreeMap<String, Weak<Mutex<()>>>>,
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
            workflow_start_gates: ParkingMutex::new(BTreeMap::new()),
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
        state.register(run_id, coordinator, run_state.active);
        Ok(())
    }

    fn workflow_start_gate(&self, workflow_id: &str) -> Arc<Mutex<()>> {
        let mut gates = self.workflow_start_gates.lock();
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(workflow_id).and_then(Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(Mutex::new(()));
        gates.insert(workflow_id.to_string(), Arc::downgrade(&gate));
        gate
    }

    async fn ensure_workflow_inactive(&self, workflow_id: &str) -> Result<(), BackendError> {
        let active_run = self
            .active_run_states()
            .await
            .into_iter()
            .find(|state| state.workflow_id.as_deref() == Some(workflow_id));
        match active_run {
            Some(state) => Err(BackendError::RunAlreadyActive(
                state.run_id.unwrap_or_else(|| workflow_id.to_string()),
            )),
            None => Ok(()),
        }
    }

    async fn record_run_state(&self, run_state: &WorkflowRunState) {
        let Some(run_id) = run_state.run_id.as_deref() else {
            return;
        };
        self.state
            .write()
            .await
            .set_active(run_id, run_state.active);
    }

    pub async fn coordinator_for(&self, run_id: &str) -> Result<Arc<RunCoordinator>, BackendError> {
        self.state
            .read()
            .await
            .sessions
            .get(run_id)
            .map(|entry| Arc::clone(&entry.coordinator))
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
            .map(|entry| Arc::clone(&entry.coordinator))
            .ok_or_else(|| BackendError::RunNotFound(run_id.clone()))
    }

    /// Start and register a fresh independent session.
    pub async fn start_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let workflow_id = params.workflow.id.to_string();
        let start_gate = self.workflow_start_gate(&workflow_id);
        let _start_guard = start_gate.lock().await;
        self.ensure_workflow_inactive(&workflow_id).await?;
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
        let workflow_id = params.record.workflow_id.clone();
        let start_gate = self.workflow_start_gate(&workflow_id);
        let _start_guard = start_gate.lock().await;
        self.ensure_workflow_inactive(&workflow_id).await?;
        let existing = self
            .state
            .read()
            .await
            .sessions
            .get(params.run_id)
            .map(|entry| Arc::clone(&entry.coordinator));
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
        let coordinator = self
            .state
            .read()
            .await
            .sessions
            .get(run_id)
            .map(|entry| Arc::clone(&entry.coordinator))?;
        coordinator.get_run_state().await
    }

    pub async fn active_run_states(&self) -> Vec<WorkflowRunState> {
        let coordinators = self
            .state
            .read()
            .await
            .sessions
            .values()
            .map(|entry| Arc::clone(&entry.coordinator))
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
        let run_state = self
            .coordinator_for(run_id)
            .await?
            .stop_run_and_persist(run_store)
            .await?;
        self.record_run_state(&run_state).await;
        Ok(run_state)
    }

    pub async fn apply_execution_event_for(
        &self,
        run_id: &str,
        event: ExecutionEvent,
        run_store: &dyn RunCheckpointStore,
    ) -> Result<WorkflowRunState, BackendError> {
        let run_state = self
            .coordinator_for(run_id)
            .await?
            .apply_execution_event(event, run_store)
            .await?;
        self.record_run_state(&run_state).await;
        Ok(run_state)
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
        let run_state = self
            .latest_coordinator()
            .await?
            .stop_run_and_persist(run_store)
            .await?;
        self.record_run_state(&run_state).await;
        Ok(run_state)
    }

    pub async fn continue_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let workflow_id = params.workflow.id.to_string();
        let start_gate = self.workflow_start_gate(&workflow_id);
        let _start_guard = start_gate.lock().await;
        self.ensure_workflow_inactive(&workflow_id).await?;
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
        let workflow_id = params.workflow.id.to_string();
        let start_gate = self.workflow_start_gate(&workflow_id);
        let _start_guard = start_gate.lock().await;
        self.ensure_workflow_inactive(&workflow_id).await?;
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
        let run_state = self
            .latest_coordinator()
            .await?
            .apply_execution_event(event, run_store)
            .await?;
        self.record_run_state(&run_state).await;
        Ok(run_state)
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
            .map(|entry| Arc::clone(&entry.coordinator))
            .collect::<Vec<_>>();
        for coordinator in coordinators {
            if coordinator.is_run_active().await {
                if let Ok(run_state) = coordinator.stop_run_and_persist(run_store).await {
                    self.record_run_state(&run_state).await;
                }
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
        run_state
            .run_id
            .get_or_insert_with(|| format!("test-seed-{}", uuid::Uuid::new_v4()));
        run_state
            .workflow_id
            .get_or_insert_with(|| workflow.id.to_string());
        let coordinator = self.new_coordinator();
        let registered_state = run_state.clone();
        coordinator
            .test_seed_session(workflow, run_state, action_tx)
            .await;
        self.register(coordinator, &registered_state)
            .await
            .expect("seeded run must have an id");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::storage::run_attachment_store::FileRunAttachmentStore;
    use crate::run::resources::SharedRunResources;
    use std::sync::Arc;
    use std::time::Duration;

    fn registry() -> RunRegistry {
        RunRegistry::new(
            tokio::runtime::Handle::current(),
            Arc::new(FileRunAttachmentStore::default()),
        )
    }

    #[tokio::test]
    async fn same_workflow_cannot_own_two_active_sessions() {
        let registry = registry();
        let workflow = Workflow::new("workflow");
        let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
        run_state.run_id = Some("active-run".to_string());
        let (action_tx, _action_rx) = tokio::sync::mpsc::unbounded_channel();
        registry
            .test_seed_session(workflow.clone(), run_state, action_tx)
            .await;

        assert!(matches!(
            registry.ensure_workflow_inactive(&workflow.id.to_string()).await,
            Err(BackendError::RunAlreadyActive(run_id)) if run_id == "active-run"
        ));
        assert!(registry
            .ensure_workflow_inactive("other-workflow")
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn registry_evicts_oldest_inactive_sessions_above_retention_limit() {
        let registry = registry();
        let workflow = Workflow::new("workflow");
        for index in 0..=MAX_RETAINED_RUN_SESSIONS {
            let mut run_state = WorkflowRunState::idle_for_workflow(&workflow);
            run_state.run_id = Some(format!("run-{index:03}"));
            let (action_tx, _action_rx) = tokio::sync::mpsc::unbounded_channel();
            registry
                .test_seed_session(workflow.clone(), run_state, action_tx)
                .await;
        }

        assert!(registry.get_run_state_for("run-000").await.is_none());
        assert!(registry.get_run_state_for("run-001").await.is_some());
        assert_eq!(registry.current_run_id().await.as_deref(), Some("run-064"));
    }

    #[tokio::test]
    async fn registry_never_evicts_active_sessions_to_meet_retention_limit() {
        let registry = registry();
        let workflow = Workflow::new("workflow");
        for index in 0..=MAX_RETAINED_RUN_SESSIONS {
            let mut run_state = WorkflowRunState::running_for_workflow(&workflow);
            run_state.run_id = Some(format!("run-{index:03}"));
            let (action_tx, _action_rx) = tokio::sync::mpsc::unbounded_channel();
            registry
                .test_seed_session(workflow.clone(), run_state, action_tx)
                .await;
        }

        assert!(registry.get_run_state_for("run-000").await.is_some());
        let mut inactive = WorkflowRunState::idle_for_workflow(&workflow);
        inactive.run_id = Some("run-000".to_string());
        registry.record_run_state(&inactive).await;
        assert!(registry.get_run_state_for("run-000").await.is_none());
    }

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
