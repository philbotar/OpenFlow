//! Tool application layer: registry, execution, and output handling.
//!
//! This module provides the orchestration-level tool management:
//! - Registry: tool catalog and lookup
//! - Runner: tool execution and orchestration
//! - Output: artifact storage and tool result handling

pub mod errors;
pub mod output;
pub mod ports;
pub mod registry;
pub mod runner;

pub use output::{ArtifactStore, ToolArtifactRecord};
pub use registry::{ToolRegistry, ToolRegistryError};
pub use runner::{ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError};
