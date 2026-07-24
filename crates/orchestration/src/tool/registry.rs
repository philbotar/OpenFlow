use engine::{
    NodeToolConfig, ToolAccessPolicy, ToolConcurrency, ToolDefinition, ToolTier,
    WRITE_PLAN_ARTIFACT_TOOL,
};
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
    WebSearch,
    WritePlanArtifact,
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
        register(&mut tools, write_plan_artifact_tool());
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

    /// Register the web_search builtin. Not part of `new()` — callers opt in
    /// when search is enabled, keys are configured, and the binary resolves.
    pub fn register_web_search(&mut self) {
        register(&mut self.tools, web_search_tool());
    }

    #[must_use]
    pub fn definitions_for(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        self.definitions_for_policy(config, ToolAccessPolicy::Execution)
    }

    #[must_use]
    pub fn definitions_for_policy(
        &self,
        config: &NodeToolConfig,
        policy: ToolAccessPolicy,
    ) -> Vec<ToolDefinition> {
        self.definitions_for_policy_and_plan_scope(config, policy, true)
    }

    #[must_use]
    pub fn definitions_for_policy_and_plan_scope(
        &self,
        config: &NodeToolConfig,
        policy: ToolAccessPolicy,
        can_manage_plan: bool,
    ) -> Vec<ToolDefinition> {
        let mut defs = self.filtered_builtins(config, policy, can_manage_plan);
        if matches!(policy, ToolAccessPolicy::Execution) && !Self::is_read_only(config) {
            defs.push(declare_subagents_tool().definition);
            defs.push(call_subagent_tool().definition);
        }
        defs
    }

    /// Tool definitions for subagent contexts — excludes `openflow_call_subagent`
    /// to prevent recursive invocation.
    #[must_use]
    pub fn definitions_for_subagent(&self, config: &NodeToolConfig) -> Vec<ToolDefinition> {
        let mut defs = self.filtered_builtins(config, ToolAccessPolicy::Execution, false);
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

    fn filtered_builtins(
        &self,
        config: &NodeToolConfig,
        policy: ToolAccessPolicy,
        can_manage_plan: bool,
    ) -> Vec<ToolDefinition> {
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
                if tool.definition.name == WRITE_PLAN_ARTIFACT_TOOL
                    && (!matches!(policy, ToolAccessPolicy::Planning) || !can_manage_plan)
                {
                    return false;
                }
                if read_only
                    && tool.definition.tier != ToolTier::Read
                    && !(matches!(policy, ToolAccessPolicy::Planning)
                        && can_manage_plan
                        && (tool.definition.name == WRITE_PLAN_ARTIFACT_TOOL
                            || matches!(tool.definition.name.as_str(), "write" | "edit")))
                {
                    return false;
                }
                match policy {
                    ToolAccessPolicy::Execution => true,
                    ToolAccessPolicy::Planning => {
                        tool.kind != BuiltinToolKind::Mcp
                            && (tool.definition.tier == ToolTier::Read
                                || (can_manage_plan
                                    && tool.definition.name == WRITE_PLAN_ARTIFACT_TOOL)
                                || (!read_only
                                    && matches!(tool.definition.name.as_str(), "write" | "edit"))
                                || (can_manage_plan
                                    && matches!(tool.definition.name.as_str(), "write" | "edit")))
                    }
                }
            })
            .map(|tool| {
                let mut definition = tool.definition.clone();
                if matches!(policy, ToolAccessPolicy::Planning)
                    && matches!(definition.name.as_str(), "write" | "edit")
                {
                    if can_manage_plan && read_only {
                        definition.description.push_str(&format!(
                            " During Plan Mode planning, use replace mode only on the run-local \
                             plan draft at {}; repository files remain read-only.",
                            engine::PLAN_DRAFT_PATH
                        ));
                    } else if can_manage_plan {
                        definition.description.push_str(&format!(
                            " During Plan Mode planning, use {} for the run-local plan draft; \
                             repository writes remain limited to docs/**/*.md.",
                            engine::PLAN_DRAFT_PATH
                        ));
                    } else {
                        definition.description.push_str(
                            " During Plan Mode planning, this node does not own the run-local plan \
                             draft; repository writes remain limited to docs/**/*.md.",
                        );
                    }
                }
                definition
            })
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
                        "description": "Repository-relative local path, URL, or artifact:{id}. Prefer relative paths (e.g. src/lib.rs). Append :start-end for a line range or :raw for full content (e.g. note.txt:1-50, artifact:abc-123:1000-1200)."
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
                        "description": "Repository-relative file, directory, or glob to search (string or array). Prefer relative paths.",
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
            description: "Create or overwrite a file under the execution folder. Requires both path and content — never path only. Prefer edit for existing files; write replaces the whole file. For large docs, write a small stub first, then edit in ~40-line chunks.".to_string(),
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

fn write_plan_artifact_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: WRITE_PLAN_ARTIFACT_TOOL.to_string(),
            description: format!(
                "Seal the completed run-local plan draft at {} as this run's one immutable \
                 Markdown plan artifact. Create the draft with write, update it incrementally \
                 with replace-mode edit, then call this tool without copying plan text into the \
                 arguments. This call always pauses for explicit human approval; denial leaves \
                 the draft mutable. The host returns the artifact id and hash after approval.",
                engine::PLAN_DRAFT_PATH
            ),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {},
                "required": []
            })),
            tier: ToolTier::Write,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::WritePlanArtifact,
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
            concurrency: ToolConcurrency::NodeExclusive,
        },
        kind: BuiltinToolKind::Bash,
    }
}

fn web_search_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "web_search".to_string(),
            description: "Search the web via the local search-cli aggregator. Fans the query out to configured search providers in parallel and returns rank-fused results as JSON. Distinct from `search`, which greps local file contents.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The web search query."
                    },
                    "mode": {
                        "type": ["string", "null"],
                        "description": "Search mode: general (default), news, academic, scholar, deep, people, social, patents, images, places, extract, similar."
                    },
                    "count": {
                        "type": ["integer", "null"],
                        "description": "Number of results to return (default 10)."
                    }
                },
                "required": ["query"]
            })),
            tier: ToolTier::Read,
            concurrency: ToolConcurrency::Shared,
        },
        kind: BuiltinToolKind::WebSearch,
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
    fn bash_is_node_exclusive_and_file_mutators_stay_run_exclusive() {
        let registry = ToolRegistry::new();
        let concurrency = |name: &str| registry.get(name).expect(name).definition.concurrency;
        assert_eq!(concurrency("bash"), engine::ToolConcurrency::NodeExclusive);
        assert_eq!(concurrency("edit"), engine::ToolConcurrency::Exclusive);
        assert_eq!(concurrency("write"), engine::ToolConcurrency::Exclusive);
        assert_eq!(
            concurrency("apply_patch"),
            engine::ToolConcurrency::Exclusive
        );
    }

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
        assert!(!definitions
            .iter()
            .any(|tool| tool.name == WRITE_PLAN_ARTIFACT_TOOL));
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
    fn planning_policy_gives_read_only_nodes_only_run_plan_draft_writers() {
        let registry = ToolRegistry::new();
        let config = NodeToolConfig {
            approval_mode: Some(engine::ApprovalMode::ReadOnly),
        };
        let definitions = registry.definitions_for_policy(&config, ToolAccessPolicy::Planning);
        assert!(definitions
            .iter()
            .any(|tool| tool.name == WRITE_PLAN_ARTIFACT_TOOL));
        let write = definitions
            .iter()
            .find(|tool| tool.name == "write")
            .expect("plan draft write");
        let edit = definitions
            .iter()
            .find(|tool| tool.name == "edit")
            .expect("plan draft edit");
        assert!(write.description.contains(engine::PLAN_DRAFT_PATH));
        assert!(edit.description.contains(engine::PLAN_DRAFT_PATH));
        assert!(!write.description.contains("docs/**/*.md"));
        assert!(definitions.iter().any(|tool| tool.name == "read"));

        let seal = definitions
            .iter()
            .find(|tool| tool.name == WRITE_PLAN_ARTIFACT_TOOL)
            .expect("plan seal");
        assert!(seal.description.contains(engine::PLAN_DRAFT_PATH));
        assert!(seal.input_schema["properties"].get("markdown").is_none());
    }

    #[test]
    fn planning_policy_exposes_read_tools_plan_artifact_and_docs_writers() {
        let registry = ToolRegistry::new();
        let definitions =
            registry.definitions_for_policy(&NodeToolConfig::default(), ToolAccessPolicy::Planning);
        assert!(definitions
            .iter()
            .any(|tool| tool.name == WRITE_PLAN_ARTIFACT_TOOL));
        assert!(definitions.iter().any(|tool| tool.name == "write"));
        assert!(definitions.iter().any(|tool| tool.name == "edit"));
        assert!(!definitions.iter().any(|tool| tool.name == "bash"));
        assert!(!definitions
            .iter()
            .any(|tool| tool.name == "openflow_call_subagent"));
        assert!(!definitions.iter().any(|tool| tool.name.starts_with("mcp/")));
        let write = definitions
            .iter()
            .find(|tool| tool.name == "write")
            .expect("write");
        assert!(write.description.contains("docs/**/*.md"));
        assert!(write.description.contains("small stub first"));
    }

    #[test]
    fn planning_policy_hides_plan_capabilities_from_non_source_nodes() {
        let registry = ToolRegistry::new();
        let definitions = registry.definitions_for_policy_and_plan_scope(
            &NodeToolConfig::default(),
            ToolAccessPolicy::Planning,
            false,
        );

        assert!(!definitions
            .iter()
            .any(|tool| tool.name == WRITE_PLAN_ARTIFACT_TOOL));
        let write = definitions
            .iter()
            .find(|tool| tool.name == "write")
            .expect("docs writer");
        assert!(write.description.contains("does not own"));
        assert!(!write.description.contains(engine::PLAN_DRAFT_PATH));

        let read_only = NodeToolConfig {
            approval_mode: Some(engine::ApprovalMode::ReadOnly),
        };
        let read_only_definitions = registry.definitions_for_policy_and_plan_scope(
            &read_only,
            ToolAccessPolicy::Planning,
            false,
        );
        assert!(!read_only_definitions
            .iter()
            .any(|tool| matches!(tool.name.as_str(), "write" | "edit")));
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
    fn web_search_absent_by_default_and_present_after_registration() {
        let mut registry = ToolRegistry::new();
        assert!(registry.get("web_search").is_err());

        registry.register_web_search();
        let tool = registry.get("web_search").expect("web_search registered");
        assert_eq!(tool.definition.tier, ToolTier::Read);

        let config = NodeToolConfig {
            approval_mode: Some(engine::ApprovalMode::ReadOnly),
        };
        assert!(registry
            .definitions_for(&config)
            .iter()
            .any(|tool| tool.name == "web_search"));

        let properties = tool
            .definition
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("schema properties");
        assert!(properties.contains_key("_i"));
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
            WRITE_PLAN_ARTIFACT_TOOL,
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
            WRITE_PLAN_ARTIFACT_TOOL,
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
