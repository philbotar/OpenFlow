use crate::agent_store::AgentDefinition;
use domain::{
    Node, NodeId, SubagentDeclaration, SubagentStatus, SubagentSummary, ToolDefinition, Workflow,
};
use std::collections::BTreeMap;

pub(super) fn agent_purpose(agent: &AgentDefinition) -> String {
    let first_line = agent.task_prompt.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        "Saved agent".to_string()
    } else {
        first_line.to_string()
    }
}

pub(super) fn append_shared_context(workflow: &Workflow, base: &str) -> String {
    let shared = workflow.settings.shared_context.trim();
    if shared.is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\n--- Workflow context ---\n{shared}")
    }
}

pub(super) fn adhoc_subagent_base_index(
    node_id: &NodeId,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
) -> usize {
    let prefix = format!("{node_id}-subagent-");
    declared_subagents
        .keys()
        .filter(|id| id.starts_with(&prefix))
        .count()
}

pub(super) fn build_adhoc_subagent_summaries(
    node_id: &NodeId,
    declarations: &[SubagentDeclaration],
    base_index: usize,
) -> Vec<SubagentSummary> {
    declarations
        .iter()
        .enumerate()
        .map(|(i, dec)| SubagentSummary {
            id: format!("{}-subagent-{}", node_id, base_index + i + 1),
            name: dec.name.clone(),
            purpose: dec.purpose.clone(),
            status: SubagentStatus::Declared,
        })
        .collect()
}

pub(super) fn build_predefined_subagent_summaries(
    node: &Node,
    agent_snapshots: &BTreeMap<String, AgentDefinition>,
) -> Vec<SubagentSummary> {
    if node.agent.allow_all_callable_agents {
        return agent_snapshots
            .values()
            .map(|agent| SubagentSummary {
                id: agent.id.clone(),
                name: agent.name.clone(),
                purpose: agent_purpose(agent),
                status: SubagentStatus::Declared,
            })
            .collect();
    }

    node.agent
        .callable_agents
        .iter()
        .filter_map(|id| agent_snapshots.get(id))
        .map(|agent| SubagentSummary {
            id: agent.id.clone(),
            name: agent.name.clone(),
            purpose: agent_purpose(agent),
            status: SubagentStatus::Declared,
        })
        .collect()
}

pub(super) fn merge_subagent_summaries_into_map(
    declared_subagents: &mut BTreeMap<String, SubagentSummary>,
    summaries: &[SubagentSummary],
) {
    for summary in summaries {
        declared_subagents.insert(summary.id.clone(), summary.clone());
    }
}

pub(super) fn subagents_for_node(
    node: &Node,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
    agent_snapshots: &BTreeMap<String, AgentDefinition>,
) -> Vec<SubagentSummary> {
    let mut result = Vec::new();
    if node.agent.allow_all_callable_agents {
        for agent_id in agent_snapshots.keys() {
            if let Some(summary) = declared_subagents.get(agent_id) {
                result.push(summary.clone());
            }
        }
    } else {
        for agent_id in &node.agent.callable_agents {
            if let Some(summary) = declared_subagents.get(agent_id) {
                result.push(summary.clone());
            }
        }
    }
    let prefix = format!("{}-subagent-", node.id);
    for summary in declared_subagents.values() {
        if summary.id.starts_with(&prefix) && !result.iter().any(|item| item.id == summary.id) {
            result.push(summary.clone());
        }
    }
    result
}

pub(super) fn augment_call_subagent_tool_description(
    tools: &mut [ToolDefinition],
    node: &Node,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
    agent_snapshots: &BTreeMap<String, AgentDefinition>,
) {
    let available = subagents_for_node(node, declared_subagents, agent_snapshots);
    if available.is_empty() {
        return;
    }
    let listing = available
        .iter()
        .map(|summary| format!("- {} (id: {})", summary.name, summary.id))
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(tool) = tools
        .iter_mut()
        .find(|def| def.name == "openflow_call_subagent")
    {
        tool.description = format!(
            "{}\n\nAvailable subagents for this node:\n{listing}",
            tool.description
        );
    }
}
