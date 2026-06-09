//! Shared node input assembly and [`AgentRequest`] construction for execution engines.

use crate::conversation::AgentTranscriptItem;
use crate::execution::RunError;
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::AgentRequest;
use crate::tools::{merge_file_change_record, FileChangeRecord, ToolDefinition};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

/// Collect file-change records from all transitive upstream nodes (deduped by path, latest timestamp wins).
#[must_use]
pub fn upstream_changed_files(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>>,
    changed_files_by_node: &BTreeMap<NodeId, Vec<FileChangeRecord>>,
) -> Vec<FileChangeRecord> {
    let mut by_path: BTreeMap<String, FileChangeRecord> = BTreeMap::new();
    for upstream_id in transitive_upstream_ids(node_id, upstream_by_node) {
        if let Some(records) = changed_files_by_node.get(&upstream_id) {
            for record in records {
                merge_file_change_record(&mut by_path, record.clone());
            }
        }
    }
    by_path.into_values().collect()
}

fn transitive_upstream_ids(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>>,
) -> Vec<NodeId> {
    let mut visited = BTreeSet::new();
    let mut stack: Vec<NodeId> = upstream_by_node.get(node_id).cloned().unwrap_or_default();
    let mut collected = Vec::new();
    while let Some(id) = stack.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        collected.push(id.clone());
        if let Some(parents) = upstream_by_node.get(&id) {
            stack.extend(parents.iter().cloned());
        }
    }
    collected.sort();
    collected
}

/// Build the JSON input payload for a node from upstream outputs and optional entrypoint text.
#[must_use]
pub fn build_node_input(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>>,
    outputs_by_node: &BTreeMap<NodeId, Value>,
    entrypoint_text: Option<&str>,
    changed_files_by_node: &BTreeMap<NodeId, Vec<FileChangeRecord>>,
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
    let changed_files = upstream_changed_files(node_id, upstream_by_node, changed_files_by_node);

    if upstream.is_empty() {
        if let Some(text) = entrypoint_text.filter(|text| !text.trim().is_empty()) {
            let mut payload = json!({
                "entrypoint": { "text": text },
                "upstream": []
            });
            if !changed_files.is_empty() {
                payload["changed_files"] = json!(changed_files);
            }
            return payload;
        }
    }

    let mut payload = json!({ "upstream": upstream });
    if !changed_files.is_empty() {
        payload["changed_files"] = json!(changed_files);
    }
    payload
}

/// Snapshot of runtime state needed to build an [`AgentRequest`].
pub struct NodeInvocationContext<'a> {
    pub workflow: &'a Workflow,
    pub upstream_map: &'a HashMap<NodeId, Vec<NodeId>>,
    pub outputs: &'a BTreeMap<NodeId, Value>,
    pub changed_files_by_node: &'a BTreeMap<NodeId, Vec<FileChangeRecord>>,
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
        input: build_node_input(
            &node.id,
            ctx.upstream_map,
            ctx.outputs,
            ctx.entrypoint_text,
            ctx.changed_files_by_node,
        ),
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
            &BTreeMap::new(),
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

        let input = build_node_input("join", &upstream_map, &outputs, None, &BTreeMap::new());
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

    #[test]
    fn downstream_input_includes_upstream_changed_files() {
        let upstream_map =
            HashMap::from([(NodeId("join".to_string()), vec![NodeId("alpha".into())])]);
        let mut outputs = BTreeMap::new();
        outputs.insert(NodeId("alpha".into()), json!({"summary": "done"}));
        let mut changed_files_by_node = BTreeMap::new();
        changed_files_by_node.insert(
            NodeId("alpha".into()),
            vec![FileChangeRecord {
                path: "src/main.rs".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: Some("+1|fn main()".to_string()),
                timestamp_ms: 1,
            }],
        );

        let input = build_node_input(
            "join",
            &upstream_map,
            &outputs,
            None,
            &changed_files_by_node,
        );

        assert_eq!(
            input["changed_files"],
            json!([{
                "path": "src/main.rs",
                "op": "update",
                "diffSummary": "+1|fn main()",
                "timestampMs": 1
            }])
        );
    }

    #[test]
    fn upstream_changed_files_dedupes_renames_by_destination() {
        let upstream_map =
            HashMap::from([(NodeId("join".to_string()), vec![NodeId("alpha".into())])]);
        let mut changed_files_by_node = BTreeMap::new();
        changed_files_by_node.insert(
            NodeId("alpha".into()),
            vec![
                FileChangeRecord {
                    path: "old.rs".to_string(),
                    op: crate::tools::FileChangeOp::Rename,
                    rename_to: Some("new.rs".to_string()),
                    diff_summary: None,
                    timestamp_ms: 1,
                },
                FileChangeRecord {
                    path: "new.rs".to_string(),
                    op: crate::tools::FileChangeOp::Update,
                    rename_to: None,
                    diff_summary: None,
                    timestamp_ms: 2,
                },
            ],
        );

        let files = upstream_changed_files("join", &upstream_map, &changed_files_by_node);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new.rs");
    }

    #[test]
    fn transitive_upstream_changed_files_reach_multi_hop_downstream() {
        let upstream_map = HashMap::from([
            (NodeId("beta".to_string()), vec![NodeId("alpha".into())]),
            (NodeId("gamma".to_string()), vec![NodeId("beta".into())]),
        ]);
        let mut changed_files_by_node = BTreeMap::new();
        changed_files_by_node.insert(
            NodeId("alpha".into()),
            vec![FileChangeRecord {
                path: "src/main.rs".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                timestamp_ms: 1,
            }],
        );

        let input = build_node_input(
            "gamma",
            &upstream_map,
            &BTreeMap::new(),
            None,
            &changed_files_by_node,
        );

        assert_eq!(input["changed_files"].as_array().map(Vec::len), Some(1));
        assert_eq!(input["changed_files"][0]["path"], "src/main.rs");
    }
}
