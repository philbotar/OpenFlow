//! Outbound ports for provider-internal integrations.

use workflow_core::{AgentError, AgentTurnOutcome};

pub type ProviderInvokeResult = Result<AgentTurnOutcome, AgentError>;