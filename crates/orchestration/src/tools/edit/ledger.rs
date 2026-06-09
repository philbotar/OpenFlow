//! Per-tool-call accumulator for file mutations during a run.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::{FileChangeOp, FileChangeRecord};

#[derive(Debug, Clone, Default)]
pub struct FileChangeLedger {
    inner: Arc<Mutex<Vec<FileChangeRecord>>>,
}

impl FileChangeLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &self,
        path: impl Into<String>,
        op: FileChangeOp,
        rename_to: Option<String>,
        diff_summary: Option<String>,
    ) {
        let record = FileChangeRecord {
            path: path.into(),
            op,
            rename_to,
            diff_summary,
            timestamp_ms: now_millis(),
        };
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(record);
    }

    #[must_use]
    pub fn take(&self) -> Vec<FileChangeRecord> {
        std::mem::take(
            &mut *self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}
