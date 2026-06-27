//! File read records forwarded to downstream workflow nodes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadRecord {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<String>,
}

/// Merge a read record into `by_path`, keeping the latest outline per path.
pub fn merge_read_record(
    by_path: &mut std::collections::BTreeMap<String, ReadRecord>,
    record: ReadRecord,
) {
    let key = record.path.clone();
    by_path
        .entry(key)
        .and_modify(|existing| {
            if record.outline.is_some() {
                *existing = record.clone();
            }
        })
        .or_insert(record);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_read_record_latest_outline_wins() {
        let mut by_path = std::collections::BTreeMap::new();
        merge_read_record(
            &mut by_path,
            ReadRecord {
                path: "src/lib.rs".to_string(),
                outline: Some("fn old".to_string()),
            },
        );
        merge_read_record(
            &mut by_path,
            ReadRecord {
                path: "src/lib.rs".to_string(),
                outline: Some("fn new".to_string()),
            },
        );
        assert_eq!(
            by_path.get("src/lib.rs").and_then(|r| r.outline.as_deref()),
            Some("fn new")
        );
    }

    #[test]
    fn merge_read_record_keeps_existing_outline_when_later_has_none() {
        let mut by_path = std::collections::BTreeMap::new();
        merge_read_record(
            &mut by_path,
            ReadRecord {
                path: "notes.txt".to_string(),
                outline: Some("1:alpha".to_string()),
            },
        );
        merge_read_record(
            &mut by_path,
            ReadRecord {
                path: "notes.txt".to_string(),
                outline: None,
            },
        );
        assert_eq!(
            by_path.get("notes.txt").and_then(|r| r.outline.as_deref()),
            Some("1:alpha")
        );
    }
}
