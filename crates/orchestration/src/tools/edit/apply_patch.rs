//! Codex `*** Begin Patch` envelope parser (OMP `apply-patch/parser.ts` port).

use super::errors::{ApplyPatchError, ParseError};
use super::patch::{PatchInput, PatchOp};

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";

fn is_patch_hunk_header(line: &str) -> bool {
    line.starts_with(ADD_FILE_MARKER)
        || line.starts_with(DELETE_FILE_MARKER)
        || line.starts_with(UPDATE_FILE_MARKER)
}

#[derive(Debug, Clone, Copy, Default)]
struct ParseApplyPatchOptions {
    streaming: bool,
}

/// Parse a Codex `*** Begin Patch` envelope into single-file patch inputs.
pub fn parse_apply_patch(patch_text: &str) -> Result<Vec<PatchInput>, ParseError> {
    parse_apply_patch_with_options(patch_text, ParseApplyPatchOptions::default())
}

/// Best-effort parser for in-progress previews; tolerates missing envelope markers.
pub fn parse_apply_patch_streaming(patch_text: &str) -> Vec<PatchInput> {
    parse_apply_patch_with_options(patch_text, ParseApplyPatchOptions { streaming: true })
        .unwrap_or_default()
}

/// Expand envelope text to patch inputs; errors when the envelope modifies no files.
pub fn expand_apply_patch_to_inputs(input: &str) -> Result<Vec<PatchInput>, ApplyPatchError> {
    let hunks = parse_apply_patch(input).map_err(|err| ApplyPatchError(err.to_string()))?;
    if hunks.is_empty() {
        return Err(ApplyPatchError(
            "No files were modified.".to_string(),
        ));
    }
    Ok(hunks)
}

fn parse_apply_patch_with_options(
    patch_text: &str,
    options: ParseApplyPatchOptions,
) -> Result<Vec<PatchInput>, ParseError> {
    let streaming = options.streaming;
    let mut lines: Vec<String> = patch_text.trim().split('\n').map(str::to_string).collect();

    if lines.len() >= 2 {
        let first = lines[0].as_str();
        let last = lines[lines.len() - 1].trim();
        let valid_openers = ["<<EOF", "<<'EOF'", "<<\"EOF\""];
        if valid_openers.contains(&first) && last == "EOF" {
            lines = lines[1..lines.len() - 1].to_vec();
        }
    }

    if lines.is_empty() || lines[0].trim() != BEGIN_PATCH_MARKER {
        if streaming {
            return Ok(Vec::new());
        }
        return Err(ParseError::new(
            "The first line of the patch must be '*** Begin Patch'",
            None,
        ));
    }

    let has_end_marker = lines.last().is_some_and(|line| line.trim() == END_PATCH_MARKER);
    if !has_end_marker && !streaming {
        return Err(ParseError::new(
            "The last line of the patch must be '*** End Patch'",
            None,
        ));
    }

    let mut hunks = Vec::new();
    let mut remaining = if has_end_marker {
        lines[1..lines.len() - 1].to_vec()
    } else {
        lines[1..].to_vec()
    };
    let mut line_number = 2usize;

    while !remaining.is_empty() {
        if remaining[0].trim().is_empty() {
            remaining.remove(0);
            line_number += 1;
            continue;
        }

        let first_line = remaining[0].trim();

        if let Some(path) = first_line.strip_prefix(ADD_FILE_MARKER) {
            let path = path.to_string();
            let mut contents = String::new();
            let mut consumed = 1usize;

            for line in remaining.iter().skip(1) {
                if let Some(body) = line.strip_prefix('+') {
                    contents.push_str(body);
                    contents.push('\n');
                    consumed += 1;
                } else {
                    break;
                }
            }

            hunks.push(PatchInput {
                path,
                op: PatchOp::Create,
                rename: None,
                diff: Some(contents),
            });
            remaining = remaining.split_off(consumed);
            line_number += consumed;
            continue;
        }

        if let Some(path) = first_line.strip_prefix(DELETE_FILE_MARKER) {
            hunks.push(PatchInput {
                path: path.to_string(),
                op: PatchOp::Delete,
                rename: None,
                diff: None,
            });
            remaining.remove(0);
            line_number += 1;
            continue;
        }

        if let Some(path) = first_line.strip_prefix(UPDATE_FILE_MARKER) {
            let path = path.to_string();
            remaining.remove(0);
            line_number += 1;

            let mut move_path = None;
            if let Some(next) = remaining.first() {
                if let Some(dest) = next.strip_prefix(MOVE_TO_MARKER) {
                    move_path = Some(dest.to_string());
                    remaining.remove(0);
                    line_number += 1;
                }
            }

            let mut diff_lines = Vec::new();
            while let Some(line) = remaining.first() {
                if is_patch_hunk_header(line) {
                    break;
                }
                diff_lines.push(remaining.remove(0));
                line_number += 1;
            }

            if diff_lines.is_empty() {
                if streaming {
                    hunks.push(PatchInput {
                        path,
                        op: PatchOp::Update,
                        rename: move_path,
                        diff: Some(String::new()),
                    });
                    continue;
                }
                return Err(ParseError::new(
                    format!("Update file hunk for path '{path}' is empty"),
                    Some(line_number),
                ));
            }

            hunks.push(PatchInput {
                path,
                op: PatchOp::Update,
                rename: move_path,
                diff: Some(diff_lines.join("\n")),
            });
            continue;
        }

        if streaming {
            break;
        }

        return Err(ParseError::new(
            format!(
                "'{first_line}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'"
            ),
            Some(line_number),
        ));
    }

    Ok(hunks)
}
