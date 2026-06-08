//! Thin `apply_patch` tool handler (Tier C).
//!
//! Patches in one envelope are applied sequentially. If a later entry fails, earlier
//! entries remain on disk (non-atomic multi-file apply).

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use super::apply_patch::expand_apply_patch_to_inputs;
use super::diff::generate_diff_string;
use super::errors::ApplyPatchError;
use super::patch::{
    apply_patch_entry, PatchApplyResult, PatchError, PatchInput, PatchOp, PatchOptions,
    StdPatchFileSystem,
};
use super::replace::DEFAULT_FUZZY_THRESHOLD;
use crate::tools::errors::ToolError;

#[derive(Debug, Deserialize)]
struct ApplyPatchArgs {
    input: String,
}

pub fn execute_apply_patch(cwd: PathBuf, args: Value) -> Result<String, ToolError> {
    let args: ApplyPatchArgs = serde_json::from_value(args).map_err(|error| {
        ToolError::Failed(format!("invalid apply_patch args: {error}"))
    })?;

    let inputs = expand_apply_patch_to_inputs(&args.input).map_err(map_apply_patch_error)?;
    let options = PatchOptions {
        cwd: cwd.clone(),
        dry_run: false,
        allow_fuzzy: allow_fuzzy(),
        fuzzy_threshold: fuzzy_threshold(),
    };
    let fs = StdPatchFileSystem;
    let mut lines = Vec::new();

    for input in inputs {
        let result = apply_patch_entry(&input, &options, &fs).map_err(map_patch_error)?;
        lines.push(summarize_patch(&input, &result));

        if let (Some(old), Some(new)) = (&result.old_content, &result.new_content) {
            if old != new {
                let diff = generate_diff_string(old, new, 2);
                if !diff.diff.is_empty() {
                    lines.push(diff.diff);
                }
            }
        }
    }

    Ok(lines.join("\n\n"))
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

fn allow_fuzzy() -> bool {
    !matches!(
        std::env::var("PI_EDIT_FUZZY").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}

fn fuzzy_threshold() -> f64 {
    std::env::var("PI_EDIT_FUZZY_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_FUZZY_THRESHOLD)
}

fn map_apply_patch_error(error: ApplyPatchError) -> ToolError {
    ToolError::Failed(error.0)
}

fn map_patch_error(error: PatchError) -> ToolError {
    ToolError::Failed(error.to_string())
}
