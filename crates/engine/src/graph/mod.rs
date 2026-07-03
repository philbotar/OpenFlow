//! Workflow graph model and DAG validation.

pub mod callable_agent;
pub mod validation;
pub mod workflow;

pub use callable_agent::{
    build_predefined_subagent_summaries, resolve_callable_agent_snapshots, CallableAgent,
};
pub use validation::{execution_layers, validate_workflow, WorkflowValidationError};
pub(crate) use workflow::default_structured_output_schema;
pub use workflow::{
    effective_output_schema, AgentNodeConfig, Edge, EdgeId, Node, NodeId, NodeKind, NodePosition,
    RetryPolicy, Workflow, WorkflowId, WorkflowSchedule, WorkflowSettings,
};
