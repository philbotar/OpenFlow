//! Workflow graph: nodes, edges, and per-workflow settings.

#![allow(clippy::use_self, clippy::derive_partial_eq_without_eq)]

use crate::tools::NodeToolConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::ops::Deref;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for NodeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        NodeId(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        NodeId(s.to_string())
    }
}

impl From<NodeId> for String {
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl PartialEq<str> for NodeId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for NodeId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for NodeId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub String);

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for EdgeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for EdgeId {
    fn from(s: String) -> Self {
        EdgeId(s)
    }
}

impl From<&str> for EdgeId {
    fn from(s: &str) -> Self {
        EdgeId(s.to_string())
    }
}

impl From<EdgeId> for String {
    fn from(id: EdgeId) -> Self {
        id.0
    }
}

impl PartialEq<str> for EdgeId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for EdgeId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for EdgeId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for WorkflowId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for WorkflowId {
    fn from(s: String) -> Self {
        WorkflowId(s)
    }
}

impl From<&str> for WorkflowId {
    fn from(s: &str) -> Self {
        WorkflowId(s.to_string())
    }
}

impl From<WorkflowId> for String {
    fn from(id: WorkflowId) -> Self {
        id.0
    }
}

impl PartialEq<str> for WorkflowId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for WorkflowId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for WorkflowId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryPolicy {
    #[serde(default = "default_retry_max_attempts")]
    pub max_attempts: u8,
    #[serde(default = "default_retry_backoff_ms")]
    pub backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: default_retry_max_attempts(),
            backoff_ms: default_retry_backoff_ms(),
        }
    }
}

impl RetryPolicy {
    pub const MAX_BACKOFF_MS: u64 = 30_000;

    /// Exponential backoff for `attempt` (1-based retry counter after increment).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u8) -> Duration {
        let exp = u32::from(attempt.saturating_sub(1)).min(16);
        let delay_ms = self
            .backoff_ms
            .saturating_mul(1 << exp)
            .min(Self::MAX_BACKOFF_MS);
        Duration::from_millis(delay_ms)
    }
}

const fn default_retry_max_attempts() -> u8 {
    3
}

const fn default_retry_backoff_ms() -> u64 {
    1_000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorkflowSchedule {
    pub cron: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub timezone: String,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorkflowSettings {
    #[serde(default)]
    pub shared_context: String,
    #[serde(default)]
    pub schedule: Option<WorkflowSchedule>,
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    #[serde(default)]
    pub provider_id: Option<String>,
}

/// A directed workflow graph with settings applied at run time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub id: WorkflowId,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub settings: WorkflowSettings,
}

impl Workflow {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: WorkflowId(Uuid::new_v4().to_string()),
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            settings: WorkflowSettings::default(),
        }
    }
}

/// One canvas node. Agent nodes carry an [`AgentNodeConfig`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub label: String,
    pub kind: NodeKind,
    pub position: NodePosition,
    pub agent: AgentNodeConfig,
}

impl Node {
    pub fn agent(label: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            id: NodeId(Uuid::new_v4().to_string()),
            label: label.into(),
            kind: NodeKind::Agent,
            position: NodePosition { x, y },
            agent: AgentNodeConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized as `snake_case`; legacy `PascalCase` values remain accepted for saved workflows.
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    #[serde(alias = "Agent")]
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

/// Per-node agent invocation settings: prompts, model, tools, and callable subagents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentNodeConfig {
    pub system_prompt: String,
    pub task_prompt: String,
    #[serde(default)]
    pub model: String,
    pub output_schema: Value,
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,
    #[serde(default)]
    pub tools: NodeToolConfig,
    #[serde(default, rename = "callableAgents")]
    pub callable_agents: Vec<String>,
    #[serde(default, rename = "allowAllCallableAgents")]
    pub allow_all_callable_agents: bool,
    /// Opaque reasoning effort level passed through to the provider (e.g. "none", "adaptive", "low", "medium", "high").
    #[serde(default, rename = "reasoningEffort", alias = "reasoning_effort")]
    pub reasoning_effort: Option<String>,
    /// Optional budget token count for reasoning effort, forwarded to the provider.
    #[serde(
        default,
        rename = "reasoningBudgetTokens",
        alias = "reasoning_budget_tokens"
    )]
    pub reasoning_budget_tokens: Option<u32>,
    /// Optional provider ID override at the node level.
    #[serde(default, rename = "providerId")]
    pub provider_id: Option<String>,
}

const fn default_auto_start() -> bool {
    true
}

impl Default for AgentNodeConfig {
    fn default() -> Self {
        Self {
            system_prompt: "You are a focused AI agent in a node workflow.".to_string(),
            task_prompt: "Return a concise JSON object for this node.".to_string(),
            model: String::new(),
            output_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
            auto_start: true,
            tools: NodeToolConfig::default(),
            callable_agents: Vec::new(),
            allow_all_callable_agents: false,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            provider_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
}

impl Edge {
    pub fn new(from: impl Into<NodeId>, to: impl Into<NodeId>) -> Self {
        Self {
            id: EdgeId(Uuid::new_v4().to_string()),
            from: from.into(),
            to: to.into(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_workflow_starts_empty_with_name() {
        let workflow = Workflow::new("Feature planner");

        assert_eq!(workflow.name, "Feature planner");
        assert!(workflow.nodes.is_empty());
        assert!(workflow.edges.is_empty());
        assert!(!workflow.id.is_empty());
    }

    #[test]
    fn agent_node_defaults_to_structured_summary_output() {
        let node = Node::agent("Plan", 24.0, 48.0);

        assert_eq!(node.label, "Plan");
        assert_eq!(node.kind, NodeKind::Agent);
        assert_eq!(node.position, NodePosition { x: 24.0, y: 48.0 });
        assert_eq!(node.agent.model, "");
        assert_eq!(node.agent.output_schema["required"], json!(["summary"]));
    }

    #[test]
    fn agent_node_defaults_auto_start_true() {
        let node = Node::agent("Plan", 0.0, 0.0);
        assert!(node.agent.auto_start);
    }

    #[test]
    fn agent_node_defaults_tools_enabled() {
        let node = Node::agent("Plan", 0.0, 0.0);
        assert!(node.agent.tools.is_enabled());
        assert_eq!(node.agent.tools.catalog.tools.len(), 4);
    }

    #[test]
    fn agent_node_config_serde_backfills_new_tool_fields() {
        let config: AgentNodeConfig = serde_json::from_value(json!({
            "system_prompt": "sys",
            "task_prompt": "task",
            "model": "gpt-test",
            "output_schema": { "type": "object" }
        }))
        .unwrap();

        assert!(config.auto_start);
        assert_eq!(config.tools, NodeToolConfig::default());
        assert!(config.callable_agents.is_empty());
        assert!(!config.allow_all_callable_agents);
    }

    #[test]
    fn agent_node_config_callable_agents_serde_roundtrip() {
        let config = AgentNodeConfig {
            callable_agents: vec!["agent-1".to_string(), "agent-2".to_string()],
            ..AgentNodeConfig::default()
        };
        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["callableAgents"], json!(["agent-1", "agent-2"]));
        let back: AgentNodeConfig = serde_json::from_value(value).unwrap();
        assert_eq!(back.callable_agents, config.callable_agents);
    }

    #[test]
    fn node_kind_serializes_snake_case_and_accepts_legacy_pascal_case() {
        assert_eq!(
            serde_json::to_value(NodeKind::Agent).unwrap(),
            json!("agent")
        );
        assert_eq!(
            serde_json::from_value::<NodeKind>(json!("Agent")).unwrap(),
            NodeKind::Agent
        );
    }

    #[test]
    fn agent_node_config_serde_roundtrip_with_reasoning_effort() {
        let config = AgentNodeConfig {
            reasoning_effort: Some("adaptive".to_string()),
            reasoning_budget_tokens: Some(40960),
            provider_id: Some("anthropic".to_string()),
            ..AgentNodeConfig::default()
        };
        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["reasoningEffort"], json!("adaptive"));
        assert_eq!(value["reasoningBudgetTokens"], json!(40960));
        assert_eq!(value["providerId"], json!("anthropic"));
        let back: AgentNodeConfig = serde_json::from_value(value).unwrap();
        assert_eq!(back.reasoning_effort, config.reasoning_effort);
        assert_eq!(back.reasoning_budget_tokens, config.reasoning_budget_tokens);
        assert_eq!(back.provider_id, config.provider_id);
    }

    #[test]
    fn agent_node_config_serde_backfills_without_reasoning_effort() {
        let config: AgentNodeConfig = serde_json::from_value(json!({
            "system_prompt": "sys",
            "task_prompt": "task",
            "model": "gpt-test",
            "output_schema": { "type": "object" }
        }))
        .unwrap();
        assert!(config.reasoning_effort.is_none());
        assert!(config.reasoning_budget_tokens.is_none());
        assert!(config.provider_id.is_none());
    }

    #[test]
    fn agent_node_config_default_has_no_reasoning_effort() {
        let config = AgentNodeConfig::default();
        assert!(config.reasoning_effort.is_none());
        assert!(config.reasoning_budget_tokens.is_none());
        assert!(config.provider_id.is_none());
    }

    #[test]
    fn retry_policy_default_matches_serde_defaults() {
        assert_eq!(
            RetryPolicy::default(),
            RetryPolicy {
                max_attempts: 3,
                backoff_ms: 1_000,
            }
        );
        let parsed: RetryPolicy = serde_json::from_value(json!({})).unwrap();
        assert_eq!(parsed, RetryPolicy::default());
    }

    #[test]
    fn retry_policy_delay_for_attempt_exponential_with_cap() {
        let policy = RetryPolicy {
            max_attempts: 3,
            backoff_ms: 1_000,
        };
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(1_000));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(1_000));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(2_000));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(4_000));
        assert_eq!(
            policy.delay_for_attempt(10),
            Duration::from_millis(RetryPolicy::MAX_BACKOFF_MS)
        );
    }
}
