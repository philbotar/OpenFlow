mod support;

use engine::{ApprovalMode, NodeId};
use orchestration::run::execution::WorkflowExecutionError;
use orchestration::run::state::TraceStatus;
use serde_json::json;
use std::time::Duration;
use support::{
    branch_join_workflow, linear_workflow, run_headless_script, single_agent_workflow,
    spawn_interactive_script, HeadlessRunOpts, MockAiStack, MockTurn,
};
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn happy_path_linear_workflow() {
    let ai = MockAiStack::from_invocation_order([
        MockTurn::ok_summary("step-a done"),
        MockTurn::ok_summary("step-b done"),
        MockTurn::ok_summary("step-c done"),
    ]);

    let snapshot = run_headless_script(linear_workflow(), ai, HeadlessRunOpts::default())
        .await
        .expect("linear workflow should complete");

    assert_eq!(snapshot.outputs.len(), 3);
    assert!(
        snapshot
            .run_trace
            .iter()
            .filter(|entry| entry.status == TraceStatus::Completed)
            .count()
            >= 3
    );
    assert_eq!(
        snapshot.outputs[&NodeId("step-c".into())],
        json!({"summary": "step-c done"})
    );
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn branch_join_completes_with_scripted_stack() {
    let ai = MockAiStack::from_invocation_order([
        MockTurn::ok_summary("idea out"),
        MockTurn::ok_summary("plan out"),
        MockTurn::ok_summary("risk out"),
        MockTurn::ok_summary("join out"),
    ]);

    let snapshot = run_headless_script(branch_join_workflow(), ai, HeadlessRunOpts::default())
        .await
        .expect("branch join should complete");

    assert_eq!(snapshot.outputs.len(), 4);
    assert!(
        snapshot
            .run_trace
            .iter()
            .filter(|entry| entry.status == TraceStatus::Completed)
            .count()
            >= 4
    );
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn transient_then_success_auto_retries() {
    let ai = MockAiStack::from_invocation_order([
        MockTurn::transient("timeout"),
        MockTurn::transient("timeout"),
        MockTurn::ok_summary("ok"),
    ]);

    let mut workflow = single_agent_workflow();
    workflow.settings.retry_policy.max_attempts = 2;

    let snapshot = run_headless_script(workflow, ai.clone(), HeadlessRunOpts::default())
        .await
        .expect("should complete after auto-retry");

    assert_eq!(snapshot.outputs.len(), 1);
    assert_eq!(
        snapshot.outputs[&NodeId("first".into())],
        json!({"summary": "ok"})
    );
    assert_eq!(
        ai.recorded_requests().len(),
        3,
        "transient errors should re-invoke"
    );
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn permanent_failure_surfaces_missing_retry() {
    let ai = MockAiStack::from_invocation_order([MockTurn::permanent("boom")]);

    let error = run_headless_script(single_agent_workflow(), ai, HeadlessRunOpts::default())
        .await
        .expect_err("permanent failure should stop headless run");

    assert!(matches!(
        error,
        WorkflowExecutionError::MissingRetry(node_id) if node_id.0 == "first"
    ));
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn exhausted_stack_fails_cleanly() {
    let ai = MockAiStack::empty();

    let error = run_headless_script(single_agent_workflow(), ai, HeadlessRunOpts::default())
        .await
        .expect_err("empty stack should fail");

    assert!(matches!(error, WorkflowExecutionError::MissingRetry(node_id) if node_id.0 == "first"));
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn invalid_output_schema_completes_without_panic() {
    let ai = MockAiStack::from_invocation_order([MockTurn::ok_json(json!({"wrong_field": "x"}))]);

    let snapshot = run_headless_script(single_agent_workflow(), ai, HeadlessRunOpts::default())
        .await
        .expect("run should complete without panic even when output shape differs");

    assert_eq!(
        snapshot.outputs[&NodeId("first".into())],
        json!({"wrong_field": "x"})
    );
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn missing_manual_input_surfaces_typed_error() {
    let ai = MockAiStack::from_invocation_order([MockTurn::ok_summary("unused")]);

    let mut workflow = single_agent_workflow();
    workflow.nodes[0].agent.auto_start = false;

    let error = run_headless_script(
        workflow,
        ai,
        HeadlessRunOpts {
            manual_inputs: Vec::new(),
            ..HeadlessRunOpts::default()
        },
    )
    .await
    .expect_err("manual node without input should fail");

    assert!(matches!(
        error,
        WorkflowExecutionError::MissingManualInput(node_id) if node_id.0 == "first"
    ));
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn missing_approval_surfaces_typed_error() {
    let ai = MockAiStack::from_invocation_order([MockTurn::tool_read("README.md")]);

    let mut workflow = single_agent_workflow();
    workflow.nodes[0].agent.tools.approval_mode = Some(ApprovalMode::AlwaysAsk);

    let error = run_headless_script(workflow, ai, HeadlessRunOpts::default())
        .await
        .expect_err("prompted tool without approval should fail");

    assert!(matches!(error, WorkflowExecutionError::MissingApproval(_)));
}

#[cfg_attr(all(miri, target_os = "macos"), ignore)]
#[tokio::test]
async fn interrupt_during_slow_tool_emits_node_interrupted() {
    let temp = TempDir::new().expect("tempdir");
    let ai = MockAiStack::from_invocation_order([MockTurn::tool_bash("sleep 30", 30)]);

    let mut workflow = single_agent_workflow();
    workflow.nodes[0].agent.tools.approval_mode = Some(ApprovalMode::Yolo);
    let node_id = workflow.nodes[0].id.clone();

    let mut run = spawn_interactive_script(workflow, temp.path().to_path_buf(), ai);

    let mut tool_started = false;
    let mut interrupted = false;
    while let Ok(Some(event)) = timeout(Duration::from_secs(10), run.event_rx.recv()).await {
        match event {
            orchestration::run::execution::ExecutionEvent::ToolStarted { node_id: id, .. }
                if id == node_id =>
            {
                tool_started = true;
                if let Some((_, token)) = run.node_interrupts.lock().get(&node_id) {
                    token.cancel();
                }
            }
            orchestration::run::execution::ExecutionEvent::NodeInterrupted {
                node_id: id, ..
            } if id == node_id => {
                interrupted = true;
                break;
            }
            orchestration::run::execution::ExecutionEvent::Finished(_)
            | orchestration::run::execution::ExecutionEvent::Aborted => break,
            orchestration::run::execution::ExecutionEvent::NodeFailed { node_id: id, .. }
                if id == node_id =>
            {
                break;
            }
            _ => {}
        }
    }

    run.handle.abort();
    assert!(tool_started, "expected bash tool to start before interrupt");
    assert!(
        interrupted,
        "expected NodeInterrupted after per-node cancel"
    );
}
