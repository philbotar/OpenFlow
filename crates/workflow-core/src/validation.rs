use crate::{NodeId, Workflow};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkflowValidationError {
    #[error("workflow must contain at least one node")]
    EmptyWorkflow,
    #[error("duplicate node id: {0}")]
    DuplicateNodeId(String),
    #[error("duplicate edge id: {0}")]
    DuplicateEdgeId(String),
    #[error("edge {edge_id} references missing node {node_id}")]
    MissingEndpoint { edge_id: String, node_id: String },
    #[error("edge {0} connects a node to itself")]
    SelfEdge(String),
    #[error("workflow contains a cycle")]
    Cycle,
}

pub fn validate_workflow(workflow: &Workflow) -> Result<(), WorkflowValidationError> {
    execution_layers(workflow).map(|_| ())
}

pub fn execution_layers(workflow: &Workflow) -> Result<Vec<Vec<NodeId>>, WorkflowValidationError> {
    if workflow.nodes.is_empty() {
        return Err(WorkflowValidationError::EmptyWorkflow);
    }

    let mut node_ids = HashSet::new();
    for node in &workflow.nodes {
        if !node_ids.insert(node.id.clone()) {
            return Err(WorkflowValidationError::DuplicateNodeId(node.id.clone()));
        }
    }

    let mut edge_ids = HashSet::new();
    let mut incoming: HashMap<NodeId, usize> =
        workflow.nodes.iter().map(|node| (node.id.clone(), 0)).collect();
    let mut outgoing: HashMap<NodeId, Vec<NodeId>> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), Vec::new()))
        .collect();

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

        *incoming.entry(edge.to.clone()).or_insert(0) += 1;
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut ready: Vec<NodeId> = incoming
        .iter()
        .filter_map(|(node_id, count)| (*count == 0).then(|| node_id.clone()))
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
                    let count = incoming
                        .get_mut(child_id)
                        .expect("child id was validated before layer build");
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
mod tests {
    use super::*;
    use crate::{Edge, Node, Workflow};

    fn workflow_with_nodes(labels: &[&str]) -> Workflow {
        let mut workflow = Workflow::new("test");
        workflow.nodes = labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let mut node = Node::agent(*label, index as f32 * 120.0, 0.0);
                node.id = (*label).to_string();
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
                vec!["idea".to_string()],
                vec!["plan".to_string(), "risk".to_string()],
                vec!["final".to_string()]
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
                node_id: "plan".to_string()
            })
        );
    }

    #[test]
    fn rejects_cycles() {
        let mut workflow = workflow_with_nodes(&["a", "b"]);
        workflow.edges = vec![Edge::new("a", "b"), Edge::new("b", "a")];

        assert_eq!(validate_workflow(&workflow), Err(WorkflowValidationError::Cycle));
    }
}
