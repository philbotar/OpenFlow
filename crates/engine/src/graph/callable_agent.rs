//! Saved agent definitions invocable as subagents during a run.

use super::workflow::{AgentNodeConfig, Node, Workflow};
use crate::tools::{SubagentStatus, SubagentSummary};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

/// A saved agent definition snapshotted at run start for subagent invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallableAgent {
    pub id: String,
    pub name: String,
    #[serde(default, alias = "systemPrompt")]
    pub system_prompt: String,
    #[serde(default, alias = "taskPrompt")]
    pub task_prompt: String,
    #[serde(default)]
    pub model: String,
    #[serde(alias = "outputSchema")]
    pub output_schema: Value,
    #[serde(default, alias = "autoStart")]
    pub auto_start: bool,
    #[serde(default)]
    pub tools: crate::NodeToolConfig,
}

impl CallableAgent {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let defaults = AgentNodeConfig::default();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            system_prompt: defaults.system_prompt,
            task_prompt: defaults.task_prompt,
            model: defaults.model,
            output_schema: defaults.output_schema,
            auto_start: defaults.auto_start,
            tools: defaults.tools,
        }
    }

    /// First non-empty line of `task_prompt`, or a generic label.
    #[must_use]
    pub fn purpose(&self) -> String {
        let first_line = self.task_prompt.lines().next().unwrap_or("").trim();
        if first_line.is_empty() {
            "Saved agent".to_string()
        } else {
            first_line.to_string()
        }
    }

    #[must_use]
    pub fn to_subagent_summary(&self) -> SubagentSummary {
        SubagentSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            purpose: self.purpose(),
            status: SubagentStatus::Declared,
        }
    }
}

/// Collect snapshotted callable agents referenced by workflow node settings.
#[must_use]
pub fn resolve_callable_agent_snapshots(
    workflow: &Workflow,
    agents: &[CallableAgent],
) -> BTreeMap<String, CallableAgent> {
    let mut requested = HashSet::new();
    for node in &workflow.nodes {
        if node.agent.allow_all_callable_agents {
            for agent in agents {
                requested.insert(agent.id.clone());
            }
        } else {
            for id in &node.agent.callable_agents {
                if !id.trim().is_empty() {
                    requested.insert(id.clone());
                }
            }
        }
    }
    agents
        .iter()
        .filter(|agent| requested.contains(&agent.id))
        .map(|agent| (agent.id.clone(), agent.clone()))
        .collect()
}

/// Build declared subagent summaries for saved agents configured on a node.
#[must_use]
pub fn build_predefined_subagent_summaries(
    node: &Node,
    agent_snapshots: &BTreeMap<String, CallableAgent>,
) -> Vec<SubagentSummary> {
    if node.agent.allow_all_callable_agents {
        return agent_snapshots
            .values()
            .map(CallableAgent::to_subagent_summary)
            .collect();
    }

    node.agent
        .callable_agents
        .iter()
        .filter_map(|id| agent_snapshots.get(id))
        .map(CallableAgent::to_subagent_summary)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeId, Workflow};

    fn workflow() -> Workflow {
        let mut workflow = Workflow::new("Test");
        workflow.nodes.push(crate::Node::agent("First", 0.0, 0.0));
        workflow.nodes[0].id = NodeId("first".to_string());
        workflow
    }

    #[test]
    fn resolve_callable_agent_snapshots_collects_referenced_agents() {
        let mut workflow = workflow();
        workflow.nodes[0].agent.callable_agents = vec![
            "agent-a".to_string(),
            "missing".to_string(),
            "agent-b".to_string(),
        ];
        let agents = [CallableAgent::new("Alpha"), CallableAgent::new("Beta")];
        let mut agent_a = agents[0].clone();
        agent_a.id = "agent-a".to_string();
        let mut agent_b = agents[1].clone();
        agent_b.id = "agent-b".to_string();

        let snapshots = resolve_callable_agent_snapshots(&workflow, &[agent_a.clone(), agent_b]);

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots["agent-a"].name, "Alpha");
        assert_eq!(snapshots["agent-b"].name, "Beta");
    }

    #[test]
    fn resolve_callable_agent_snapshots_includes_all_agents_when_allow_all() {
        let mut workflow = workflow();
        workflow.nodes[0].agent.allow_all_callable_agents = true;
        let agents = [CallableAgent::new("Alpha"), CallableAgent::new("Beta")];
        let mut agent_a = agents[0].clone();
        agent_a.id = "agent-a".to_string();
        let mut agent_b = agents[1].clone();
        agent_b.id = "agent-b".to_string();

        let snapshots = resolve_callable_agent_snapshots(&workflow, &[agent_a, agent_b]);

        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn purpose_uses_first_task_prompt_line() {
        let mut agent = CallableAgent::new("Research");
        agent.task_prompt = "Summarize findings\nMore detail".to_string();
        assert_eq!(agent.purpose(), "Summarize findings");
    }

    #[test]
    fn purpose_falls_back_when_task_prompt_empty() {
        let mut agent = CallableAgent::new("Research");
        agent.task_prompt.clear();
        assert_eq!(agent.purpose(), "Saved agent");
    }
}
