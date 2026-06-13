//! Workflow execution engines and shared node invocation.

pub mod artifacts;
pub mod interactive_engine;
pub mod node_invocation;
pub(crate) mod retry;
pub mod subagent_runtime;
pub mod subagents;
pub mod telemetry;
pub(crate) mod tool_results;
pub mod workflow_runner;

pub use artifacts::{NodeFailureKind, NodeRunOutput, RunError, RunEvent, RunEventKind, RunReport};
pub use interactive_engine::{
    CheckpointError, EngineAwaitApproval, EngineAwaitInput, EngineInputError, EnginePollResult,
    EngineRetryableNode, EngineRunResult, InteractiveEngine, InteractiveEngineCheckpoint,
};
pub use node_invocation::{
    build_agent_request, build_node_input, build_system_messages, build_upstream_map,
    merge_shared_context, upstream_changed_files, NodeInvocationContext, NODE_RUNTIME_PREAMBLE,
};
pub use subagent_runtime::{
    advance_subagent_invoke, handle_declare_subagents, is_subagent_runtime_builtin,
    start_subagent_invoke, subagent_runtime_builtin_denied, CallSubagentArgs,
    DeclareSubagentsOutcome, SubagentInvokeSession, SubagentInvokeStep, SubagentStartOutcome,
    DECLARE_SUBAGENTS_TOOL,
};
pub use subagents::{
    adhoc_subagent_base_index, augment_call_subagent_tool_description,
    build_adhoc_subagent_summaries, merge_subagent_summaries, subagents_for_node,
    CALL_SUBAGENT_TOOL,
};
pub use telemetry::RunTelemetry;
pub use workflow_runner::WorkflowRunner;
