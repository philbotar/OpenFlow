//! Dry-run diff preview for write-tier edit tools (Phase 6).

use std::path::PathBuf;

use domain::FileChangeOp;
use serde::Deserialize;
use serde_json::Value;

use super::apply_patch::expand_apply_patch_to_inputs;
use super::diff::{generate_diff_string, replace_text, ReplaceOptions};
use super::io::EditIo;
use super::patch::{apply_patch_entry, PatchOp, PatchOptions, StdPatchFileSystem};
use super::replace::DEFAULT_FUZZY_THRESHOLD;
use crate::api::{FileEditPreview, FileEditPreviewEntry};

#[derive(Debug, Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
}

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
struct ApplyPatchArgs {
    input: String,
}

/// Compute numbered diffs without writing to disk.
pub fn preview_file_edit(
    cwd: PathBuf,
    tool_name: &str,
    args: &Value,
) -> Result<FileEditPreview, String> {
    match tool_name {
        "write" => preview_write(cwd, args),
        "edit" => preview_edit(cwd, args),
        "apply_patch" => preview_apply_patch(cwd, args),
        other => Err(format!("tool '{other}' does not support file edit preview")),
    }
}

fn preview_write(cwd: PathBuf, args: &Value) -> Result<FileEditPreview, String> {
    let args: WriteArgs = serde_json::from_value(args.clone())
        .map_err(|error| format!("invalid write args: {error}"))?;
    let io = EditIo::new(cwd);

    let entry = if io.exists(&args.path).map_err(map_io_error)? {
        let old_content = io.read_text(&args.path).map_err(map_io_error)?;
        let new_content = io
            .preview_text_after_write(&args.path, &args.content)
            .map_err(map_io_error)?;
        if old_content == new_content {
            return Err(format!("No changes would be made to {}.", args.path));
        }
        let diff = generate_diff_string(&old_content, &new_content, 2);
        preview_entry(&args.path, FileChangeOp::Update, diff.diff, None)
    } else {
        let normalized = io
            .preview_text_after_write(&args.path, &args.content)
            .map_err(map_io_error)?;
        let diff = generate_diff_string("", &normalized, 2);
        preview_entry(&args.path, FileChangeOp::Create, diff.diff, None)
    };

    Ok(FileEditPreview {
        entries: vec![entry],
        error: None,
    })
}

fn preview_edit(cwd: PathBuf, args: &Value) -> Result<FileEditPreview, String> {
    let args: EditArgs = serde_json::from_value(args.clone())
        .map_err(|error| format!("invalid edit args: {error}"))?;
    if args.edits.is_empty() {
        return Err("edits must contain at least one entry".to_string());
    }

    let io = EditIo::new(cwd);
    let original = io.read_text(&args.path).map_err(map_io_error)?;
    let mut content = original.clone();
    let options = ReplaceOptions {
        fuzzy: allow_fuzzy(),
        all: false,
        threshold: edit_fuzzy_threshold(),
    };

    for edit in &args.edits {
        if edit.old_text.is_empty() {
            return Err("old_text must not be empty".to_string());
        }
        let mut edit_options = options.clone();
        edit_options.all = edit.all;
        let result = replace_text(&content, &edit.old_text, &edit.new_text, &edit_options)?;
        content = result.content;
    }

    if content == original {
        return Err(format!(
            "Edits to {} resulted in no changes being made.",
            args.path
        ));
    }

    let diff = generate_diff_string(&original, &content, 2);
    Ok(FileEditPreview {
        entries: vec![preview_entry(
            &args.path,
            FileChangeOp::Update,
            diff.diff,
            None,
        )],
        error: None,
    })
}

fn preview_apply_patch(cwd: PathBuf, args: &Value) -> Result<FileEditPreview, String> {
    let args: ApplyPatchArgs = serde_json::from_value(args.clone())
        .map_err(|error| format!("invalid apply_patch args: {error}"))?;
    let inputs = expand_apply_patch_to_inputs(&args.input).map_err(|error| error.0)?;
    let options = PatchOptions {
        cwd: cwd.clone(),
        dry_run: true,
        allow_fuzzy: allow_fuzzy(),
        fuzzy_threshold: patch_fuzzy_threshold(),
    };
    let fs = StdPatchFileSystem;
    let mut entries = Vec::new();

    for input in inputs {
        let result = apply_patch_entry(&input, &options, &fs).map_err(|error| error.to_string())?;
        let diff = match (&result.old_content, &result.new_content) {
            (None, Some(new)) => generate_diff_string("", new, 2).diff,
            (Some(old), Some(new)) if old != new => generate_diff_string(old, new, 2).diff,
            (Some(old), None) => generate_diff_string(old, "", 2).diff,
            _ => String::new(),
        };
        if diff.is_empty() && input.op != PatchOp::Delete {
            continue;
        }
        let op = match input.op {
            PatchOp::Create => FileChangeOp::Create,
            PatchOp::Delete => FileChangeOp::Delete,
            PatchOp::Update => {
                if input.rename.is_some() {
                    FileChangeOp::Rename
                } else {
                    FileChangeOp::Update
                }
            }
        };
        entries.push(preview_entry(&input.path, op, diff, input.rename.clone()));
    }

    if entries.is_empty() {
        return Err("Patch would not change any files.".to_string());
    }

    Ok(FileEditPreview {
        entries,
        error: None,
    })
}

fn preview_entry(
    path: &str,
    op: FileChangeOp,
    diff: String,
    rename_to: Option<String>,
) -> FileEditPreviewEntry {
    FileEditPreviewEntry {
        path: path.to_string(),
        op,
        diff,
        rename_to,
    }
}

fn allow_fuzzy() -> bool {
    !matches!(
        std::env::var("PI_EDIT_FUZZY").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}

fn edit_fuzzy_threshold() -> Option<f64> {
    std::env::var("PI_EDIT_FUZZY_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
}

fn patch_fuzzy_threshold() -> f64 {
    std::env::var("PI_EDIT_FUZZY_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_FUZZY_THRESHOLD)
}

fn map_io_error(error: super::io::EditIoError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preview_write_shows_create_diff() {
        let temp = tempfile::tempdir().expect("tempdir");
        let preview = preview_file_edit(
            temp.path().to_path_buf(),
            "write",
            &serde_json::json!({"path": "new.txt", "content": "hello\n"}),
        )
        .expect("preview");

        assert_eq!(preview.entries.len(), 1);
        assert_eq!(preview.entries[0].path, "new.txt");
        assert!(preview.entries[0].diff.contains("hello"));
    }

    #[test]
    fn preview_write_shows_update_diff() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("note.txt"), "old\n").expect("seed");

        let preview = preview_file_edit(
            temp.path().to_path_buf(),
            "write",
            &serde_json::json!({"path": "note.txt", "content": "new\n"}),
        )
        .expect("preview");

        assert!(preview.entries[0].diff.contains("old"));
        assert!(preview.entries[0].diff.contains("new"));
    }

    #[test]
    fn preview_edit_shows_replacement_diff() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("note.txt"), "alpha\nbeta\n").expect("seed");

        let preview = preview_file_edit(
            temp.path().to_path_buf(),
            "edit",
            &serde_json::json!({
                "path": "note.txt",
                "edits": [{"old_text": "beta", "new_text": "gamma"}]
            }),
        )
        .expect("preview");

        assert!(preview.entries[0].diff.contains("beta"));
        assert!(preview.entries[0].diff.contains("gamma"));
    }
}
