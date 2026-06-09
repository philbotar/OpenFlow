//! Pre-edit snapshots for revert support (Phase 9).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use domain::{EditBatch, FileSnapshot};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::apply_patch::expand_apply_patch_to_inputs;
use super::file_snapshot_store::{canonical_snapshot_path, record_file_snapshot};
use super::hashline::input::Patch;
use super::hashline::snapshots::{InMemorySnapshotStore, SnapshotStore};
use super::hashline::types::SplitOptions;
use super::path::resolve_writable;
use crate::tools::registry::BuiltinToolKind;

pub fn capture_edit_batch(
    cwd: &Path,
    node_id: &str,
    tool_call_id: &str,
    tool_name: &str,
    kind: BuiltinToolKind,
    args: &Value,
) -> Option<EditBatch> {
    let paths = collect_edit_paths(kind, args, cwd)?;
    if paths.is_empty() {
        return None;
    }
    let snapshots = capture_snapshots(cwd, &paths);
    if snapshots.is_empty() {
        return None;
    }
    Some(EditBatch {
        batch_id: Uuid::new_v4().to_string(),
        node_id: node_id.to_string(),
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        timestamp_ms: now_ms(),
        snapshots,
    })
}

pub fn revert_edit_batch(cwd: &Path, batch: &EditBatch) -> Result<(), String> {
    let (removals, restores): (Vec<&FileSnapshot>, Vec<&FileSnapshot>) = batch
        .snapshots
        .iter()
        .partition(|snapshot| !snapshot.existed);

    for snapshot in removals {
        revert_remove_created(cwd, snapshot)?;
    }
    for snapshot in restores {
        revert_restore_existing(cwd, snapshot)?;
    }
    Ok(())
}

pub fn sync_hashline_snapshots_after_revert(
    cwd: &Path,
    store: &InMemorySnapshotStore,
    batch: &EditBatch,
) {
    for snapshot in &batch.snapshots {
        let Ok(canonical) = canonical_snapshot_path(cwd, &snapshot.path) else {
            continue;
        };
        store.invalidate(&canonical);
        if snapshot.existed {
            if let Some(content) = &snapshot.content {
                let _ = record_file_snapshot(store, &canonical, content);
            }
        }
    }
}

fn revert_remove_created(cwd: &Path, snapshot: &FileSnapshot) -> Result<(), String> {
    let absolute = resolve_writable(cwd, &snapshot.path).map_err(|error| error.to_string())?;
    if absolute.exists() {
        fs::remove_file(&absolute).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn revert_restore_existing(cwd: &Path, snapshot: &FileSnapshot) -> Result<(), String> {
    let content = snapshot
        .content
        .as_ref()
        .ok_or_else(|| format!("missing snapshot content for {}", snapshot.path))?;
    let absolute = resolve_writable(cwd, &snapshot.path).map_err(|error| error.to_string())?;
    if let Some(parent) = absolute.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }
    fs::write(&absolute, content).map_err(|error| error.to_string())
}

fn collect_edit_paths(kind: BuiltinToolKind, args: &Value, cwd: &Path) -> Option<Vec<String>> {
    match kind {
        BuiltinToolKind::Write => {
            #[derive(Deserialize)]
            struct WriteArgs {
                path: String,
            }
            let args: WriteArgs = serde_json::from_value(args.clone()).ok()?;
            Some(vec![args.path])
        }
        BuiltinToolKind::Edit => {
            if args.get("input").is_some() {
                #[derive(Deserialize)]
                struct HashlineArgs {
                    input: String,
                }
                let args: HashlineArgs = serde_json::from_value(args.clone()).ok()?;
                let cwd_display = cwd.to_string_lossy().into_owned();
                let patch = Patch::parse(
                    &args.input,
                    SplitOptions {
                        cwd: Some(cwd_display),
                        path: None,
                    },
                )
                .ok()?;
                let paths = patch
                    .sections
                    .iter()
                    .map(|section| section.path.clone())
                    .collect::<Vec<_>>();
                Some(paths)
            } else {
                #[derive(Deserialize)]
                struct EditArgs {
                    path: String,
                }
                let args: EditArgs = serde_json::from_value(args.clone()).ok()?;
                Some(vec![args.path])
            }
        }
        BuiltinToolKind::ApplyPatch => {
            #[derive(Deserialize)]
            struct ApplyPatchArgs {
                input: String,
            }
            let args: ApplyPatchArgs = serde_json::from_value(args.clone()).ok()?;
            let inputs = expand_apply_patch_to_inputs(&args.input).ok()?;
            let mut paths = BTreeSet::new();
            for input in inputs {
                paths.insert(input.path);
                if let Some(rename) = input.rename {
                    paths.insert(rename);
                }
            }
            Some(paths.into_iter().collect())
        }
        _ => None,
    }
}

fn capture_snapshots(cwd: &Path, paths: &[String]) -> Vec<FileSnapshot> {
    paths
        .iter()
        .filter_map(|path| {
            let absolute = resolve_writable(cwd, path).ok()?;
            snapshot_path(path, &absolute)
        })
        .collect()
}

fn snapshot_path(path: &str, absolute: &Path) -> Option<FileSnapshot> {
    if absolute.exists() {
        let content = fs::read_to_string(absolute).ok()?;
        Some(FileSnapshot {
            path: path.to_string(),
            existed: true,
            content: Some(content),
        })
    } else {
        Some(FileSnapshot {
            path: path.to_string(),
            existed: false,
            content: None,
        })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::BuiltinToolKind;

    #[test]
    fn captures_existing_file_content() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        fs::write(temp.path().join("note.txt"), "before\n").expect("write");
        let batch = capture_edit_batch(
            temp.path(),
            "node-1",
            "call-1",
            "write",
            BuiltinToolKind::Write,
            &serde_json::json!({"path": "note.txt", "content": "after\n"}),
        )
        .expect("batch");
        assert_eq!(batch.snapshots.len(), 1);
        assert_eq!(
            batch.snapshots[0].content.as_deref(),
            Some("before\n")
        );
    }

    #[test]
    fn revert_removes_created_file_before_restoring_source() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        fs::write(temp.path().join("src.txt"), "original\n").expect("write");
        let batch = EditBatch {
            batch_id: "b-move".to_string(),
            node_id: "n1".to_string(),
            tool_call_id: "c1".to_string(),
            tool_name: "apply_patch".to_string(),
            timestamp_ms: 1,
            snapshots: vec![
                FileSnapshot {
                    path: "src.txt".to_string(),
                    existed: true,
                    content: Some("original\n".to_string()),
                },
                FileSnapshot {
                    path: "dest.txt".to_string(),
                    existed: false,
                    content: None,
                },
            ],
        };
        fs::remove_file(temp.path().join("src.txt")).expect("remove source");
        fs::write(temp.path().join("dest.txt"), "moved\n").expect("write dest");
        revert_edit_batch(temp.path(), &batch).expect("revert");
        assert_eq!(
            fs::read_to_string(temp.path().join("src.txt")).expect("read src"),
            "original\n"
        );
        assert!(!temp.path().join("dest.txt").exists());
    }

    #[test]
    fn revert_restores_deleted_create() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let batch = EditBatch {
            batch_id: "b1".to_string(),
            node_id: "n1".to_string(),
            tool_call_id: "c1".to_string(),
            tool_name: "write".to_string(),
            timestamp_ms: 1,
            snapshots: vec![FileSnapshot {
                path: "new.txt".to_string(),
                existed: false,
                content: None,
            }],
        };
        fs::write(temp.path().join("new.txt"), "created\n").expect("write");
        revert_edit_batch(temp.path(), &batch).expect("revert");
        assert!(!temp.path().join("new.txt").exists());
    }
}
