use engine::{NodeToolConfig, ToolConcurrency, ToolDefinition, ToolTier};
use serde_json::Value;
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
    Bash,
    DeclareSubagents,
    CallSubagent,
    Mcp,
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
    #[error("external tool {0} collides with a registered tool")]
    BuiltinCollision(String),
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
        register(&mut tools, bash_tool());
        register(&mut tools, declare_subagents_tool());
        register(&mut tools, call_subagent_tool());
        Self { tools }
    }

    pub fn get(&self, name: &str) -> Result<&RegisteredTool, ToolRegistryError> {
        self.tools
            .get(name)
            .ok_or_else(|| ToolRegistryError::Missing(name.to_string()))
    }

    pub fn extend_mcp(&mut self, tools: Vec<RegisteredTool>) -> Result<(), ToolRegistryError> {
        for tool in tools {
            if self.tools.contains_key(&tool.definition.name) {
                return Err(ToolRegistryError::BuiltinCollision(
                    tool.definition.name.clone(),
                ));
            }
            register(&mut self.tools, tool);
        }
        Ok(())
    }

    #[must_use]
    pub fn definitions_for(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        let mut defs = self.filtered_builtins(config);
        if !Self::is_read_only(config) {
            defs.push(declare_subagents_tool().definition);
            defs.push(call_subagent_tool().definition);
        }
        defs
    }

    /// Tool definitions for subagent contexts — excludes `openflow_call_subagent`
    /// to prevent recursive invocation.
    #[must_use]
    pub fn definitions_for_subagent(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        let mut defs = self.filtered_builtins(config);
        if !Self::is_read_only(config) {
            defs.push(declare_subagents_tool().definition);
        }
        defs
    }

    fn is_read_only(config: &NodeToolConfig) -> bool {
        matches!(
            config.effective_approval_mode(),
            engine::ApprovalMode::ReadOnly
        )
    }

    fn filtered_builtins(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        let read_only = Self::is_read_only(config);
        let mut defs: Vec<ToolDefinition> = self
            .tools
            .values()
            .filter(|tool| {
                let name = tool.definition.name.as_str();
                if matches!(
                    name,
                    "openflow_declare_subagents" | "openflow_call_subagent"
                ) {
                    return false;
                }
                !read_only || tool.definition.tier == ToolTier::Read
            })
            .map(|tool| tool.definition.clone())
            .collect();
        defs.sort_by(|left, right| left.name.cmp(&right.name));
        defs
    }
}

fn register(tools: &mut BTreeMap<String, RegisteredTool>, tool: RegisteredTool) {
    tools.insert(tool.definition.name.clone(), tool);
}

fn with_intent_field(mut schema: Value) -> Value {
    if let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    {
        properties.insert(
            "_i".to_string(),
            serde_json::json!({
                "type": ["string", "null"],
                "description": "Optional human-readable intent for this tool call. Keep it short; it is shown in the UI and ignored by the tool implementation."
            }),
        );
    }
    schema
}

fn read_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "read".to_string(),
            description: "Read a local file, directory listing, HTTP(S) URL, or spilled tool artifact. Default output is numbered lines capped at 3000 lines; append :N-M for a line range (e.g. src/lib.rs:10-20) or :raw for full unnumbered content. Truncated tool output can be read via artifact:{id} (supports the same selectors).".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Local path, URL, or artifact:{id}. Append :start-end for a line range or :raw for full content (e.g. note.txt:1-50, artifact:abc-123:1000-1200)."
                    }
                },
                "required": ["path"]
            })),
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
            description: "Search file contents by regular expression (ripgrep/Rust regex syntax; no backrefs or lookaround). Gitignore-aware by default. Results cap at 500 matches — narrow the pattern or paths if you hit the limit.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Ripgrep/Rust regex pattern to match in file contents."
                    },
                    "paths": {
                        "description": "File, directory, or glob to search (string or array of strings).",
                        "oneOf": [
                            { "type": "string" },
                            {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        ]
                    },
                    "i": {
                        "type": ["boolean", "null"],
                        "description": "Case-insensitive matching when true."
                    },
                    "gitignore": {
                        "type": ["boolean", "null"],
                        "description": "Respect .gitignore rules when true (default true)."
                    }
                },
                "required": ["pattern", "paths"]
            })),
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
            description: "Find files and directories matching glob patterns (e.g. **/*.rs, src/**/*.ts). Gitignore-aware by default. Results cap at 200 paths — narrow the pattern if you hit the limit.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "paths": {
                        "description": "Glob pattern or array of patterns relative to the execution folder (e.g. **/*.rs).",
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
            })),
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
            description: "Create or overwrite a file under the execution folder. Prefer edit for existing files; write replaces the whole file.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path under the execution folder."
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file content to write."
                    }
                },
                "required": ["path", "content"]
            })),
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
            description: "Edit files under the execution folder. Two modes: (1) replace-mode — path + edits[] where old_text must match exactly and uniquely unless all:true; (2) hashline-mode — input string with ¶path#TAG sections copied from read output.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Replace-mode only: relative path to the file to edit."
                    },
                    "edits": {
                        "type": "array",
                        "description": "Replace-mode only: one or more search/replace operations.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "old_text": {
                                    "type": "string",
                                    "description": "Exact text to find in the file."
                                },
                                "new_text": {
                                    "type": "string",
                                    "description": "Replacement text."
                                },
                                "all": {
                                    "type": "boolean",
                                    "description": "Replace every match when true; default replaces only a unique match."
                                }
                            },
                            "required": ["old_text", "new_text"]
                        }
                    },
                    "input": {
                        "type": "string",
                        "description": "Hashline-mode only: patch text with ¶path#TAG sections from read output."
                    }
                },
                "required": []
            })),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::Edit,
    }
}

fn bash_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command in the execution folder. Use cwd for the working directory (not cd dir && …). Prefer dedicated read/search/find/edit/write tools when they suffice. Output over 50KB is truncated to an artifact (read it via artifact:{id}). Returns merged stdout/stderr, wall time, and exit code.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default 300, max 3600)"
                    },
                    "env": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Extra environment variables"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory under the execution folder"
                    }
                },
                "required": ["command"]
            })),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::Bash,
    }
}

fn apply_patch_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "apply_patch".to_string(),
            description: "Apply a Codex-style *** Begin Patch / *** End Patch envelope to files under the execution folder. Usually prefer edit for targeted changes.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Full patch envelope text (*** Begin Patch … *** End Patch)."
                    }
                },
                "required": ["input"]
            })),
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
            description: "Search code structurally using ast-grep patterns ($VAR metavariables). Prefer over search when matching syntax trees rather than raw text.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "pat": {
                        "type": "string",
                        "description": "ast-grep pattern (use $VAR for metavariables)."
                    },
                    "paths": {
                        "type": "array",
                        "description": "Files or directories to scan.",
                        "items": { "type": "string" }
                    }
                },
                "required": ["pat", "paths"]
            })),
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
    fn registry_returns_all_builtins_plus_subagent_tools_by_default() {
        let registry = ToolRegistry::new();
        let config = NodeToolConfig::default();
        let definitions = registry.definitions_for(&config);
        assert_eq!(definitions.len(), 10);
        assert!(definitions
            .iter()
            .any(|tool| tool.name == "openflow_declare_subagents"));
        assert!(definitions
            .iter()
            .any(|tool| tool.name == "openflow_call_subagent"));
    }

    #[test]
    fn read_only_mode_exposes_read_tools_only() {
        let registry = ToolRegistry::new();
        let config = NodeToolConfig {
            approval_mode: Some(engine::ApprovalMode::ReadOnly),
        };
        let definitions = registry.definitions_for(&config);
        assert_eq!(definitions.len(), 4);
        assert!(definitions
            .iter()
            .all(|tool| tool.tier == engine::ToolTier::Read));
    }

    #[test]
    fn definitions_always_includes_subagent_tools_in_write_mode() {
        let registry = ToolRegistry::new();
        let config = NodeToolConfig::default();
        let definitions = registry.definitions_for(&config);
        assert!(definitions
            .iter()
            .any(|tool| tool.name == "openflow_declare_subagents"));
        assert!(definitions
            .iter()
            .any(|tool| tool.name == "openflow_call_subagent"));
    }

    #[test]
    fn every_builtin_schema_accepts_optional_intent_field() {
        let registry = ToolRegistry::new();
        let builtins = [
            "read",
            "search",
            "find",
            "ast_grep",
            "write",
            "edit",
            "apply_patch",
            "bash",
        ];
        for name in builtins {
            let tool = registry.get(name).expect("builtin tool");
            let properties = tool
                .definition
                .input_schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
                .expect("tool schema has properties");
            assert!(properties.contains_key("_i"), "missing _i on {name}");
            let required = tool
                .definition
                .input_schema
                .get("required")
                .and_then(serde_json::Value::as_array)
                .expect("tool schema has required");
            assert!(
                !required.iter().any(|value| value.as_str() == Some("_i")),
                "_i must remain optional on {name}"
            );
        }
    }

    #[test]
    fn every_schema_property_has_description() {
        let registry = ToolRegistry::new();
        let builtins = [
            "read",
            "search",
            "find",
            "ast_grep",
            "write",
            "edit",
            "apply_patch",
            "bash",
        ];
        for name in builtins {
            let tool = registry.get(name).expect("builtin tool");
            assert_schema_properties_have_descriptions(
                &tool.definition.input_schema,
                &tool.definition.name,
            );
        }
    }

    fn assert_schema_properties_have_descriptions(schema: &serde_json::Value, path: &str) {
        let Some(properties) = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
        else {
            if let Some(items) = schema.get("items") {
                assert_schema_properties_have_descriptions(items, &format!("{path}.items"));
            }
            return;
        };
        for (key, property) in properties {
            let property_path = format!("{path}.{key}");
            assert!(
                property.get("description").is_some(),
                "missing description at {property_path}"
            );
            if property.get("properties").is_some() || property.get("items").is_some() {
                assert_schema_properties_have_descriptions(property, &property_path);
            }
        }
    }

    #[test]
    fn registry_extends_mcp_without_shadowing_builtins() {
        let mut registry = ToolRegistry::new();
        registry
            .extend_mcp(vec![RegisteredTool {
                definition: ToolDefinition {
                    name: "mcp/gh/search".into(),
                    description: "Search GitHub".into(),
                    input_schema: serde_json::json!({"type":"object","properties":{}}),
                    tier: ToolTier::Write,
                    concurrency: ToolConcurrency::Shared,
                },
                kind: BuiltinToolKind::Mcp,
            }])
            .unwrap();
        assert!(registry.get("read").is_ok());
        assert!(registry.get("mcp/gh/search").is_ok());
        assert!(registry.get("search").is_ok());
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
