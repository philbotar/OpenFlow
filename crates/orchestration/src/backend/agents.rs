use engine::CallableAgent;

use super::{AgentDefinitionSummary, AppBackend, BackendError};

impl AppBackend {
    pub fn load_agents(&self) -> Result<Vec<CallableAgent>, BackendError> {
        self.agents.load()
    }

    pub fn save_agents(&self, agents: &[CallableAgent]) -> Result<(), BackendError> {
        self.agents.save(agents)
    }

    pub fn create_agent_definition(&self, name: String) -> Result<CallableAgent, BackendError> {
        self.agents.create(name)
    }

    pub fn list_agents(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        self.agents.list()
    }
}
