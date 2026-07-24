//! Live-LLM A/B for upstream read outline forwarding.

mod support;

use engine::{Edge, Node, NodeId, Workflow};
use orchestration::run::execution::run_workflow_headless;
use providers::{
    AiClient, AiClientConfig, AuthConfig, OpenAiCompatibleConfig, ProviderAdapterConfig,
    ProviderId, WireApi,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use support::{agent_node, spawn_interactive_script};
use tempfile::TempDir;

#[derive(Debug)]
struct LiveAbReport {
    read_calls: u32,
    redundant_reads: u32,
    tokens_in: u32,
    consumer_summary: String,
}

struct LiveProviderConfig {
    api_key: String,
    model: String,
    base_url: String,
    wire_api: WireApi,
    responses_path: String,
}

fn live_config_from_env() -> Result<LiveProviderConfig, String> {
    let opencode_key = env::var("OPENCODE_ZEN_API_KEY")
        .or_else(|_| env::var("OPENCODE_API_KEY"))
        .ok();
    let openai_key = env::var("STEP_WORKFLOW_LIVE_API_KEY")
        .or_else(|_| env::var("OPENAI_API_KEY"))
        .ok();

    let (api_key, default_base_url, default_model, default_responses_path) = if let Some(key) =
        opencode_key
    {
        (
            key,
            "https://opencode.ai/zen".to_string(),
            "deepseek-v4-flash-free".to_string(),
            "v1/responses".to_string(),
        )
    } else if let Some(key) = openai_key {
        (
            key,
            "https://api.openai.com".to_string(),
            "gpt-4o-mini".to_string(),
            "v1/responses".to_string(),
        )
    } else {
        return Err(
                "set OPENCODE_ZEN_API_KEY, OPENCODE_API_KEY, STEP_WORKFLOW_LIVE_API_KEY, or OPENAI_API_KEY"
                    .to_string(),
            );
    };

    let model = env::var("STEP_WORKFLOW_LIVE_MODEL").unwrap_or(default_model);
    let base_url = env::var("STEP_WORKFLOW_LIVE_BASE_URL").unwrap_or(default_base_url);
    let responses_path =
        env::var("STEP_WORKFLOW_LIVE_RESPONSES_PATH").unwrap_or(default_responses_path);
    let wire_api = match env::var("STEP_WORKFLOW_LIVE_WIRE_API").as_deref() {
        Ok("chat-completions") => WireApi::ChatCompletions,
        Ok("responses") => WireApi::Responses,
        // Zen Responses API fails on tool-result round-trips for deepseek models.
        _ if base_url.contains("opencode.ai/zen") => WireApi::ChatCompletions,
        _ => WireApi::Responses,
    };

    Ok(LiveProviderConfig {
        api_key,
        model,
        base_url,
        wire_api,
        responses_path,
    })
}

fn live_client(config: &LiveProviderConfig) -> AiClient {
    AiClient::with_config(AiClientConfig {
        provider_id: ProviderId::from("live_read_forward"),
        provider_label: "Live read-forward A/B".to_string(),
        auth: AuthConfig::Bearer {
            api_key: Some(config.api_key.clone()),
            required: true,
        },
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url: config.base_url.clone(),
            wire_api: config.wire_api,
            responses_path: config.responses_path.clone(),
            chat_completions_path: env::var("STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH")
                .unwrap_or_else(|_| "v1/chat/completions".to_string()),
            model_transports: BTreeMap::default(),
            request_timeout: std::time::Duration::from_mins(5),
        }),
        debug_output: false,
    })
}

fn read_forward_workflow(model: &str, forward_upstream_reads: bool) -> Workflow {
    let mut workflow = Workflow::new("read-forward-live");
    workflow.settings.forward_upstream_reads = forward_upstream_reads;
    workflow.nodes = vec![
        live_node(
            "reader",
            "Reader",
            model,
            "Read lib.rs in the execution folder. Submit a one-sentence summary naming the public function you found.",
        ),
        live_node(
            "consumer",
            "Consumer",
            model,
            "Describe what is in lib.rs using upstream output and your input JSON. \
             If reads[] already lists lib.rs with an outline, use that to orient — \
             do not call read on lib.rs unless you need exact file bytes beyond the outline. \
             Submit a one-sentence summary.",
        ),
    ];
    workflow.edges = vec![Edge::new("reader", "consumer")];
    workflow
}

fn live_node(id: &str, label: &str, model: &str, task: &str) -> Node {
    let mut node = agent_node(id, label);
    node.agent.model = model.to_string();
    node.agent.task_prompt = task.to_string();
    node.agent.system_prompt = concat!(
        "You are in an automated read-forwarding experiment. ",
        "Use tools when needed. Finish by calling openflow_submit_node_output once."
    )
    .to_string();
    node
}

fn write_fixture(cwd: &Path) {
    fs::write(cwd.join("lib.rs"), "pub fn hello() {}\n").expect("write fixture");
}

async fn run_live_variant(
    client: AiClient,
    model: &str,
    forward_upstream_reads: bool,
    cwd: PathBuf,
) -> Result<LiveAbReport, String> {
    let snapshot = run_workflow_headless(
        read_forward_workflow(model, forward_upstream_reads),
        None,
        client,
        vec![],
        vec![],
        BTreeMap::new(),
        Some(cwd),
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    let consumer_summary = snapshot
        .outputs
        .get(&NodeId("consumer".into()))
        .and_then(|output| output["summary"].as_str())
        .unwrap_or("")
        .to_string();

    Ok(LiveAbReport {
        read_calls: snapshot.report.read_calls,
        redundant_reads: snapshot.report.redundant_reads,
        tokens_in: snapshot.report.tokens_in,
        consumer_summary,
    })
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "requires STEP_WORKFLOW_LIVE_AI=1, API key, and STEP_WORKFLOW_LIVE_MODEL"]
async fn live_read_forward_ab_reports_metrics() {
    if env::var("STEP_WORKFLOW_LIVE_AI").as_deref() != Ok("1") {
        eprintln!("skipping: set STEP_WORKFLOW_LIVE_AI=1");
        return;
    }

    let config = live_config_from_env().unwrap_or_else(|error| panic!("{error}"));
    eprintln!(
        "live provider: base_url={} model={} wire={:?}",
        config.base_url, config.model, config.wire_api
    );
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    let cwd = dir.path().to_path_buf();

    let control = run_live_variant(live_client(&config), &config.model, false, cwd.clone())
        .await
        .expect("control run");
    let treatment = run_live_variant(live_client(&config), &config.model, true, cwd)
        .await
        .expect("treatment run");

    eprintln!("live read-forward A/B:");
    eprintln!(
        "  control:   read_calls={} redundant_reads={} tokens_in={} summary={:?}",
        control.read_calls, control.redundant_reads, control.tokens_in, control.consumer_summary
    );
    eprintln!(
        "  treatment: read_calls={} redundant_reads={} tokens_in={} summary={:?}",
        treatment.read_calls,
        treatment.redundant_reads,
        treatment.tokens_in,
        treatment.consumer_summary
    );

    assert!(
        !control.consumer_summary.trim().is_empty()
            && !treatment.consumer_summary.trim().is_empty(),
        "both runs should produce consumer summaries"
    );
    assert!(
        treatment.redundant_reads <= control.redundant_reads,
        "forwarding should not increase redundant reads (control={}, treatment={})",
        control.redundant_reads,
        treatment.redundant_reads
    );
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "debug helper for live provider failures"]
async fn debug_live_reader_events() {
    if env::var("STEP_WORKFLOW_LIVE_AI").as_deref() != Ok("1") {
        return;
    }
    let config = live_config_from_env().unwrap_or_else(|error| panic!("{error}"));
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    let mut workflow = Workflow::new("debug-reader");
    workflow.nodes = vec![live_node(
        "reader",
        "Reader",
        &config.model,
        "Read lib.rs and submit a one-sentence summary naming the public function.",
    )];
    let mut handle =
        spawn_interactive_script(workflow, dir.path().to_path_buf(), live_client(&config));
    while let Some(event) = handle.event_rx.recv().await {
        eprintln!("debug event: {event:?}");
        if matches!(
            event,
            orchestration::run::execution::ExecutionEvent::Finished(_)
                | orchestration::run::execution::ExecutionEvent::Error(_)
                | orchestration::run::execution::ExecutionEvent::NodeFailed { .. }
                | orchestration::run::execution::ExecutionEvent::NodeErrored { .. }
                | orchestration::run::execution::ExecutionEvent::NodeInterrupted { .. }
                | orchestration::run::execution::ExecutionEvent::Aborted
        ) {
            break;
        }
    }
    handle.handle.abort();
}
