pub mod adapters;
pub mod agent;
pub mod api;
pub mod backend;
pub mod error;
pub mod project;
pub mod settings;
pub mod workflow;

// Re-expose public domain modules
#[path = "agent/library.rs"]
pub mod agent_library;

#[path = "workflow/catalog.rs"]
pub mod workflow_catalog;

#[path = "project/registry.rs"]
pub mod project_registry;

#[path = "run/coordinator.rs"]
pub mod run_coordinator;

#[path = "run/execution/mod.rs"]
pub mod execution;

#[path = "run/state/mod.rs"]
pub mod state;

#[path = "settings/facade.rs"]
pub mod settings_facade;

// Tool application modules (domain-level)
#[path = "tool/errors.rs"]
pub(crate) mod tool_errors;

#[path = "tool/ports.rs"]
pub(crate) mod tool_ports;

#[path = "tool/registry.rs"]
pub mod tool_registry;

#[path = "tool/runner.rs"]
pub mod tool_runner;

#[path = "tool/output.rs"]
pub mod tool_output;

// Storage adapters (internal); aliases avoid compiling storage modules twice.
pub(crate) use adapters::storage::project_store;
pub(crate) use adapters::storage::settings_store;
pub(crate) use adapters::storage::skill_store;
pub(crate) use adapters::storage::workflow_storage as storage;
pub(crate) use adapters::storage::workflow_store as flow_store;

// Tool implementation (adapters - internal); alias avoids compiling tool_impl twice.
pub(crate) use adapters::tool_impl as tools;

// Infrastructure adapters; aliases avoid compiling infrastructure modules twice.
pub use adapters::infrastructure::git;
pub use adapters::infrastructure::lsp;

// Re-exports of engine types consumed by downstream layers
pub use engine::CallableAgent as AgentDefinition;
pub use engine::{
    CallableAgent, Node, RunTelemetry, Template, TemplateStore, TemplateStoreError, Workflow,
};
pub use project::ports::Project;
pub use settings::model::{AppSettings, SkillSummary};
pub use settings::ports::{LspSettings, ProviderProfile};
