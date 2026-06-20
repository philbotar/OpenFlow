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
pub mod error;
pub mod incident;
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
    CallableAgent, Node, NodeId, NodeRunOutput, PendingToolApproval, RunReport, RunTelemetry,
    Template, TemplateStore, TemplateStoreError, ToolCall, ToolTier, Workflow, WorkflowId,
};
pub use project::ports::Project;
pub use settings::model::{AppSettings, SkillSummary};
pub use settings::ports::{LspSettings, ProviderProfile};
