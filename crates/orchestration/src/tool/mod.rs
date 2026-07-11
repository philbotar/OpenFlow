//! Tool application layer: registry, execution, and output handling.
//!
//! This module provides the orchestration-level tool management:
//! - Registry: tool catalog and lookup
//! - Runner: tool execution and orchestration
//! - Output: artifact storage and tool result handling

pub(crate) mod blocking_ops;
pub mod cache;
pub mod errors;
pub mod output;
pub(crate) mod read;
pub mod registry;
pub mod retry;
pub mod runner;
pub(crate) mod web_search;

pub use cache::ToolResultCache;
pub use output::{ArtifactStore, ToolArtifactRecord};
pub use registry::{ToolRegistry, ToolRegistryError};
pub use runner::{
    ToolExecutionContext, ToolExecutionRecord, ToolExecutionUpdate, ToolRunner, ToolRunnerError,
};
pub use web_search::set_bundled_search_binary;
