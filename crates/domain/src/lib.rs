#![allow(
    clippy::doc_markdown,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::multiple_crate_versions,
    clippy::needless_pass_by_value,
    clippy::redundant_clone,
    clippy::significant_drop_tightening,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::use_self,
    clippy::derive_partial_eq_without_eq
)]

pub mod adapters;
pub mod interactive;
pub mod model;
pub mod ports;
pub mod runner;
pub mod template;
pub mod template_store;
pub mod tools;
pub mod validation;

pub use interactive::{EnginePollResult, InteractiveEngine};
pub use model::{
    AgentNodeConfig, ChatMessage, ChatRole, Edge, EdgeId, Node, NodeId, NodeKind, NodePosition,
    NodeRunOutput, NodeTemplate, RunEvent, RunEventKind, RunReport, Workflow, WorkflowId,
};
pub use ports::{
    AgentError, AgentNeedUserInput, AgentRequest, AgentToolCallBatch, AgentTurnOutcome,
    AgentTurnSuccess, AiPort,
};
pub use runner::{RunError, WorkflowRunner};
pub use tools::{
    AgentTranscriptItem, ApprovalMode, NodeToolConfig, PendingToolApproval, ToolCall,
    ToolCallStatus, ToolCatalogSelection, ToolConcurrency, ToolDefinition, ToolOutputMeta,
    ToolPolicy, ToolPolicyOverride, ToolRef, ToolResult, ToolTier, ToolTruncation,
    ToolTruncationStrategy,
};
pub use validation::{execution_layers, validate_workflow, WorkflowValidationError};
