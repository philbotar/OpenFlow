//! Domain-level outbound adapters.

use crate::{AgentError, AgentRequest, AgentTurnOutcome, AiPort};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

pub type DynAiPort = Box<dyn AiPort>;

#[must_use]
pub fn boxed_ai_port(port: impl AiPort + 'static) -> DynAiPort {
    Box::new(port)
}

#[derive(Debug, Clone)]
pub struct ScriptedAiAdapter {
    outcomes: Arc<Mutex<VecDeque<Result<AgentTurnOutcome, AgentError>>>>,
}

impl ScriptedAiAdapter {
    #[must_use]
    pub fn from_outcomes(
        outcomes: impl IntoIterator<Item = Result<AgentTurnOutcome, AgentError>>,
    ) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into_iter().collect())),
        }
    }
}

#[async_trait]
impl AiPort for ScriptedAiAdapter {
    async fn invoke(&self, _request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
        self.outcomes.lock().pop_front().unwrap_or_else(|| {
            Err(AgentError::Permanent(
                "ScriptedAiAdapter exhausted outcomes".to_string(),
            ))
        })
    }
}
