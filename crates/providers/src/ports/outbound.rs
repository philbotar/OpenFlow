//! Outbound ports for provider-internal integrations.

use domain::{AgentError, AgentTurnOutcome};

pub type ProviderInvokeResult = Result<AgentTurnOutcome, AgentError>;
