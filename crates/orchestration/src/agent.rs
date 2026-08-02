use crate::api::AgentDefinitionSummary;
use crate::error::BackendError;
use engine::{
    AgentMessageTurn, AgentNeedUserInput, AgentRequest, AgentTranscriptItem, AgentTurnOutcome,
    AgentTurnSuccess, AiPort, CallableAgent, Node, NodeId, ToolAccessPolicy, WorkflowId,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io;

const AGENT_AUTHORING_SYSTEM_PROMPT: &str = include_str!("agent_authoring_system.txt");

pub trait AgentStore: Send + Sync {
    fn load(&self) -> io::Result<Vec<CallableAgent>>;
    fn save(&self, agents: &[CallableAgent]) -> io::Result<()>;
}

pub struct AgentLibrary {
    store: Box<dyn AgentStore>,
}

impl AgentLibrary {
    #[must_use]
    pub fn new(store: Box<dyn AgentStore>) -> Self {
        Self { store }
    }

    /// # Errors
    /// Returns an error if the agent store cannot be read.
    pub fn load(&self) -> Result<Vec<CallableAgent>, BackendError> {
        self.store.load().map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be written.
    pub fn save(&self, agents: &[CallableAgent]) -> Result<(), BackendError> {
        self.store.save(agents).map_err(BackendError::from)
    }

    /// # Errors
    /// Returns an error if the agent store cannot be written.
    pub fn create(&self, name: String) -> Result<CallableAgent, BackendError> {
        let mut agents = self.store.load()?;
        let agent = CallableAgent::new(name);
        agents.push(agent.clone());
        self.store.save(&agents)?;
        Ok(agent)
    }

    /// Generate and persist one reusable agent definition.
    ///
    /// # Errors
    /// Returns an error when the description is empty, the AI response is invalid, or the agent
    /// store cannot be read or written.
    pub async fn create_with_ai(
        &self,
        description: String,
        model: String,
        reasoning_effort: Option<String>,
        reasoning_budget_tokens: Option<u32>,
        ai: &dyn AiPort,
    ) -> Result<CallableAgent, BackendError> {
        let description = description.trim();
        if description.is_empty() {
            return Err(BackendError::AgentAuthoringFailed(
                "describe the agent you want to create".to_string(),
            ));
        }

        let request = AgentRequest {
            workflow_id: WorkflowId::from("agent-authoring"),
            node_id: NodeId::from("agent-authoring"),
            node_label: "Agent authoring".to_string(),
            model: model.clone(),
            provider_id: None,
            max_output_tokens: None,
            system_messages: vec![AGENT_AUTHORING_SYSTEM_PROMPT.to_string()],
            task_prompt: "Create a complete reusable agent definition from the user's description."
                .to_string(),
            input: json!({ "description": description }),
            output_schema: agent_authoring_output_schema(),
            tool_config: Default::default(),
            available_tools: Vec::new(),
            transcript: vec![AgentTranscriptItem::UserMessage {
                content: description.to_string(),
                attachments: Vec::new(),
            }],
            entrypoint_attachments: Vec::new(),
            resolved_attachments: Default::default(),
            model_attempt: 1,
            reasoning_effort,
            reasoning_budget_tokens,
            fast_mode: false,
            allow_user_input: false,
            conversation_mode: false,
            tool_access_policy: ToolAccessPolicy::Execution,
        };

        let outcome = ai.invoke(request).await.map_err(|error| {
            BackendError::AgentAuthoringFailed(format!("provider request failed: {error}"))
        })?;
        let output = match outcome {
            AgentTurnOutcome::Completed(AgentTurnSuccess { output, .. }) => output,
            AgentTurnOutcome::Message(AgentMessageTurn {
                assistant_message, ..
            }) => {
                return Err(BackendError::AgentAuthoringFailed(format!(
                    "model returned text instead of an agent definition: {assistant_message}"
                )));
            }
            AgentTurnOutcome::NeedsUserInput(AgentNeedUserInput {
                assistant_message, ..
            }) => {
                return Err(BackendError::AgentAuthoringFailed(format!(
                    "model requested more input instead of creating the agent: {assistant_message}"
                )));
            }
            AgentTurnOutcome::ToolCalls(_) => {
                return Err(BackendError::AgentAuthoringFailed(
                    "model attempted unsupported tool calls".to_string(),
                ));
            }
        };

        let draft: AgentAuthoringDraft = serde_json::from_value(output).map_err(|error| {
            BackendError::AgentAuthoringFailed(format!(
                "model returned an invalid agent definition: {error}"
            ))
        })?;
        let agent = materialize_agent_draft(draft, model)?;
        let mut agents = self.store.load()?;
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
        node.agent.handoff = agent.handoff.clone();
        node.agent.tools = agent.tools.clone();

        Ok(node)
    }

    pub(crate) fn store(&self) -> &dyn AgentStore {
        &*self.store
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentAuthoringDraft {
    name: String,
    system_prompt: String,
    task_prompt: String,
    output_schema_json: String,
}

fn materialize_agent_draft(
    draft: AgentAuthoringDraft,
    model: String,
) -> Result<CallableAgent, BackendError> {
    let name = required_agent_text("name", draft.name)?;
    let system_prompt = required_agent_text("system prompt", draft.system_prompt)?;
    let task_prompt = required_agent_text("task prompt", draft.task_prompt)?;
    let output_schema: Value =
        serde_json::from_str(&draft.output_schema_json).map_err(|error| {
            BackendError::AgentAuthoringFailed(format!(
                "model returned invalid output schema JSON: {error}"
            ))
        })?;
    if !output_schema.is_object() {
        return Err(BackendError::AgentAuthoringFailed(
            "model output schema must be a JSON object".to_string(),
        ));
    }

    let mut agent = CallableAgent::new(name);
    agent.system_prompt = system_prompt;
    agent.task_prompt = task_prompt;
    agent.model = model;
    agent.output_schema = output_schema;
    Ok(agent)
}

fn required_agent_text(field: &str, value: String) -> Result<String, BackendError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(BackendError::AgentAuthoringFailed(format!(
            "model returned an empty {field}"
        )));
    }
    Ok(value.to_string())
}

fn agent_authoring_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "name": { "type": "string" },
            "systemPrompt": { "type": "string" },
            "taskPrompt": { "type": "string" },
            "outputSchemaJson": { "type": "string" }
        },
        "required": [
            "name",
            "systemPrompt",
            "taskPrompt",
            "outputSchemaJson"
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use engine::{AgentError, AgentTurnOutcome};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemoryAgentStore {
        agents: Arc<Mutex<Vec<CallableAgent>>>,
    }

    impl AgentStore for MemoryAgentStore {
        fn load(&self) -> io::Result<Vec<CallableAgent>> {
            Ok(self.agents.lock().expect("agent store lock").clone())
        }

        fn save(&self, agents: &[CallableAgent]) -> io::Result<()> {
            *self.agents.lock().expect("agent store lock") = agents.to_vec();
            Ok(())
        }
    }

    struct AgentAuthoringAi;

    #[async_trait]
    impl AiPort for AgentAuthoringAi {
        async fn invoke(&self, request: AgentRequest) -> Result<AgentTurnOutcome, AgentError> {
            assert_eq!(request.workflow_id.0, "agent-authoring");
            assert_eq!(request.model, "gpt-test");
            assert!(request.available_tools.is_empty());
            assert!(request.system_messages[0].contains("reusable OpenFlow saved agent"));
            Ok(AgentTurnOutcome::Completed(AgentTurnSuccess {
                handoff: None,
                output: json!({
                    "name": "Research Reviewer",
                    "systemPrompt": "You review research with a skeptical eye.",
                    "taskPrompt": "Review the supplied research and identify weak claims.",
                    "outputSchemaJson": "{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{\"findings\":{\"type\":\"array\",\"items\":{\"type\":\"string\"}}},\"required\":[\"findings\"]}"
                }),
                raw_text: String::new(),
                assistant_message: None,
                reasoning: Vec::new(),
                usage: None,
            }))
        }
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn create_with_ai_materializes_and_persists_agent() {
        let store = MemoryAgentStore::default();
        let library = AgentLibrary::new(Box::new(store.clone()));

        let agent = library
            .create_with_ai(
                "Review research before publication".to_string(),
                "gpt-test".to_string(),
                Some("high".to_string()),
                None,
                &AgentAuthoringAi,
            )
            .await
            .expect("create agent with ai");

        assert_eq!(agent.name, "Research Reviewer");
        assert_eq!(agent.model, "gpt-test");
        assert!(agent.auto_start);
        assert_eq!(agent.output_schema["required"], json!(["findings"]));
        assert_eq!(store.load().expect("load agents"), vec![agent]);
    }

    #[cfg_attr(miri, ignore)]
    #[tokio::test]
    async fn create_with_ai_rejects_empty_description_before_invocation() {
        let library = AgentLibrary::new(Box::new(MemoryAgentStore::default()));

        let error = library
            .create_with_ai(
                "   ".to_string(),
                "gpt-test".to_string(),
                None,
                None,
                &AgentAuthoringAi,
            )
            .await
            .expect_err("empty description");

        assert_eq!(
            error.to_string(),
            "agent authoring failed: describe the agent you want to create"
        );
    }
}
