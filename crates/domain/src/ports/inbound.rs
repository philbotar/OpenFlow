//! Inbound ports owned by the domain.

use crate::{EngineInputError, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanInput {
    pub node_id: NodeId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApprovalInput {
    pub approval_id: String,
    pub allow: bool,
}

pub trait HumanInputPort {
    /// # Errors
    /// Returns an error when the engine is not awaiting this node's human input.
    fn submit_human_input(&mut self, input: HumanInput) -> Result<(), EngineInputError>;
}

pub trait ToolApprovalPort {
    /// # Errors
    /// Returns an error when the engine has no matching pending tool approval.
    fn submit_tool_approval(&mut self, input: ToolApprovalInput) -> Result<(), EngineInputError>;
}
