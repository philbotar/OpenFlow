use super::error::AuthoringError;
use engine::{Edge, EdgeId, Node, NodeId, NodeKind, Workflow, WorkflowId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringDraft {
    pub name: String,
    #[serde(default)]
    pub shared_context: String,
    pub nodes: Vec<WorkflowAuthoringNodeDraft>,
    pub edges: Vec<WorkflowAuthoringEdgeDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringNodeDraft {
    pub id: String,
    pub label: String,
    pub system_prompt: String,
    pub task_prompt: String,
    #[serde(default)]
    pub output_schema: Value,
    #[serde(default)]
    pub auto_start: bool,
}

#[must_use]
pub fn default_node_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": { "summary": { "type": "string" } },
        "required": ["summary"]
    })
}

#[must_use]
pub fn workflow_to_authoring_draft(workflow: &Workflow) -> WorkflowAuthoringDraft {
    WorkflowAuthoringDraft {
        name: workflow.name.clone(),
        shared_context: workflow.settings.shared_context.clone(),
        nodes: workflow
            .nodes
            .iter()
            .map(|node| WorkflowAuthoringNodeDraft {
                id: node.id.to_string(),
                label: node.label.clone(),
                system_prompt: node.agent.system_prompt.clone(),
                task_prompt: node.agent.task_prompt.clone(),
                output_schema: node.agent.output_schema.clone(),
                auto_start: node.agent.auto_start,
            })
            .collect(),
        edges: workflow
            .edges
            .iter()
            .map(|edge| WorkflowAuthoringEdgeDraft {
                id: edge.id.to_string(),
                from: edge.from.to_string(),
                to: edge.to.to_string(),
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAuthoringEdgeDraft {
    pub id: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftParseError {
    InvalidJson(String),
    MissingName,
    NoNodes,
}

/// Extract the workflow draft object from a model submit_output payload.
///
/// Accepts camelCase/snake_case wrappers or a flattened top-level draft shape.
///
/// # Errors
/// Returns an error when no draft object is present.
pub fn workflow_draft_value_from_model_output(output: &Value) -> Result<Value, AuthoringError> {
    let output = output
        .get("output")
        .filter(|value| value.is_object())
        .unwrap_or(output);

    if let Some(draft) = output
        .get("workflowDraft")
        .or_else(|| output.get("workflow_draft"))
    {
        return Ok(draft.clone());
    }

    let Some(map) = output.as_object() else {
        return Err(AuthoringError::MissingDraft(
            "missing workflowDraft in model output".to_string(),
        ));
    };

    if map.contains_key("name") && map.contains_key("nodes") {
        let mut draft = map.clone();
        draft.remove("assistantMessage");
        draft.remove("assistant_message");
        return Ok(Value::Object(draft));
    }

    Err(AuthoringError::MissingDraft(
        "missing workflowDraft in model output — the model must include a workflowDraft object with name, nodes, and edges".to_string(),
    ))
}

/// # Errors
/// Returns an error when the draft JSON is invalid or missing required fields.
pub fn parse_authoring_draft(raw: &str) -> Result<WorkflowAuthoringDraft, DraftParseError> {
    let draft: WorkflowAuthoringDraft = serde_json::from_str(raw)
        .map_err(|error| DraftParseError::InvalidJson(error.to_string()))?;
    if draft.name.trim().is_empty() {
        return Err(DraftParseError::MissingName);
    }
    if draft.nodes.is_empty() {
        return Err(DraftParseError::NoNodes);
    }
    Ok(draft)
}

#[must_use]
pub fn materialize_authoring_draft(
    draft: WorkflowAuthoringDraft,
    base_workflow_id: Option<WorkflowId>,
    default_model: &str,
) -> Workflow {
    let workflow_id = base_workflow_id.unwrap_or_else(|| WorkflowId(Uuid::new_v4().to_string()));
    let mut workflow = Workflow {
        id: workflow_id,
        name: draft.name,
        nodes: Vec::new(),
        edges: Vec::new(),
        settings: engine::WorkflowSettings {
            shared_context: draft.shared_context,
            ..Default::default()
        },
    };

    for node_draft in draft.nodes {
        let node_id = NodeId(node_draft.id);
        workflow.nodes.push(Node {
            id: node_id,
            label: node_draft.label,
            kind: NodeKind::Agent,
            position: engine::NodePosition { x: 0.0, y: 0.0 },
            agent: engine::AgentNodeConfig {
                system_prompt: node_draft.system_prompt,
                task_prompt: node_draft.task_prompt,
                model: default_model.to_string(),
                output_schema: if node_draft.output_schema.is_null() {
                    default_node_output_schema()
                } else {
                    node_draft.output_schema
                },
                auto_start: node_draft.auto_start,
                ..Default::default()
            },
        });
    }

    for edge_draft in draft.edges {
        workflow.edges.push(Edge {
            id: EdgeId(edge_draft.id),
            from: NodeId(edge_draft.from),
            to: NodeId(edge_draft.to),
        });
    }

    workflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_draft_value_accepts_submit_output_wrapper() {
        let output = json!({
            "output": {
                "assistantMessage": "Here is a draft.",
                "workflowDraft": {
                    "name": "Demo",
                    "nodes": [{ "id": "root", "label": "Root", "systemPrompt": "s", "taskPrompt": "t" }],
                    "edges": []
                }
            }
        });
        let draft = workflow_draft_value_from_model_output(&output).expect("draft");
        assert_eq!(draft["name"], "Demo");
    }

    #[test]
    fn parse_and_materialize_feature_plan_shape() {
        let raw = include_str!("../../../../../examples/feature_plan.workflow.json");
        let example: Workflow = serde_json::from_str(raw).expect("example workflow json");
        let draft = WorkflowAuthoringDraft {
            name: example.name.clone(),
            shared_context: String::new(),
            nodes: example
                .nodes
                .iter()
                .map(|node| WorkflowAuthoringNodeDraft {
                    id: node.id.to_string(),
                    label: node.label.clone(),
                    system_prompt: node.agent.system_prompt.clone(),
                    task_prompt: node.agent.task_prompt.clone(),
                    output_schema: node.agent.output_schema.clone(),
                    auto_start: node.agent.auto_start,
                })
                .collect(),
            edges: example
                .edges
                .iter()
                .map(|edge| WorkflowAuthoringEdgeDraft {
                    id: edge.id.to_string(),
                    from: edge.from.to_string(),
                    to: edge.to.to_string(),
                })
                .collect(),
        };
        let json = serde_json::to_string(&draft).expect("draft json");
        let parsed = parse_authoring_draft(&json).expect("parse draft");
        let workflow = materialize_authoring_draft(parsed, Some(example.id.clone()), "gpt-5.5");
        assert_eq!(workflow.nodes.len(), 4);
        assert_eq!(workflow.edges.len(), 4);
        assert_eq!(workflow.nodes[0].agent.model, "gpt-5.5");
    }
}
