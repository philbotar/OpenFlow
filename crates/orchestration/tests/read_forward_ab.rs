//! A/B harness: upstream read outline forwarding vs redundant downstream reads.

mod support;

use engine::{Edge, NodeId, Workflow};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use support::{agent_node, run_headless_script, HeadlessRunOpts, MockAiStack, MockTurn};
use tempfile::TempDir;

fn read_forward_workflow(forward_upstream_reads: bool) -> Workflow {
    let mut workflow = Workflow::new("read-forward-ab");
    workflow.settings.forward_upstream_reads = forward_upstream_reads;
    workflow.nodes = vec![
        agent_node("reader", "reader"),
        agent_node("consumer", "consumer"),
    ];
    workflow.edges = vec![Edge::new("reader", "consumer")];
    workflow
}

fn write_fixture(cwd: &Path) {
    fs::write(cwd.join("lib.rs"), "pub fn hello() {}\n").expect("write fixture");
}

#[derive(Debug)]
struct AbReport {
    read_calls: u32,
    redundant_reads: u32,
    tokens_in: u32,
    outputs: serde_json::Map<String, serde_json::Value>,
}

async fn run_ab_variant(forward_upstream_reads: bool, cwd: PathBuf) -> AbReport {
    let ai = if forward_upstream_reads {
        MockAiStack::from_invocation_order([
            MockTurn::tool_read("lib.rs"),
            MockTurn::ok_json(json!({"done": true})),
            MockTurn::ok_json(json!({"summary": "consumed"})),
        ])
    } else {
        MockAiStack::from_invocation_order([
            MockTurn::tool_read("lib.rs"),
            MockTurn::ok_json(json!({"done": true})),
            MockTurn::tool_read("lib.rs"),
            MockTurn::ok_json(json!({"summary": "consumed"})),
        ])
    };

    let snapshot = run_headless_script(
        read_forward_workflow(forward_upstream_reads),
        ai,
        HeadlessRunOpts {
            cwd: Some(cwd),
            ..HeadlessRunOpts::default()
        },
    )
    .await
    .expect("headless run");

    AbReport {
        read_calls: snapshot.report.read_calls,
        redundant_reads: snapshot.report.redundant_reads,
        tokens_in: snapshot.report.tokens_in,
        outputs: snapshot
            .outputs
            .into_iter()
            .map(|(id, value)| (id.0, value))
            .collect(),
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn downstream_input_includes_upstream_read_outline() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());

    let ai = MockAiStack::from_invocation_order([
        MockTurn::tool_read("lib.rs"),
        MockTurn::ok_json(json!({"done": true})),
        MockTurn::ok_json(json!({"summary": "consumed"})),
    ]);

    let _snapshot = run_headless_script(
        read_forward_workflow(true),
        ai.clone(),
        HeadlessRunOpts {
            cwd: Some(dir.path().to_path_buf()),
            ..HeadlessRunOpts::default()
        },
    )
    .await
    .expect("run");

    let consumer_request = ai
        .recorded_requests()
        .into_iter()
        .find(|request| request.node_id == NodeId("consumer".into()))
        .expect("consumer request");
    let reads = consumer_request.input["reads"]
        .as_array()
        .expect("reads array");
    assert_eq!(reads[0]["path"], "lib.rs");
    assert!(reads[0]["outline"]
        .as_str()
        .is_some_and(|outline| outline.contains("pub fn hello")));
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn ab_harness_reports_redundant_read_drop_with_identical_outputs() {
    let dir = TempDir::new().expect("tempdir");
    let cwd = dir.path().to_path_buf();
    write_fixture(&cwd);

    let control = run_ab_variant(false, cwd.clone()).await;
    let treatment = run_ab_variant(true, cwd).await;

    eprintln!(
        "read-forward A/B: control={{read_calls:{}, redundant_reads:{}, tokens_in:{}}} treatment={{read_calls:{}, redundant_reads:{}, tokens_in:{}}}",
        control.read_calls,
        control.redundant_reads,
        control.tokens_in,
        treatment.read_calls,
        treatment.redundant_reads,
        treatment.tokens_in
    );

    assert_eq!(control.outputs, treatment.outputs);
    assert!(control.redundant_reads > treatment.redundant_reads);
    assert_eq!(treatment.redundant_reads, 0);
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
async fn downstream_raw_read_still_dispatched_when_bytes_needed() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());

    let ai = MockAiStack::from_invocation_order([
        MockTurn::tool_read("lib.rs"),
        MockTurn::ok_json(json!({"done": true})),
        MockTurn::tool_read("lib.rs:raw"),
        MockTurn::ok_json(json!({"summary": "consumed"})),
    ]);

    let snapshot = run_headless_script(
        read_forward_workflow(true),
        ai,
        HeadlessRunOpts {
            cwd: Some(dir.path().to_path_buf()),
            ..HeadlessRunOpts::default()
        },
    )
    .await
    .expect("run");

    assert_eq!(snapshot.report.read_calls, 2);
    assert_eq!(
        snapshot.outputs[&NodeId("consumer".into())],
        json!({"summary": "consumed"})
    );
}
