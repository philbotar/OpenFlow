// reason: `cargo clippy-max` enables `clippy::cargo`; current Tauri/WASI transitive
// dependencies pull two `wit-bindgen` versions that this crate does not select directly.
#![allow(clippy::multiple_crate_versions)]

pub mod conversation;
pub mod execution;
pub mod graph;
pub mod ports;
pub mod template;
pub mod tools;

pub use conversation::{
    filter_tool_turn_assistant_message, is_redundant_tool_call_markup, strip_tool_call_markup,
    summary_from_node_output, AgentTranscriptItem, ChatMessage, ChatMessageKind, ChatRole,
};
pub use execution::{
    advance_subagent_invoke, augment_call_subagent_tool_description, build_agent_request,
    build_node_input, build_upstream_map, handle_declare_subagents, is_subagent_runtime_builtin,
    merge_shared_context, merge_subagent_summaries, start_subagent_invoke,
    subagent_runtime_builtin_denied, workflow_system_prompt, CallSubagentArgs, EngineInputError,
    EnginePollResult, InteractiveEngine, NodeInvocationContext, NodeRunOutput, RunError, RunEvent,
    RunEventKind, RunReport, RunTelemetry, SubagentInvokeSession, SubagentInvokeStep,
    SubagentStartOutcome,
    WorkflowRunner, CALL_SUBAGENT_TOOL, DECLARE_SUBAGENTS_TOOL,
};
pub use graph::{
    build_predefined_subagent_summaries, execution_layers, resolve_callable_agent_snapshots,
    validate_workflow, AgentNodeConfig, CallableAgent, Edge, EdgeId, Node, NodeId, NodeKind,
    NodePosition, RetryPolicy, Workflow, WorkflowId, WorkflowSchedule, WorkflowSettings,
    WorkflowValidationError,
};
pub use ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, HumanInput, HumanInputPort, ToolApprovalInput, ToolApprovalPort,
};
pub use template::{default_templates, Template, TemplateStore, TemplateStoreError};
pub use tools::{
    override_policy_for_call, requires_approval, tool_tier_for_call, ApprovalMode, NodeToolConfig,
    PendingToolApproval, SubagentDeclaration, SubagentStatus, SubagentSummary, ToolCall,
    ToolCallStatus, ToolCatalogSelection, ToolConcurrency, ToolDecision, ToolDefinition,
    ToolOutputMeta, ToolPolicy, ToolPolicyOverride, ToolRef, ToolResult, ToolTier, ToolTruncation,
    ToolTruncationStrategy,
};
