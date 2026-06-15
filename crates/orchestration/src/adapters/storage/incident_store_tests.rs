use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::incident::{
    IncidentCategory, IncidentListOptions, IncidentRecord, IncidentScope, IncidentSeverity,
    IncidentStore,
};
use tempfile::tempdir;

fn sample_record(id: &str) -> IncidentRecord {
    IncidentRecord {
        id: id.to_string(),
        created_at_ms: 1,
        severity: IncidentSeverity::Error,
        category: IncidentCategory::Tool,
        scope: IncidentScope::App,
        code: "tool.failed".to_string(),
        message: "boom".to_string(),
        hint: None,
        retryable: false,
        context: Default::default(),
        resolved: false,
    }
}

#[test]
fn append_and_list_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("incidents.jsonl");
    let store = FileIncidentStore::new(path.clone());

    store.append(&sample_record("a")).unwrap();
    store.append(&sample_record("b")).unwrap();

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

    store.append(&sample_record("x")).unwrap();
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

    store.append(&sample_record("dismissed")).unwrap();
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

    store.append(&sample_record("keep")).unwrap();
    store.append(&sample_record("remove")).unwrap();
    store.dismiss("remove").unwrap();

    let removed = store.clear_resolved().unwrap();
    assert_eq!(removed, 1);

    let listed = store.list(None).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "keep");
    assert!(!listed[0].resolved);
}
