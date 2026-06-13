//! `edit` tool handler — replace mode and hashline mode (Tier C).

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use super::diff::{generate_diff_string, replace_text, ReplaceOptions};
use super::errors::EditMatchError;
use super::hashline::execute::execute_hashline;
use super::hashline::snapshots::InMemorySnapshotStore;
use super::io::{EditIo, EditIoError};
use super::ledger::FileChangeLedger;
use super::replace::{find_match, FindMatchOptions, DEFAULT_FUZZY_THRESHOLD};
use crate::lsp::{append_writethrough_to_output, LspSettings};
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

#[derive(Debug, Deserialize)]
struct HashlineArgs {
    input: String,
}

pub fn execute_edit(
    cwd: PathBuf,
    args: Value,
    ledger: FileChangeLedger,
    snapshots: Arc<InMemorySnapshotStore>,
    lsp: LspSettings,
) -> Result<String, ToolError> {
    if args.get("input").is_some() {
        let args: HashlineArgs =
            serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
                tool: "edit".to_string(),
                problem: error.to_string(),
                hint:
                    "hashline mode requires input (string) with ¶path#TAG sections from read output"
                        .to_string(),
            })?;
        return execute_hashline(cwd, args.input, ledger, snapshots, lsp);
    }

    let args: EditArgs = serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
        tool: "edit".to_string(),
        problem: error.to_string(),
        hint: "replace mode requires path + edits[]; hashline mode requires input (string)"
            .to_string(),
    })?;
    if args.edits.is_empty() {
        return Err(ToolError::InvalidArgs {
            tool: "edit".to_string(),
            problem: "edits must contain at least one entry".to_string(),
            hint: "each edit needs old_text and new_text; set all:true to replace every match"
                .to_string(),
        });
    }

    let io = EditIo::new(cwd).with_ledger(ledger).with_lsp_settings(lsp);
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
            return Err(ToolError::InvalidArgs {
                tool: "edit".to_string(),
                problem: "old_text must not be empty".to_string(),
                hint: "old_text must match file content exactly (unless all:true)".to_string(),
            });
        }
        let mut edit_options = options.clone();
        edit_options.all = edit.all;
        let result = replace_text(&content, &edit.old_text, &edit.new_text, &edit_options)
            .map_err(ToolError::failed)?;
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
                return Err(ToolError::failed(format!(
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
            return Err(ToolError::failed(message));
        }
        content = result.content;
        total_replacements += result.count;
    }

    if content == original {
        return Err(ToolError::failed(format!(
            "Edits to {} resulted in no changes being made.",
            args.path
        )));
    }

    let outcome = io.write_text(&args.path, &content).map_err(map_io_error)?;
    let final_content = outcome.disk_normalized.unwrap_or(content);
    let diff = generate_diff_string(&original, &final_content, 2);
    let summary = if total_replacements > 1 {
        format!(
            "Successfully replaced {total_replacements} occurrences in {}.",
            args.path
        )
    } else {
        format!("Updated {}", args.path)
    };
    let mut output = format!("{summary}\n\n{}", diff.diff);
    if let Some(diagnostics) = outcome.diagnostics {
        output = append_writethrough_to_output(&output, std::slice::from_ref(&diagnostics));
    }
    Ok(output)
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
        EditIoError::AutoGenerated(blocked) => ToolError::failed(blocked.0),
        EditIoError::Path(path) if path.0.contains("path escapes execution folder") => {
            ToolError::PermissionDenied {
                what: path.0,
                hint: "paths must stay under the execution folder; use a relative path".to_string(),
            }
        }
        EditIoError::Path(path) => ToolError::failed(path.0),
        EditIoError::NotebookNotSupported(path) => {
            ToolError::failed(format!("notebook paths are not supported: {path}"))
        }
        EditIoError::NotFound(path) => ToolError::NotFound {
            what: format!("file not found: {path}"),
            hint: "use find to locate the file".to_string(),
        },
        EditIoError::Io {
            operation,
            path,
            source,
        } => ToolError::failed(format!("{operation} failed for {path}: {source}")),
    }
}
