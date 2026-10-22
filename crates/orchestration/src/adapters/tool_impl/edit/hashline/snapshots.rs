//! Per-session snapshot store for hashline section tags.

use std::collections::HashMap;

use super::format::compute_file_hash;

const DEFAULT_MAX_PATHS: usize = 30;
const DEFAULT_MAX_VERSIONS_PER_PATH: usize = 4;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub path: String,
    pub text: String,
    pub hash: String,
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
    state: parking_lot::Mutex<SnapshotState>,
    max_paths: usize,
    max_versions_per_path: usize,
}

#[derive(Debug, Default)]
struct SnapshotState {
    versions: HashMap<String, Vec<Snapshot>>,
    lru_paths: Vec<String>,
}

impl InMemorySnapshotStore {
    pub fn new() -> Self {
        Self::with_options(InMemorySnapshotStoreOptions::default())
    }

    pub fn with_options(options: InMemorySnapshotStoreOptions) -> Self {
        Self {
            state: parking_lot::Mutex::new(SnapshotState::default()),
            max_paths: options.max_paths,
            max_versions_per_path: options.max_versions_per_path,
        }
    }
}

impl Default for InMemorySnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySnapshotStore {
    pub fn head(&self, path: &str) -> Option<Snapshot> {
        self.state
            .lock()
            .versions
            .get(path)
            .and_then(|v| v.first().cloned())
    }

    pub fn by_hash(&self, path: &str, hash: &str) -> Option<Snapshot> {
        self.state
            .lock()
            .versions
            .get(path)
            .and_then(|history| history.iter().find(|v| v.hash == hash).cloned())
    }

    pub fn record(&self, path: &str, full_text: &str) -> String {
        let hash = compute_file_hash(full_text);
        // ponytail: one lock keeps eviction and insertion atomic across tool threads.
        let mut state = self.state.lock();
        state.lru_paths.retain(|candidate| candidate != path);
        state.lru_paths.push(path.to_string());
        while state.lru_paths.len() > self.max_paths {
            let evicted = state.lru_paths.remove(0);
            state.versions.remove(&evicted);
        }
        let history = state.versions.entry(path.to_string()).or_default();
        if let Some(index) = history.iter().position(|snapshot| snapshot.hash == hash) {
            let existing = history.remove(index);
            history.insert(0, existing);
            return hash;
        }
        let snapshot = Snapshot {
            path: path.to_string(),
            text: full_text.to_string(),
            hash: hash.clone(),
        };
        history.insert(0, snapshot);
        history.truncate(self.max_versions_per_path);
        hash
    }

    pub fn invalidate(&self, path: &str) {
        let mut state = self.state.lock();
        state.versions.remove(path);
        state.lru_paths.retain(|p| p != path);
    }

    pub fn clear(&self) {
        *self.state.lock() = SnapshotState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_versions_refresh_recency_without_evicting_other_versions() {
        let store = InMemorySnapshotStore::with_options(InMemorySnapshotStoreOptions {
            max_paths: 2,
            max_versions_per_path: 2,
        });
        let first = store.record("a", "first");
        let second = store.record("a", "second");
        store.record("b", "other");
        assert_eq!(store.record("a", "first"), first);
        assert_eq!(store.head("a").unwrap().hash, first);
        assert!(store.by_hash("a", &second).is_some());
        store.record("c", "third path");
        assert!(store.head("b").is_none());
        store.record("a", "third version");
        assert!(store.by_hash("a", &second).is_none());
        assert!(store.by_hash("a", &first).is_some());
        store.invalidate("a");
        assert!(store.head("a").is_none());
        store.clear();
        assert!(store.head("c").is_none());
    }

    #[test]
    fn concurrent_records_preserve_the_path_limit() {
        for _ in 0..20 {
            let store = InMemorySnapshotStore::with_options(InMemorySnapshotStoreOptions {
                max_paths: 1,
                max_versions_per_path: 2,
            });
            let barrier = std::sync::Barrier::new(16);
            std::thread::scope(|scope| {
                for index in 0..16 {
                    let store = &store;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        for version in 0..100 {
                            store.record(&index.to_string(), &version.to_string());
                        }
                    });
                }
            });
            let retained = (0..16)
                .filter(|index| store.head(&index.to_string()).is_some())
                .count();
            assert_eq!(retained, 1);
        }
    }
}
