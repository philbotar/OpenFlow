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
    Bash,
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
            description: "Read a local file, directory listing, HTTP(S) URL, or spilled tool artifact. Default output is numbered lines capped at 300 lines; append :N-M for a line range (e.g. src/lib.rs:10-20) or :raw for full unnumbered content. Truncated tool output can be read via artifact:{id} (supports the same selectors).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Local path, URL, or artifact:{id}. Append :start-end for a line range or :raw for full content (e.g. note.txt:1-50, artifact:abc-123:1000-1200)."
                    }
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
            description: "Search file contents by regular expression (ripgrep/Rust regex syntax; no backrefs or lookaround). Gitignore-aware by default. Results cap at 500 matches — narrow the pattern or paths if you hit the limit.".to_string(),
            input_schema: serde_json::json!({
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
            description: "Find files and directories matching glob patterns (e.g. **/*.rs, src/**/*.ts). Results cap at 200 paths — narrow the pattern if you hit the limit.".to_string(),
            input_schema: serde_json::json!({
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
            description: "Create or overwrite a file under the execution folder. Prefer edit for existing files; write replaces the whole file.".to_string(),
            input_schema: serde_json::json!({
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
            description: "Edit files under the execution folder. Two modes: (1) replace-mode — path + edits[] where old_text must match exactly and uniquely unless all:true; (2) hashline-mode — input string with ¶path#TAG sections copied from read output.".to_string(),
            input_schema: serde_json::json!({
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
                }
            }),
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
            input_schema: serde_json::json!({
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
            }),
            tier: ToolTier::Exec,
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
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Full patch envelope text (*** Begin Patch … *** End Patch)."
                    }
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
            description: "Search code structurally using ast-grep patterns ($VAR metavariables). Prefer over search when matching syntax trees rather than raw text.".to_string(),
            input_schema: serde_json::json!({
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
    fn definitions_for_subagent_excludes_call_tool() {
        let registry = ToolRegistry::new();
        let config = NodeToolConfig::default();
        let definitions = registry.definitions_for_subagent(&config);
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"openflow_declare_subagents"));
        assert!(!names.contains(&"openflow_call_subagent"));
    }
}
