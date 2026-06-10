//! Per-run cache of read-only tool results.
//!
//! Identical read-tier tool calls repeated later in a run (typically by a
//! downstream node re-orienting itself) are served from this cache instead of
//! re-executing, with a provenance note so the model knows the result is
//! unchanged. Entries validate before every hit: file reads against an
//! mtime+len stamp, multi-file results (search/find/ast_grep/directory reads)
//! against a write epoch bumped whenever a write-capable tool runs or an edit
//! batch is reverted.

use engine::ToolOutputMeta;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Default)]
pub struct ToolResultCache {
    write_epoch: AtomicU64,
    entries: Mutex<HashMap<String, CacheEntry>>,
}

/// How a cache entry proves it is still current at hit time.
#[derive(Debug, Clone)]
pub enum CacheValidation {
    /// Valid while the file keeps the same modification time and size.
    FileStamp {
        path: PathBuf,
        modified: SystemTime,
        len: u64,
    },
    /// Valid while no write-capable tool has run since insertion.
    WriteEpoch(u64),
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Transcript the original result was returned into (node id for node
    /// turns, a unique session id for subagent invocations).
    pub conversation_id: String,
    /// Node that triggered the original call (for provenance notes).
    pub node_id: String,
    /// Tool call id of the original invocation.
    pub tool_call_id: String,
    pub content: String,
    pub artifact_ids: Vec<String>,
    pub output_meta: Option<ToolOutputMeta>,
    pub validation: CacheValidation,
}

#[derive(Debug, Clone)]
pub struct CacheHit {
    pub content: String,
    pub artifact_ids: Vec<String>,
    pub output_meta: Option<ToolOutputMeta>,
    pub node_id: String,
    pub tool_call_id: String,
    /// The requesting transcript already contains this result verbatim.
    pub same_conversation: bool,
}

impl ToolResultCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_epoch(&self) -> u64 {
        self.write_epoch.load(Ordering::SeqCst)
    }

    /// Invalidate all epoch-validated entries (any tool or revert that may
    /// have changed files under the execution folder).
    pub fn bump_write_epoch(&self) {
        self.write_epoch.fetch_add(1, Ordering::SeqCst);
    }

    pub fn lookup(&self, key: &str, conversation_id: &str) -> Option<CacheHit> {
        let mut entries = self.entries.lock();
        let entry = entries.get(key)?;
        if !self.is_valid(&entry.validation) {
            entries.remove(key);
            return None;
        }
        Some(CacheHit {
            content: entry.content.clone(),
            artifact_ids: entry.artifact_ids.clone(),
            output_meta: entry.output_meta.clone(),
            node_id: entry.node_id.clone(),
            tool_call_id: entry.tool_call_id.clone(),
            same_conversation: entry.conversation_id == conversation_id,
        })
    }

    pub fn insert(&self, key: String, entry: CacheEntry) {
        self.entries.lock().insert(key, entry);
    }

    fn is_valid(&self, validation: &CacheValidation) -> bool {
        match validation {
            CacheValidation::FileStamp {
                path,
                modified,
                len,
            } => std::fs::metadata(path).is_ok_and(|meta| {
                meta.is_file()
                    && meta.len() == *len
                    && meta.modified().ok().as_ref() == Some(modified)
            }),
            CacheValidation::WriteEpoch(epoch) => *epoch == self.current_epoch(),
        }
    }
}

/// Cache key for a tool invocation. `serde_json::Value` objects are
/// `BTreeMap`-backed (no `preserve_order` feature), so serialization is
/// canonical with sorted keys.
#[must_use]
pub fn cache_key(tool_name: &str, arguments: &Value) -> String {
    format!("{tool_name}\u{1f}{arguments}")
}
