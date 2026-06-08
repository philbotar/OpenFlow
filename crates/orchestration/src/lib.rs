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

pub mod agent_library;
pub mod agent_store;
pub mod api;
pub mod backend;
pub mod error;
pub mod execution;
pub mod flow_store;
pub mod project_registry;
pub mod project_store;
pub mod provider_config;
pub mod run_coordinator;
pub mod settings_facade;
pub mod settings_store;
pub mod skill_store;
pub mod state;
pub mod storage;
pub mod template_store;
pub mod tools;
pub mod workflow_catalog;

// Re-exports of domain types consumed by downstream layers
pub use domain::{
    CallableAgent, Node, RunTelemetry, Template, TemplateStore, TemplateStoreError, Workflow,
};
pub use project_store::Project;
pub use template_store::FileTemplateStore;
