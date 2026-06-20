use crate::error::BackendError;
use crate::incident::{
    IncidentCategory, IncidentContext, IncidentListOptions, IncidentRecord, IncidentScope,
    IncidentSeverity, IncidentStore,
};
use crate::tool::errors::ToolError;
use engine::AgentError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_RETENTION_MAX: u32 = 500;

pub struct IncidentRecorder {
    store: Arc<dyn IncidentStore>,
    retention_max: AtomicU32,
}

impl IncidentRecorder {
    pub fn new(store: Arc<dyn IncidentStore>) -> Self {
        Self::with_retention_max(store, DEFAULT_RETENTION_MAX)
    }

    pub fn with_retention_max(store: Arc<dyn IncidentStore>, retention_max: u32) -> Self {
        Self {
            store,
            retention_max: AtomicU32::new(retention_max),
        }
    }

    pub fn set_retention_max(&self, retention_max: u32) {
        self.retention_max.store(retention_max, Ordering::Relaxed);
    }

    pub fn record(&self, record: IncidentRecord) -> io::Result<()> {
        self.store.append(&record)?;
        let max = self.retention_max.load(Ordering::Relaxed);
        if max > 0 {
            self.store.prune_to_max(max)?;
        }
        Ok(())
    }

    pub fn list_unresolved(&self, limit: usize) -> io::Result<Vec<IncidentRecord>> {
        self.store.list(Some(IncidentListOptions {
            include_resolved: false,
            limit: Some(limit),
        }))
    }

    pub fn dismiss(&self, id: &str) -> io::Result<()> {
        self.store.dismiss(id)
    }

    pub fn clear_resolved(&self) -> io::Result<usize> {
        self.store.clear_resolved()
    }

    pub fn record_backend(&self, error: &BackendError, ctx: &IncidentContext) -> io::Result<()> {
        let record = build_record(NewIncidentRecord {
            scope: scope_from_context(ctx),
            severity: IncidentSeverity::Error,
            category: IncidentCategory::Backend,
            code: backend_error_code(error).to_string(),
            message: error.to_string(),
            hint: None,
            retryable: false,
            context: context_from_incident(ctx),
        });
        self.record(record)
    }

    pub fn record_custom(
        &self,
        ctx: &IncidentContext,
        severity: IncidentSeverity,
        category: IncidentCategory,
        code: &str,
        message: &str,
    ) -> io::Result<()> {
        let record = build_record(NewIncidentRecord {
            scope: scope_from_context(ctx),
            severity,
            category,
            code: code.to_string(),
            message: message.to_string(),
            hint: None,
            retryable: false,
            context: context_from_incident(ctx),
        });
        self.record(record)
    }

    pub fn record_agent_error(&self, error: &AgentError, ctx: &IncidentContext) -> io::Result<()> {
        let (code, severity, retryable) = match error {
            AgentError::Transient(_) => ("ai.transient", IncidentSeverity::Error, true),
            AgentError::Permanent(_) => ("ai.permanent", IncidentSeverity::Error, false),
            AgentError::Failed(_) => ("ai.failed", IncidentSeverity::Error, false),
            AgentError::Interrupted => ("ai.interrupted", IncidentSeverity::Warning, false),
        };
        let record = build_record(NewIncidentRecord {
            scope: scope_from_context(ctx),
            severity,
            category: IncidentCategory::AiInvoke,
            code: code.to_string(),
            message: error.to_string(),
            hint: None,
            retryable,
            context: context_from_incident(ctx),
        });
        self.record(record)
    }
}

pub fn incident_from_tool_error(
    error: &ToolError,
    tool_call_id: &str,
    ctx: &IncidentContext,
) -> IncidentRecord {
    let (code, hint, tool_name) = match error {
        ToolError::NotFound { hint, .. } => ("tool.not_found", Some(hint.clone()), None),
        ToolError::PermissionDenied { hint, .. } => {
            ("tool.permission_denied", Some(hint.clone()), None)
        }
        ToolError::InvalidArgs { tool, hint, .. } => {
            ("tool.invalid_args", Some(hint.clone()), Some(tool.clone()))
        }
        ToolError::Timeout { tool, hint, .. } => {
            ("tool.timeout", Some(hint.clone()), Some(tool.clone()))
        }
        ToolError::Cancelled { tool } => ("tool.cancelled", None, Some(tool.clone())),
        ToolError::ExecutionFailed { hint, .. } => ("tool.failed", hint.clone(), None),
    };
    let mut context = context_from_incident(ctx);
    context.insert("toolCallId".to_string(), json!(tool_call_id));
    if let Some(name) = tool_name {
        context.insert("toolName".to_string(), json!(name));
    }
    build_record(NewIncidentRecord {
        scope: scope_from_context(ctx),
        severity: IncidentSeverity::Error,
        category: IncidentCategory::Tool,
        code: code.to_string(),
        message: error.to_string(),
        hint,
        retryable: error.is_retryable(),
        context,
    })
}

pub(crate) fn scope_from_context(ctx: &IncidentContext) -> IncidentScope {
    if let (Some(run_id), Some(workflow_id), Some(node_id)) =
        (&ctx.run_id, &ctx.workflow_id, &ctx.node_id)
    {
        IncidentScope::Node {
            run_id: run_id.clone(),
            workflow_id: workflow_id.clone(),
            node_id: node_id.clone(),
        }
    } else if let (Some(run_id), Some(workflow_id)) = (&ctx.run_id, &ctx.workflow_id) {
        IncidentScope::Run {
            run_id: run_id.clone(),
            workflow_id: workflow_id.clone(),
        }
    } else if let Some(project_id) = &ctx.project_id {
        IncidentScope::Project {
            project_id: project_id.clone(),
        }
    } else {
        IncidentScope::App
    }
}

pub(crate) fn context_from_incident(ctx: &IncidentContext) -> BTreeMap<String, Value> {
    let mut context = BTreeMap::new();
    if let Some(label) = &ctx.node_label {
        context.insert("nodeLabel".to_string(), json!(label));
    }
    context
}

pub(crate) struct NewIncidentRecord {
    pub scope: IncidentScope,
    pub severity: IncidentSeverity,
    pub category: IncidentCategory,
    pub code: String,
    pub message: String,
    pub hint: Option<String>,
    pub retryable: bool,
    pub context: BTreeMap<String, Value>,
}

pub(crate) fn build_record(input: NewIncidentRecord) -> IncidentRecord {
    IncidentRecord {
        id: Uuid::new_v4().to_string(),
        created_at_ms: now_ms(),
        severity: input.severity,
        category: input.category,
        scope: input.scope,
        code: input.code,
        message: input.message,
        hint: input.hint,
        retryable: input.retryable,
        context: input.context,
        resolved: false,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn backend_error_code(error: &BackendError) -> &'static str {
    match error {
        BackendError::Io(_) => "backend.io",
        BackendError::Validation(_) => "backend.validation",
        BackendError::ProviderConfig(_) => "backend.provider_config",
        BackendError::WorkflowNotFound(_) => "backend.workflow_not_found",
        BackendError::ProjectNotFound(_) => "backend.project_not_found",
        BackendError::AgentNotFound(_) => "backend.agent_not_found",
        BackendError::InvalidExecutionCwd(_) => "backend.invalid_execution_cwd",
        BackendError::ProjectOperation(_) => "backend.project_operation",
        BackendError::NoActiveRun => "backend.no_active_run",
        BackendError::NoExecutionCwd => "backend.no_execution_cwd",
        BackendError::NoAwaitingInput => "backend.no_awaiting_input",
        BackendError::NoPendingApproval => "backend.no_pending_approval",
        BackendError::WrongAwaitingNode { .. } => "backend.wrong_awaiting_node",
        BackendError::WrongApprovalId { .. } => "backend.wrong_approval_id",
        BackendError::RunChannelClosed => "backend.run_channel_closed",
        BackendError::PreviewFailed(_) => "backend.preview_failed",
        BackendError::AuthoringFailed(_) => "backend.authoring_failed",
        BackendError::GitFailed(_) => "backend.git_failed",
        BackendError::EditBatchNotFound(_) => "backend.edit_batch_not_found",
        BackendError::NodeNotInterruptible(_) => "backend.node_not_interruptible",
        BackendError::NodeNotRetryable(_) => "backend.node_not_retryable",
        BackendError::NoContinuableRun => "backend.no_continuable_run",
        BackendError::CheckpointWorkflowMismatch => "backend.checkpoint_workflow_mismatch",
        BackendError::CheckpointIncompatible(_) => "backend.checkpoint_incompatible",
        BackendError::Schedule(_) => "backend.schedule",
        BackendError::RunNotFound(_) => "backend.run_not_found",
        BackendError::RunHasNoCheckpoints(_) => "backend.run_has_no_checkpoints",
        BackendError::RunWorkflowChanged(_, _) => "backend.run_workflow_changed",
    }
}
