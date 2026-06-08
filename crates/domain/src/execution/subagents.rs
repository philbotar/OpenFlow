//! Subagent declaration helpers and tool-description augmentation.

use crate::graph::callable_agent::CallableAgent;
use crate::graph::{Node, NodeId};
use crate::tools::{SubagentDeclaration, SubagentStatus, SubagentSummary, ToolDefinition};
use std::collections::BTreeMap;

pub const CALL_SUBAGENT_TOOL: &str = "openflow_call_subagent";

#[must_use]
pub fn adhoc_subagent_base_index(
    node_id: &NodeId,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
) -> usize {
    let prefix = format!("{node_id}-subagent-");
    declared_subagents
        .keys()
        .filter(|id| id.starts_with(&prefix))
        .count()
}

#[must_use]
pub fn build_adhoc_subagent_summaries(
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

pub fn merge_subagent_summaries(
    declared_subagents: &mut BTreeMap<String, SubagentSummary>,
    summaries: &[SubagentSummary],
) {
    for summary in summaries {
        declared_subagents.insert(summary.id.clone(), summary.clone());
    }
}

#[must_use]
pub fn subagents_for_node(
    node: &Node,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
    agent_snapshots: &BTreeMap<String, CallableAgent>,
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

pub fn augment_call_subagent_tool_description(
    tools: &mut [ToolDefinition],
    node: &Node,
    declared_subagents: &BTreeMap<String, SubagentSummary>,
    agent_snapshots: &BTreeMap<String, CallableAgent>,
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
    if let Some(tool) = tools.iter_mut().find(|def| def.name == CALL_SUBAGENT_TOOL) {
        tool.description = format!(
            "{}\n\nAvailable subagents for this node:\n{listing}",
            tool.description
        );
    }
}
