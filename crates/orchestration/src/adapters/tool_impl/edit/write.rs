//! Thin `write` tool handler (Tier C).

use std::path::PathBuf;

use serde_json::Value;

use super::diff::generate_diff_string;
use super::io::{map_io_error, EditIo, WriteTextOutcome};
use super::ledger::FileChangeLedger;
use super::tool_args::WriteToolArgs;
use crate::lsp::{append_writethrough_to_output, LspSettings};
use crate::tools::errors::ToolError;

pub fn execute_write(
    cwd: PathBuf,
    args: Value,
    ledger: FileChangeLedger,
    lsp: LspSettings,
) -> Result<String, ToolError> {
    let args: WriteToolArgs =
        serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
            tool: "write".to_string(),
            problem: error.to_string(),
            hint: "required fields: path (string), content (string)".to_string(),
        })?;
    let io = EditIo::new(cwd).with_ledger(ledger).with_lsp_settings(lsp);

    let existed = io.exists(&args.path).map_err(map_io_error)?;

    if existed {
        let old_content = io.read_text(&args.path).map_err(map_io_error)?;
        let intended = io
            .preview_text_after_write(&args.path, &args.content)
            .map_err(map_io_error)?;
        if old_content == intended {
            return Err(ToolError::failed(format!(
                "No changes would be made to {}.",
                args.path
            )));
        }
        let outcome = io
            .write_text(&args.path, &args.content)
            .map_err(map_io_error)?;
        let new_content = outcome.disk_normalized.clone().unwrap_or(intended);
        let diff = generate_diff_string(&old_content, &new_content, 2);
        return Ok(finish_write_output(
            format!("Updated {}\n\n{}", args.path, diff.diff),
            outcome,
        ));
    }

    write_create_with_summary(&io, &args.path, &args.content)
}

fn write_create_with_summary(io: &EditIo, path: &str, content: &str) -> Result<String, ToolError> {
    let mut payload = content.to_string();
    if !payload.ends_with('\n') {
        payload.push('\n');
    }
    let intended = io
        .preview_text_after_write(path, &payload)
        .map_err(map_io_error)?;
    let outcome = io.write_text(path, &payload).map_err(map_io_error)?;
    let normalized = outcome.disk_normalized.clone().unwrap_or(intended);
    let diff = generate_diff_string("", &normalized, 2);
    Ok(finish_write_output(
        format!("Created {path}\n\n{}", diff.diff),
        outcome,
    ))
}

fn finish_write_output(mut output: String, outcome: WriteTextOutcome) -> String {
    if let Some(diagnostics) = outcome.diagnostics {
        output = append_writethrough_to_output(&output, std::slice::from_ref(&diagnostics));
    }
    output
}
