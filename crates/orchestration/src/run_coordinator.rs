use crate::agent_store::FileAgentStore;
use crate::error::BackendError;
use crate::execution::{
    apply_event_to_run_state, record_user_input, resolve_execution_cwd,
    spawn_interactive_workflow_run, ExecutionAction, ExecutionEvent,
};
use domain::resolve_callable_agent_snapshots;
use crate::provider_config::{resolve_provider_config, ProviderEnv};
use crate::settings_store::{AppSettings, FileSettingsStore};
use crate::state::WorkflowRunState;
use domain::{validate_workflow, NodeId, Workflow};
use providers::{create_provider, ProviderId};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

#[derive(Debug)]
struct RunSession {
    workflow: Option<Workflow>,
    run_state: Option<WorkflowRunState>,
    action_tx: Option<UnboundedSender<ExecutionAction>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

pub struct RunStartParams<'a> {
    pub workflow: Workflow,
    pub entrypoint: Option<String>,
    pub execution_cwd: Option<String>,
    pub settings: &'a AppSettings,
    pub transient_api_key: Option<&'a str>,
    pub agent_store: &'a FileAgentStore,
    pub settings_store: &'a FileSettingsStore,
    pub env: &'a ProviderEnv,
}

#[derive(Debug)]
pub struct RunCoordinator {
    runtime: tokio::runtime::Runtime,
    session: Mutex<RunSession>,
}

impl RunCoordinator {
    #[must_use]
    pub fn new(runtime: tokio::runtime::Runtime) -> Self {
        Self {
            runtime,
            session: Mutex::new(RunSession {
                workflow: None,
                run_state: None,
                action_tx: None,
                handle: None,
            }),
        }
    }

    /// # Errors
    /// Returns an error if the workflow fails validation or provider configuration fails.
    pub async fn start_run(
        &self,
        params: RunStartParams<'_>,
    ) -> Result<(WorkflowRunState, UnboundedReceiver<ExecutionEvent>), BackendError> {
        let RunStartParams {
            workflow,
            entrypoint,
            execution_cwd,
            settings,
            transient_api_key,
            agent_store,
            settings_store,
            env,
        } = params;

        validate_workflow(&workflow)?;
        let resolved_cwd = resolve_execution_cwd(execution_cwd.as_deref())
            .map_err(BackendError::InvalidExecutionCwd)?;
        let provider_settings = workflow
            .settings
            .provider_id
            .as_ref()
            .filter(|provider_id| !provider_id.trim().is_empty())
            .map(|provider_id| {
                let mut settings = settings.clone();
                settings.active_provider = ProviderId::from(provider_id.as_str());
                settings
            });
        let provider_settings = provider_settings.as_ref().unwrap_or(settings);
        let provider_config = resolve_provider_config(
            provider_settings,
            transient_api_key,
            env,
            settings_store.credential_store(),
        )?;
        let ai = create_provider(provider_config);

        let agents = agent_store.load()?;
        let agent_snapshots = resolve_callable_agent_snapshots(&workflow, &agents);
        let (handle, event_rx, action_tx) = spawn_interactive_workflow_run(
            &self.runtime,
            workflow.clone(),
            entrypoint,
            resolved_cwd,
            ai,
            agent_snapshots,
        );

        let mut session = self.session.lock().await;
        if let Some(existing) = session.handle.take() {
            existing.abort();
        }
        let initial_state = WorkflowRunState::running_for_workflow(&workflow);
        session.workflow = Some(workflow);
        session.run_state = Some(initial_state.clone());
        session.action_tx = Some(action_tx);
        session.handle = Some(handle);
        Ok((initial_state, event_rx))
    }

    /// # Errors
    /// Returns an error if there is no active run.
    pub async fn apply_execution_event(
        &self,
        event: ExecutionEvent,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let workflow = session.workflow.clone().ok_or(BackendError::NoActiveRun)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        apply_event_to_run_state(&workflow, run_state, event);
        let finished = !run_state.active;
        let snapshot = run_state.clone();
        if finished {
            session.action_tx = None;
            session.handle = None;
        }
        Ok(snapshot)
    }

    /// # Errors
    /// Returns an error if there is no active run or the wrong node is selected.
    pub async fn submit_user_input(
        &self,
        node_id: &str,
        text: String,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let expected = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .awaiting_node_id
            .clone()
            .ok_or(BackendError::NoAwaitingInput)?;
        if expected != node_id {
            return Err(BackendError::WrongAwaitingNode {
                expected,
                received: NodeId(node_id.to_string()),
            });
        }
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::ProvideInput(text.clone()))
            .map_err(|_| BackendError::RunChannelClosed)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        record_user_input(run_state, node_id, text);
        Ok(run_state.clone())
    }

    /// # Errors
    /// Returns an error if there is no active run or the wrong approval is selected.
    pub async fn submit_tool_approval(
        &self,
        approval_id: &str,
        allow: bool,
    ) -> Result<WorkflowRunState, BackendError> {
        let mut session = self.session.lock().await;
        let expected = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .pending_approvals
            .first()
            .cloned()
            .ok_or(BackendError::NoPendingApproval)?;
        if expected.approval_id != approval_id {
            return Err(BackendError::WrongApprovalId {
                expected: expected.approval_id,
                received: approval_id.to_string(),
            });
        }
        session
            .action_tx
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?
            .send(ExecutionAction::ResolveApproval {
                approval_id: approval_id.to_string(),
                allow,
            })
            .map_err(|_| BackendError::RunChannelClosed)?;
        let run_state = session
            .run_state
            .as_mut()
            .ok_or(BackendError::NoActiveRun)?;
        run_state.pending_approvals.clear();
        Ok(run_state.clone())
    }

    /// Returns an error because conversational paused nodes advance via `submit_user_input`.
    pub async fn complete_manual_node(&self) -> Result<WorkflowRunState, BackendError> {
        let session = self.session.lock().await;
        let run_state = session
            .run_state
            .as_ref()
            .ok_or(BackendError::NoActiveRun)?;
        if run_state.awaiting_node_id.is_some() {
            return Err(BackendError::NoAwaitingInput);
        }
        Err(BackendError::NoAwaitingInput)
    }

    #[must_use]
    pub async fn get_run_state(&self) -> Option<WorkflowRunState> {
        self.session.lock().await.run_state.clone()
    }

    pub async fn clear_run_trace(&self) -> Result<Option<WorkflowRunState>, BackendError> {
        let mut session = self.session.lock().await;
        let workflow = session.workflow.clone();
        let run_state = session.run_state.as_mut();
        match (workflow, run_state) {
            (Some(workflow), Some(run_state)) => {
                let mut cleared = WorkflowRunState::idle_for_workflow(&workflow);
                cleared.chat_logs = run_state.chat_logs.clone();
                cleared.outputs = run_state.outputs.clone();
                *run_state = cleared;
                Ok(Some(run_state.clone()))
            }
            _ => Ok(None),
        }
    }

    #[cfg(test)]
    pub(crate) fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }

    #[cfg(test)]
    pub(crate) async fn test_abort_handle(&self) {
        let mut session = self.session.lock().await;
        if let Some(handle) = session.handle.take() {
            handle.abort();
        }
    }

    #[cfg(test)]
    pub(crate) async fn test_seed_session(
        &self,
        workflow: Workflow,
        run_state: WorkflowRunState,
        action_tx: UnboundedSender<ExecutionAction>,
    ) {
        let mut session = self.session.lock().await;
        session.workflow = Some(workflow);
        session.run_state = Some(run_state);
        session.action_tx = Some(action_tx);
    }
}
