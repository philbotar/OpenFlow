//! Workflow execution engines and shared node invocation.

pub mod artifacts;
pub mod interactive_engine;
pub mod node_invocation;
pub mod subagent_runtime;
pub mod subagents;
pub mod telemetry;
pub mod workflow_runner;

pub use artifacts::{NodeRunOutput, RunError, RunEvent, RunEventKind, RunReport};
pub use interactive_engine::{EngineInputError, EnginePollResult, InteractiveEngine};
pub use node_invocation::{
    build_agent_request, build_node_input, build_upstream_map, merge_shared_context,
    workflow_system_prompt, NodeInvocationContext,
};
pub use subagent_runtime::{
    advance_subagent_invoke, handle_declare_subagents, is_subagent_runtime_builtin,
    start_subagent_invoke, subagent_runtime_builtin_denied, CallSubagentArgs,
    DeclareSubagentsOutcome, SubagentInvokeSession, SubagentInvokeStep, SubagentStartOutcome,
    DECLARE_SUBAGENTS_TOOL,
};
pub use subagents::{
    adhoc_subagent_base_index, augment_call_subagent_tool_description, build_adhoc_subagent_summaries,
    merge_subagent_summaries, subagents_for_node, CALL_SUBAGENT_TOOL,
};
pub use telemetry::RunTelemetry;
pub use workflow_runner::WorkflowRunner;
