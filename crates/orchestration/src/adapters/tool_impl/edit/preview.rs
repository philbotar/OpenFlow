//! Dry-run diff preview for write-tier edit tools (Phase 6).

use std::path::PathBuf;

use engine::FileChangeOp;
use serde_json::Value;

use super::apply_patch::expand_apply_patch_to_inputs;
use super::diff::{generate_diff_string, replace_text, ReplaceOptions};
use super::fuzzy_settings::{allow_fuzzy, edit_fuzzy_threshold, patch_fuzzy_threshold};
use super::io::EditIo;
use super::patch::{apply_patch_entry, PatchOp, PatchOptions, StdPatchFileSystem};
use super::tool_args::{EditToolArgs, PatchEnvelopeArgs, WriteToolArgs};
use crate::api::{FileEditPreview, FileEditPreviewEntry};

/// Compute numbered diffs without writing to disk.
pub fn preview_file_edit(
    cwd: PathBuf,
    tool_name: &str,
    args: &Value,
    snapshots: std::sync::Arc<super::hashline::snapshots::InMemorySnapshotStore>,
) -> Result<FileEditPreview, String> {
    match tool_name {
        "write" => preview_write(cwd, args),
        "edit" => preview_edit(cwd, args, snapshots),
        "apply_patch" => preview_apply_patch(cwd, args),
        other => Err(format!("tool '{other}' does not support file edit preview")),
    }
}

fn preview_write(cwd: PathBuf, args: &Value) -> Result<FileEditPreview, String> {
    let args: WriteToolArgs = serde_json::from_value(args.clone())
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

fn preview_edit(
    cwd: PathBuf,
    args: &Value,
    snapshots: std::sync::Arc<super::hashline::snapshots::InMemorySnapshotStore>,
) -> Result<FileEditPreview, String> {
    if args.get("input").is_some() {
        return preview_hashline_edit(cwd, args, snapshots);
    }

    let args: EditToolArgs = serde_json::from_value(args.clone())
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

fn preview_hashline_edit(
    cwd: PathBuf,
    args: &Value,
    snapshots: std::sync::Arc<super::hashline::snapshots::InMemorySnapshotStore>,
) -> Result<FileEditPreview, String> {
    let args: PatchEnvelopeArgs = serde_json::from_value(args.clone())
        .map_err(|error| format!("invalid hashline edit args: {error}"))?;
    use super::hashline::execute::EditHashlineFs;
    use super::hashline::input::Patch;
    use super::hashline::patcher::{Patcher, PatcherOptions};
    use super::hashline::types::SplitOptions;

    let cwd_display = cwd.to_string_lossy().into_owned();
    let patch = Patch::parse(
        &args.input,
        SplitOptions {
            cwd: Some(cwd_display),
            path: None,
        },
    )?;
    if patch.sections.is_empty() {
        return Err("No hashline sections found in input.".to_string());
    }

    let fs = EditHashlineFs::new(EditIo::new(cwd));
    let patcher = Patcher::new(PatcherOptions {
        fs,
        snapshots,
        block_resolver: None,
    });
    patcher.preflight(&patch)?;

    let mut entries = Vec::new();
    for section in &patch.sections {
        let mut section = section.clone();
        let prepared = patcher.prepare(&mut section)?;
        if prepared.is_noop() {
            return Err(format!(
                "Edits to {} resulted in no changes being made.",
                section.path
            ));
        }
        let diff = generate_diff_string(&prepared.normalized, &prepared.apply_result.text, 2);
        entries.push(preview_entry(
            &section.path,
            FileChangeOp::Update,
            diff.diff,
            None,
        ));
    }

    Ok(FileEditPreview {
        entries,
        error: None,
    })
}

fn preview_apply_patch(cwd: PathBuf, args: &Value) -> Result<FileEditPreview, String> {
    let args: PatchEnvelopeArgs = serde_json::from_value(args.clone())
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

fn map_io_error(error: super::io::EditIoError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;

    fn empty_snapshots() -> Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore> {
        Arc::new(crate::tools::edit::hashline::snapshots::InMemorySnapshotStore::new())
    }

    #[test]
    fn preview_write_shows_create_diff() {
        let temp = tempfile::tempdir().expect("tempdir");
        let preview = preview_file_edit(
            temp.path().to_path_buf(),
            "write",
            &serde_json::json!({"path": "new.txt", "content": "hello\n"}),
            empty_snapshots(),
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
            empty_snapshots(),
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
            empty_snapshots(),
        )
        .expect("preview");

        assert!(preview.entries[0].diff.contains("beta"));
        assert!(preview.entries[0].diff.contains("gamma"));
    }
}
