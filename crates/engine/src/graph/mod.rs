//! Workflow graph model and DAG validation.

pub mod callable_agent;
pub mod runtime_config;
pub mod validation;
pub mod workflow;

pub use callable_agent::{
    build_predefined_subagent_summaries, resolve_callable_agent_snapshots, CallableAgent,
};
pub use runtime_config::{
    apply_runtime_patch_to_agent, apply_runtime_patch_to_request,
    apply_runtime_patch_to_tool_config, new_runtime_config_store, runtime_patch_for,
    upsert_runtime_patch, NodeRuntimeConfigPatch, NodeRuntimeConfigStore,
};
pub use validation::{execution_layers, validate_workflow, WorkflowValidationError};
pub(crate) use workflow::default_structured_output_schema;
pub use workflow::{
    default_markdown_handoff_template, effective_output_schema, submission_output_schema,
    validate_markdown_handoff, validate_markdown_handoff_template, AgentNodeConfig, Edge, EdgeId,
    HandoffSpec, MarkdownHandoffError, Node, NodeId, NodeKind, NodePosition, PlanModeConfig,
    RetryPolicy, Workflow, WorkflowId, WorkflowSchedule, WorkflowSettings,
};
