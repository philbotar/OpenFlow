// reason: `cargo clippy-max` enables `clippy::cargo`; current Tauri/WASI transitive
// dependencies pull two `wit-bindgen` versions that this crate does not select directly.
#![allow(clippy::multiple_crate_versions)]

pub mod interactive;
pub mod model;
pub mod ports;
pub mod runner;
pub mod template;
pub mod template_store;
pub mod tools;
pub mod validation;

pub use interactive::{EngineInputError, EnginePollResult, InteractiveEngine};
pub use model::{
    filter_tool_turn_assistant_message, is_redundant_tool_call_markup, AgentNodeConfig,
    ChatMessage, ChatRole, Edge, EdgeId, Node, NodeId, NodeKind, NodePosition, NodeRunOutput,
    RetryPolicy, RunEvent, RunEventKind, RunReport, Workflow, WorkflowId, WorkflowSchedule,
    WorkflowSettings,
};
pub use ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, HumanInput, HumanInputPort, ToolApprovalInput, ToolApprovalPort,
};
pub use runner::{RunError, WorkflowRunner};
pub use template::{default_templates, Template};
pub use tools::{
    override_policy_for_call, requires_approval, tool_tier_for_call, AgentTranscriptItem,
    ApprovalMode, NodeToolConfig, PendingToolApproval, SubagentDeclaration, SubagentStatus,
    SubagentSummary, ToolCall, ToolCallStatus, ToolCatalogSelection, ToolConcurrency, ToolDecision,
    ToolDefinition, ToolOutputMeta, ToolPolicy, ToolPolicyOverride, ToolRef, ToolResult, ToolTier,
    ToolTruncation, ToolTruncationStrategy,
};
pub use validation::{execution_layers, validate_workflow, WorkflowValidationError};
