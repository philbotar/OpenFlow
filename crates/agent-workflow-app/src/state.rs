use serde_json::Value;
use workflow_core::{
    validate_workflow, Edge, Node, NodeId, RunReport, Workflow, WorkflowValidationError,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub workflow: Workflow,
    pub selected_node_id: Option<NodeId>,
    pub link_from_node_id: Option<NodeId>,
    pub schema_editor_text: String,
    pub last_run: Option<RunReport>,
    pub last_error: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        let mut workflow = Workflow::new("New workflow");
        workflow.nodes.push(Node::agent("Idea", 80.0, 120.0));
        let selected_node_id = workflow.nodes.first().map(|node| node.id.clone());
        let schema_editor_text = workflow
            .nodes
            .first()
            .map(|node| serde_json::to_string_pretty(&node.agent.output_schema).unwrap())
            .unwrap_or_default();

        Self {
            workflow,
            selected_node_id,
            link_from_node_id: None,
            schema_editor_text,
            last_run: None,
            last_error: None,
        }
    }

    pub fn add_agent_node(&mut self) -> NodeId {
        let index = self.workflow.nodes.len() + 1;
        let node = Node::agent(
            format!("Agent {index}"),
            80.0 + (index as f32 * 48.0),
            120.0 + (index as f32 * 24.0),
        );
        let node_id = node.id.clone();
        self.workflow.nodes.push(node);
        self.select_node(node_id.clone());
        node_id
    }

    pub fn select_node(&mut self, node_id: NodeId) {
        self.selected_node_id = Some(node_id);
        self.refresh_schema_editor();
    }

    pub fn selected_node_mut(&mut self) -> Option<&mut Node> {
        let selected = self.selected_node_id.clone()?;
        self.workflow
            .nodes
            .iter_mut()
            .find(|node| node.id == selected)
    }

    pub fn selected_node(&self) -> Option<&Node> {
        let selected = self.selected_node_id.as_ref()?;
        self.workflow.nodes.iter().find(|node| &node.id == selected)
    }

    pub fn begin_link_from_selected(&mut self) {
        self.link_from_node_id = self.selected_node_id.clone();
    }

    pub fn connect_link_to(&mut self, to_node_id: NodeId) {
        let Some(from_node_id) = self.link_from_node_id.take() else {
            return;
        };
        if from_node_id == to_node_id {
            self.last_error = Some("cannot connect a node to itself".to_string());
            return;
        }
        self.workflow.edges.push(Edge::new(from_node_id, to_node_id));
        self.last_error = None;
    }

    pub fn apply_schema_editor(&mut self) {
        match serde_json::from_str::<Value>(&self.schema_editor_text) {
            Ok(schema) => {
                if let Some(node) = self.selected_node_mut() {
                    node.agent.output_schema = schema;
                    self.last_error = None;
                }
            }
            Err(error) => {
                self.last_error = Some(format!("output schema JSON invalid: {error}"));
            }
        }
    }

    pub fn validate(&mut self) -> Result<(), WorkflowValidationError> {
        let result = validate_workflow(&self.workflow);
        self.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    pub fn set_run_report(&mut self, report: RunReport) {
        self.last_run = Some(report);
        self.last_error = None;
    }

    fn refresh_schema_editor(&mut self) {
        self.schema_editor_text = self
            .selected_node()
            .map(|node| serde_json::to_string_pretty(&node.agent.output_schema).unwrap())
            .unwrap_or_default();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn starts_with_selected_idea_node() {
        let state = AppState::new();

        assert_eq!(state.workflow.nodes.len(), 1);
        assert_eq!(state.selected_node().unwrap().label, "Idea");
    }

    #[test]
    fn adds_and_connects_agent_nodes() {
        let mut state = AppState::new();
        let first = state.selected_node_id.clone().unwrap();
        let second = state.add_agent_node();
        state.select_node(first.clone());
        state.begin_link_from_selected();
        state.connect_link_to(second.clone());

        assert_eq!(state.workflow.edges.len(), 1);
        assert_eq!(state.workflow.edges[0].from, first);
        assert_eq!(state.workflow.edges[0].to, second);
    }

    #[test]
    fn applies_schema_editor_json_to_selected_node() {
        let mut state = AppState::new();
        state.schema_editor_text = r#"{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "decision": { "type": "string" }
  },
  "required": ["decision"]
}"#
        .to_string();

        state.apply_schema_editor();

        assert_eq!(
            state.selected_node().unwrap().agent.output_schema["required"],
            json!(["decision"])
        );
        assert!(state.last_error.is_none());
    }
}
