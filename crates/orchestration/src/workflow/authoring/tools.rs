use super::draft::{
    default_node_output_schema, materialize_authoring_draft, workflow_to_authoring_draft,
    WorkflowAuthoringDraft, WorkflowAuthoringEdgeDraft, WorkflowAuthoringNodeDraft,
};
use super::error::AuthoringError;
use super::layout::layout_workflow_by_layers;
use super::validate::validate_authoring_workflow;
use crate::api::WorkflowAuthoringValidation;
use engine::{
    ToolCall, ToolConcurrency, ToolDefinition, ToolResult, ToolTier, Workflow, WorkflowId,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;

pub const SET_WORKFLOW_META_TOOL: &str = "openflow_set_workflow_meta";
pub const ADD_NODE_TOOL: &str = "openflow_add_node";
pub const UPDATE_NODE_TOOL: &str = "openflow_update_node";
pub const ADD_EDGE_TOOL: &str = "openflow_add_edge";
pub const REMOVE_NODE_TOOL: &str = "openflow_remove_node";
pub const REMOVE_EDGE_TOOL: &str = "openflow_remove_edge";

const AUTHORING_TOOL_NAMES: [&str; 6] = [
    SET_WORKFLOW_META_TOOL,
    ADD_NODE_TOOL,
    UPDATE_NODE_TOOL,
    ADD_EDGE_TOOL,
    REMOVE_NODE_TOOL,
    REMOVE_EDGE_TOOL,
];

pub const MAX_AUTHORING_TOOL_ROUNDS: u8 = 24;

#[must_use]
pub fn is_authoring_tool(name: &str) -> bool {
    AUTHORING_TOOL_NAMES.contains(&name)
}

#[must_use]
pub fn authoring_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: SET_WORKFLOW_META_TOOL.to_string(),
            description: "Set workflow name, optional shared context, and optional Plan → Execute review/freeze node. The Plan Mode source must be an existing node with requestUserInput true.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string" },
                    "sharedContext": { "type": "string" },
                    "planModeSourceNodeId": {
                        "type": ["string", "null"],
                        "description": "Node id that conversationally reviews and freezes the approved change evidence packet. Null disables Plan → Execute mode."
                    }
                }
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        ToolDefinition {
            name: ADD_NODE_TOOL.to_string(),
            description: "Add one agent node. Call once per node with short prompts (1-2 sentences). outputSchema is optional — defaults to { summary: string }.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string" },
                    "label": { "type": "string" },
                    "systemPrompt": { "type": "string" },
                    "taskPrompt": { "type": "string" },
                    "autoStart": { "type": "boolean" },
                    "requestUserInput": {
                        "type": "boolean",
                        "description": "True only when this node genuinely needs an ongoing human conversation. Use false for autonomous planning, coding, searching, reviewing, and verification nodes."
                    },
                    "outputSchema": { "type": "object" }
                },
                "required": ["id", "label", "systemPrompt", "taskPrompt", "autoStart"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        ToolDefinition {
            name: UPDATE_NODE_TOOL.to_string(),
            description: "Update fields on an existing node by id.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string" },
                    "label": { "type": "string" },
                    "systemPrompt": { "type": "string" },
                    "taskPrompt": { "type": "string" },
                    "autoStart": { "type": "boolean" },
                    "requestUserInput": { "type": "boolean" },
                    "outputSchema": { "type": "object" }
                },
                "required": ["id"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        ToolDefinition {
            name: ADD_EDGE_TOOL.to_string(),
            description: "Connect two existing nodes. from and to must match node ids.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string" },
                    "from": { "type": "string" },
                    "to": { "type": "string" }
                },
                "required": ["id", "from", "to"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        ToolDefinition {
            name: REMOVE_NODE_TOOL.to_string(),
            description: "Remove a node and any edges touching it.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        ToolDefinition {
            name: REMOVE_EDGE_TOOL.to_string(),
            description: "Remove an edge by id.".to_string(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
    ]
}

pub struct AuthoringToolState {
    draft: WorkflowAuthoringDraft,
    base_workflow_id: Option<WorkflowId>,
    default_model: String,
}

impl AuthoringToolState {
    #[must_use]
    pub fn new(current_draft: Option<&Workflow>, default_model: &str) -> Self {
        match current_draft {
            Some(workflow) => Self {
                draft: workflow_to_authoring_draft(workflow),
                base_workflow_id: Some(workflow.id.clone()),
                default_model: default_model.to_string(),
            },
            None => Self {
                draft: WorkflowAuthoringDraft {
                    name: String::new(),
                    shared_context: String::new(),
                    plan_mode_source_node_id: None,
                    nodes: Vec::new(),
                    edges: Vec::new(),
                },
                base_workflow_id: None,
                default_model: default_model.to_string(),
            },
        }
    }

    pub fn execute(&mut self, call: &ToolCall) -> ToolResult {
        let outcome = match call.name.as_str() {
            SET_WORKFLOW_META_TOOL => self.set_workflow_meta(&call.arguments),
            ADD_NODE_TOOL => self.add_node(&call.arguments),
            UPDATE_NODE_TOOL => self.update_node(&call.arguments),
            ADD_EDGE_TOOL => self.add_edge(&call.arguments),
            REMOVE_NODE_TOOL => self.remove_node(&call.arguments),
            REMOVE_EDGE_TOOL => self.remove_edge(&call.arguments),
            other => Err(format!("unknown authoring tool {other}")),
        };
        match outcome {
            Ok(()) => tool_success(call, self.snapshot()),
            Err(message) => tool_error(call, message),
        }
    }

    pub fn materialize_workflow(
        &self,
    ) -> Result<(Workflow, WorkflowAuthoringValidation), AuthoringError> {
        if self.draft.nodes.is_empty() {
            return Err(AuthoringError::InvalidDraft(
                "workflow has no nodes — add at least one with openflow_add_node".to_string(),
            ));
        }
        let mut draft = self.draft.clone();
        if draft.name.trim().is_empty() {
            draft.name = "Untitled workflow".to_string();
        }
        let mut workflow =
            materialize_authoring_draft(draft, self.base_workflow_id.clone(), &self.default_model);
        layout_workflow_by_layers(&mut workflow)
            .map_err(|error| AuthoringError::LayoutFailed(error.to_string()))?;
        let validation = validate_authoring_workflow(&workflow);
        Ok((workflow, validation))
    }

    fn set_workflow_meta(&mut self, args: &Value) -> Result<(), String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            name: Option<String>,
            shared_context: Option<String>,
            plan_mode_source_node_id: Option<Option<String>>,
        }
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|error| format!("invalid arguments: {error}"))?;
        if let Some(name) = args.name {
            if name.trim().is_empty() {
                return Err("name must be non-empty".to_string());
            }
            self.draft.name = name;
        }
        if let Some(shared_context) = args.shared_context {
            self.draft.shared_context = shared_context;
        }
        if let Some(plan_mode_source_node_id) = args.plan_mode_source_node_id {
            self.draft.plan_mode_source_node_id = plan_mode_source_node_id;
        }
        Ok(())
    }

    fn add_node(&mut self, args: &Value) -> Result<(), String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            id: String,
            label: String,
            system_prompt: String,
            task_prompt: String,
            auto_start: bool,
            #[serde(default)]
            request_user_input: bool,
            output_schema: Option<Value>,
        }
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|error| format!("invalid arguments: {error}"))?;
        validate_node_id(&args.id)?;
        if args.label.trim().is_empty() {
            return Err("label must be non-empty".to_string());
        }
        if args.system_prompt.trim().is_empty() || args.task_prompt.trim().is_empty() {
            return Err("systemPrompt and taskPrompt must be non-empty".to_string());
        }
        if self.draft.nodes.iter().any(|node| node.id == args.id) {
            return Err(format!("node id '{}' already exists", args.id));
        }
        self.draft.nodes.push(WorkflowAuthoringNodeDraft {
            id: args.id,
            label: args.label,
            system_prompt: args.system_prompt,
            task_prompt: args.task_prompt,
            output_schema: args
                .output_schema
                .filter(|schema| schema.is_object())
                .unwrap_or_else(default_node_output_schema),
            auto_start: args.auto_start,
            request_user_input: args.request_user_input,
        });
        Ok(())
    }

    fn update_node(&mut self, args: &Value) -> Result<(), String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            id: String,
            label: Option<String>,
            system_prompt: Option<String>,
            task_prompt: Option<String>,
            auto_start: Option<bool>,
            request_user_input: Option<bool>,
            output_schema: Option<Value>,
        }
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|error| format!("invalid arguments: {error}"))?;
        let node = self
            .draft
            .nodes
            .iter_mut()
            .find(|node| node.id == args.id)
            .ok_or_else(|| format!("node '{}' not found", args.id))?;
        if let Some(label) = args.label {
            if label.trim().is_empty() {
                return Err("label must be non-empty".to_string());
            }
            node.label = label;
        }
        if let Some(system_prompt) = args.system_prompt {
            if system_prompt.trim().is_empty() {
                return Err("systemPrompt must be non-empty".to_string());
            }
            node.system_prompt = system_prompt;
        }
        if let Some(task_prompt) = args.task_prompt {
            if task_prompt.trim().is_empty() {
                return Err("taskPrompt must be non-empty".to_string());
            }
            node.task_prompt = task_prompt;
        }
        if let Some(auto_start) = args.auto_start {
            node.auto_start = auto_start;
        }
        if let Some(request_user_input) = args.request_user_input {
            node.request_user_input = request_user_input;
        }
        if let Some(output_schema) = args.output_schema {
            if !output_schema.is_object() {
                return Err("outputSchema must be a JSON object".to_string());
            }
            node.output_schema = output_schema;
        }
        Ok(())
    }

    fn add_edge(&mut self, args: &Value) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Args {
            id: String,
            from: String,
            to: String,
        }
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|error| format!("invalid arguments: {error}"))?;
        if args.id.trim().is_empty() {
            return Err("edge id must be non-empty".to_string());
        }
        if args.from == args.to {
            return Err("edge cannot connect a node to itself".to_string());
        }
        let node_ids: HashSet<_> = self
            .draft
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        if !node_ids.contains(args.from.as_str()) {
            return Err(format!("from node '{}' does not exist", args.from));
        }
        if !node_ids.contains(args.to.as_str()) {
            return Err(format!("to node '{}' does not exist", args.to));
        }
        if self.draft.edges.iter().any(|edge| edge.id == args.id) {
            return Err(format!("edge id '{}' already exists", args.id));
        }
        self.draft.edges.push(WorkflowAuthoringEdgeDraft {
            id: args.id,
            from: args.from,
            to: args.to,
        });
        Ok(())
    }

    fn remove_node(&mut self, args: &Value) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Args {
            id: String,
        }
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|error| format!("invalid arguments: {error}"))?;
        let before = self.draft.nodes.len();
        self.draft.nodes.retain(|node| node.id != args.id);
        if self.draft.nodes.len() == before {
            return Err(format!("node '{}' not found", args.id));
        }
        self.draft
            .edges
            .retain(|edge| edge.from != args.id && edge.to != args.id);
        Ok(())
    }

    fn remove_edge(&mut self, args: &Value) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Args {
            id: String,
        }
        let args: Args = serde_json::from_value(args.clone())
            .map_err(|error| format!("invalid arguments: {error}"))?;
        let before = self.draft.edges.len();
        self.draft.edges.retain(|edge| edge.id != args.id);
        if self.draft.edges.len() == before {
            return Err(format!("edge '{}' not found", args.id));
        }
        Ok(())
    }

    fn snapshot(&self) -> Value {
        let validation = self.validation_summary();
        json!({
            "name": self.draft.name,
            "nodeCount": self.draft.nodes.len(),
            "edgeCount": self.draft.edges.len(),
            "nodeIds": self.draft.nodes.iter().map(|node| &node.id).collect::<Vec<_>>(),
            "validation": {
                "valid": validation.valid,
                "errors": validation.errors,
                "warnings": validation.warnings,
            }
        })
    }

    pub fn validation_summary(&self) -> WorkflowAuthoringValidation {
        if self.draft.nodes.is_empty() {
            return WorkflowAuthoringValidation {
                valid: false,
                errors: vec!["Workflow has no nodes yet".to_string()],
                warnings: Vec::new(),
                dag: None,
            };
        }
        let mut draft = self.draft.clone();
        if draft.name.trim().is_empty() {
            draft.name = "Untitled workflow".to_string();
        }
        let mut workflow =
            materialize_authoring_draft(draft, self.base_workflow_id.clone(), &self.default_model);
        if layout_workflow_by_layers(&mut workflow).is_err() {
            return WorkflowAuthoringValidation {
                valid: false,
                errors: vec!["Workflow graph is not a valid DAG yet".to_string()],
                warnings: Vec::new(),
                dag: None,
            };
        }
        validate_authoring_workflow(&workflow)
    }
}

fn validate_node_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("node id must be non-empty".to_string());
    }
    Ok(())
}

fn tool_success(call: &ToolCall, snapshot: Value) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: snapshot.to_string(),
        is_error: false,
        artifact_ids: Vec::new(),
        output_meta: None,
    }
}

fn tool_error(call: &ToolCall, message: String) -> ToolResult {
    ToolResult {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: json!({ "error": message }).to_string(),
        is_error: true,
        artifact_ids: Vec::new(),
        output_meta: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::ToolCall;

    fn call(name: &str, arguments: Value) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    #[test]
    fn incremental_tools_build_valid_workflow() {
        let mut state = AuthoringToolState::new(None, "gpt-5.5");
        assert!(state
            .execute(&call(
                SET_WORKFLOW_META_TOOL,
                json!({ "name": "Demo", "sharedContext": "Be concise." })
            ))
            .content
            .contains("\"nodeCount\":0"));
        assert!(
            !state
                .execute(&call(
                    ADD_NODE_TOOL,
                    json!({
                        "id": "root",
                        "label": "Root",
                        "systemPrompt": "You clarify ideas.",
                        "taskPrompt": "Summarize the user goal.",
                        "autoStart": true
                    })
                ))
                .is_error
        );
        assert!(
            !state
                .execute(&call(
                    ADD_NODE_TOOL,
                    json!({
                        "id": "plan",
                        "label": "Plan",
                        "systemPrompt": "You plan work.",
                        "taskPrompt": "Plan from upstream.",
                        "autoStart": true
                    })
                ))
                .is_error
        );
        assert!(
            !state
                .execute(&call(
                    ADD_EDGE_TOOL,
                    json!({ "id": "root-plan", "from": "root", "to": "plan" })
                ))
                .is_error
        );
        let (workflow, validation) = state.materialize_workflow().expect("materialize");
        assert!(validation.valid, "{:?}", validation.errors);
        assert_eq!(workflow.nodes.len(), 2);
        assert_eq!(workflow.edges.len(), 1);
        assert!(workflow
            .nodes
            .iter()
            .all(|node| !node.agent.request_user_input));
    }

    #[test]
    fn meta_tool_round_trips_plan_mode_source() {
        let mut state = AuthoringToolState::new(None, "gpt-5.5");
        assert!(
            !state
                .execute(&call(
                    SET_WORKFLOW_META_TOOL,
                    json!({ "name": "Plan then execute", "planModeSourceNodeId": "freeze" })
                ))
                .is_error
        );
        assert!(
            !state
                .execute(&call(
                    ADD_NODE_TOOL,
                    json!({
                        "id": "freeze",
                        "label": "Review",
                        "systemPrompt": "Review the change.",
                        "taskPrompt": "Ask for approval, then emit the packet.",
                        "autoStart": true,
                        "requestUserInput": true
                    })
                ))
                .is_error
        );

        let (workflow, validation) = state.materialize_workflow().expect("materialize");
        assert!(validation.valid, "{:?}", validation.errors);
        assert_eq!(
            workflow
                .settings
                .plan_mode
                .expect("plan mode")
                .evidence_source_node_id,
            engine::NodeId::from("freeze")
        );
    }
}
