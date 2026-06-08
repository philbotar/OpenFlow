use crate::agent_store::{AgentDefinition, FileAgentStore};
use crate::api::AgentDefinitionSummary;
use crate::error::BackendError;
use domain::Node;

#[derive(Debug)]
pub struct AgentLibrary {
    store: FileAgentStore,
}

impl AgentLibrary {
    #[must_use]
    pub fn new(store: FileAgentStore) -> Self {
        Self { store }
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read.
    pub fn load(&self) -> Result<Vec<AgentDefinition>, BackendError> {
        self.store.load().map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be written.
    pub fn save(&self, agents: &[AgentDefinition]) -> Result<(), BackendError> {
        self.store.save(agents).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be written.
    pub fn create(&self, name: String) -> Result<AgentDefinition, BackendError> {
        let mut agents = self.store.load()?;
        let agent = AgentDefinition::new(name);
        agents.push(agent.clone());
        self.store.save(&agents)?;
        Ok(agent)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read.
    pub fn list(&self) -> Result<Vec<AgentDefinitionSummary>, BackendError> {
        Ok(self
            .store
            .load()?
            .into_iter()
            .map(|agent| AgentDefinitionSummary {
                id: agent.id,
                name: agent.name,
                model: agent.model,
            })
            .collect())
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read or the selected agent does not exist.
    pub fn create_node(
        &self,
        index: usize,
        x: f32,
        y: f32,
        agent_id: Option<&str>,
    ) -> Result<Node, BackendError> {
        let default_name = format!("Agent {}", index + 1);
        let Some(agent_id) = agent_id else {
            return Ok(Node::agent(default_name, x, y));
        };

        let agents = self.store.load()?;
        let agent = agents
            .iter()
            .find(|agent| agent.id == agent_id)
            .ok_or_else(|| BackendError::AgentNotFound(agent_id.to_string()))?;

        let label = if agent.name.trim().is_empty() {
            default_name
        } else {
            agent.name.clone()
        };
        let mut node = Node::agent(label, x, y);
        node.agent.system_prompt = agent.system_prompt.clone();
        node.agent.task_prompt = agent.task_prompt.clone();
        node.agent.model = agent.model.clone();
        node.agent.output_schema = agent.output_schema.clone();
        node.agent.auto_start = agent.auto_start;
        node.agent.tools = agent.tools.clone();

        Ok(node)
    }

    pub(crate) fn store(&self) -> &FileAgentStore {
        &self.store
    }
}
