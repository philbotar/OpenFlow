use engine::{summarize_diff, FileChangeOp, FileChangeRecord};

const DIFF_SUMMARY_LINES: usize = 8;

/// A file change while its exact diff still lives in process memory.
///
/// The runner persists `diff` as an immutable run artifact before exposing
/// `record` to the engine or UI projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedFileChange {
    pub record: FileChangeRecord,
    pub diff: Option<String>,
}

impl CapturedFileChange {
    pub(crate) fn new(
        path: String,
        op: FileChangeOp,
        rename_to: Option<String>,
        diff: Option<String>,
        timestamp_ms: u64,
    ) -> Self {
        let diff_summary = diff
            .as_deref()
            .map(|value| summarize_diff(value, DIFF_SUMMARY_LINES));
        Self {
            record: FileChangeRecord {
                path,
                op,
                rename_to,
                diff_summary,
                batch_id: None,
                tool_call_id: None,
                tool_name: None,
                diff_artifact_id: None,
                diff_size_bytes: None,
                timestamp_ms,
            },
            diff,
        }
    }
}
