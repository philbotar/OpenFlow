use engine::{AiPort, CallableAgent, Workflow};
use orchestration::run::execution::{
    new_artifact_root, new_in_memory_snapshot_store, run_workflow_headless,
    spawn_interactive_workflow_run, ApprovalResponse, ExecutionAction, ExecutionEvent,
    InteractiveWorkflowRunParams, ManualInput, NodeInterrupts, WorkflowExecutionError,
    WorkflowRunSnapshot,
};
use orchestration::settings::model::{McpSettings, ProviderProfile};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct HeadlessRunOpts {
    pub entrypoint: Option<String>,
    pub manual_inputs: Vec<ManualInput>,
    pub approvals: Vec<ApprovalResponse>,
    pub cwd: Option<PathBuf>,
    pub provider_profile: Option<ProviderProfile>,
}

pub async fn run_headless_script<A>(
    workflow: Workflow,
    ai: A,
    opts: HeadlessRunOpts,
) -> Result<WorkflowRunSnapshot, WorkflowExecutionError>
where
    A: AiPort + Send + Sync + 'static,
{
    run_workflow_headless(
        workflow,
        opts.entrypoint,
        ai,
        opts.manual_inputs,
        opts.approvals,
        BTreeMap::new(),
        opts.cwd,
        opts.provider_profile.as_ref(),
    )
    .await
}

pub struct InteractiveRunHandle {
    pub handle: tokio::task::JoinHandle<()>,
    pub event_rx: UnboundedReceiver<ExecutionEvent>,
    #[allow(dead_code, reason = "exposed for interactive test harness extensions")]
    pub action_tx: UnboundedSender<ExecutionAction>,
    #[allow(dead_code, reason = "exposed for interactive test harness extensions")]
    pub cancel_token: CancellationToken,
    pub node_interrupts: NodeInterrupts,
}

pub fn spawn_interactive_script<A>(
    workflow: Workflow,
    execution_cwd: PathBuf,
    ai: A,
) -> InteractiveRunHandle
where
    A: AiPort + Send + Sync + 'static,
{
    let params = InteractiveWorkflowRunParams {
        workflow,
        entrypoint: None,
        execution_cwd,
        project_repository_root: None,
        artifact_root: new_artifact_root(),
        resume_checkpoint: None,
        checkpoint_sink: Arc::new(parking_lot::Mutex::new(None)),
        ai,
        agent_snapshots: BTreeMap::<String, CallableAgent>::new(),
        snapshot_store: new_in_memory_snapshot_store(),
        lsp: orchestration::lsp::LspSettings::from_env(),
        pending_engine_reverts: Arc::new(parking_lot::Mutex::new(Vec::new())),
        node_interrupts: Arc::new(parking_lot::Mutex::new(BTreeMap::new())),
        context_window_sizes: BTreeMap::new(),
        mcp: McpSettings {
            discover_external: false,
            ..McpSettings::default()
        },
        search: orchestration::settings::model::SearchSettings::default(),
        runtime_config_store: engine::new_runtime_config_store(),
    };

    let (handle, event_rx, action_tx, cancel_token, node_interrupts) =
        spawn_interactive_workflow_run(&tokio::runtime::Handle::current(), params);

    InteractiveRunHandle {
        handle,
        event_rx,
        action_tx,
        cancel_token,
        node_interrupts,
    }
}
