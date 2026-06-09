//! Ripgrep-backed content search adapter (`grep-searcher` + `ignore`).

use crate::tool_errors::ToolError;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::WalkBuilder;
use serde::Deserialize;
use serde_json::Value;
use std::io;
use std::path::{Path, PathBuf};

use crate::tool_ports::ContentSearch;

pub const MAX_SEARCH_MATCHES: usize = 500;

/// Ripgrep-library implementation of [`ContentSearch`].
#[derive(Debug, Clone)]
pub struct RipgrepSearch {
    cwd: PathBuf,
}

impl RipgrepSearch {
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }
}

impl ContentSearch for RipgrepSearch {
    fn search(&self, args: Value) -> Result<String, ToolError> {
        search_at(&self.cwd, args)
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

#[derive(Debug, Deserialize)]
struct SearchArgs {
    pattern: String,
    paths: StringOrMany,
    #[serde(default)]
    i: Option<bool>,
    #[serde(default)]
    gitignore: Option<bool>,
}

struct MatchSink {
    display_path: String,
    lines: Vec<String>,
    max_matches: usize,
    limit_reached: bool,
}

impl MatchSink {
    fn new(display_path: String, max_matches: usize) -> Self {
        Self {
            display_path,
            lines: Vec::new(),
            max_matches,
            limit_reached: false,
        }
    }
}

impl Sink for MatchSink {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.limit_reached {
            return Ok(false);
        }
        let line_number = mat.line_number().unwrap_or(0);
        let line = String::from_utf8_lossy(mat.bytes()).trim_end().to_string();
        self.lines
            .push(format!("{}:{}:{}", self.display_path, line_number, line));
        if self.lines.len() >= self.max_matches {
            self.limit_reached = true;
            return Ok(false);
        }
        Ok(true)
    }
}

/// Search files under `cwd` using ripgrep libraries.
pub fn search_at(cwd: &Path, args: Value) -> Result<String, ToolError> {
    let args: SearchArgs = serde_json::from_value(args)
        .map_err(|error| ToolError::Failed(format!("invalid search args: {error}")))?;
    let pattern = args.pattern.trim();
    if pattern.is_empty() {
        return Err(ToolError::Failed(
            "search pattern must not be empty".to_string(),
        ));
    }

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(args.i.unwrap_or(false))
        .build(pattern)
        .map_err(|error| ToolError::Failed(format!("invalid search regex: {error}")))?;

    let gitignore = args.gitignore.unwrap_or(true);
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\0'))
        .line_number(true)
        .build();

    let mut all_lines = Vec::new();
    let mut limit_reached = false;

    for path_spec in args.paths.into_vec() {
        if limit_reached {
            break;
        }
        for target in resolve_search_targets(cwd, &path_spec)? {
            if limit_reached {
                break;
            }
            let remaining = MAX_SEARCH_MATCHES.saturating_sub(all_lines.len());
            if remaining == 0 {
                limit_reached = true;
                break;
            }
            let (lines, truncated) =
                search_target(cwd, &mut searcher, &matcher, &target, gitignore, remaining)?;
            if truncated {
                limit_reached = true;
            }
            all_lines.extend(lines);
        }
    }

    if all_lines.is_empty() {
        return Ok("No matches found".to_string());
    }

    let mut output = all_lines.join("\n");
    if limit_reached {
        output.push_str(&format!(
            "\n… search truncated after {MAX_SEARCH_MATCHES} matches; narrow paths or pattern …"
        ));
    }
    Ok(output)
}

fn search_target(
    cwd: &Path,
    searcher: &mut Searcher,
    matcher: &grep_regex::RegexMatcher,
    target: &Path,
    gitignore: bool,
    max_matches: usize,
) -> Result<(Vec<String>, bool), ToolError> {
    if max_matches == 0 {
        return Ok((Vec::new(), true));
    }

    if target.is_file() {
        let display = display_path(cwd, target);
        return search_file(searcher, matcher, target, display, max_matches);
    }

    if target.is_dir() {
        return search_directory(cwd, searcher, matcher, target, gitignore, max_matches);
    }

    Ok((Vec::new(), false))
}

fn search_file(
    searcher: &mut Searcher,
    matcher: &grep_regex::RegexMatcher,
    path: &Path,
    display_path: String,
    max_matches: usize,
) -> Result<(Vec<String>, bool), ToolError> {
    let mut sink = MatchSink::new(display_path, max_matches);
    searcher
        .search_path(matcher, path, &mut sink)
        .map_err(|error| {
            ToolError::Failed(format!("search failed for {}: {error}", path.display()))
        })?;
    Ok((sink.lines, sink.limit_reached))
}

fn search_directory(
    cwd: &Path,
    searcher: &mut Searcher,
    matcher: &grep_regex::RegexMatcher,
    root: &Path,
    gitignore: bool,
    max_matches: usize,
) -> Result<(Vec<String>, bool), ToolError> {
    let mut walker = WalkBuilder::new(root);
    walker.git_ignore(gitignore);
    walker.hidden(false);

    let mut lines = Vec::new();
    let mut limit_reached = false;

    for entry in walker.build().filter_map(Result::ok) {
        if limit_reached {
            break;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let remaining = max_matches.saturating_sub(lines.len());
        if remaining == 0 {
            limit_reached = true;
            break;
        }
        let display = display_path(cwd, path);
        let (file_lines, truncated) = search_file(searcher, matcher, path, display, remaining)?;
        if truncated {
            limit_reached = true;
        }
        lines.extend(file_lines);
    }

    Ok((lines, limit_reached))
}

fn resolve_search_targets(cwd: &Path, pattern: &str) -> Result<Vec<PathBuf>, ToolError> {
    let absolute = resolve_local(cwd, pattern);
    if absolute.exists() {
        return Ok(vec![absolute]);
    }

    let glob_pattern = cwd.join(pattern).display().to_string();
    let mut matches = Vec::new();
    for entry in glob::glob(&glob_pattern)
        .map_err(|error| ToolError::Failed(format!("invalid glob pattern: {error}")))?
    {
        matches.push(entry.map_err(|error| ToolError::Failed(format!("glob failed: {error}")))?);
    }
    Ok(matches)
}

fn resolve_local(cwd: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_path_buf();
        (dir, cwd)
    }

    #[test]
    fn finds_matching_line() {
        let (_dir, cwd) = fixture();
        fs::write(cwd.join("note.txt"), "alpha\nbeta\n").unwrap();
        let output = RipgrepSearch::new(cwd.clone())
            .search(serde_json::json!({"pattern": "beta", "paths": "note.txt"}))
            .unwrap();
        assert!(output.contains("note.txt:2:beta"));
    }

    #[test]
    fn respects_gitignore() {
        let (_dir, cwd) = fixture();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&cwd)
            .status()
            .expect("git init for gitignore test");
        fs::write(cwd.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(cwd.join("ignored.txt"), "secret_match\n").unwrap();
        fs::write(cwd.join("visible.txt"), "secret_match\n").unwrap();
        let output = RipgrepSearch::new(cwd)
            .search(serde_json::json!({"pattern": "secret_match", "paths": "."}))
            .unwrap();
        assert!(output.contains("visible.txt"));
        assert!(!output.contains("ignored.txt"));
    }

    #[test]
    fn case_insensitive() {
        let (_dir, cwd) = fixture();
        fs::write(cwd.join("note.txt"), "Beta\n").unwrap();
        let output = RipgrepSearch::new(cwd)
            .search(serde_json::json!({"pattern": "beta", "paths": "note.txt", "i": true}))
            .unwrap();
        assert!(output.contains("note.txt:1:Beta"));
    }

    #[test]
    fn caps_match_count() {
        let (_dir, cwd) = fixture();
        let lines = (0..600)
            .map(|index| format!("match_{index}\n"))
            .collect::<String>();
        fs::write(cwd.join("many.txt"), lines).unwrap();
        let output = RipgrepSearch::new(cwd)
            .search(serde_json::json!({"pattern": "match_", "paths": "many.txt"}))
            .unwrap();
        assert!(output.contains("truncated"));
        assert_eq!(
            output
                .lines()
                .filter(|line| line.contains("many.txt:"))
                .count(),
            MAX_SEARCH_MATCHES
        );
    }

    #[test]
    fn invalid_regex() {
        let (_dir, cwd) = fixture();
        let error = RipgrepSearch::new(cwd)
            .search(serde_json::json!({"pattern": "[", "paths": "."}))
            .unwrap_err();
        assert!(error.to_string().contains("invalid search regex"));
    }

    #[test]
    fn empty_pattern_errors() {
        let (_dir, cwd) = fixture();
        let error = RipgrepSearch::new(cwd)
            .search(serde_json::json!({"pattern": "  ", "paths": "."}))
            .unwrap_err();
        assert!(error.to_string().contains("must not be empty"));
    }
}
