#![allow(
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    reason = "pedantic/nursery/cargo lint backlog in adapters; engine and providers are clippy-max clean"
)]

pub mod adapters;
pub mod agent;
pub mod api;
pub mod backend;
pub mod diagnostics;
pub mod error;
pub mod project;
pub mod run;
pub mod schedule;
pub mod settings;
pub mod terminal;
pub mod tool;
pub mod workflow;

// Tool implementation (adapters - internal); alias avoids compiling tool_impl twice.
pub(crate) use adapters::tool_impl as tools;

// Infrastructure adapters; aliases avoid compiling infrastructure modules twice.
pub use adapters::infrastructure::git;
pub use adapters::infrastructure::lsp;

// Re-exports of engine types consumed by downstream layers
pub use api::{ProjectFileReference, ProjectFileReferenceContent, ProjectFileReferenceKind};
pub use engine::CallableAgent as AgentDefinition;
pub use engine::{
    Node, NodeId, NodeRunOutput, PendingToolApproval, RunReport, RunTelemetry, ToolCall, ToolTier,
    Workflow, WorkflowId, WorkflowSchedule,
};
pub use project::ports::Project;
pub use settings::model::{AppSettings, McpServerConfig, McpSettings, SkillSummary};
