#![allow(
    clippy::assigning_clones,
    clippy::derive_partial_eq_without_eq,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::multiple_crate_versions,
    clippy::must_use_candidate,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::redundant_clone,
    clippy::redundant_closure_for_method_calls,
    clippy::significant_drop_tightening,
    clippy::suboptimal_flops,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unused_self
)]

pub mod api;
pub mod error;

#[path = "backend/mod.rs"]
pub mod backend;

#[path = "workflow/application/catalog.rs"]
pub mod workflow_catalog;

#[path = "workflow/adapters/flow_store.rs"]
pub mod flow_store;

#[path = "workflow/adapters/storage.rs"]
pub mod storage;

#[path = "agent/application/library.rs"]
pub mod agent_library;

#[path = "agent/adapters/store.rs"]
pub mod agent_store;

#[path = "project/application/registry.rs"]
pub mod project_registry;

#[path = "project/adapters/store.rs"]
pub mod project_store;

#[path = "run/application/coordinator.rs"]
pub mod run_coordinator;

#[path = "run/application/execution/mod.rs"]
pub mod execution;

#[path = "run/state/mod.rs"]
pub mod state;

#[path = "settings/application/facade.rs"]
pub mod settings_facade;

#[path = "settings/adapters/store.rs"]
pub mod settings_store;

#[path = "settings/adapters/provider_config.rs"]
pub mod provider_config;

#[path = "template/store.rs"]
pub mod template_store;

#[path = "skill/store.rs"]
pub mod skill_store;

#[path = "adapters/infrastructure/tools/mod.rs"]
pub mod tools;

#[path = "adapters/infrastructure/lsp/mod.rs"]
pub mod lsp;

#[path = "adapters/infrastructure/git/mod.rs"]
pub mod git;

// Re-exports of domain types consumed by downstream layers
pub use domain::{
    CallableAgent, Node, RunTelemetry, Template, TemplateStore, TemplateStoreError, Workflow,
};
pub use project_store::Project;
pub use template_store::FileTemplateStore;
