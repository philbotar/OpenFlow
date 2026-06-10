#![allow(clippy::too_many_lines, clippy::float_cmp)]

use super::Template;
use crate::graph::AgentNodeConfig;
use std::collections::HashSet;

/// Curated builtin templates for the template library.
#[must_use]
pub fn default_templates() -> Vec<Template> {
    vec![
        Template::with_id(
            "builtin.simple-agent",
            "Simple Agent",
            "A basic agent node with a concise agent prompt and minimal output schema. Good starting point for most tasks.",
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
                reasoning_effort: None,
                reasoning_budget_tokens: None,
                provider_id: None,
            },
            HashSet::new(),
        ),
        Template::with_id(
            "builtin.code-reviewer",
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
                reasoning_effort: None,
                reasoning_budget_tokens: None,
                provider_id: None,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("output_schema".to_string());
                locked
            },
        ),
        Template::with_id(
            "builtin.document-summarizer",
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
                reasoning_effort: None,
                reasoning_budget_tokens: None,
                provider_id: None,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("output_schema".to_string());
                locked
            },
        ),
        Template::with_id(
            "builtin.classifier",
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
                reasoning_effort: None,
                reasoning_budget_tokens: None,
                provider_id: None,
            },
            {
                let mut locked = HashSet::new();
                locked.insert("output_schema".to_string());
                locked
            },
        ),
        Template::with_id(
            "builtin.manual-start-node",
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
                reasoning_effort: None,
                reasoning_budget_tokens: None,
                provider_id: None,
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
        let ids: HashSet<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            templates.len(),
            "all default templates must have unique generated IDs"
        );
    }

    #[test]
    fn default_templates_have_stable_builtin_ids_and_real_configs() {
        let templates = default_templates();
        let ids: Vec<&str> = templates
            .iter()
            .map(|template| template.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "builtin.simple-agent",
                "builtin.code-reviewer",
                "builtin.document-summarizer",
                "builtin.classifier",
                "builtin.manual-start-node",
            ]
        );
        assert!(templates
            .iter()
            .all(|template| !template.description.contains("promot")));
        assert!(templates
            .iter()
            .any(|template| !template.locked_fields.is_empty()));
    }
}
