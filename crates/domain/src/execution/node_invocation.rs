//! Shared node input assembly and [`AgentRequest`] construction for execution engines.

use crate::conversation::AgentTranscriptItem;
use crate::execution::RunError;
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::AgentRequest;
use crate::tools::ToolDefinition;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

/// Resolved upstream adjacency for a workflow graph.
#[must_use]
pub fn build_upstream_map(workflow: &Workflow) -> HashMap<NodeId, Vec<NodeId>> {
    let mut upstream_map: HashMap<NodeId, Vec<NodeId>> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), Vec::new()))
        .collect();
    for edge in &workflow.edges {
        upstream_map
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }
    for ids in upstream_map.values_mut() {
        ids.sort();
    }
    upstream_map
}

/// Append workflow shared context to an arbitrary system prompt base.
#[must_use]
pub fn merge_shared_context(workflow: &Workflow, base: &str) -> String {
    let shared = workflow.settings.shared_context.trim();
    if shared.is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\n--- Workflow context ---\n{shared}")
    }
}

/// Merge per-workflow shared context into a node's system prompt.
#[must_use]
pub fn workflow_system_prompt(workflow: &Workflow, node: &Node) -> String {
    merge_shared_context(workflow, &node.agent.system_prompt)
}

/// Build the JSON input payload for a node from upstream outputs and optional entrypoint text.
#[must_use]
pub fn build_node_input(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>>,
    outputs_by_node: &BTreeMap<NodeId, Value>,
    entrypoint_text: Option<&str>,
) -> Value {
    let upstream = upstream_by_node
        .get(node_id)
        .into_iter()
        .flat_map(|ids| ids.iter())
        .filter_map(|id| {
            outputs_by_node.get(id).map(|output| {
                json!({
                    "node_id": id,
                    "output": output
                })
            })
        })
        .collect::<Vec<_>>();

    if upstream.is_empty() {
        if let Some(text) = entrypoint_text.filter(|text| !text.trim().is_empty()) {
            return json!({
                "entrypoint": { "text": text },
                "upstream": []
            });
        }
    }

    json!({
        "upstream": upstream
    })
}

/// Snapshot of runtime state needed to build an [`AgentRequest`].
pub struct NodeInvocationContext<'a> {
    pub workflow: &'a Workflow,
    pub upstream_map: &'a HashMap<NodeId, Vec<NodeId>>,
    pub outputs: &'a BTreeMap<NodeId, Value>,
    pub entrypoint_text: Option<&'a str>,
    pub transcript: &'a [AgentTranscriptItem],
    pub available_tools: &'a [ToolDefinition],
}

/// # Errors
/// Returns [`RunError::NodeFailed`] when the node has no model configured.
pub fn build_agent_request(
    ctx: &NodeInvocationContext<'_>,
    node: &Node,
    require_model: bool,
) -> Result<AgentRequest, RunError> {
    if require_model && node.agent.model.trim().is_empty() {
        return Err(RunError::NodeFailed {
            node_id: node.id.clone(),
            message: format!(
                "node \"{}\" has no model configured — select a model in the inspector before running",
                node.label
            ),
        });
    }

    Ok(AgentRequest {
        workflow_id: ctx.workflow.id.clone(),
        node_id: node.id.clone(),
        node_label: node.label.clone(),
        model: node.agent.model.clone(),
        system_prompt: workflow_system_prompt(ctx.workflow, node),
        task_prompt: node.agent.task_prompt.clone(),
        input: build_node_input(&node.id, ctx.upstream_map, ctx.outputs, ctx.entrypoint_text),
        output_schema: node.agent.output_schema.clone(),
        tool_config: node.agent.tools.clone(),
        available_tools: ctx.available_tools.to_vec(),
        transcript: ctx.transcript.to_vec(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Workflow};

    #[test]
    fn blank_entrypoint_is_not_injected_into_root_input() {
        let input = build_node_input(
            "idea",
            &HashMap::from([(NodeId("idea".to_string()), Vec::new())]),
            &BTreeMap::new(),
            Some("   "),
        );
        assert_eq!(input, json!({"upstream": []}));
    }

    #[test]
    fn downstream_input_receives_sorted_upstream_outputs() {
        let mut workflow = Workflow::new("join");
        workflow.nodes = vec![
            crate::graph::Node::agent("root", 0.0, 0.0),
            crate::graph::Node::agent("alpha", 0.0, 0.0),
            crate::graph::Node::agent("beta", 0.0, 0.0),
            crate::graph::Node::agent("join", 0.0, 0.0),
        ];
        workflow.nodes[0].id = NodeId("root".into());
        workflow.nodes[1].id = NodeId("alpha".into());
        workflow.nodes[2].id = NodeId("beta".into());
        workflow.nodes[3].id = NodeId("join".into());
        workflow.edges = vec![
            Edge::new("root", "beta"),
            Edge::new("root", "alpha"),
            Edge::new("beta", "join"),
            Edge::new("alpha", "join"),
        ];

        let upstream_map = build_upstream_map(&workflow);
        let mut outputs = BTreeMap::new();
        outputs.insert(NodeId("alpha".into()), json!({"summary": "from alpha"}));
        outputs.insert(NodeId("beta".into()), json!({"summary": "from beta"}));

        let input = build_node_input("join", &upstream_map, &outputs, None);
        assert_eq!(
            input,
            json!({
                "upstream": [
                    { "node_id": "alpha", "output": { "summary": "from alpha" } },
                    { "node_id": "beta", "output": { "summary": "from beta" } }
                ]
            })
        );
    }
}
