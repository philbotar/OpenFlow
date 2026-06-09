//! File mutation records from write-tier edit tools.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChangeOp {
    Create,
    Update,
    Delete,
    Rename,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRecord {
    pub path: String,
    pub op: FileChangeOp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    pub timestamp_ms: u64,
}

/// Dedup key for the current on-disk path represented by a change record.
#[must_use]
pub fn effective_change_path(record: &FileChangeRecord) -> &str {
    if record.op == FileChangeOp::Rename {
        record.rename_to.as_deref().unwrap_or(&record.path)
    } else {
        &record.path
    }
}

/// Merge a file-change record into `by_path`, keeping the latest timestamp per effective path.
/// Rename records drop a stale entry keyed by the source path.
pub fn merge_file_change_record(
    by_path: &mut std::collections::BTreeMap<String, FileChangeRecord>,
    record: FileChangeRecord,
) {
    if record.op == FileChangeOp::Rename {
        if let Some(existing) = by_path.get(&record.path) {
            if record.timestamp_ms >= existing.timestamp_ms {
                by_path.remove(&record.path);
            }
        }
    }
    let key = effective_change_path(&record).to_string();
    by_path
        .entry(key)
        .and_modify(|existing| {
            if record.timestamp_ms >= existing.timestamp_ms {
                *existing = record.clone();
            }
        })
        .or_insert(record);
}

#[must_use]
pub fn summarize_diff(diff: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = diff.lines().take(max_lines).collect();
    let mut summary = lines.join("\n");
    if diff.lines().count() > max_lines {
        summary.push_str("\n…");
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_file_change_record_prefers_rename_destination() {
        let mut by_path = std::collections::BTreeMap::new();
        merge_file_change_record(
            &mut by_path,
            FileChangeRecord {
                path: "old.rs".to_string(),
                op: FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: None,
                timestamp_ms: 1,
            },
        );
        merge_file_change_record(
            &mut by_path,
            FileChangeRecord {
                path: "old.rs".to_string(),
                op: FileChangeOp::Rename,
                rename_to: Some("new.rs".to_string()),
                diff_summary: None,
                batch_id: None,
                timestamp_ms: 2,
            },
        );
        merge_file_change_record(
            &mut by_path,
            FileChangeRecord {
                path: "new.rs".to_string(),
                op: FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: None,
                timestamp_ms: 3,
            },
        );
        assert_eq!(by_path.len(), 1);
        assert_eq!(
            by_path.get("new.rs").expect("entry").op,
            FileChangeOp::Update
        );
    }

    #[test]
    fn summarize_diff_truncates_long_output() {
        let diff = (0..12)
            .map(|index| format!("+line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let summary = summarize_diff(&diff, 8);
        assert!(summary.ends_with('…'));
        assert_eq!(summary.lines().count(), 9);
    }
}
