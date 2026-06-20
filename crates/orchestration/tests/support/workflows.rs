use engine::{Edge, Node, NodeId, Workflow};
use serde_json::json;

pub fn agent_node(id: &str, label: &str) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = NodeId(id.to_string());
    node.agent.model = "test-model".to_string();
    node.agent.output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "summary": { "type": "string" }
        },
        "required": ["summary"]
    });
    node
}

pub fn single_agent_workflow() -> Workflow {
    let mut workflow = Workflow::new("single agent");
    workflow.nodes = vec![agent_node("first", "First")];
    workflow
}

pub fn linear_workflow() -> Workflow {
    let mut workflow = Workflow::new("linear");
    workflow.nodes = vec![
        agent_node("step-a", "Step A"),
        agent_node("step-b", "Step B"),
        agent_node("step-c", "Step C"),
    ];
    workflow.edges = vec![
        Edge::new("step-a", "step-b"),
        Edge::new("step-b", "step-c"),
    ];
    workflow
}

pub fn branch_join_workflow() -> Workflow {
    let mut workflow = Workflow::new("branch join");
    workflow.nodes = vec![
        agent_node("idea", "Idea"),
        agent_node("plan", "Plan"),
        agent_node("risk", "Risk"),
        agent_node("join", "Join"),
    ];
    workflow.edges = vec![
        Edge::new("idea", "plan"),
        Edge::new("idea", "risk"),
        Edge::new("plan", "join"),
        Edge::new("risk", "join"),
    ];
    workflow
}
