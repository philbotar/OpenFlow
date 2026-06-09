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
    pub timestamp_ms: u64,
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
