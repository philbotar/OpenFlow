use super::from_event::incident_from_execution_event;
use crate::incident::{IncidentCategory, IncidentContext, IncidentSeverity};
use engine::{NodeId, RunTelemetry};

#[test]
fn node_errored_becomes_node_incident() {
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        ..Default::default()
    };
    let event = RunTelemetry::NodeErrored {
        node_id: NodeId("n1".to_string()),
        label: "Worker".to_string(),
        error: "model refused".to_string(),
    };
    let record = incident_from_execution_event(&event, &ctx).expect("record");
    assert_eq!(record.category, IncidentCategory::Node);
    assert_eq!(record.code, "node.errored");
    assert_eq!(record.severity, IncidentSeverity::Error);
}

#[test]
fn tool_completed_error_becomes_tool_incident() {
    let ctx = IncidentContext {
        run_id: Some("run-1".to_string()),
        workflow_id: Some("wf-1".to_string()),
        ..Default::default()
    };
    let event = RunTelemetry::ToolCompleted {
        node_id: NodeId("n1".to_string()),
        tool_call_id: "tc-1".to_string(),
        tool_name: "read".to_string(),
        content: "[not_found] missing — use grep".to_string(),
        is_error: true,
        output_meta: None,
        artifact_ids: vec![],
    };
    let record = incident_from_execution_event(&event, &ctx).expect("record");
    assert_eq!(record.category, IncidentCategory::Tool);
    assert_eq!(record.code, "tool.not_found");
}

#[test]
fn finished_does_not_emit_incident() {
    let ctx = IncidentContext::default();
    let event = RunTelemetry::Finished(engine::RunReport {
        workflow_id: "wf".into(),
        events: vec![],
        outputs: vec![],
    });
    assert!(incident_from_execution_event(&event, &ctx).is_none());
}
