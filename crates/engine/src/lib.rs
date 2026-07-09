// reason: `cargo clippy-max` enables `clippy::cargo`; current Tauri/WASI transitive
// dependencies pull two `wit-bindgen` versions that this crate does not select directly.
#![allow(
    clippy::multiple_crate_versions,
    reason = "transitive dependency version duplicates are not selected by this crate"
)]

pub mod conversation;
pub mod execution;
pub mod graph;
pub mod ports;
pub mod template;
pub mod tools;

pub use conversation::{
    filter_tool_turn_assistant_message, strip_tool_call_markup, summary_from_node_output,
    AgentTranscriptItem, ChatMessage, ChatMessageKind, ChatRole,
};
pub use execution::{
    advance_subagent_invoke, augment_call_subagent_tool_description, handle_declare_subagents,
    is_subagent_runtime_builtin, merge_subagent_summaries, start_subagent_invoke,
    subagent_runtime_builtin_denied, validate_checkpoint_against_workflow, EngineAwaitApproval,
    EngineAwaitInput, EngineRetryableNode, EngineRunResult, InteractiveEngine,
    InteractiveEngineCheckpoint, NodeRunOutput, RunError, RunReport, RunTelemetry,
    SubagentInvokeSession, SubagentInvokeStep, SubagentStartOutcome, CALL_SUBAGENT_TOOL,
    DECLARE_SUBAGENTS_TOOL,
};
pub use graph::{
    apply_runtime_patch_to_agent, apply_runtime_patch_to_request,
    apply_runtime_patch_to_tool_config, build_predefined_subagent_summaries,
    effective_output_schema, execution_layers, new_runtime_config_store, resolve_callable_agent_snapshots,
    runtime_patch_for, upsert_runtime_patch, validate_workflow, AgentNodeConfig, CallableAgent,
    Edge, EdgeId, Node, NodeId, NodeKind, NodePosition, NodeRuntimeConfigPatch,
    NodeRuntimeConfigStore, RetryPolicy, Workflow, WorkflowId, WorkflowSchedule, WorkflowSettings,
    WorkflowValidationError,
};
pub use ports::{
    emit_assistant_deltas_from_outcome, AgentError, AgentNeedUserInput, AgentRequest,
    AgentToolCallBatch, AgentTurnOutcome, AgentTurnSuccess, AiPort, AiStreamEvent, AiStreamSink,
    ToolBatchEffects, ToolBatchOutput, ToolPort, UsageReport,
};
pub use template::{default_templates, Template, TemplateStore, TemplateStoreError};
pub use tools::{
    summarize_diff, tool_intent_from_arguments, tool_tier_for_call, ApprovalMode, EditBatch,
    FileChangeOp, FileChangeRecord, FileSnapshot, NodeToolConfig, PendingToolApproval, ReadRecord,
    SubagentStatus, SubagentSummary, ToolCall, ToolCallStatus, ToolConcurrency, ToolDefinition,
    ToolOutputMeta, ToolResult, ToolTier, ToolTruncation, ToolTruncationStrategy,
};
