use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::incident::{
    IncidentCategory, IncidentListOptions, IncidentRecord, IncidentScope, IncidentSeverity,
    IncidentStore,
};
use tempfile::tempdir;

fn sample_record(id: &str, created_at_ms: u64, resolved: bool) -> IncidentRecord {
    IncidentRecord {
        id: id.to_string(),
        created_at_ms,
        severity: IncidentSeverity::Error,
        category: IncidentCategory::Tool,
        scope: IncidentScope::App,
        code: "tool.failed".to_string(),
        message: "boom".to_string(),
        hint: None,
        retryable: false,
        context: Default::default(),
        resolved,
    }
}

fn sample_record_unresolved(id: &str) -> IncidentRecord {
    sample_record(id, 1, false)
}

#[test]
fn append_and_list_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path.clone());

    store.append(&sample_record_unresolved("a")).unwrap();
    store.append(&sample_record_unresolved("b")).unwrap();

    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "a");
    assert_eq!(listed[1].id, "b");
}

#[test]
fn dismiss_marks_record_resolved() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path);

    store.append(&sample_record_unresolved("x")).unwrap();
    store.dismiss("x").unwrap();

    let listed = store
        .list(Some(IncidentListOptions {
            include_resolved: true,
            limit: None,
        }))
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].resolved);
}

#[test]
fn list_none_excludes_dismissed_incidents() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path);

    store
        .append(&sample_record_unresolved("dismissed"))
        .unwrap();
    store.dismiss("dismissed").unwrap();

    let active = store.list(None).unwrap();
    assert!(active.is_empty());

    let all = store
        .list(Some(IncidentListOptions {
            include_resolved: true,
            limit: None,
        }))
        .unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, "dismissed");
    assert!(all[0].resolved);
}

#[test]
fn clear_resolved_removes_dismissed_rows() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path);

    store.append(&sample_record_unresolved("keep")).unwrap();
    store.append(&sample_record_unresolved("remove")).unwrap();
    store.dismiss("remove").unwrap();

    let removed = store.clear_resolved().unwrap();
    assert_eq!(removed, 1);

    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "keep");
    assert!(!listed[0].resolved);
}

#[test]
fn prune_to_max_drops_oldest_resolved_first() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path);

    store
        .append(&sample_record("old-resolved", 1, true))
        .unwrap();
    store
        .append(&sample_record("new-resolved", 2, true))
        .unwrap();
    store.append(&sample_record_unresolved("active")).unwrap();

    let removed = store.prune_to_max(2).unwrap();
    assert_eq!(removed, 1);

    let listed = store
        .list(Some(IncidentListOptions {
            include_resolved: true,
            limit: None,
        }))
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "new-resolved");
    assert_eq!(listed[1].id, "active");
}

#[test]
fn prune_to_max_drops_oldest_unresolved_when_no_resolved() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path);

    store.append(&sample_record_unresolved("oldest")).unwrap();
    store.append(&sample_record("middle", 2, false)).unwrap();
    store.append(&sample_record("newest", 3, false)).unwrap();

    let removed = store.prune_to_max(2).unwrap();
    assert_eq!(removed, 1);

    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "middle");
    assert_eq!(listed[1].id, "newest");
}
