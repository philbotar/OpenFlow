use engine::{Edge, Node, NodeId, ToolRef, Workflow};
use orchestration::execution::run_workflow_headless;
use orchestration::state::TraceStatus;
use providers::{
    AiClient, AiClientConfig, AuthConfig, OpenAiCompatibleConfig, ProviderAdapterConfig,
    ProviderId, WireApi,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveWorkflowConfig {
    api_key: String,
    model: String,
    base_url: String,
    wire_api: WireApi,
    responses_path: String,
    chat_completions_path: String,
}

fn parse_live_wire_api(value: Option<String>) -> Result<WireApi, String> {
    match value.as_deref().unwrap_or("responses") {
        "responses" => Ok(WireApi::Responses),
        "chat-completions" => Ok(WireApi::ChatCompletions),
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

fn live_client(config: &LiveWorkflowConfig) -> AiClient {
    AiClient::with_config(AiClientConfig {
        provider_id: ProviderId::from("live_openai_compatible"),
        provider_label: "Live OpenAI-compatible".to_string(),
        auth: AuthConfig::Bearer {
            api_key: Some(config.api_key.clone()),
            required: true,
        },
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: config.base_url.clone(),
            wire_api: config.wire_api,
            responses_path: config.responses_path.clone(),
            chat_completions_path: config.chat_completions_path.clone(),
        }),
    })
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

fn compatible_probe_request_body(config: &LiveWorkflowConfig) -> Value {
    json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": "You are running an automated smoke test. Return valid JSON or a tool call."
            },
            {
                "role": "user",
                "content": "Read README.md if needed. Preserve project_code exactly as ORCHID-91. Return a JSON object with project_code and summary."
            }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "read",
                    "description": "Read a file or URL.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    },
                    "strict": true
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "openflow_submit_node_output",
                    "description": "Submit the final structured node output when the task is complete.",
                    "parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "output": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "project_code": { "type": "string" },
                                    "summary": { "type": "string" }
                                },
                                "required": ["project_code", "summary"]
                            },
                            "assistant_message": {
                                "type": ["string", "null"]
                            }
                        },
                        "required": ["output", "assistant_message"]
                    },
                    "strict": true
                }
            }
        ]
    })
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

    let client = live_client(&config);

    let snapshot = run_workflow_headless(
        workflow,
        Some("This is a live smoke test. project_code: ORCHID-91".to_string()),
        client,
        vec![],
        vec![],
        BTreeMap::new(),
        None,
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

#[tokio::test]
#[ignore = "requires STEP_WORKFLOW_LIVE_AI=1 with STEP_WORKFLOW_LIVE_WIRE_API=chat-completions"]
async fn live_chat_completions_provider_returns_supported_message_shape() {
    if env::var("STEP_WORKFLOW_LIVE_AI").as_deref() != Ok("1") {
        eprintln!("skipping live smoke: set STEP_WORKFLOW_LIVE_AI=1 to enable");
        return;
    }

    let config = live_workflow_config_from_env().unwrap_or_else(|error| panic!("{error}"));
    if config.wire_api != WireApi::ChatCompletions {
        eprintln!("skipping live smoke: requires STEP_WORKFLOW_LIVE_WIRE_API=chat-completions");
        return;
    }

    let response = Client::new()
        .post(format!(
            "{}/{}",
            config.base_url.trim_end_matches('/'),
            config.chat_completions_path.trim_start_matches('/')
        ))
        .bearer_auth(&config.api_key)
        .json(&compatible_probe_request_body(&config))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let payload: Value = response.json().await.unwrap();
    assert!(status.is_success(), "provider returned {status}: {payload}");

    let choice = payload["choices"]
        .as_array()
        .and_then(|choices| choices.first())
        .unwrap_or_else(|| panic!("missing choices[0]: {payload}"));
    let message = choice
        .get("message")
        .unwrap_or_else(|| panic!("missing message: {payload}"));
    let supported_shape = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|calls| !calls.is_empty())
        || message.get("function_call").is_some()
        || message
            .get("content")
            .is_some_and(|content| content.is_string() || content.is_array());

    assert!(
        supported_shape,
        "unsupported compatible response shape: {payload}"
    );
}

#[tokio::test]
#[ignore = "requires STEP_WORKFLOW_LIVE_AI=1 with STEP_WORKFLOW_LIVE_WIRE_API=chat-completions"]
async fn live_chat_completions_tool_enabled_workflow_completes() {
    if env::var("STEP_WORKFLOW_LIVE_AI").as_deref() != Ok("1") {
        eprintln!("skipping live smoke: set STEP_WORKFLOW_LIVE_AI=1 to enable");
        return;
    }

    let config = live_workflow_config_from_env().unwrap_or_else(|error| panic!("{error}"));
    if config.wire_api != WireApi::ChatCompletions {
        eprintln!("skipping live smoke: requires STEP_WORKFLOW_LIVE_WIRE_API=chat-completions");
        return;
    }

    let mut workflow = Workflow::new("Live compatible tooling smoke");
    let mut node = live_smoke_node(
        "tooling",
        "Tooling",
        "Use the available tools if needed. Preserve project_code exactly as ORCHID-91 and provide a non-empty summary.",
        &config.model,
    );
    node.agent.tools.catalog.tools = vec![ToolRef {
        name: "read".to_string(),
        tier: Some(engine::ToolTier::Read),
    }];
    workflow.nodes = vec![node];

    let client = live_client(&config);

    let snapshot = run_workflow_headless(
        workflow,
        Some("This is a live smoke test. project_code: ORCHID-91".to_string()),
        client,
        vec![],
        vec![],
        BTreeMap::new(),
        None,
    )
    .await
    .unwrap();

    assert_eq!(snapshot.outputs["tooling"]["project_code"], "ORCHID-91");
    assert!(snapshot.outputs["tooling"]["summary"]
        .as_str()
        .is_some_and(|value| !value.trim().is_empty()));
}

fn vars(items: &[(&str, &str)], key: &str) -> Option<String> {
    items
        .iter()
        .find_map(|(item_key, value)| (*item_key == key).then(|| (*value).to_string()))
}

#[test]
fn live_wire_api_defaults_to_responses() {
    assert_eq!(parse_live_wire_api(None).unwrap(), WireApi::Responses);
}

#[test]
fn live_wire_api_accepts_chat_completions() {
    assert_eq!(
        parse_live_wire_api(Some("chat-completions".to_string())).unwrap(),
        WireApi::ChatCompletions
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
    assert_eq!(config.wire_api, WireApi::Responses);
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
    assert_eq!(config.wire_api, WireApi::ChatCompletions);
    assert_eq!(config.chat_completions_path, "chat/completions");
}
