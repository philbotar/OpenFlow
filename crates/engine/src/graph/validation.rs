use crate::graph::workflow::{effective_output_schema, Workflow, MCP_CONTEXT_MAX_BYTES};
use crate::graph::{validate_markdown_handoff_template, EdgeId, HandoffSpec, NodeId};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkflowValidationError {
    #[error("workflow must contain at least one node")]
    EmptyWorkflow,
    #[error("duplicate node id: {0}")]
    DuplicateNodeId(NodeId),
    #[error("duplicate edge id: {0}")]
    DuplicateEdgeId(EdgeId),
    #[error("edge {edge_id} references missing node {node_id}")]
    MissingEndpoint { edge_id: EdgeId, node_id: NodeId },
    #[error("edge {0} connects a node to itself")]
    SelfEdge(EdgeId),
    #[error("workflow contains a cycle")]
    Cycle,
    #[error("plan mode source node {0} does not exist")]
    PlanModeSourceMissing(NodeId),
    #[error("plan mode source node {0} must allow user input")]
    PlanModeSourceNotInteractive(NodeId),
    #[error("node {node_id} output schema is invalid: {detail}")]
    InvalidOutputSchema { node_id: NodeId, detail: String },
    #[error("node {node_id} Markdown handoff template is invalid: {detail}")]
    InvalidHandoffTemplate { node_id: NodeId, detail: String },
    #[error("node {node_id} MCP context selection is invalid: {detail}")]
    InvalidMcpContextSelection { node_id: NodeId, detail: String },
    #[error("internal consistency: {0}")]
    InternalConsistency(String),
}

fn check_duplicate_nodes(workflow: &Workflow) -> Result<HashSet<NodeId>, WorkflowValidationError> {
    let mut node_ids = HashSet::new();
    for node in &workflow.nodes {
        if !node_ids.insert(node.id.clone()) {
            return Err(WorkflowValidationError::DuplicateNodeId(node.id.clone()));
        }
    }
    Ok(node_ids)
}

fn check_duplicate_edges_and_endpoints(
    workflow: &Workflow,
    node_ids: &HashSet<NodeId>,
) -> Result<(), WorkflowValidationError> {
    let mut edge_ids = HashSet::new();
    for edge in &workflow.edges {
        if !edge_ids.insert(edge.id.clone()) {
            return Err(WorkflowValidationError::DuplicateEdgeId(edge.id.clone()));
        }
        if edge.from == edge.to {
            return Err(WorkflowValidationError::SelfEdge(edge.id.clone()));
        }
        if !node_ids.contains(&edge.from) {
            return Err(WorkflowValidationError::MissingEndpoint {
                edge_id: edge.id.clone(),
                node_id: edge.from.clone(),
            });
        }
        if !node_ids.contains(&edge.to) {
            return Err(WorkflowValidationError::MissingEndpoint {
                edge_id: edge.id.clone(),
                node_id: edge.to.clone(),
            });
        }
    }
    Ok(())
}

fn check_plan_mode_source(workflow: &Workflow) -> Result<(), WorkflowValidationError> {
    let Some(plan_mode) = &workflow.settings.plan_mode else {
        return Ok(());
    };
    let Some(source) = workflow
        .nodes
        .iter()
        .find(|node| node.id == plan_mode.evidence_source_node_id)
    else {
        return Err(WorkflowValidationError::PlanModeSourceMissing(
            plan_mode.evidence_source_node_id.clone(),
        ));
    };
    if !source.agent.request_user_input {
        return Err(WorkflowValidationError::PlanModeSourceNotInteractive(
            source.id.clone(),
        ));
    }
    Ok(())
}

fn check_output_schemas(workflow: &Workflow) -> Result<(), WorkflowValidationError> {
    for node in &workflow.nodes {
        match &node.agent.handoff {
            HandoffSpec::Markdown { template } => {
                if let Err(error) = validate_markdown_handoff_template(template) {
                    return Err(WorkflowValidationError::InvalidHandoffTemplate {
                        node_id: node.id.clone(),
                        detail: error.to_string(),
                    });
                }
            }
            HandoffSpec::Legacy | HandoffSpec::Json => {
                let schema = effective_output_schema(&node.agent.output_schema);
                if let Err(error) = jsonschema::validator_for(&schema) {
                    return Err(WorkflowValidationError::InvalidOutputSchema {
                        node_id: node.id.clone(),
                        detail: error.to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn check_mcp_context_selections(workflow: &Workflow) -> Result<(), WorkflowValidationError> {
    for node in &workflow.nodes {
        let selection_count = node.agent.mcp_resources.len() + node.agent.mcp_prompts.len();
        if selection_count > 64 {
            return Err(WorkflowValidationError::InvalidMcpContextSelection {
                node_id: node.id.clone(),
                detail: "at most 64 resources and prompts may be selected".to_string(),
            });
        }
        let mut requested_bytes = 0_u64;
        for (server_id, source, max_bytes) in
            node.agent
                .mcp_resources
                .iter()
                .map(|selection| (&selection.server_id, &selection.uri, selection.max_bytes))
                .chain(
                    node.agent.mcp_prompts.iter().map(|selection| {
                        (&selection.server_id, &selection.name, selection.max_bytes)
                    }),
                )
        {
            if server_id.trim().is_empty() || server_id.len() > 128 || server_id.contains('/') {
                return Err(WorkflowValidationError::InvalidMcpContextSelection {
                    node_id: node.id.clone(),
                    detail: "server ID must be 1-128 characters without '/'".to_string(),
                });
            }
            if source.trim().is_empty()
                || source.len() > 4096
                || source.chars().any(char::is_control)
            {
                return Err(WorkflowValidationError::InvalidMcpContextSelection {
                    node_id: node.id.clone(),
                    detail: "resource URI or prompt name must be 1-4096 printable characters"
                        .to_string(),
                });
            }
            if max_bytes == 0 || max_bytes > MCP_CONTEXT_MAX_BYTES {
                return Err(WorkflowValidationError::InvalidMcpContextSelection {
                    node_id: node.id.clone(),
                    detail: format!("maxBytes must be 1-{MCP_CONTEXT_MAX_BYTES}"),
                });
            }
            requested_bytes = requested_bytes.saturating_add(u64::from(max_bytes));
        }
        if requested_bytes > u64::from(MCP_CONTEXT_MAX_BYTES) {
            return Err(WorkflowValidationError::InvalidMcpContextSelection {
                node_id: node.id.clone(),
                detail: format!(
                    "combined context budget must not exceed {MCP_CONTEXT_MAX_BYTES} bytes"
                ),
            });
        }
        for prompt in &node.agent.mcp_prompts {
            if prompt.arguments.len() > 64
                || prompt.arguments.iter().any(|(name, value)| {
                    name.trim().is_empty()
                        || name.len() > 256
                        || name.chars().any(char::is_control)
                        || value.len() > 16_384
                })
            {
                return Err(WorkflowValidationError::InvalidMcpContextSelection {
                    node_id: node.id.clone(),
                    detail: "prompt arguments exceed count, name, or value limits".to_string(),
                });
            }
        }
        let snapshot_bytes = node
            .agent
            .mcp_context_snapshots
            .iter()
            .fold(0_u64, |total, snapshot| {
                total.saturating_add(snapshot.content.len() as u64)
            });
        if snapshot_bytes > u64::from(MCP_CONTEXT_MAX_BYTES) {
            return Err(WorkflowValidationError::InvalidMcpContextSelection {
                node_id: node.id.clone(),
                detail: format!("resolved context must not exceed {MCP_CONTEXT_MAX_BYTES} bytes"),
            });
        }
    }
    Ok(())
}

/// # Errors
/// Returns an error if the workflow is invalid.
pub fn validate_workflow(workflow: &Workflow) -> Result<(), WorkflowValidationError> {
    execution_layers(workflow).map(|_| ())
}

/// # Errors
/// Returns an error if the workflow is invalid (empty, malformed output schema, duplicate ids,
/// missing endpoints, cycles, or internal consistency violation).
pub fn execution_layers(workflow: &Workflow) -> Result<Vec<Vec<NodeId>>, WorkflowValidationError> {
    if workflow.nodes.is_empty() {
        return Err(WorkflowValidationError::EmptyWorkflow);
    }

    let node_ids = check_duplicate_nodes(workflow)?;
    check_output_schemas(workflow)?;
    check_mcp_context_selections(workflow)?;
    check_duplicate_edges_and_endpoints(workflow, &node_ids)?;
    check_plan_mode_source(workflow)?;

    let mut incoming: HashMap<NodeId, usize> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), 0))
        .collect();
    let mut outgoing: HashMap<NodeId, Vec<NodeId>> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), Vec::new()))
        .collect();

    for edge in &workflow.edges {
        *incoming.entry(edge.to.clone()).or_insert(0) += 1;
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut ready: Vec<NodeId> = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node_id, _)| node_id.clone())
        .collect();
    ready.sort();

    let mut layers = Vec::new();
    let mut visited_count = 0usize;

    while !ready.is_empty() {
        let layer = ready;
        visited_count += layer.len();
        let mut next = Vec::new();

        for node_id in &layer {
            if let Some(children) = outgoing.get(node_id) {
                for child_id in children {
                    let count = incoming.get_mut(child_id).ok_or_else(|| {
                        WorkflowValidationError::InternalConsistency(
                            "child id was validated before layer build".to_string(),
                        )
                    })?;
                    *count -= 1;
                    if *count == 0 {
                        next.push(child_id.clone());
                    }
                }
            }
        }

        next.sort();
        layers.push(layer);
        ready = next;
    }

    if visited_count != workflow.nodes.len() {
        return Err(WorkflowValidationError::Cycle);
    }

    Ok(layers)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test fixtures use unwrap/expect for brevity"
)]
mod tests {
    use super::*;
    use crate::graph::workflow::{Edge, Node, PlanModeConfig, Workflow};

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "test layout uses integer coordinates cast to f32 canvas positions"
    )]
    fn workflow_with_nodes(labels: &[&str]) -> Workflow {
        let mut workflow = Workflow::new("test");
        workflow.nodes = labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let mut node = Node::agent(*label, index as f32 * 120.0, 0.0);
                node.id = NodeId((*label).to_string());
                node
            })
            .collect();
        workflow
    }

    #[test]
    fn rejects_empty_workflow() {
        let workflow = Workflow::new("empty");

        assert_eq!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::EmptyWorkflow)
        );
    }

    #[test]
    fn returns_dependency_layers_for_branch_and_join() {
        let mut workflow = workflow_with_nodes(&["idea", "plan", "risk", "final"]);
        workflow.edges = vec![
            Edge::new("idea", "plan"),
            Edge::new("idea", "risk"),
            Edge::new("plan", "final"),
            Edge::new("risk", "final"),
        ];

        let layers = execution_layers(&workflow).unwrap();

        assert_eq!(
            layers,
            vec![
                vec![NodeId("idea".into())],
                vec![NodeId("plan".into()), NodeId("risk".into())],
                vec![NodeId("final".into())]
            ]
        );
    }

    #[test]
    fn rejects_missing_edge_endpoint() {
        let mut workflow = workflow_with_nodes(&["idea"]);
        workflow.edges = vec![Edge::new("idea", "plan")];
        let edge_id = workflow.edges[0].id.clone();

        assert_eq!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::MissingEndpoint {
                edge_id,
                node_id: NodeId("plan".to_string())
            })
        );
    }

    #[test]
    fn rejects_cycles() {
        let mut workflow = workflow_with_nodes(&["a", "b"]);
        workflow.edges = vec![Edge::new("a", "b"), Edge::new("b", "a")];

        assert_eq!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::Cycle)
        );
    }

    #[test]
    fn rejects_duplicate_node_ids() {
        let mut workflow = workflow_with_nodes(&["a", "a"]);

        let error = validate_workflow(&workflow).unwrap_err();

        assert_eq!(
            error,
            WorkflowValidationError::DuplicateNodeId(NodeId("a".to_string()))
        );
        workflow.nodes[1].id = NodeId("b".to_string());
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn rejects_duplicate_edge_ids() {
        let mut workflow = workflow_with_nodes(&["a", "b", "c"]);
        let mut first = Edge::new("a", "b");
        first.id = EdgeId("edge-1".to_string());
        let mut second = Edge::new("a", "c");
        second.id = EdgeId("edge-1".to_string());
        workflow.edges = vec![first, second];

        let error = validate_workflow(&workflow).unwrap_err();

        assert_eq!(
            error,
            WorkflowValidationError::DuplicateEdgeId(EdgeId("edge-1".to_string()))
        );
    }

    #[test]
    fn rejects_self_edges_before_layer_execution() {
        let mut workflow = workflow_with_nodes(&["a"]);
        let mut edge = Edge::new("a", "a");
        edge.id = EdgeId("self-edge".to_string());
        workflow.edges = vec![edge];

        let error = validate_workflow(&workflow).unwrap_err();

        assert_eq!(
            error,
            WorkflowValidationError::SelfEdge(EdgeId("self-edge".to_string()))
        );
    }

    #[test]
    fn rejects_missing_plan_mode_source() {
        let mut workflow = workflow_with_nodes(&["plan"]);
        workflow.settings.plan_mode = Some(PlanModeConfig {
            evidence_source_node_id: NodeId::from("freeze"),
        });

        assert_eq!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::PlanModeSourceMissing(
                NodeId::from("freeze")
            ))
        );
    }

    #[test]
    fn rejects_non_conversational_plan_mode_source() {
        let mut workflow = workflow_with_nodes(&["freeze"]);
        workflow.settings.plan_mode = Some(PlanModeConfig {
            evidence_source_node_id: NodeId::from("freeze"),
        });

        assert_eq!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::PlanModeSourceNotInteractive(
                NodeId::from("freeze")
            ))
        );

        workflow.nodes[0].agent.request_user_input = true;
        assert!(validate_workflow(&workflow).is_ok());
    }

    #[test]
    fn rejects_malformed_effective_output_schema_before_provider_invocation() {
        let mut workflow = workflow_with_nodes(&["broken"]);
        workflow.nodes[0].agent.handoff = crate::graph::HandoffSpec::Json;
        workflow.nodes[0].agent.output_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "summary": "string"
            }
        });

        let error = validate_workflow(&workflow).unwrap_err();
        assert!(error
            .to_string()
            .starts_with("node broken output schema is invalid:"));

        assert!(
            matches!(
                error,
                WorkflowValidationError::InvalidOutputSchema { ref node_id, .. }
                    if node_id == &NodeId::from("broken")
            ),
            "{error}"
        );
    }

    #[test]
    fn rejects_markdown_handoff_template_without_headings() {
        let mut workflow = workflow_with_nodes(&["broken"]);
        workflow.nodes[0].agent.handoff = crate::graph::HandoffSpec::Markdown {
            template: "Write a useful handoff.".to_string(),
        };

        assert_eq!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::InvalidHandoffTemplate {
                node_id: NodeId::from("broken"),
                detail: "template requires at least one Markdown heading".to_string(),
            })
        );
    }

    #[test]
    fn rejects_mcp_context_budget_above_node_limit() {
        let mut workflow = workflow_with_nodes(&["context"]);
        workflow.nodes[0]
            .agent
            .mcp_resources
            .push(crate::graph::McpResourceSelection {
                server_id: "docs".to_string(),
                uri: "docs://one".to_string(),
                max_bytes: MCP_CONTEXT_MAX_BYTES,
            });
        workflow.nodes[0]
            .agent
            .mcp_prompts
            .push(crate::graph::McpPromptSelection {
                server_id: "docs".to_string(),
                name: "review".to_string(),
                arguments: std::collections::BTreeMap::default(),
                max_bytes: 1,
            });

        assert!(matches!(
            validate_workflow(&workflow),
            Err(WorkflowValidationError::InvalidMcpContextSelection { ref detail, .. })
                if detail.contains("combined context budget")
        ));
    }
}
