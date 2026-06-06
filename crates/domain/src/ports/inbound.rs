//! Inbound ports owned by the domain.

use crate::NodeId;

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
	fn submit_human_input(&mut self, input: HumanInput) -> Result<(), String>;
}

pub trait ToolApprovalPort {
	fn submit_tool_approval(&mut self, input: ToolApprovalInput) -> Result<(), String>;
}