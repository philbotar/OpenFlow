use crate::settings::model::LspSettings;
use crate::tool::blocking_ops::{split_selector, BlockingToolOps};
use crate::tool::errors::ToolError;
use crate::tool::registry::{BuiltinToolKind, ToolRegistry};
use engine::{ToolCall, ToolDefinition, ToolResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const PROJECT_READ_TOOL_NAMES: [&str; 3] = ["read", "search", "find"];

pub(crate) struct ProjectReadTools {
    cwd: PathBuf,
    snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
}

impl ProjectReadTools {
    pub(crate) fn new(cwd: &str) -> Result<Self, ToolError> {
        let cwd = Path::new(cwd)
            .canonicalize()
            .map_err(|error| ToolError::NotFound {
                what: format!("project execution folder {cwd}: {error}"),
                hint: "update the project's execution folder".to_string(),
            })?;
        if !cwd.is_dir() {
            return Err(ToolError::NotFound {
                what: format!("project execution folder {}", cwd.display()),
                hint: "choose an existing directory".to_string(),
            });
        }
        Ok(Self {
            cwd,
            snapshots: Arc::new(
                crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new(),
            ),
        })
    }

    pub(crate) fn definitions() -> Vec<ToolDefinition> {
        let registry = ToolRegistry::new();
        PROJECT_READ_TOOL_NAMES
            .iter()
            .filter_map(|name| registry.get(name).ok().map(|tool| tool.definition.clone()))
            .collect()
    }

    pub(crate) fn handles(&self, name: &str) -> bool {
        PROJECT_READ_TOOL_NAMES.contains(&name)
    }

    pub(crate) async fn execute(&self, call: &ToolCall) -> ToolResult {
        let cwd = self.cwd.clone();
        let snapshots = Arc::clone(&self.snapshots);
        let name = call.name.clone();
        let arguments = call.arguments.clone();
        let output = tokio::task::spawn_blocking(move || {
            execute_project_read(cwd, snapshots, &name, arguments)
        })
        .await
        .map_err(|error| ToolError::failed(format!("project read task failed: {error}")))
        .and_then(|result| result);

        match output {
            Ok(content) => ToolResult {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                content,
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
            Err(error) => ToolResult {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                content: json!({ "error": error.to_string() }).to_string(),
                is_error: true,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        }
    }
}

fn execute_project_read(
    cwd: PathBuf,
    snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    name: &str,
    arguments: Value,
) -> Result<String, ToolError> {
    match name {
        "read" => {
            #[derive(Deserialize)]
            struct ReadArgs {
                path: String,
            }

            let args: ReadArgs =
                serde_json::from_value(arguments).map_err(|error| ToolError::InvalidArgs {
                    tool: "read".to_string(),
                    problem: error.to_string(),
                    hint: "required field: path (project-relative string)".to_string(),
                })?;
            let (path, _) = split_selector(&args.path);
            validate_project_relative_path("read", &path)?;
            BlockingToolOps::read_local_at(cwd, snapshots, &args.path)
        }
        "search" | "find" => {
            validate_project_paths(name, &arguments)?;
            let kind = if name == "search" {
                BuiltinToolKind::Search
            } else {
                BuiltinToolKind::Find
            };
            let output = BlockingToolOps::run_blocking(
                cwd.clone(),
                snapshots,
                LspSettings::default(),
                kind,
                arguments,
                None,
            )
            .output?;
            if name == "find" {
                return Ok(output
                    .lines()
                    .map(|path| {
                        Path::new(path)
                            .strip_prefix(&cwd)
                            .unwrap_or_else(|_| Path::new(path))
                            .display()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("\n"));
            }
            Ok(output)
        }
        _ => Err(ToolError::InvalidArgs {
            tool: name.to_string(),
            problem: "tool is not available during project workflow authoring".to_string(),
            hint: "use read, search, or find".to_string(),
        }),
    }
}

fn validate_project_paths(tool: &str, arguments: &Value) -> Result<(), ToolError> {
    let paths = arguments
        .get("paths")
        .ok_or_else(|| ToolError::InvalidArgs {
            tool: tool.to_string(),
            problem: "missing paths".to_string(),
            hint: "provide a project-relative path or array of paths".to_string(),
        })?;
    match paths {
        Value::String(path) => validate_project_relative_path(tool, path),
        Value::Array(paths) => paths.iter().try_for_each(|path| {
            path.as_str().map_or_else(
                || {
                    Err(ToolError::InvalidArgs {
                        tool: tool.to_string(),
                        problem: "paths must contain strings".to_string(),
                        hint: "provide project-relative path strings".to_string(),
                    })
                },
                |path| validate_project_relative_path(tool, path),
            )
        }),
        _ => Err(ToolError::InvalidArgs {
            tool: tool.to_string(),
            problem: "paths must be a string or array of strings".to_string(),
            hint: "provide project-relative path strings".to_string(),
        }),
    }
}

fn validate_project_relative_path(tool: &str, path: &str) -> Result<(), ToolError> {
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(ToolError::PermissionDenied {
            what: format!("{tool} path must stay inside the project execution folder: {path}"),
            hint: "use a project-relative path without '..'".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_project_execution_folder() {
        let error = validate_project_relative_path("read", "../secrets.txt")
            .expect_err("parent traversal should fail");
        assert!(matches!(error, ToolError::PermissionDenied { .. }));
    }

    #[tokio::test]
    async fn find_returns_paths_relative_to_project_execution_folder() {
        let project_dir = tempfile::tempdir().expect("project tempdir");
        std::fs::write(project_dir.path().join("PROJECT.md"), "# Project\n")
            .expect("seed project file");
        let tools =
            ProjectReadTools::new(&project_dir.path().display().to_string()).expect("read tools");
        let result = tools
            .execute(&ToolCall {
                id: "find-project-files".to_string(),
                provider_call_id: None,
                name: "find".to_string(),
                arguments: json!({ "paths": "*.md" }),
            })
            .await;

        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "PROJECT.md");
    }
}
