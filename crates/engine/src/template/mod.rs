pub mod builtins;
pub mod store;

use crate::graph::{AgentNodeConfig, Node, NodeId, NodeKind, NodePosition};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub use builtins::default_templates;
pub use store::{TemplateStore, TemplateStoreError};

/// A reusable, parameterized node definition with default configuration and locked fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Template {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub default_config: AgentNodeConfig,
    /// Valid field names: `system_prompt`, `task_prompt`, `model`, `output_schema`, `auto_start`
    pub locked_fields: HashSet<String>,
}

impl Template {
    #[must_use]
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

    #[must_use]
    pub(crate) fn with_id(
        id: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        default_config: AgentNodeConfig,
        locked_fields: HashSet<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            description: description.into(),
            default_config,
            locked_fields,
        }
    }

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

    #[must_use]
    pub fn is_field_locked(&self, field_name: &str) -> bool {
        self.locked_fields.contains(field_name)
    }

    pub fn lock_field(&mut self, field_name: impl Into<String>) -> bool {
        self.locked_fields.insert(field_name.into())
    }

    pub fn unlock_field(&mut self, field_name: &str) -> bool {
        self.locked_fields.remove(field_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test fixtures use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "template positions are exact layout coordinates"
    )]
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
    fn template_lock_and_unlock_field() {
        let mut template =
            Template::new("Test", "desc", AgentNodeConfig::default(), HashSet::new());

        assert!(!template.is_field_locked("model"));
        assert!(template.lock_field("model"));
        assert!(template.is_field_locked("model"));
        assert!(!template.lock_field("model"));
        assert!(template.unlock_field("model"));
        assert!(!template.is_field_locked("model"));
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
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            provider_id: None,
        };
        let template = Template::new("Test", "desc", config.clone(), HashSet::new());
        let node = template.instantiate(0.0, 0.0);
        assert_eq!(node.agent, config);
    }
}
