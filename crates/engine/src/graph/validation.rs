use crate::graph::workflow::Workflow;
use crate::graph::{EdgeId, NodeId};
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

/// # Errors
/// Returns an error if the workflow is invalid.
pub fn validate_workflow(workflow: &Workflow) -> Result<(), WorkflowValidationError> {
    execution_layers(workflow).map(|_| ())
}

/// # Errors
/// Returns an error if the workflow is invalid (empty, duplicate ids, missing endpoints, cycles,
/// or internal consistency violation).
pub fn execution_layers(workflow: &Workflow) -> Result<Vec<Vec<NodeId>>, WorkflowValidationError> {
    if workflow.nodes.is_empty() {
        return Err(WorkflowValidationError::EmptyWorkflow);
    }

    let node_ids = check_duplicate_nodes(workflow)?;
    check_duplicate_edges_and_endpoints(workflow, &node_ids)?;

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
    use crate::graph::workflow::{Edge, Node, Workflow};

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
}
