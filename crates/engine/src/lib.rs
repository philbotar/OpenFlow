// reason: `cargo clippy-max` enables `clippy::cargo`; current Tauri/WASI transitive
// dependencies pull two `wit-bindgen` versions that this crate does not select directly.
#![allow(
    clippy::multiple_crate_versions,
    reason = "transitive dependency version duplicates are not selected by this crate"
)]

pub mod conversation;
pub mod execution;
pub mod graph;
pub mod model_context;
pub mod ports;
pub mod template;
pub mod tools;

pub use conversation::{
    filter_tool_turn_assistant_message, is_clarifying_question, is_redundant_tool_call_markup,
    strip_tool_call_markup, summary_from_node_output, AgentTranscriptItem, ChatMessage,
    ChatMessageKind, ChatRole,
};
pub use execution::{
    advance_subagent_invoke, augment_call_subagent_tool_description, build_agent_request,
    build_node_input, build_system_messages, build_upstream_map, collect_checkpoint_node_ids,
    handle_declare_subagents, is_subagent_runtime_builtin, merge_shared_context,
    merge_subagent_summaries, start_subagent_invoke, subagent_runtime_builtin_denied,
    upstream_changed_files, validate_checkpoint_against_workflow, CallSubagentArgs,
    CheckpointError, EngineAwaitApproval, EngineAwaitInput, EngineInputError, EnginePollResult,
    EngineRetryableNode, EngineRunResult, InteractiveEngine, InteractiveEngineCheckpoint,
    NodeInvocationContext, NodeRunOutput, RunError, RunEvent, RunEventKind, RunReport,
    RunTelemetry, SubagentInvokeSession, SubagentInvokeStep, SubagentStartOutcome, WorkflowRunner,
    CALL_SUBAGENT_TOOL, DECLARE_SUBAGENTS_TOOL, NODE_RUNTIME_PREAMBLE,
};
pub use graph::{
    build_predefined_subagent_summaries, default_structured_output_schema, effective_output_schema,
    execution_layers, resolve_callable_agent_snapshots, validate_workflow, AgentNodeConfig,
    CallableAgent, Edge, EdgeId, Node, NodeId, NodeKind, NodePosition, RetryPolicy, Workflow,
    WorkflowId, WorkflowSchedule, WorkflowSettings, WorkflowValidationError,
};
pub use model_context::{default_context_window_sizes, lookup_context_window_size};
pub use ports::{
    emit_assistant_deltas_from_outcome, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, AiPort, AiStreamEvent, AiStreamSink,
    HumanInput, HumanInputPort, ToolApprovalInput, ToolApprovalPort, ToolPort, UsageReport,
};
pub use template::{default_templates, Template, TemplateStore, TemplateStoreError};
pub use tools::{
    requires_approval, summarize_diff, tool_decision_for_call, tool_intent_from_arguments,
    tool_tier_for_call, ApprovalMode, EditBatch, FileChangeOp, FileChangeRecord, FileSnapshot,
    NodeToolConfig, PendingToolApproval, SubagentDeclaration, SubagentStatus, SubagentSummary,
    ToolCall, ToolCallStatus, ToolConcurrency, ToolDecision, ToolDefinition, ToolOutputMeta,
    ToolResult, ToolTier, ToolTruncation, ToolTruncationStrategy,
};
