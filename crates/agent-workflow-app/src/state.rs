use crate::canvas_math::clamp_node_position;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use workflow_core::{
    validate_workflow, ChatMessage, ChatRole, Edge, Node, NodeId, RunEventKind, RunReport,
    Workflow, WorkflowValidationError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentStatus {
    Idle,
    Queued,
    Started,
    AwaitingInput,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TraceStatus {
    Queued,
    Running,
    Paused,
    Failed,
    Completed,
}

impl TraceStatus {
    const fn from_run_event_kind(kind: &RunEventKind) -> Self {
        match kind {
            RunEventKind::Queued => Self::Queued,
            RunEventKind::Started => Self::Running,
            RunEventKind::Completed => Self::Completed,
            RunEventKind::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTraceEntry {
    pub node_id: NodeId,
    pub node_label: String,
    pub status: TraceStatus,
    pub message: String,
    pub output: Option<Value>,
}

/// Live run state pushed to the frontend via Tauri events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunState {
    pub active: bool,
    pub awaiting_node_id: Option<NodeId>,
    pub active_manual_node_id: Option<NodeId>,
    pub status_by_node: BTreeMap<NodeId, AgentStatus>,
    pub last_report: Option<RunReport>,
    pub last_error: Option<String>,
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub run_trace: Vec<RunTraceEntry>,
    pub outputs: BTreeMap<NodeId, Value>,
}

impl WorkflowRunState {
    #[must_use]
    pub fn running_for_workflow(workflow: &Workflow) -> Self {
        let status_by_node = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), AgentStatus::Idle))
            .collect();
        Self {
            active: true,
            awaiting_node_id: None,
            active_manual_node_id: None,
            status_by_node,
            last_report: None,
            last_error: None,
            chat_logs: BTreeMap::new(),
            run_trace: Vec::new(),
            outputs: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn idle_for_workflow(workflow: &Workflow) -> Self {
        Self {
            active: false,
            ..Self::running_for_workflow(workflow)
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub workflow: Workflow,
    pub selected_node_id: Option<NodeId>,
    pub link_from_node_id: Option<NodeId>,
    pub schema_editor_text: String,
    pub provider_api_key_input: String,
    pub entrypoint_text: String,
    pub status_by_node: BTreeMap<NodeId, AgentStatus>,
    pub last_run: Option<RunReport>,
    pub last_error: Option<String>,
    // NEW
    pub chat_logs: BTreeMap<NodeId, Vec<ChatMessage>>,
    pub run_trace: Vec<RunTraceEntry>,
    pub selected_trace_index: Option<usize>,
    pub node_auto_start: BTreeMap<NodeId, bool>,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let mut workflow = Workflow::new("New workflow");
        workflow.nodes.push(Node::agent("Idea", 80.0, 120.0));
        Self::from_workflow(workflow, String::new())
    }

    #[must_use]
    pub fn from_workflow(workflow: Workflow, api_key: String) -> Self {
        let selected_node_id = workflow.nodes.first().map(|node| node.id.clone());
        let schema_editor_text = workflow
            .nodes
            .first()
            .map(|node| serde_json::to_string_pretty(&node.agent.output_schema).unwrap_or_default())
            .unwrap_or_default();
        let status_by_node = workflow
            .nodes
            .iter()
            .map(|node| (node.id.clone(), AgentStatus::Idle))
            .collect();
        let chat_logs = workflow
            .nodes
            .iter()
            .map(|n| (n.id.clone(), Vec::new()))
            .collect();
        let node_auto_start = workflow
            .nodes
            .iter()
            .map(|n| (n.id.clone(), n.agent.auto_start))
            .collect();
        Self {
            workflow,
            selected_node_id,
            link_from_node_id: None,
            schema_editor_text,
            provider_api_key_input: api_key,
            entrypoint_text: String::new(),
            status_by_node,
            last_run: None,
            last_error: None,
            chat_logs,
            run_trace: Vec::new(),
            selected_trace_index: None,
            node_auto_start,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn add_agent_node(&mut self) -> NodeId {
        let index = self.workflow.nodes.len() + 1;
        let node = Node::agent(
            format!("Agent {index}"),
            (index as f32).mul_add(48.0, 80.0),
            (index as f32).mul_add(24.0, 120.0),
        );
        let node_id = node.id.clone();
        self.workflow.nodes.push(node);
        self.status_by_node
            .insert(node_id.clone(), AgentStatus::Idle);
        self.chat_logs.insert(node_id.clone(), Vec::new());
        self.node_auto_start.insert(node_id.clone(), true);
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

    #[must_use]
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

    pub fn resolve_provider_api_key(&self, env_key: Option<&str>) -> Option<String> {
        if !self.provider_api_key_input.trim().is_empty() {
            return Some(self.provider_api_key_input.trim().to_string());
        }
        env_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    #[must_use]
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
        if let Some(node) = self
            .workflow
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
        {
            let (new_x, new_y) = clamp_node_position(
                (node.position.x + dx, node.position.y + dy),
                node_size,
                canvas_size,
            );
            node.position.x = new_x;
            node.position.y = new_y;
        }
    }

    pub fn refresh_statuses_from_report(&mut self) {
        self.status_by_node.clear();
        for node in &self.workflow.nodes {
            self.status_by_node
                .insert(node.id.clone(), AgentStatus::Idle);
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

    pub fn remove_selected_node(&mut self) {
        let Some(selected) = self.selected_node_id.clone() else {
            return;
        };
        self.workflow.nodes.retain(|node| node.id != selected);
        self.workflow
            .edges
            .retain(|edge| edge.from != selected && edge.to != selected);
        self.status_by_node.remove(&selected);
        self.chat_logs.remove(&selected);
        self.node_auto_start.remove(&selected);
        self.run_trace.retain(|entry| entry.node_id != selected);
        if self
            .selected_trace_index
            .is_some_and(|index| index >= self.run_trace.len())
        {
            self.selected_trace_index = None;
        }
        self.link_from_node_id = None;
        self.selected_node_id = self.workflow.nodes.first().map(|node| node.id.clone());
        self.refresh_schema_editor();
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

    /// # Errors
    /// Returns an error if the workflow fails validation.
    pub fn validate(&mut self) -> Result<(), WorkflowValidationError> {
        let result = validate_workflow(&self.workflow);
        self.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    pub fn set_run_report(&mut self, report: RunReport) {
        if self.run_trace.is_empty() {
            self.run_trace = report
                .events
                .iter()
                .map(|event| RunTraceEntry {
                    node_id: event.node_id.clone(),
                    node_label: self.node_label(&event.node_id),
                    status: TraceStatus::from_run_event_kind(&event.kind),
                    message: event.message.clone(),
                    output: event.output.clone(),
                })
                .collect();
        }
        self.last_run = Some(report);
        self.last_error = None;
        self.refresh_statuses_from_report();
    }

    pub fn add_chat_message(&mut self, node_id: &str, role: ChatRole, content: String) {
        self.chat_logs
            .entry(NodeId(node_id.to_string()))
            .or_default()
            .push(ChatMessage { role, content });
    }

    pub fn push_run_trace(&mut self, entry: RunTraceEntry) {
        self.run_trace.push(entry);
    }

    pub fn clear_run_trace(&mut self) {
        self.run_trace.clear();
        self.selected_trace_index = None;
    }

    pub const fn select_trace_event(&mut self, index: usize) {
        if index < self.run_trace.len() {
            self.selected_trace_index = Some(index);
        }
    }

    #[must_use]
    pub fn selected_trace_event(&self) -> Option<&RunTraceEntry> {
        let index = self.selected_trace_index?;
        self.run_trace.get(index)
    }

    pub fn set_node_auto_start(&mut self, node_id: &str, value: bool) {
        self.node_auto_start
            .insert(NodeId(node_id.to_string()), value);
        if let Some(node) = self.workflow.nodes.iter_mut().find(|n| n.id == node_id) {
            node.agent.auto_start = value;
        }
    }

    fn node_label(&self, node_id: &str) -> String {
        self.workflow
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .map_or_else(|| "Unknown".to_string(), |node| node.label.clone())
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
        assert!(!rows[0].contains(&*first));
        assert!(!rows[0].contains(&*second));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn moves_node_with_drag_delta_and_clamps_to_canvas() {
        let mut state = AppState::new();
        let node_id = state.selected_node_id.clone().unwrap();

        state.move_node_by_delta(&node_id, 20.0, 10.0, (640.0, 480.0), (220.0, 120.0));
        let moved = state.selected_node().unwrap().position.clone();
        assert!(moved.x >= 0.0);
        assert!(moved.y >= 0.0);

        state.move_node_by_delta(
            &node_id,
            -10_000.0,
            -10_000.0,
            (640.0, 480.0),
            (220.0, 120.0),
        );
        let clamped = state.selected_node().unwrap().position.clone();
        assert_eq!(clamped.x, 0.0);
        assert_eq!(clamped.y, 0.0);
    }

    #[test]
    fn ui_key_overrides_env_key_resolution() {
        let mut state = AppState::new();
        state.provider_api_key_input = "sk-ui-123".to_string();

        let key = state.resolve_provider_api_key(Some("sk-env-456"));

        assert_eq!(key.as_deref(), Some("sk-ui-123"));
    }

    #[test]
    fn edge_composer_connects_selected_source_and_target() {
        let mut state = AppState::new();
        let source = state.selected_node_id.clone().unwrap();
        let target = state.add_agent_node();

        state.link_from_node_id = Some(source);
        state.connect_link_to(target);

        assert_eq!(state.edge_rows(), vec!["Idea -> Agent 2".to_string()]);
    }

    #[test]
    fn removing_selected_node_also_removes_incident_edges() {
        let mut state = AppState::new();
        let first = state.selected_node_id.clone().unwrap();
        let second = state.add_agent_node();

        state.select_node(first);
        state.begin_link_from_selected();
        state.connect_link_to(second.clone());

        state.select_node(second);
        state.remove_selected_node();

        assert_eq!(state.workflow.nodes.len(), 1);
        assert!(state.workflow.edges.is_empty());
    }

    #[test]
    fn awaiting_input_status_is_new_variant() {
        let status = AgentStatus::AwaitingInput;
        assert_ne!(status, AgentStatus::Idle);
    }

    #[test]
    fn stores_chat_message_for_node() {
        let mut state = AppState::new();
        let id = state.selected_node_id.clone().unwrap();
        state.add_chat_message(&id, ChatRole::System, "test".to_string());
        assert_eq!(state.chat_logs[&id].len(), 1);
        assert_eq!(state.chat_logs[&id][0].content, "test");
    }

    #[test]
    fn toggles_node_auto_start() {
        let mut state = AppState::new();
        let id = state.selected_node_id.clone().unwrap();
        state.set_node_auto_start(&id, false);
        assert!(!state.node_auto_start[&id]);
        assert!(!state.selected_node().unwrap().agent.auto_start);
    }

    #[test]
    fn rejects_self_edge_and_clears_pending_link() {
        let mut state = AppState::new();
        let first = state.selected_node_id.clone().unwrap();

        state.begin_link_from_selected();
        state.connect_link_to(first);

        assert!(state.workflow.edges.is_empty());
        assert_eq!(
            state.last_error.as_deref(),
            Some("cannot connect a node to itself")
        );
        assert!(state.link_from_node_id.is_none());
    }

    #[test]
    fn invalid_schema_editor_preserves_existing_schema() {
        let mut state = AppState::new();
        let original_schema = state.selected_node().unwrap().agent.output_schema.clone();
        state.schema_editor_text = "{not json".to_string();

        state.apply_schema_editor();

        assert_eq!(
            state.selected_node().unwrap().agent.output_schema,
            original_schema
        );
        assert!(state
            .last_error
            .as_deref()
            .unwrap()
            .contains("output schema JSON invalid"));
    }

    #[test]
    fn removing_last_node_clears_selection_and_schema_editor() {
        let mut state = AppState::new();

        state.remove_selected_node();

        assert!(state.workflow.nodes.is_empty());
        assert!(state.selected_node_id.is_none());
        assert!(state.schema_editor_text.is_empty());
        assert!(state.status_by_node.is_empty());
        assert!(state.chat_logs.is_empty());
        assert!(state.node_auto_start.is_empty());
    }

    #[test]
    fn refresh_statuses_from_report_uses_latest_event_per_node() {
        let mut state = AppState::new();
        let first = state.selected_node_id.clone().unwrap();
        state.set_run_report(RunReport {
            workflow_id: state.workflow.id.clone(),
            events: vec![
                workflow_core::RunEvent {
                    node_id: first.clone(),
                    kind: RunEventKind::Queued,
                    message: "queued".to_string(),
                    output: None,
                },
                workflow_core::RunEvent {
                    node_id: first.clone(),
                    kind: RunEventKind::Completed,
                    message: "completed".to_string(),
                    output: Some(json!({"summary": "done"})),
                },
            ],
            outputs: vec![],
        });

        assert_eq!(state.status_by_node[&first], AgentStatus::Completed);
    }

    #[test]
    fn starts_with_empty_run_trace() {
        let state = AppState::new();

        assert!(state.run_trace.is_empty());
        assert!(state.selected_trace_index.is_none());
    }

    #[test]
    fn records_and_selects_run_trace_entries() {
        let mut state = AppState::new();
        let id = state.selected_node_id.clone().unwrap();

        state.push_run_trace(RunTraceEntry {
            node_id: id.clone(),
            node_label: "Idea".to_string(),
            status: TraceStatus::Running,
            message: "started OpenAI node call".to_string(),
            output: None,
        });
        state.select_trace_event(0);

        let selected = state.selected_trace_event().unwrap();
        assert_eq!(selected.node_id, id);
        assert_eq!(selected.status, TraceStatus::Running);
    }

    #[test]
    fn clearing_run_trace_clears_selected_trace_event() {
        let mut state = AppState::new();
        let id = state.selected_node_id.clone().unwrap();
        state.push_run_trace(RunTraceEntry {
            node_id: id,
            node_label: "Idea".to_string(),
            status: TraceStatus::Failed,
            message: "bad key".to_string(),
            output: None,
        });
        state.select_trace_event(0);

        state.clear_run_trace();

        assert!(state.run_trace.is_empty());
        assert!(state.selected_trace_index.is_none());
    }

    #[test]
    fn blank_ui_api_key_falls_back_to_trimmed_env_key() {
        let mut state = AppState::new();
        state.provider_api_key_input = "   ".to_string();

        let key = state.resolve_provider_api_key(Some("  sk-env-456  "));

        assert_eq!(key.as_deref(), Some("sk-env-456"));
    }
}
