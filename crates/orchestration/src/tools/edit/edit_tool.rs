//! Thin `edit` (replace-mode) tool handler (Tier C).

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use super::diff::{generate_diff_string, replace_text, ReplaceOptions};
use super::errors::EditMatchError;
use super::io::{EditIo, EditIoError};
use super::replace::{find_match, FindMatchOptions, DEFAULT_FUZZY_THRESHOLD};
use crate::tools::errors::ToolError;

#[derive(Debug, Deserialize)]
struct EditArgs {
    path: String,
    edits: Vec<EditEntry>,
}

#[derive(Debug, Deserialize)]
struct EditEntry {
    old_text: String,
    new_text: String,
    #[serde(default)]
    all: bool,
}

pub fn execute_edit(cwd: PathBuf, args: Value) -> Result<String, ToolError> {
    let args: EditArgs = serde_json::from_value(args)
        .map_err(|error| ToolError::Failed(format!("invalid edit args: {error}")))?;
    if args.edits.is_empty() {
        return Err(ToolError::Failed("edits must contain at least one entry".to_string()));
    }

    let io = EditIo::new(cwd);
    let original = io.read_text(&args.path).map_err(map_io_error)?;
    let mut content = original.clone();
    let options = ReplaceOptions {
        fuzzy: allow_fuzzy(),
        all: false,
        threshold: fuzzy_threshold(),
    };

    let mut total_replacements = 0usize;
    for edit in &args.edits {
        if edit.old_text.is_empty() {
            return Err(ToolError::Failed("old_text must not be empty".to_string()));
        }
        let mut edit_options = options.clone();
        edit_options.all = edit.all;
        let result = replace_text(&content, &edit.old_text, &edit.new_text, &edit_options)
            .map_err(ToolError::Failed)?;
        if result.count == 0 {
            let match_outcome = find_match(
                &content,
                &edit.old_text,
                &FindMatchOptions {
                    allow_fuzzy: options.fuzzy,
                    threshold: options.threshold,
                },
            );
            if match_outcome.occurrences.is_some_and(|count| count > 1) {
                return Err(ToolError::Failed(format!(
                    "Found {} occurrences in {}. Add more context to disambiguate.",
                    match_outcome.occurrences.unwrap_or(0),
                    args.path
                )));
            }
            let message = EditMatchError::format_message_with(
                &args.path,
                &edit.old_text,
                match_outcome.closest.as_ref(),
                options.fuzzy,
                options.threshold.unwrap_or(DEFAULT_FUZZY_THRESHOLD),
                match_outcome.fuzzy_matches,
            );
            return Err(ToolError::Failed(message));
        }
        content = result.content;
        total_replacements += result.count;
    }

    if content == original {
        return Err(ToolError::Failed(format!(
            "Edits to {} resulted in no changes being made.",
            args.path
        )));
    }

    io.write_text(&args.path, &content).map_err(map_io_error)?;
    let diff = generate_diff_string(&original, &content, 2);
    let summary = if total_replacements > 1 {
        format!(
            "Successfully replaced {total_replacements} occurrences in {}.",
            args.path
        )
    } else {
        format!("Updated {}", args.path)
    };
    Ok(format!("{summary}\n\n{}", diff.diff))
}

fn allow_fuzzy() -> bool {
    !matches!(
        std::env::var("PI_EDIT_FUZZY").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}

fn fuzzy_threshold() -> Option<f64> {
    std::env::var("PI_EDIT_FUZZY_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
}

fn map_io_error(error: EditIoError) -> ToolError {
    match error {
        EditIoError::Path(path) => ToolError::Failed(path.0),
        EditIoError::NotebookNotSupported(path) => {
            ToolError::Failed(format!("notebook paths are not supported: {path}"))
        }
        EditIoError::NotFound(path) => ToolError::Failed(format!("file not found: {path}")),
        EditIoError::Io {
            operation,
            path,
            source,
        } => ToolError::Failed(format!("{operation} failed for {path}: {source}")),
    }
}
