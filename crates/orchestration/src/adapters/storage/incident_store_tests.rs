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
