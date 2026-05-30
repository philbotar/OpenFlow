use agent_workflow_app::execution::run_workflow_headless;
use agent_workflow_app::state::TraceStatus;
use openai_client::{OpenAiClient, OpenAiClientConfig, OpenAiWireApi};
use serde_json::json;
use std::env;
use workflow_core::{Edge, Node, NodeId, Workflow};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveWorkflowConfig {
    api_key: String,
    model: String,
    base_url: String,
    wire_api: OpenAiWireApi,
    responses_path: String,
    chat_completions_path: String,
}

fn parse_live_wire_api(value: Option<String>) -> Result<OpenAiWireApi, String> {
    match value.as_deref().unwrap_or("responses") {
        "responses" => Ok(OpenAiWireApi::Responses),
        "chat-completions" => Ok(OpenAiWireApi::ChatCompletions),
        other => Err(format!(
            "STEP_WORKFLOW_LIVE_WIRE_API must be 'responses' or 'chat-completions', got '{other}'"
        )),
    }
}

fn live_workflow_config_from_vars(
    get: impl Fn(&str) -> Option<String>,
) -> Result<LiveWorkflowConfig, String> {
    let api_key = get("STEP_WORKFLOW_LIVE_API_KEY")
        .or_else(|| get("OPENAI_API_KEY"))
        .ok_or_else(|| "STEP_WORKFLOW_LIVE_API_KEY or OPENAI_API_KEY must be set".to_string())?;
    let model = get("STEP_WORKFLOW_LIVE_MODEL").ok_or_else(|| {
        "STEP_WORKFLOW_LIVE_MODEL must be set to the model under test".to_string()
    })?;
    let base_url =
        get("STEP_WORKFLOW_LIVE_BASE_URL").unwrap_or_else(|| "https://api.openai.com".to_string());
    let wire_api = parse_live_wire_api(get("STEP_WORKFLOW_LIVE_WIRE_API"))?;
    let responses_path =
        get("STEP_WORKFLOW_LIVE_RESPONSES_PATH").unwrap_or_else(|| "v1/responses".to_string());
    let chat_completions_path = get("STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH")
        .unwrap_or_else(|| "v1/chat/completions".to_string());

    Ok(LiveWorkflowConfig {
        api_key,
        model,
        base_url,
        wire_api,
        responses_path,
        chat_completions_path,
    })
}

fn live_workflow_config_from_env() -> Result<LiveWorkflowConfig, String> {
    live_workflow_config_from_vars(|key| env::var(key).ok())
}

fn live_smoke_node(id: &str, label: &str, task: &str, model: &str) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = NodeId(id.to_string());
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
#[ignore = "requires STEP_WORKFLOW_LIVE_AI=1, STEP_WORKFLOW_LIVE_API_KEY or OPENAI_API_KEY, and STEP_WORKFLOW_LIVE_MODEL"]
async fn live_openai_workflow_preserves_sentinel_and_schema_contract() {
    if env::var("STEP_WORKFLOW_LIVE_AI").as_deref() != Ok("1") {
        eprintln!("skipping live smoke: set STEP_WORKFLOW_LIVE_AI=1 to enable");
        return;
    }

    let config = live_workflow_config_from_env().unwrap_or_else(|error| panic!("{error}"));

    let mut workflow = Workflow::new("Live AI smoke");
    workflow.nodes = vec![
        live_smoke_node(
            "extract",
            "Extract sentinel",
            "Read the entrypoint and return project_code exactly as ORCHID-91 with a short summary.",
            &config.model,
        ),
        live_smoke_node(
            "summarize",
            "Summarize",
            "Use upstream JSON. Preserve project_code exactly and summarize the upstream output.",
            &config.model,
        ),
    ];
    workflow.edges = vec![Edge::new("extract", "summarize")];

    let client = OpenAiClient::with_config(OpenAiClientConfig {
        api_key: config.api_key,
        base_url: config.base_url,
        wire_api: config.wire_api,
        responses_path: config.responses_path,
        chat_completions_path: config.chat_completions_path,
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

fn vars(items: &[(&str, &str)], key: &str) -> Option<String> {
    items
        .iter()
        .find_map(|(item_key, value)| (*item_key == key).then(|| (*value).to_string()))
}

#[test]
fn live_wire_api_defaults_to_responses() {
    assert_eq!(parse_live_wire_api(None).unwrap(), OpenAiWireApi::Responses);
}

#[test]
fn live_wire_api_accepts_chat_completions() {
    assert_eq!(
        parse_live_wire_api(Some("chat-completions".to_string())).unwrap(),
        OpenAiWireApi::ChatCompletions
    );
}

#[test]
fn live_wire_api_rejects_unknown_values() {
    let error = parse_live_wire_api(Some("completion".to_string())).unwrap_err();
    assert_eq!(
        error,
        "STEP_WORKFLOW_LIVE_WIRE_API must be 'responses' or 'chat-completions', got 'completion'"
    );
}

#[test]
fn live_config_prefers_live_api_key_and_defaults_openai_paths() {
    let items = [
        ("STEP_WORKFLOW_LIVE_API_KEY", "live-key"),
        ("OPENAI_API_KEY", "openai-key"),
        ("STEP_WORKFLOW_LIVE_MODEL", "gpt-test"),
    ];

    let config = live_workflow_config_from_vars(|key| vars(&items, key)).unwrap();

    assert_eq!(config.api_key, "live-key");
    assert_eq!(config.model, "gpt-test");
    assert_eq!(config.base_url, "https://api.openai.com");
    assert_eq!(config.wire_api, OpenAiWireApi::Responses);
    assert_eq!(config.responses_path, "v1/responses");
    assert_eq!(config.chat_completions_path, "v1/chat/completions");
}

#[test]
fn live_config_supports_deepinfra_chat_completions_path() {
    let items = [
        ("OPENAI_API_KEY", "fallback-key"),
        ("STEP_WORKFLOW_LIVE_MODEL", "deepseek-ai/DeepSeek-V4-Flash"),
        (
            "STEP_WORKFLOW_LIVE_BASE_URL",
            "https://api.deepinfra.com/v1/openai",
        ),
        ("STEP_WORKFLOW_LIVE_WIRE_API", "chat-completions"),
        (
            "STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH",
            "chat/completions",
        ),
    ];

    let config = live_workflow_config_from_vars(|key| vars(&items, key)).unwrap();

    assert_eq!(config.api_key, "fallback-key");
    assert_eq!(config.base_url, "https://api.deepinfra.com/v1/openai");
    assert_eq!(config.wire_api, OpenAiWireApi::ChatCompletions);
    assert_eq!(config.chat_completions_path, "chat/completions");
}
