//! Per-session snapshot store for hashline section tags.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::format::compute_file_hash;

const DEFAULT_MAX_PATHS: usize = 30;
const DEFAULT_MAX_VERSIONS_PER_PATH: usize = 4;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub path: String,
    pub text: String,
    pub hash: String,
    pub recorded_at: u64,
}

pub trait SnapshotStore: Send + Sync {
    fn head(&self, path: &str) -> Option<Snapshot>;
    fn by_hash(&self, path: &str, hash: &str) -> Option<Snapshot>;
    fn record(&self, path: &str, full_text: &str) -> String;
    fn invalidate(&self, path: &str);
    fn clear(&self);
}

#[derive(Debug, Clone)]
pub struct InMemorySnapshotStoreOptions {
    pub max_paths: usize,
    pub max_versions_per_path: usize,
}

impl Default for InMemorySnapshotStoreOptions {
    fn default() -> Self {
        Self {
            max_paths: DEFAULT_MAX_PATHS,
            max_versions_per_path: DEFAULT_MAX_VERSIONS_PER_PATH,
        }
    }
}

/// In-memory snapshot store with bounded path LRU and per-path version ring.
#[derive(Debug)]
pub struct InMemorySnapshotStore {
    versions: parking_lot::Mutex<HashMap<String, Vec<Snapshot>>>,
    lru_paths: parking_lot::Mutex<Vec<String>>,
    max_paths: usize,
    max_versions_per_path: usize,
}

impl InMemorySnapshotStore {
    pub fn new() -> Self {
        Self::with_options(InMemorySnapshotStoreOptions::default())
    }

    pub fn with_options(options: InMemorySnapshotStoreOptions) -> Self {
        Self {
            versions: parking_lot::Mutex::new(HashMap::new()),
            lru_paths: parking_lot::Mutex::new(Vec::new()),
            max_paths: options.max_paths,
            max_versions_per_path: options.max_versions_per_path,
        }
    }

    fn touch_path(&self, path: &str) {
        let mut lru = self.lru_paths.lock();
        lru.retain(|p| p != path);
        lru.push(path.to_string());
        while lru.len() > self.max_paths {
            if let Some(evicted) = lru.first().cloned() {
                lru.remove(0);
                self.versions.lock().remove(&evicted);
            }
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn head(&self, path: &str) -> Option<Snapshot> {
        self.versions
            .lock()
            .get(path)
            .and_then(|v| v.first().cloned())
    }

    fn by_hash(&self, path: &str, hash: &str) -> Option<Snapshot> {
        self.versions
            .lock()
            .get(path)
            .and_then(|history| history.iter().find(|v| v.hash == hash).cloned())
    }

    fn record(&self, path: &str, full_text: &str) -> String {
        let hash = compute_file_hash(full_text);
        self.touch_path(path);
        let mut versions = self.versions.lock();
        let history = versions.entry(path.to_string()).or_default();
        if let Some(existing) = history.iter_mut().find(|v| v.hash == hash) {
            existing.recorded_at = Self::now_ms();
            let existing = existing.clone();
            let filtered: Vec<_> = history.iter().filter(|v| v.hash != hash).cloned().collect();
            *history = std::iter::once(existing).chain(filtered).collect();
            return hash;
        }
        let snapshot = Snapshot {
            path: path.to_string(),
            text: full_text.to_string(),
            hash: hash.clone(),
            recorded_at: Self::now_ms(),
        };
        history.insert(0, snapshot);
        history.truncate(self.max_versions_per_path);
        hash
    }

    fn invalidate(&self, path: &str) {
        self.versions.lock().remove(path);
        self.lru_paths.lock().retain(|p| p != path);
    }

    fn clear(&self) {
        self.versions.lock().clear();
        self.lru_paths.lock().clear();
    }
}
