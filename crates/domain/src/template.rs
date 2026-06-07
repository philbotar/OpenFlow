#![allow(clippy::too_many_lines, clippy::float_cmp, clippy::doc_markdown)]

use crate::model::{AgentNodeConfig, Node, NodeId, NodeKind, NodePosition};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// A pre-configured node template with smart defaults for quick workflow construction.
///
/// Templates define a reusable starting point for a node: the configuration that a
/// user is expected to customize (override) and the fields that should remain locked
/// (e.g., a specific model or output schema constraint for a particular use case).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Template {
    /// Unique identifier for this template.
    pub id: String,

    /// Human-readable display name (shown in the template library).
    pub display_name: String,

    /// Longer description explaining what the template is for.
    pub description: String,

    /// The default agent configuration to apply when a node is created from this template.
    pub default_config: AgentNodeConfig,

    /// Field names that should be locked (non-editable) when this template is applied.
    /// Valid field names: "system_prompt", "task_prompt", "model", "output_schema", "auto_start"
    pub locked_fields: HashSet<String>,
}

impl Template {
    /// Create a new template with generated ID.
    pub fn new(
        display_name: impl Into<String>,
        description: impl Into<String>,
        default_config: AgentNodeConfig,
        locked_fields: HashSet<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            display_name: display_name.into(),
            description: description.into(),
            default_config,
            locked_fields,
        }
    }

    /// Instantiate a new `Node` from this template at the given canvas position.
    /// The node gets the template's default config and the template's display_name as its label.
    #[must_use]
    pub fn instantiate(&self, x: f32, y: f32) -> Node {
        Node {
            id: NodeId(Uuid::new_v4().to_string()),
            label: self.display_name.clone(),
            kind: NodeKind::Agent,
            position: NodePosition { x, y },
            agent: self.default_config.clone(),
        }
    }

    /// Check whether a given field name is locked in this template.
    #[must_use]
    pub fn is_field_locked(&self, field_name: &str) -> bool {
        self.locked_fields.contains(field_name)
    }

    /// Lock a single field by name. Returns true if the field was newly locked.
    pub fn lock_field(&mut self, field_name: impl Into<String>) -> bool {
        self.locked_fields.insert(field_name.into())
    }

    /// Unlock a single field by name. Returns true if the field was previously locked.
    pub fn unlock_field(&mut self, field_name: &str) -> bool {
        self.locked_fields.remove(field_name)
    }
}

/// Returns a curated set of default templates for the template library.
#[must_use]
pub fn default_templates() -> Vec<Template> {
    vec![
        Template::new(
            "Simple Agent",
            "A basic agent node with a concise agent promot and minimal output schema. Good starting point for most tasks.",
            AgentNodeConfig {
                system_prompt: "You are a focused AI agent in a node workflow.".to_string(),
                task_prompt: "Return a concise JSON object for this node.".to_string(),
                model: String::new(),
                output_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "summary": { "type": "string" }
                    },
                    "required": ["summary"]
                }),
                auto_start: true,
                tools: AgentNodeConfig::default().tools,
                callable_agents: Vec::new(),
                allow_all_callable_agents: false,
            },
            HashSet::new(),
        ),
        Template::new(
            "Code Reviewer",
            "An agent configured to review code and provide structured feedback with severity ratings.",
            AgentNodeConfig {
                system_prompt: "You are a code reviewer. Analyze code for bugs, style issues, security problems, and performance concerns. Be thorough and specific.".to_string(),
                task_prompt: "Review the provided code and return findings as a structured object with severity ratings.".to_string(),
                model: String::new(),
                output_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "findings": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "severity": { "type": "string", "enum": ["critical", "warning", "info"] },
                                    "file": { "type": "string" },
                                    "line": { "type": "integer" },
                                    "description": { "type": "string" },
                                    "suggestion": { "type": "string" }
                                },
                                "required": ["severity", "description"]
                            }
                        },
                        "summary": { "type": "string" }
                    },
                    "required": ["findings"]
                }),
                auto_start: true,
                tools: AgentNodeConfig::default().tools,
                callable_agents: Vec::new(),
                allow_all_callable_agents: false,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("output_schema".to_string());
                locked
            },
        ),
        Template::new(
            "Document Summarizer",
            "An agent that reads content and produces a structured summary with key points.",
            AgentNodeConfig {
                system_prompt: "You are a document summarizer. Extract the most important information and present it clearly. Include key takeaways, action items, and a concise executive summary.".to_string(),
                task_prompt: "Summarize the input and return a structured digest.".to_string(),
                model: String::new(),
                output_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "executive_summary": { "type": "string" },
                        "key_points": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "action_items": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["executive_summary", "key_points"]
                }),
                auto_start: true,
                tools: AgentNodeConfig::default().tools,
                callable_agents: Vec::new(),
                allow_all_callable_agents: false,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("output_schema".to_string());
                locked
            },
        ),
        Template::new(
            "Classifier",
            "An agent that classifies input into predefined categories with confidence scores.",
            AgentNodeConfig {
                system_prompt: "You are a classifier. Determine which category the input belongs to and provide a confidence score. If none fit, use 'other'.".to_string(),
                task_prompt: "Classify the input and return the result.".to_string(),
                model: String::new(),
                output_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "category": { "type": "string" },
                        "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                        "rationale": { "type": "string" }
                    },
                    "required": ["category", "confidence"]
                }),
                auto_start: true,
                tools: AgentNodeConfig::default().tools,
                callable_agents: Vec::new(),
                allow_all_callable_agents: false,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("output_schema".to_string());
                locked
            },
        ),
        Template::new(
            "Manual Start Node",
            "An agent that must be manually triggered (auto_start: false). Useful for approval gates or human-in-the-loop steps.",
            AgentNodeConfig {
                system_prompt: "You are a reviewer agent. Wait to be triggered before processing.".to_string(),
                task_prompt: "Review the upstream output and return your assessment.".to_string(),
                model: String::new(),
                output_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "approved": { "type": "boolean" },
                        "comments": { "type": "string" }
                    },
                    "required": ["approved"]
                }),
                auto_start: false,
                tools: AgentNodeConfig::default().tools,
                callable_agents: Vec::new(),
                allow_all_callable_agents: false,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("auto_start".to_string());
                locked
            },
        ),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn template_instantiate_uses_display_name_as_label() {
        let template = Template::new(
            "Code Reviewer",
            "desc",
            AgentNodeConfig::default(),
            HashSet::new(),
        );
        let node = template.instantiate(100.0, 200.0);

        assert_eq!(node.label, "Code Reviewer");
        assert_eq!(node.position.x, 100.0);
        assert_eq!(node.position.y, 200.0);
        assert_eq!(node.kind, NodeKind::Agent);
    }

    #[test]
    fn template_lock_field() {
        let mut template =
            Template::new("Test", "desc", AgentNodeConfig::default(), HashSet::new());

        assert!(!template.is_field_locked("model"));

        let newly_locked = template.lock_field("model");
        assert!(newly_locked);
        assert!(template.is_field_locked("model"));

        // Locking again returns false (already locked)
        let already = template.lock_field("model");
        assert!(!already);
    }

    #[test]
    fn template_unlock_field() {
        let mut locked = HashSet::new();
        locked.insert("model".to_string());

        let mut template = Template::new("Test", "desc", AgentNodeConfig::default(), locked);

        assert!(template.is_field_locked("model"));
        assert!(template.unlock_field("model"));
        assert!(!template.is_field_locked("model"));
        assert!(!template.unlock_field("model"));
    }

    #[test]
    fn template_instantiate_preserves_config() {
        let config = AgentNodeConfig {
            system_prompt: "custom prompt".to_string(),
            task_prompt: "custom task".to_string(),
            model: "o3".to_string(),
            output_schema: serde_json::json!({"custom": true}),
            auto_start: false,
            tools: AgentNodeConfig::default().tools,
            callable_agents: Vec::new(),
            allow_all_callable_agents: false,
        };
        let template = Template::new("Test", "desc", config.clone(), HashSet::new());
        let node = template.instantiate(0.0, 0.0);

        assert_eq!(node.agent, config);
    }

    #[test]
    fn default_templates_are_non_empty() {
        let templates = default_templates();
        assert!(
            !templates.is_empty(),
            "must provide at least one default template"
        );
    }

    #[test]
    fn default_templates_have_unique_ids() {
        let templates = default_templates();
        let ids: HashSet<&String> = templates.iter().map(|t| &t.id).collect();
        assert_eq!(
            ids.len(),
            templates.len(),
            "all default templates must have unique generated IDs"
        );
    }
}
