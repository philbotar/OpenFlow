use super::model::{IncidentCategory, IncidentRecord, IncidentScope, IncidentSeverity};
use engine::NodeId;
use std::collections::BTreeMap;

#[test]
fn incident_record_serializes_camel_case_for_ipc() {
    let record = IncidentRecord {
        id: "inc-1".to_string(),
        created_at_ms: 1_700_000_000_000,
        severity: IncidentSeverity::Error,
        category: IncidentCategory::Tool,
        scope: IncidentScope::Node {
            run_id: "run-1".to_string(),
            workflow_id: "wf-1".to_string(),
            node_id: NodeId("n1".to_string()),
        },
        code: "tool.timeout".to_string(),
        message: "[timeout] bash timed out after 300s".to_string(),
        hint: Some("increase timeout".to_string()),
        retryable: true,
        context: BTreeMap::from([
            ("toolName".to_string(), serde_json::json!("bash")),
            ("toolCallId".to_string(), serde_json::json!("tc-1")),
        ]),
        resolved: false,
    };
    let json = serde_json::to_value(&record).expect("serialize");
    assert_eq!(json["severity"], "error");
    assert_eq!(json["category"], "tool");
    assert_eq!(json["scope"]["type"], "node");
    assert_eq!(json["scope"]["runId"], "run-1");
    assert_eq!(json["retryable"], true);
    assert_eq!(json["resolved"], false);
}

#[test]
fn incident_record_deserializes_from_jsonl_line() {
    let record = IncidentRecord {
        id: "inc-2".to_string(),
        created_at_ms: 1_700_000_000_001,
        severity: IncidentSeverity::Warning,
        category: IncidentCategory::Run,
        scope: IncidentScope::Run {
            run_id: "run-2".to_string(),
            workflow_id: "wf-2".to_string(),
        },
        code: "run.stalled".to_string(),
        message: "run stalled".to_string(),
        hint: None,
        retryable: false,
        context: Default::default(),
        resolved: false,
    };
    let line = serde_json::to_string(&record).expect("serialize");
    let parsed: IncidentRecord = serde_json::from_str(&line).expect("deserialize");
    assert_eq!(parsed, record);
    let json: serde_json::Value = serde_json::from_str(&line).expect("parse json");
    assert_eq!(json["scope"]["runId"], "run-2");
    assert_eq!(json["scope"]["workflowId"], "wf-2");
}
