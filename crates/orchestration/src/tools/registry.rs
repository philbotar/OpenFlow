#![allow(clippy::map_unwrap_or, clippy::missing_errors_doc)]

use std::collections::BTreeMap;
use thiserror::Error;
use workflow_core::{NodeToolConfig, ToolConcurrency, ToolDefinition, ToolTier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinToolKind {
    Read,
    Search,
    Find,
    AstGrep,
}

#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub kind: BuiltinToolKind,
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ToolRegistryError {
    #[error("tool {0} is not registered")]
    Missing(String),
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        let mut tools = BTreeMap::new();
        register(&mut tools, read_tool());
        register(&mut tools, search_tool());
        register(&mut tools, find_tool());
        register(&mut tools, ast_grep_tool());
        Self { tools }
    }

    pub fn get(&self, name: &str) -> Result<&RegisteredTool, ToolRegistryError> {
        self.tools
            .get(name)
            .ok_or_else(|| ToolRegistryError::Missing(name.to_string()))
    }

    #[must_use]
    pub fn definitions_for(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        config
            .catalog
            .tools
            .iter()
            .filter_map(|tool| self.tools.get(&tool.name))
            .map(|tool| tool.definition.clone())
            .collect()
    }
}

fn register(tools: &mut BTreeMap<String, RegisteredTool>, tool: RegisteredTool) {
    tools.insert(tool.definition.name.clone(), tool);
}

fn read_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "read".to_string(),
            description: "Read a local file, directory listing, or URL. Use path selectors like :10-20 or :raw when needed.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
            tier: ToolTier::Read,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::Read,
    }
}

fn search_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "search".to_string(),
            description: "Search files by regular expression across one or more paths.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "pattern": { "type": "string" },
                    "paths": {
                        "oneOf": [
                            { "type": "string" },
                            {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        ]
                    },
                    "i": { "type": ["boolean", "null"] }
                },
                "required": ["pattern", "paths"]
            }),
            tier: ToolTier::Read,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::Search,
    }
}

fn find_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "find".to_string(),
            description: "Find files and directories matching one or more glob patterns."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "paths": {
                        "oneOf": [
                            { "type": "string" },
                            {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        ]
                    }
                },
                "required": ["paths"]
            }),
            tier: ToolTier::Read,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::Find,
    }
}

fn ast_grep_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "ast_grep".to_string(),
            description: "Search code structurally using ast-grep AST patterns.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "pat": { "type": "string" },
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["pat", "paths"]
            }),
            tier: ToolTier::Read,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::AstGrep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_returns_requested_definitions() {
        let registry = ToolRegistry::new();
        let mut config = NodeToolConfig::default();
        config.catalog.tools = vec![workflow_core::ToolRef {
            name: "read".to_string(),
        }];
        let definitions = registry.definitions_for(&config);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "read");
    }
}
