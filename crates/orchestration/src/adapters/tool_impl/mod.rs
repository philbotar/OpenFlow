//! Tool implementation: edit tool, patching, and file system operations.

pub mod bash;
pub mod edit;
pub mod grep;

// Re-export modules from tool application layer for backward compatibility
pub mod errors {
    pub use crate::tool::errors::ToolError;
}

pub mod output {
    pub use crate::tool::output::*;
}

pub mod registry {
    pub use crate::tool::registry::*;
}

pub mod runner {
    pub use crate::tool::runner::*;
}

pub use crate::tool::output::{ArtifactStore, ToolArtifactRecord};
pub use crate::tool::registry::{ToolRegistry, ToolRegistryError};
pub use crate::tool::runner::{
    ToolExecutionContext, ToolExecutionRecord, ToolRunner, ToolRunnerError,
};
