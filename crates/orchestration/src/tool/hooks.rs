use async_trait::async_trait;
use engine::{ToolCall, ToolResult};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeforeToolDecision {
    Continue,
    Block { reason: String },
}

#[derive(Debug, Clone)]
pub struct BeforeToolContext {
    pub node_id: engine::NodeId,
    pub conversation_id: String,
    pub call: ToolCall,
}

#[derive(Debug, Clone)]
pub struct AfterToolContext {
    pub node_id: engine::NodeId,
    pub conversation_id: String,
    pub call: ToolCall,
    pub result: ToolResult,
}

#[async_trait]
pub trait ToolHook: Send + Sync {
    async fn before_tool_call(&self, _ctx: BeforeToolContext) -> BeforeToolDecision {
        BeforeToolDecision::Continue
    }

    async fn after_tool_call(&self, _ctx: AfterToolContext) {}
}

#[derive(Default, Clone)]
pub struct ToolHooks {
    hooks: Arc<Vec<Arc<dyn ToolHook>>>,
}

impl std::fmt::Debug for ToolHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolHooks")
            .field("hook_count", &self.hooks.len())
            .finish()
    }
}

impl ToolHooks {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(hooks: Vec<Arc<dyn ToolHook>>) -> Self {
        Self {
            hooks: Arc::new(hooks),
        }
    }

    pub async fn before_tool_call(&self, ctx: BeforeToolContext) -> BeforeToolDecision {
        for hook in self.hooks.iter() {
            match hook.before_tool_call(ctx.clone()).await {
                BeforeToolDecision::Continue => {}
                blocked @ BeforeToolDecision::Block { .. } => return blocked,
            }
        }
        BeforeToolDecision::Continue
    }

    pub async fn after_tool_call(&self, ctx: AfterToolContext) {
        for hook in self.hooks.iter() {
            hook.after_tool_call(ctx.clone()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_hooks_continue() {
        let hooks = ToolHooks::empty();
        let decision = hooks
            .before_tool_call(BeforeToolContext {
                node_id: engine::NodeId("node-a".to_string()),
                conversation_id: "node-a".to_string(),
                call: ToolCall {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path": "README.md"}),
                },
            })
            .await;
        assert_eq!(decision, BeforeToolDecision::Continue);
    }
}
