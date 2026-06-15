use super::recorder::{
    build_record, incident_from_tool_error, IncidentRecorder, NewIncidentRecord,
};
use crate::adapters::storage::incident_store::FileIncidentStore;
use crate::error::BackendError;
use crate::incident::{
    IncidentCategory, IncidentContext, IncidentListOptions, IncidentScope, IncidentSeverity,
    IncidentStore,
};
use crate::tool::errors::ToolError;
use engine::{AgentError, NodeId};
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn tool_timeout_maps_to_retryable_incident() {
    let err = ToolError::Timeout {
        tool: "bash".to_string(),
        after_secs: 300,
        hint: "retry".to_string(),
        partial_output: None,
    };
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        node_id: Some(NodeId("n1".to_string())),
        ..Default::default()
    };
    let record = incident_from_tool_error(&err, "tc-1", &ctx);
    assert_eq!(record.category, IncidentCategory::Tool);
    assert_eq!(record.code, "tool.timeout");
    assert!(record.retryable);
    assert_eq!(record.severity, IncidentSeverity::Error);
}

#[test]
fn agent_transient_maps_to_ai_invoke_incident() {
    let dir = tempdir().unwrap();
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let recorder = IncidentRecorder::new(store);
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        node_id: Some(NodeId("n1".to_string())),
        node_label: Some("Planner".to_string()),
        ..Default::default()
    };
    recorder
        .record_agent_error(&AgentError::Transient("rate limited".to_string()), &ctx)
        .unwrap();
    let listed = recorder.list_unresolved(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].category, IncidentCategory::AiInvoke);
    assert_eq!(listed[0].code, "ai.transient");
    assert!(listed[0].retryable);
}

#[test]
fn backend_error_maps_to_backend_category() {
    let dir = tempdir().unwrap();
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let recorder = IncidentRecorder::new(store);
    recorder
        .record_backend(&BackendError::NoActiveRun, &IncidentContext::default())
        .unwrap();
    let listed = recorder.list_unresolved(10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].category, IncidentCategory::Backend);
    assert_eq!(listed[0].code, "backend.no_active_run");
}

#[test]
fn record_prunes_when_retention_max_exceeded() {
    let dir = tempdir().unwrap();
    let store = Arc::new(FileIncidentStore::new(dir.path().join("incidents.jsonl")));
    let recorder = IncidentRecorder::with_retention_max(store.clone(), 2);

    recorder
        .record(build_record(NewIncidentRecord {
            scope: IncidentScope::App,
            severity: IncidentSeverity::Error,
            category: IncidentCategory::Backend,
            code: "one".to_string(),
            message: "one".to_string(),
            hint: None,
            retryable: false,
            context: Default::default(),
        }))
        .unwrap();
    let first = store
        .list(Some(IncidentListOptions {
            include_resolved: true,
            limit: None,
        }))
        .unwrap()
        .pop()
        .expect("first record");
    store.dismiss(&first.id).unwrap();

    recorder
        .record(build_record(NewIncidentRecord {
            scope: IncidentScope::App,
            severity: IncidentSeverity::Error,
            category: IncidentCategory::Backend,
            code: "two".to_string(),
            message: "two".to_string(),
            hint: None,
            retryable: false,
            context: Default::default(),
        }))
        .unwrap();
    recorder
        .record(build_record(NewIncidentRecord {
            scope: IncidentScope::App,
            severity: IncidentSeverity::Error,
            category: IncidentCategory::Backend,
            code: "three".to_string(),
            message: "three".to_string(),
            hint: None,
            retryable: false,
            context: Default::default(),
        }))
        .unwrap();

    let listed = store
        .list(Some(IncidentListOptions {
            include_resolved: true,
            limit: None,
        }))
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert!(
        listed
            .iter()
            .all(|record| record.code == "two" || record.code == "three"),
        "expected newest records to remain: {:?}",
        listed
    );
}
