use serde_json::Value;
use std::collections::BTreeMap;
use workflow_core::{
    validate_workflow, Edge, Node, NodeId, RunEventKind, RunReport, Workflow,
    WorkflowValidationError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Queued,
    Started,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub workflow: Workflow,
    pub selected_node_id: Option<NodeId>,
    pub link_from_node_id: Option<NodeId>,
    pub schema_editor_text: String,
    pub openai_api_key_input: String,
    pub entrypoint_text: String,
    pub status_by_node: BTreeMap<NodeId, AgentStatus>,
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
        let status_by_node = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), AgentStatus::Idle))
            .collect();

        Self {
            workflow,
            selected_node_id,
            link_from_node_id: None,
            schema_editor_text,
            openai_api_key_input: String::new(),
            entrypoint_text: String::new(),
            status_by_node,
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
        self.status_by_node.insert(node_id.clone(), AgentStatus::Idle);
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
        if self
            .workflow
            .edges
            .iter()
            .any(|edge| edge.from == from_node_id && edge.to == to_node_id)
        {
            self.last_error = Some("edge already exists".to_string());
            return;
        }
        self.workflow
            .edges
            .push(Edge::new(from_node_id, to_node_id));
        self.last_error = None;
    }

    pub fn resolve_api_key(&self, env_key: Option<&str>) -> Option<String> {
        if !self.openai_api_key_input.trim().is_empty() {
            return Some(self.openai_api_key_input.trim().to_string());
        }
        env_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    pub fn edge_rows(&self) -> Vec<String> {
        self.workflow
            .edges
            .iter()
            .map(|edge| {
                let from = self.node_label(&edge.from);
                let to = self.node_label(&edge.to);
                format!("{from} -> {to}")
            })
            .collect()
    }

    pub fn move_node_by_delta(
        &mut self,
        node_id: &str,
        dx: f32,
        dy: f32,
        canvas_size: (f32, f32),
        node_size: (f32, f32),
    ) {
        if let Some(node) = self.workflow.nodes.iter_mut().find(|node| node.id == node_id) {
            let max_x = (canvas_size.0 - node_size.0).max(0.0);
            let max_y = (canvas_size.1 - node_size.1).max(0.0);
            node.position.x = (node.position.x + dx).clamp(0.0, max_x);
            node.position.y = (node.position.y + dy).clamp(0.0, max_y);
        }
    }

    pub fn refresh_statuses_from_report(&mut self) {
        self.status_by_node.clear();
        for node in &self.workflow.nodes {
            self.status_by_node.insert(node.id.clone(), AgentStatus::Idle);
        }
        if let Some(report) = &self.last_run {
            for event in &report.events {
                let status = match event.kind {
                    RunEventKind::Queued => AgentStatus::Queued,
                    RunEventKind::Started => AgentStatus::Started,
                    RunEventKind::Completed => AgentStatus::Completed,
                    RunEventKind::Failed => AgentStatus::Failed,
                };
                self.status_by_node.insert(event.node_id.clone(), status);
            }
        }
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

    fn node_label(&self, node_id: &str) -> String {
        self.workflow
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .map(|node| node.label.clone())
            .unwrap_or_else(|| "Unknown".to_string())
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

    #[test]
    fn rejects_duplicate_edges() {
        let mut state = AppState::new();
        let first = state.selected_node_id.clone().unwrap();
        let second = state.add_agent_node();

        state.select_node(first.clone());
        state.begin_link_from_selected();
        state.connect_link_to(second.clone());

        state.select_node(first);
        state.begin_link_from_selected();
        state.connect_link_to(second);

        assert_eq!(state.workflow.edges.len(), 1);
        assert_eq!(state.last_error.as_deref(), Some("edge already exists"));
    }

    #[test]
    fn edge_rows_use_node_labels_not_ids() {
        let mut state = AppState::new();
        let first = state.selected_node_id.clone().unwrap();
        let second = state.add_agent_node();
        state.select_node(first.clone());
        state.begin_link_from_selected();
        state.connect_link_to(second.clone());

        let rows = state.edge_rows();

        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("Idea -> Agent 2"));
        assert!(!rows[0].contains(&first));
        assert!(!rows[0].contains(&second));
    }

    #[test]
    fn moves_node_with_drag_delta_and_clamps_to_canvas() {
        let mut state = AppState::new();
        let node_id = state.selected_node_id.clone().unwrap();

        state.move_node_by_delta(&node_id, 20.0, 10.0, (640.0, 480.0), (220.0, 120.0));
        let moved = state.selected_node().unwrap().position.clone();
        assert!(moved.x >= 0.0);
        assert!(moved.y >= 0.0);

        state.move_node_by_delta(&node_id, -10_000.0, -10_000.0, (640.0, 480.0), (220.0, 120.0));
        let clamped = state.selected_node().unwrap().position.clone();
        assert_eq!(clamped.x, 0.0);
        assert_eq!(clamped.y, 0.0);
    }

    #[test]
    fn ui_key_overrides_env_key_resolution() {
        let mut state = AppState::new();
        state.openai_api_key_input = "sk-ui-123".to_string();

        let key = state.resolve_api_key(Some("sk-env-456"));

        assert_eq!(key.as_deref(), Some("sk-ui-123"));
    }
}
