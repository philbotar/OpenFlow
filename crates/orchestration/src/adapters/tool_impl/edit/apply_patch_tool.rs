//! Thin `apply_patch` tool handler (Tier C).
//!
//! Patches in one envelope are applied sequentially. If a later entry fails, earlier
//! entries remain on disk (non-atomic multi-file apply).

use std::path::PathBuf;

use serde_json::Value;

use engine::{summarize_diff, FileChangeOp};

use super::apply_patch::expand_apply_patch_to_inputs;
use super::diff::generate_diff_string;
use super::errors::ApplyPatchError;
use super::fuzzy_settings::{allow_fuzzy, patch_fuzzy_threshold};
use super::ledger::FileChangeLedger;
use super::patch::{
    apply_patch_entry, PatchApplyResult, PatchError, PatchInput, PatchOp, PatchOptions,
};
use super::tool_args::PatchEnvelopeArgs;
use crate::lsp::{append_writethrough_to_output, LspSettings, WritethroughPatchFileSystem};
use crate::tools::errors::ToolError;

pub fn execute_apply_patch(
    cwd: PathBuf,
    args: Value,
    ledger: FileChangeLedger,
    lsp: LspSettings,
) -> Result<String, ToolError> {
    let args: PatchEnvelopeArgs =
        serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
            tool: "apply_patch".to_string(),
            problem: error.to_string(),
            hint: "required field: input (string) with *** Begin Patch envelope".to_string(),
        })?;

    let inputs = expand_apply_patch_to_inputs(&args.input).map_err(map_apply_patch_error)?;
    let options = PatchOptions {
        cwd: cwd.clone(),
        dry_run: false,
        allow_fuzzy: allow_fuzzy(),
        fuzzy_threshold: patch_fuzzy_threshold(),
    };
    let fs = WritethroughPatchFileSystem::new(lsp);
    let mut lines = Vec::new();

    for input in inputs {
        let mut result = match apply_patch_entry(&input, &options, &fs) {
            Ok(result) => result,
            Err(error) => {
                let prefix = if lines.is_empty() {
                    String::new()
                } else {
                    format!("{}\n\n", lines.join("\n\n"))
                };
                return Err(ToolError::failed(format!(
                    "{prefix}{}",
                    map_patch_error(error)
                )));
            }
        };
        if let Some(normalized) = fs.normalized_content(&result.dest_path) {
            result.new_content = Some(normalized);
        }
        lines.push(summarize_patch(&input, &result));

        let diff_summary = patch_diff_summary(&result);
        if let Some(ref diff) = diff_summary {
            lines.push(diff.clone());
        }

        let (op, rename_to) = patch_change_op(&input);
        if should_record_patch_change(&input, &result) {
            ledger.record(
                input.path.clone(),
                op,
                rename_to,
                diff_summary.map(|diff| summarize_diff(&diff, 8)),
            );
        }
    }

    let mut output = lines.join("\n\n");
    let diagnostics = fs.take_diagnostics();
    if !diagnostics.is_empty() {
        output = append_writethrough_to_output(&output, &diagnostics);
    }
    Ok(output)
}

fn patch_diff_summary(result: &PatchApplyResult) -> Option<String> {
    let diff = match (&result.old_content, &result.new_content) {
        (None, Some(new)) => generate_diff_string("", new, 2).diff,
        (Some(old), Some(new)) if old != new => generate_diff_string(old, new, 2).diff,
        (Some(old), None) => generate_diff_string(old, "", 2).diff,
        _ => String::new(),
    };
    if diff.is_empty() {
        None
    } else {
        Some(diff)
    }
}

fn should_record_patch_change(input: &PatchInput, result: &PatchApplyResult) -> bool {
    match input.op {
        PatchOp::Create | PatchOp::Delete => true,
        PatchOp::Update => {
            if input.rename.is_some() {
                return true;
            }
            matches!(
                (&result.old_content, &result.new_content),
                (Some(old), Some(new)) if old != new
            )
        }
    }
}

fn patch_change_op(input: &PatchInput) -> (FileChangeOp, Option<String>) {
    match input.op {
        PatchOp::Create => (FileChangeOp::Create, None),
        PatchOp::Delete => (FileChangeOp::Delete, None),
        PatchOp::Update => {
            if let Some(rename) = input.rename.clone() {
                (FileChangeOp::Rename, Some(rename))
            } else {
                (FileChangeOp::Update, None)
            }
        }
    }
}

fn summarize_patch(input: &PatchInput, _result: &PatchApplyResult) -> String {
    match input.op {
        PatchOp::Create => format!("Created {}", input.path),
        PatchOp::Delete => format!("Deleted {}", input.path),
        PatchOp::Update => {
            if let Some(rename) = &input.rename {
                format!("Updated and moved {} to {rename}", input.path)
            } else {
                format!("Updated {}", input.path)
            }
        }
    }
}

fn map_apply_patch_error(error: ApplyPatchError) -> ToolError {
    ToolError::failed(error.0)
}

fn map_patch_error(error: PatchError) -> ToolError {
    ToolError::failed(error.to_string())
}
