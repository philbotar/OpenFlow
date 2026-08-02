//! Tool application layer: registry, execution, and output handling.
//!
//! This module provides the orchestration-level tool management:
//! - Registry: tool catalog and lookup
//! - Runner: tool execution and orchestration
//! - Output: artifact storage and tool result handling

pub(crate) mod blocking_ops;
pub mod cache;
pub mod errors;
pub(crate) mod file_change;
pub mod output;
mod project_read;
pub(crate) mod read;
pub mod registry;
pub mod retry;
pub mod runner;
pub(crate) mod web_search;

pub use cache::ToolResultCache;
pub(crate) use file_change::CapturedFileChange;
pub use output::{ArtifactStore, PlanArtifact, ToolArtifactRecord, MAX_PLAN_ARTIFACT_BYTES};
pub(crate) use project_read::ProjectReadTools;
pub use registry::{ToolRegistry, ToolRegistryError};
pub use runner::{
    ToolExecutionContext, ToolExecutionRecord, ToolExecutionUpdate, ToolRunner, ToolRunnerError,
};
pub use web_search::set_bundled_search_binary;
