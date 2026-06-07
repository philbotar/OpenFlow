#![allow(
    clippy::manual_let_else,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::redundant_closure_for_method_calls,
    clippy::uninlined_format_args,
    clippy::unused_self
)]

use crate::tools::errors::ToolError;
use crate::tools::output::{ArtifactStore, ToolArtifactRecord};
use crate::tools::registry::{BuiltinToolKind, ToolRegistry, ToolRegistryError};
use domain::{ToolCall, ToolResult};
use regex::{Regex, RegexBuilder};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub result: ToolResult,
    pub artifact: Option<ToolArtifactRecord>,
}

#[derive(Debug)]
pub struct ToolRunner {
    registry: ToolRegistry,
    http: Client,
    cwd: PathBuf,
    artifacts: ArtifactStore,
}

#[derive(Debug, Error)]
pub enum ToolRunnerError {
    #[error(transparent)]
    Registry(#[from] ToolRegistryError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error("{0}")]
    InvalidArguments(String),
}

impl ToolRunner {
    pub fn new(registry: ToolRegistry, cwd: PathBuf, artifacts: ArtifactStore) -> Self {
        Self {
            registry,
            http: Client::new(),
            cwd,
            artifacts,
        }
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn artifacts(&self) -> &ArtifactStore {
        &self.artifacts
    }

    pub async fn execute(&self, call: ToolCall) -> Result<ToolExecutionRecord, ToolRunnerError> {
        let registered = self.registry.get(&call.name)?;
        let raw_output = match registered.kind {
            BuiltinToolKind::Read => self.read(call.arguments.clone()).await?,
            BuiltinToolKind::Search => self.search(call.arguments.clone())?,
            BuiltinToolKind::Find => self.find(call.arguments.clone())?,
            BuiltinToolKind::AstGrep => self.ast_grep(call.arguments.clone())?,
            BuiltinToolKind::DeclareSubagents | BuiltinToolKind::CallSubagent => {
                return Err(ToolRunnerError::InvalidArguments(format!(
                    "Tool '{}' is a runtime builtin and should not reach the filesystem runner",
                    call.name
                )));
            }
        };

        let (content, artifact, output_meta) = self.artifacts.store_text(&call.name, raw_output)?;
        Ok(ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content,
                is_error: false,
                artifact_ids: artifact
                    .as_ref()
                    .map(|record| vec![record.artifact_id.clone()])
                    .unwrap_or_default(),
                output_meta,
            },
            artifact,
        })
    }

    pub fn denied(&self, call: ToolCall, reason: impl Into<String>) -> ToolExecutionRecord {
        ToolExecutionRecord {
            result: ToolResult {
                tool_call_id: call.id,
                tool_name: call.name,
                content: reason.into(),
                is_error: true,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
            artifact: None,
        }
    }

    async fn read(&self, args: Value) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        struct ReadArgs {
            path: String,
        }
        let args: ReadArgs = serde_json::from_value(args)
            .map_err(|error| ToolError::Failed(format!("invalid read args: {error}")))?;
        if args.path.starts_with("http://") || args.path.starts_with("https://") {
            return self.read_url(&args.path).await;
        }

        let (path, selector) = split_selector(&args.path);
        let absolute = self.resolve_local(&path);
        let metadata = fs::metadata(&absolute)
            .map_err(|error| ToolError::Failed(format!("read failed for {}: {error}", path)))?;
        if metadata.is_dir() {
            return self.read_directory(&absolute);
        }
        let text = fs::read_to_string(&absolute)
            .map_err(|error| ToolError::Failed(format!("read failed for {}: {error}", path)))?;
        Ok(apply_read_selector(&path, &text, selector.as_deref()))
    }

    async fn read_url(&self, url: &str) -> Result<String, ToolError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|error| ToolError::Failed(format!("read failed for {url}: {error}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| ToolError::Failed(format!("read failed for {url}: {error}")))?;
        if !status.is_success() {
            return Err(ToolError::Failed(format!(
                "read failed for {url}: HTTP {status}"
            )));
        }
        Ok(apply_read_selector(url, &text, None))
    }

    fn read_directory(&self, path: &Path) -> Result<String, ToolError> {
        let mut entries = fs::read_dir(path)
            .map_err(|error| {
                ToolError::Failed(format!("read failed for {}: {error}", path.display()))
            })?
            .filter_map(Result::ok)
            .map(|entry| {
                let file_type = entry.file_type().ok();
                let mut name = entry.file_name().to_string_lossy().to_string();
                if file_type.as_ref().is_some_and(|kind| kind.is_dir()) {
                    name.push('/');
                }
                name
            })
            .collect::<Vec<_>>();
        entries.sort();
        Ok(entries.into_iter().take(200).collect::<Vec<_>>().join("\n"))
    }

    fn search(&self, args: Value) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        struct SearchArgs {
            pattern: String,
            paths: StringOrMany,
            #[serde(default)]
            i: Option<bool>,
        }
        let args: SearchArgs = serde_json::from_value(args)
            .map_err(|error| ToolError::Failed(format!("invalid search args: {error}")))?;
        let regex = RegexBuilder::new(&args.pattern)
            .case_insensitive(args.i.unwrap_or(false))
            .build()
            .map_err(|error| ToolError::Failed(format!("invalid search regex: {error}")))?;
        let mut results = Vec::new();
        for path in args.paths.into_vec() {
            for file in self.expand_files(&path)? {
                let text = match fs::read_to_string(&file) {
                    Ok(text) => text,
                    Err(_) => continue,
                };
                let matches = text
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| regex.is_match(line))
                    .map(|(index, line)| format!("{}:{}:{}", file.display(), index + 1, line))
                    .collect::<Vec<_>>();
                if !matches.is_empty() {
                    results.extend(matches);
                }
            }
        }
        if results.is_empty() {
            Ok("No matches found".to_string())
        } else {
            Ok(results.join("\n"))
        }
    }

    fn find(&self, args: Value) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        struct FindArgs {
            paths: StringOrMany,
        }
        let args: FindArgs = serde_json::from_value(args)
            .map_err(|error| ToolError::Failed(format!("invalid find args: {error}")))?;
        let mut matches = Vec::new();
        for pattern in args.paths.into_vec() {
            for entry in self.expand_paths(&pattern)? {
                matches.push(entry.display().to_string());
            }
        }
        matches.sort();
        matches.dedup();
        Ok(matches.into_iter().take(200).collect::<Vec<_>>().join("\n"))
    }

    fn ast_grep(&self, args: Value) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        struct AstGrepArgs {
            pat: String,
            paths: Vec<String>,
        }
        let args: AstGrepArgs = serde_json::from_value(args)
            .map_err(|error| ToolError::Failed(format!("invalid ast_grep args: {error}")))?;
        let mut command = Command::new("ast-grep");
        command.arg("scan").arg("--pattern").arg(args.pat);
        for path in args.paths {
            command.arg(path);
        }
        let output = command
            .output()
            .map_err(|error| ToolError::Failed(format!("ast_grep failed: {error}")))?;
        if !output.status.success() {
            return Err(ToolError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn resolve_local(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }

    fn expand_files(&self, pattern: &str) -> Result<Vec<PathBuf>, ToolError> {
        self.expand_paths(pattern).map(|paths| {
            paths
                .into_iter()
                .filter(|path| path.is_file())
                .collect::<Vec<_>>()
        })
    }

    fn expand_paths(&self, pattern: &str) -> Result<Vec<PathBuf>, ToolError> {
        let absolute = self.resolve_local(pattern);
        if absolute.exists() {
            if absolute.is_dir() {
                return Ok(WalkDir::new(&absolute)
                    .into_iter()
                    .filter_map(Result::ok)
                    .map(|entry| entry.path().to_path_buf())
                    .collect());
            }
            return Ok(vec![absolute]);
        }

        let pattern = self.cwd.join(pattern).display().to_string();
        let mut matches = Vec::new();
        for entry in glob::glob(&pattern)
            .map_err(|error| ToolError::Failed(format!("invalid glob pattern: {error}")))?
        {
            let path = entry.map_err(|error| ToolError::Failed(format!("glob failed: {error}")))?;
            if path.is_dir() {
                matches.extend(
                    WalkDir::new(&path)
                        .into_iter()
                        .filter_map(Result::ok)
                        .map(|entry| entry.path().to_path_buf()),
                );
            } else {
                matches.push(path);
            }
        }
        Ok(matches)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrMany {
    One(String),
    Many(Vec<String>),
}

impl StringOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

fn split_selector(path: &str) -> (String, Option<String>) {
    if let Some(index) = path.rfind(':') {
        let suffix = &path[index + 1..];
        if suffix == "raw" || Regex::new(r"^\d+(?:-\d+)?$").unwrap().is_match(suffix) {
            return (path[..index].to_string(), Some(suffix.to_string()));
        }
    }
    (path.to_string(), None)
}

fn apply_read_selector(label: &str, text: &str, selector: Option<&str>) -> String {
    match selector {
        Some("raw") => text.to_string(),
        Some(range) => {
            let (start, end) = parse_range(range);
            let lines = text.lines().collect::<Vec<_>>();
            let start_index = start.saturating_sub(1);
            let end_index = end.min(lines.len());
            let slice = lines[start_index.min(lines.len())..end_index]
                .iter()
                .enumerate()
                .map(|(offset, line)| format!("{}:{}", start_index + offset + 1, line))
                .collect::<Vec<_>>();
            format!("¶{label}\n{}", slice.join("\n"))
        }
        None => {
            let lines = text
                .lines()
                .take(300)
                .enumerate()
                .map(|(index, line)| format!("{}:{}", index + 1, line));
            format!("¶{label}\n{}", lines.collect::<Vec<_>>().join("\n"))
        }
    }
}

fn parse_range(range: &str) -> (usize, usize) {
    if let Some((start, end)) = range.split_once('-') {
        let start = start.parse::<usize>().unwrap_or(1);
        let end = end.parse::<usize>().unwrap_or(start);
        (start.max(1), end.max(start))
    } else {
        let value = range.parse::<usize>().unwrap_or(1);
        (value.max(1), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(root: &Path) -> ToolRunner {
        let registry = ToolRegistry::new();
        let artifacts = ArtifactStore::new(root.join("artifacts")).unwrap();
        ToolRunner::new(registry, root.to_path_buf(), artifacts)
    }

    #[tokio::test]
    async fn read_file_selector_returns_numbered_lines() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "a\nb\nc\n").unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(ToolCall {
                id: "call-1".to_string(),
                name: "read".to_string(),
                arguments: serde_json::json!({"path": "note.txt:2-3"}),
                intent: None,
            })
            .await
            .unwrap();
        assert!(record.result.content.contains("2:b"));
        assert!(record.result.content.contains("3:c"));
    }

    #[tokio::test]
    async fn search_finds_matching_lines() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "alpha\nbeta\n").unwrap();
        let runner = runner(dir.path());
        let record = runner
            .execute(ToolCall {
                id: "call-2".to_string(),
                name: "search".to_string(),
                arguments: serde_json::json!({"pattern": "beta", "paths": "note.txt"}),
                intent: None,
            })
            .await
            .unwrap();
        assert!(record.result.content.contains("note.txt:2:beta"));
    }
}
