use engine::{execution_layers, Workflow, WorkflowValidationError};

const COL_WIDTH: f32 = 240.0;
const ROW_HEIGHT: f32 = 140.0;
const ORIGIN_X: f32 = 80.0;
const ORIGIN_Y: f32 = 120.0;

/// # Errors
/// Returns an error when the workflow DAG is invalid.
pub fn layout_workflow_by_layers(workflow: &mut Workflow) -> Result<(), WorkflowValidationError> {
    let layers = execution_layers(workflow)?;
    for (layer_index, layer) in layers.iter().enumerate() {
        for (row_index, node_id) in layer.iter().enumerate() {
            let position = engine::NodePosition {
                x: ORIGIN_X + layer_index as f32 * COL_WIDTH,
                y: ORIGIN_Y + row_index as f32 * ROW_HEIGHT,
            };
            let node = workflow
                .nodes
                .iter_mut()
                .find(|node| node.id == *node_id)
                .ok_or_else(|| {
                    WorkflowValidationError::InternalConsistency(format!(
                        "layer node {node_id} missing from workflow.nodes"
                    ))
                })?;
            node.position = position;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::catalog::default_workflow;
    use engine::{Edge, EdgeId, Node, NodeId};

    fn diamond_workflow() -> Workflow {
        let mut workflow = default_workflow("Diamond");
        workflow.nodes = vec![
            Node::agent("Root", 0.0, 0.0),
            Node::agent("Left", 0.0, 0.0),
            Node::agent("Right", 0.0, 0.0),
            Node::agent("Join", 0.0, 0.0),
        ];
        workflow.nodes[0].id = NodeId::from("a");
        workflow.nodes[1].id = NodeId::from("b");
        workflow.nodes[2].id = NodeId::from("c");
        workflow.nodes[3].id = NodeId::from("d");
        workflow.edges = vec![
            Edge {
                id: EdgeId::from("a-b"),
                from: NodeId::from("a"),
                to: NodeId::from("b"),
            },
            Edge {
                id: EdgeId::from("a-c"),
                from: NodeId::from("a"),
                to: NodeId::from("c"),
            },
            Edge {
                id: EdgeId::from("b-d"),
                from: NodeId::from("b"),
                to: NodeId::from("d"),
            },
            Edge {
                id: EdgeId::from("c-d"),
                from: NodeId::from("c"),
                to: NodeId::from("d"),
            },
        ];
        workflow
    }

    #[test]
    fn layout_assigns_increasing_x_by_layer() {
        let mut workflow = diamond_workflow();
        layout_workflow_by_layers(&mut workflow).expect("layout");
        let root = workflow.nodes.iter().find(|n| n.id == "a").expect("root");
        let join = workflow.nodes.iter().find(|n| n.id == "d").expect("join");
        assert!(join.position.x > root.position.x);
    }
}
