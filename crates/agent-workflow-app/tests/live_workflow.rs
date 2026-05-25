use agent_workflow_app::execution::run_workflow_headless;
use agent_workflow_app::state::TraceStatus;
use openai_client::{OpenAiClient, OpenAiClientConfig, OpenAiWireApi};
use serde_json::json;
use std::env;
use workflow_core::{Edge, Node, Workflow};

fn live_smoke_node(id: &str, label: &str, task: &str, model: &str) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = id.to_string();
    node.agent.model = model.to_string();
    node.agent.system_prompt = concat!(
        "You are running an automated smoke test. ",
        "Return only valid JSON matching the schema. ",
        "Preserve the project_code exactly when it is present."
    )
    .to_string();
    node.agent.task_prompt = task.to_string();
    node.agent.output_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "project_code": { "type": "string" },
            "summary": { "type": "string" }
        },
        "required": ["project_code", "summary"]
    });
    node
}

#[tokio::test]
#[ignore = "requires STEP_WORKFLOW_LIVE_AI=1, OPENAI_API_KEY, and STEP_WORKFLOW_LIVE_MODEL"]
async fn live_openai_workflow_preserves_sentinel_and_schema_contract() {
    if env::var("STEP_WORKFLOW_LIVE_AI").as_deref() != Ok("1") {
        eprintln!("skipping live smoke: set STEP_WORKFLOW_LIVE_AI=1 to enable");
        return;
    }

    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let model = env::var("STEP_WORKFLOW_LIVE_MODEL")
        .expect("STEP_WORKFLOW_LIVE_MODEL must be set to the model under test");
    let base_url =
        env::var("STEP_WORKFLOW_LIVE_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".into());

    let mut workflow = Workflow::new("Live AI smoke");
    workflow.nodes = vec![
        live_smoke_node(
            "extract",
            "Extract sentinel",
            "Read the entrypoint and return project_code exactly as ORCHID-91 with a short summary.",
            &model,
        ),
        live_smoke_node(
            "summarize",
            "Summarize",
            "Use upstream JSON. Preserve project_code exactly and summarize the upstream output.",
            &model,
        ),
    ];
    workflow.edges = vec![Edge::new("extract", "summarize")];

    let client = OpenAiClient::with_config(OpenAiClientConfig {
        api_key,
        base_url,
        wire_api: OpenAiWireApi::Responses,
    });

    let snapshot = run_workflow_headless(
        workflow,
        Some("This is a live smoke test. project_code: ORCHID-91".to_string()),
        client,
        vec![],
    )
    .await
    .unwrap();

    assert_eq!(snapshot.outputs["extract"]["project_code"], "ORCHID-91");
    assert_eq!(snapshot.outputs["summarize"]["project_code"], "ORCHID-91");
    assert!(snapshot.outputs["summarize"]["summary"]
        .as_str()
        .is_some_and(|value| !value.trim().is_empty()));
    assert_eq!(
        snapshot
            .run_trace
            .iter()
            .filter(|entry| entry.status == TraceStatus::Completed)
            .count(),
        2
    );
}
