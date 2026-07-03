//! Workflow execution engines and shared node invocation.

pub mod artifacts;
pub mod interactive_engine;
pub mod node_invocation;
pub mod subagent_runtime;
pub mod subagents;
pub mod telemetry;
pub(crate) mod tool_results;

pub use artifacts::{NodeFailureKind, NodeRunOutput, RunError, RunReport};
pub(crate) use interactive_engine::EngineInputError;
pub use interactive_engine::{
    validate_checkpoint_against_workflow, EngineAwaitApproval, EngineAwaitInput,
    EngineRetryableNode, EngineRunResult, InteractiveEngine, InteractiveEngineCheckpoint,
};
pub(crate) use node_invocation::{build_upstream_map, upstream_reads};
pub use subagent_runtime::{
    advance_subagent_invoke, handle_declare_subagents, is_subagent_runtime_builtin,
    start_subagent_invoke, subagent_runtime_builtin_denied, DeclareSubagentsOutcome,
    SubagentInvokeSession, SubagentInvokeStep, SubagentStartOutcome, DECLARE_SUBAGENTS_TOOL,
};
pub use subagents::{
    augment_call_subagent_tool_description, merge_subagent_summaries, CALL_SUBAGENT_TOOL,
};
pub use telemetry::RunTelemetry;
