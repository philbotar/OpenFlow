//! Tool implementation: edit tool, patching, and file system operations.

pub mod edit;
pub mod errors;

// Re-export modules from tool application layer for backward compatibility
pub mod output {
    pub use crate::tool_output::*;
}

pub mod registry {
    pub use crate::tool_registry::*;
}

pub mod runner {
    pub use crate::tool_runner::*;
}

pub use crate::tool_output::{ArtifactStore, ToolArtifactRecord};
pub use crate::tool_registry::{ToolRegistry, ToolRegistryError};
pub use crate::tool_runner::{
    ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError,
};
