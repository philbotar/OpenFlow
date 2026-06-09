use engine::{NodeToolConfig, ToolConcurrency, ToolDefinition, ToolTier};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinToolKind {
    Read,
    Search,
    Find,
    AstGrep,
    Write,
    Edit,
    ApplyPatch,
    DeclareSubagents,
    CallSubagent,
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
        register(&mut tools, write_tool());
        register(&mut tools, edit_tool());
        register(&mut tools, apply_patch_tool());
        register(&mut tools, declare_subagents_tool());
        register(&mut tools, call_subagent_tool());
        Self { tools }
    }

    pub fn get(&self, name: &str) -> Result<&RegisteredTool, ToolRegistryError> {
        self.tools
            .get(name)
            .ok_or_else(|| ToolRegistryError::Missing(name.to_string()))
    }

    #[must_use]
    pub fn definitions_for(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = config
            .catalog
            .tools
            .iter()
            .filter_map(|tool| self.tools.get(&tool.name))
            .map(|tool| tool.definition.clone())
            .collect();
        // Always include the runtime subagent tools for parent agents
        defs.push(declare_subagents_tool().definition);
        defs.push(call_subagent_tool().definition);
        defs
    }

    /// Tool definitions for subagent contexts — excludes `openflow_call_subagent`
    /// to prevent recursive invocation.
    #[must_use]
    pub fn definitions_for_subagent(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = config
            .catalog
            .tools
            .iter()
            .filter_map(|tool| self.tools.get(&tool.name))
            .map(|tool| tool.definition.clone())
            .collect();
        // Subagents can declare their own sub-subagents but cannot invoke them
        defs.push(declare_subagents_tool().definition);
        defs
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

fn write_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "write".to_string(),
            description: "Create or overwrite a file under the execution folder.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::Write,
    }
}

fn edit_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "edit".to_string(),
            description:
                "Edit files under the execution folder: replace-mode (`path` + `edits`) or hashline-mode (`input` with `¶path#TAG` sections)."
                    .to_string(),
            input_schema: serde_json::json!({
                "oneOf": [
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "path": { "type": "string" },
                            "edits": {
                                "type": "array",
                                "minItems": 1,
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "old_text": { "type": "string" },
                                        "new_text": { "type": "string" },
                                        "all": { "type": "boolean" }
                                    },
                                    "required": ["old_text", "new_text"]
                                }
                            }
                        },
                        "required": ["path", "edits"]
                    },
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "input": { "type": "string" }
                        },
                        "required": ["input"]
                    }
                ]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::Edit,
    }
}

fn apply_patch_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "apply_patch".to_string(),
            description:
                "Apply a Codex *** Begin Patch envelope to files under the execution folder."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::ApplyPatch,
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

pub fn declare_subagents_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "openflow_declare_subagents".to_string(),
            description: "Declare subagents that the current agent node wants available during this workflow run. Each subagent has a name and purpose. Declarations are recorded in the run state and displayed on the node.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "subagents": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "name": { "type": "string" },
                                "purpose": { "type": "string" }
                            },
                            "required": ["name", "purpose"]
                        }
                    }
                },
                "required": ["subagents"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::DeclareSubagents,
    }
}

pub fn call_subagent_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "openflow_call_subagent".to_string(),
            description: "Invoke a previously declared subagent by ID. The subagent receives your input and upstream outputs, performs its task using available tools, and returns its output.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "subagent_id": {
                        "type": "string",
                        "description": "The ID of the declared subagent to invoke, as returned by openflow_declare_subagents."
                    },
                    "input": {
                        "type": "string",
                        "description": "The task instruction to send to the subagent."
                    }
                },
                "required": ["subagent_id", "input"]
            }),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::CallSubagent,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_returns_requested_definitions() {
        let registry = ToolRegistry::new();
        let mut config = NodeToolConfig::default();
        config.catalog.tools = vec![engine::ToolRef {
            name: "read".to_string(),
            tier: Some(engine::ToolTier::Read),
        }];
        let definitions = registry.definitions_for(&config);
        // read + openflow_declare_subagents + openflow_call_subagent
        assert_eq!(definitions.len(), 3);
        assert_eq!(definitions[0].name, "read");
        assert_eq!(definitions[1].name, "openflow_declare_subagents");
        assert_eq!(definitions[2].name, "openflow_call_subagent");
    }

    #[test]
    fn definitions_always_includes_subagent_tools() {
        let registry = ToolRegistry::new();
        let empty_config_no_tools = NodeToolConfig {
            catalog: engine::ToolCatalogSelection { tools: vec![] },
            ..Default::default()
        };
        let definitions = registry.definitions_for(&empty_config_no_tools);
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].name, "openflow_declare_subagents");
        assert_eq!(definitions[1].name, "openflow_call_subagent");
    }

    #[test]
    fn definitions_for_subagent_excludes_call_tool() {
        let registry = ToolRegistry::new();
        let config = NodeToolConfig::default();
        let definitions = registry.definitions_for_subagent(&config);
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"openflow_declare_subagents"));
        assert!(!names.contains(&"openflow_call_subagent"));
    }
}
