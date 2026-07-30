use engine::{AiPort, Edge, Node, NodeId, Workflow};
use image::{ImageFormat, Rgb, RgbImage};
use orchestration::adapters::storage::agent_store::FileAgentStore;
use orchestration::adapters::storage::app_workflow_store::FileWorkflowStore;
use orchestration::adapters::storage::chat_store::FileChatStore;
use orchestration::adapters::storage::project_store::FileProjectStore;
use orchestration::adapters::storage::project_workflow_store::FileProjectWorkflowStore;
use orchestration::adapters::storage::run_attachment_store::FileRunAttachmentStore;
use orchestration::adapters::storage::settings_store::FileSettingsStore;
use orchestration::adapters::storage::skill_store::FileSkillCatalog;
use orchestration::agent::AgentLibrary;
use orchestration::api::UserMessageInput;
use orchestration::backend::{AppBackend, AppBackendDeps};
use orchestration::chat::ChatConfig;
use orchestration::run::execution::{run_workflow_headless, WorkflowRunSnapshot};
use orchestration::run::prep::provider_reasoning_for_profile;
use orchestration::settings::model::AppSettings;
use orchestration::settings::ports::SettingsStore;
use orchestration::settings::provider::{
    attach_codex_credential_sink, resolve_provider_config, ProviderEnv,
};
use orchestration::workflow::authoring::WorkflowAuthoringService;
use providers::create_provider;
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;
use tempfile::tempdir;

const ENABLE_ENV: &str = "OPENFLOW_LIVE_AI_SMOKE";
const SENTINEL: &str = "ORCHID-91";

struct SmokeContext {
    ai: Box<dyn AiPort>,
    model: String,
    provider_id: String,
    provider_label: String,
    transport_label: &'static str,
    settings: AppSettings,
}

fn load_smoke_context() -> Result<SmokeContext, String> {
    let settings_path = FileSettingsStore::default_path();
    let concrete_store = FileSettingsStore::new(&settings_path);
    let settings = concrete_store
        .load()
        .map_err(|error| format!("load {}: {error}", settings_path.display()))?;
    let model = settings
        .active_profile()
        .default_model
        .clone()
        .ok_or_else(|| "active provider has no default model".to_string())?;
    if model.trim().is_empty() {
        return Err("active provider default model is empty".to_string());
    }

    let settings_store: Arc<dyn SettingsStore> = Arc::new(concrete_store);
    let mut provider_config = resolve_provider_config(&settings, None, &ProviderEnv::from_system())
        .map_err(|error| error.to_string())?;
    let provider_label = provider_config.provider_label.clone();
    attach_codex_credential_sink(&mut provider_config, settings_store);
    let transport = settings
        .active_profile()
        .model_transports
        .get(&model)
        .copied()
        .unwrap_or_else(|| settings.active_profile().transport.into());

    Ok(SmokeContext {
        ai: create_provider(provider_config),
        model,
        provider_id: settings.active_provider.to_string(),
        provider_label,
        transport_label: transport.label(),
        settings,
    })
}

fn write_attachment_smoke_png(path: &std::path::Path) -> Result<(), String> {
    let mut image = RgbImage::from_pixel(640, 240, Rgb([255, 255, 255]));
    for (start_x, start_y) in [(40, 40), (270, 100), (500, 40)] {
        for x in start_x..start_x + 100 {
            for y in start_y..start_y + 100 {
                image.put_pixel(x, y, Rgb([0, 70, 255]));
            }
        }
    }
    image
        .save_with_format(path, ImageFormat::Png)
        .map_err(|error| format!("write attachment smoke PNG: {error}"))
}

fn write_attachment_smoke_pdf(path: &std::path::Path) -> Result<(), String> {
    let page_one = "BT\n/F1 24 Tf\n72 700 Td\n(Page one code: EMBER-417.) Tj\nET";
    let page_two = concat!(
        "BT\n/F1 24 Tf\n72 700 Td\n",
        "(Page two animal: AXOLOTL.) Tj\n",
        "0 -40 Td\n(Page two checksum: 7319.) Tj\nET"
    );
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".to_string(),
        concat!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] ",
            "/Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R >>"
        )
        .to_string(),
        format!(
            "<< /Length {} >>\nstream\n{page_one}\nendstream",
            page_one.len()
        ),
        concat!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] ",
            "/Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>"
        )
        .to_string(),
        format!(
            "<< /Length {} >>\nstream\n{page_two}\nendstream",
            page_two.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];

    let mut pdf = "%PDF-1.4\n".to_string();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(&mut pdf, "{} 0 obj\n{object}\nendobj", index + 1)
            .expect("writing to String cannot fail");
    }
    let xref_offset = pdf.len();
    pdf.push_str("xref\n0 8\n0000000000 65535 f \n");
    for offset in offsets {
        writeln!(&mut pdf, "{offset:010} 00000 n ").expect("writing to String cannot fail");
    }
    write!(
        &mut pdf,
        "trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
    )
    .expect("writing to String cannot fail");
    std::fs::write(path, pdf).map_err(|error| format!("write attachment smoke PDF: {error}"))
}

fn assistant_messages(state: &orchestration::run::state::WorkflowRunState) -> Vec<&str> {
    state
        .chat_logs
        .values()
        .flatten()
        .filter(|message| message.role == engine::ChatRole::Assistant)
        .map(|message| message.content.as_str())
        .collect()
}

async fn wait_for_chat_pause(
    backend: &AppBackend,
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<
        orchestration::run::execution::ExecutionEvent,
    >,
    phase: &str,
) -> Result<orchestration::run::state::WorkflowRunState, String> {
    tokio::time::timeout(std::time::Duration::from_secs(90), async {
        while let Some(event) = event_rx.recv().await {
            let state = backend
                .apply_execution_event(event)
                .await
                .map_err(|error| format!("{phase}: apply execution event: {error}"))?;
            if state.awaiting_node_id.is_some() {
                return Ok(state);
            }
            if !state.active {
                return Err(format!(
                    "{phase}: chat stopped before awaiting input: {:?}",
                    state.last_error
                ));
            }
        }
        Err(format!(
            "{phase}: chat event channel closed before awaiting input"
        ))
    })
    .await
    .map_err(|_| format!("{phase}: chat did not pause within 90 seconds"))?
}

fn require_answer_tokens(
    state: &orchestration::run::state::WorkflowRunState,
    tokens: &[&str],
    phase: &str,
) -> Result<(), String> {
    let answer = assistant_messages(state)
        .last()
        .copied()
        .ok_or_else(|| format!("{phase}: assistant produced no answer"))?;
    let normalized = answer.to_ascii_uppercase().replace(char::is_whitespace, "");
    for token in tokens {
        let normalized_token = token.to_ascii_uppercase().replace(char::is_whitespace, "");
        if !normalized.contains(&normalized_token) {
            return Err(format!("{phase}: answer did not contain {token}: {answer}"));
        }
    }
    Ok(())
}

struct AttachmentSmokeCase<'a> {
    source_path: &'a std::path::Path,
    initial_prompt: &'a str,
    initial_tokens: &'a [&'a str],
    replay_prompt: &'a str,
    replay_tokens: &'a [&'a str],
    label: &'a str,
}

fn prepare_attachment_smoke_chat(
    context: &SmokeContext,
    dir: &std::path::Path,
    label: &str,
) -> Result<(AppBackend, String), String> {
    let backend = AppBackend::new(
        AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(dir.join("workflows.json"))),
            chat_store: Box::new(FileChatStore::new(dir.join("chats.json"))),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(dir.join("agents.json"))),
            project_store: Box::new(FileProjectStore::new(dir.join("projects.json"))),
            settings_store: Arc::new(FileSettingsStore::new(FileSettingsStore::default_path())),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::from_system(),
            runtime_handle: tokio::runtime::Handle::current(),
            attachment_store: Arc::new(FileRunAttachmentStore::default()),
        },
        None,
    );
    let project = backend
        .create_project_from_directory(dir.display().to_string())
        .map_err(|error| format!("{label}: create project: {error}"))?;
    let chat = backend
        .create_chat()
        .map_err(|error| format!("{label}: create chat: {error}"))?;
    let chat = backend
        .update_chat_config(
            &chat.id,
            ChatConfig {
                model: Some(context.model.clone()),
                project_id: Some(project.id),
                ..ChatConfig::default()
            },
        )
        .map_err(|error| format!("{label}: configure chat: {error}"))?;
    Ok((backend, chat.id))
}

async fn smoke_attachment_replay(
    context: &SmokeContext,
    dir: &std::path::Path,
    case: AttachmentSmokeCase<'_>,
) -> Result<(), String> {
    let AttachmentSmokeCase {
        source_path,
        initial_prompt,
        initial_tokens,
        replay_prompt,
        replay_tokens,
        label,
    } = case;
    let (backend, chat_id) = prepare_attachment_smoke_chat(context, dir, label)?;
    let (_, initial_state, mut event_rx) = backend
        .start_chat_with_message_and_skill_ids(
            &chat_id,
            Some(UserMessageInput {
                text: initial_prompt.to_string(),
                attachment_source_paths: vec![source_path.display().to_string()],
            }),
            &context.settings,
            None,
            Vec::new(),
        )
        .await
        .map_err(|error| format!("{label}: start attachment chat: {error}"))?;

    let initial_user_message = initial_state
        .chat_logs
        .values()
        .flatten()
        .find(|message| message.role == engine::ChatRole::User)
        .ok_or_else(|| format!("{label}: initial user message missing"))?;
    if initial_user_message.attachments.len() != 1 {
        return Err(format!(
            "{label}: expected one persisted attachment, got {}",
            initial_user_message.attachments.len()
        ));
    }

    let first_pause = wait_for_chat_pause(&backend, &mut event_rx, label).await?;
    require_answer_tokens(&first_pause, initial_tokens, label)?;
    println!("PASS  {label} initial attachment understanding");

    let run_id = first_pause
        .run_id
        .clone()
        .ok_or_else(|| format!("{label}: run ID missing"))?;
    backend
        .stop_run()
        .await
        .map_err(|error| format!("{label}: stop before durable replay: {error}"))?;
    drop(event_rx);

    let (resumed_state, mut resumed_event_rx, _) = backend
        .resume_durable_run(&run_id, &context.settings, None)
        .await
        .map_err(|error| format!("{label}: resume durable run: {error}"))?;
    let resumed_state = if resumed_state.awaiting_node_id.is_some()
        || !resumed_state.awaiting_node_ids.is_empty()
    {
        resumed_state
    } else {
        wait_for_chat_pause(&backend, &mut resumed_event_rx, label).await?
    };
    let node_id = resumed_state
        .awaiting_node_id
        .as_ref()
        .or_else(|| resumed_state.awaiting_node_ids.first())
        .ok_or_else(|| format!("{label}: resumed run is not awaiting input"))?
        .to_string();
    backend
        .submit_user_input(&node_id, replay_prompt.to_string())
        .await
        .map_err(|error| format!("{label}: submit replay follow-up: {error}"))?;

    let replay_pause = wait_for_chat_pause(&backend, &mut resumed_event_rx, label).await?;
    require_answer_tokens(&replay_pause, replay_tokens, label)?;
    println!("PASS  {label} durable attachment replay");
    backend
        .stop_run()
        .await
        .map_err(|error| format!("{label}: stop resumed chat: {error}"))?;
    Ok(())
}

async fn smoke_workflow_authoring() -> Result<(), String> {
    let context = load_smoke_context()?;
    let service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None).session_id;
    let result = service
        .send_turn(
            &session_id,
            concat!(
                "Replace the current draft with a small autonomous smoke-test workflow. ",
                "Use exactly three auto-start agent nodes in a linear chain: Extract, Transform, ",
                "then Verify. Do not request user input or enable repository tools. Give every ",
                "node a strict object output schema with a required summary string. Finish the ",
                "draft when it is valid."
            )
            .to_string(),
            &context.settings,
            &context.ai,
            |_| {},
            |_| {},
        )
        .await
        .map_err(|error| error.to_string())?;

    if !result.validation.valid {
        return Err(format!(
            "generated workflow is invalid: {}",
            result.validation.errors.join("; ")
        ));
    }
    let draft = result
        .draft
        .ok_or_else(|| "authoring completed without a workflow draft".to_string())?;
    if !(2..=4).contains(&draft.nodes.len()) {
        return Err(format!(
            "expected a small multi-node draft, got {} nodes",
            draft.nodes.len()
        ));
    }
    if draft.edges.is_empty() {
        return Err("generated multi-node draft has no edges".to_string());
    }
    if draft
        .nodes
        .iter()
        .any(|node| node.agent.model != context.model)
    {
        return Err(format!(
            "generated workflow did not use selected model {} on every node",
            context.model
        ));
    }

    Ok(())
}

async fn smoke_agent_authoring() -> Result<(), String> {
    let context = load_smoke_context()?;
    let dir = tempdir().map_err(|error| format!("create temp agent store: {error}"))?;
    let library = AgentLibrary::new(Box::new(FileAgentStore::new(
        dir.path().join("agents.json"),
    )));
    let (reasoning_effort, reasoning_budget_tokens) =
        provider_reasoning_for_profile(context.settings.active_profile());
    let agent = library
        .create_with_ai(
            concat!(
                "Create a reusable reviewer that checks a short brief for unsupported claims. ",
                "It must return a strict structured object containing a non-empty findings array."
            )
            .to_string(),
            context.model.clone(),
            reasoning_effort,
            reasoning_budget_tokens,
            &*context.ai,
        )
        .await
        .map_err(|error| error.to_string())?;

    if agent.name.trim().is_empty()
        || agent.system_prompt.trim().is_empty()
        || agent.task_prompt.trim().is_empty()
    {
        return Err("generated agent has an empty required field".to_string());
    }
    if agent.model != context.model {
        return Err(format!(
            "generated agent model {} does not match selected model {}",
            agent.model, context.model
        ));
    }
    if !agent.output_schema.is_object() {
        return Err("generated agent output schema is not an object".to_string());
    }
    let persisted = library.load().map_err(|error| error.to_string())?;
    if persisted.len() != 1 || persisted[0].id != agent.id {
        return Err("generated agent was not persisted to the temp store".to_string());
    }

    Ok(())
}

fn fixed_smoke_node(id: &str, label: &str, task: &str, model: &str) -> Node {
    let mut node = Node::agent(label, 0.0, 0.0);
    node.id = NodeId::from(id);
    node.agent.model = model.to_string();
    node.agent.system_prompt = concat!(
        "You are running an automated live smoke test. ",
        "Return only valid JSON matching the output schema. ",
        "Preserve project_code exactly when present."
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

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "manual only: probes direct chat through AppBackend using the saved provider"]
async fn saved_provider_direct_chat_probe() {
    assert_eq!(
        std::env::var(ENABLE_ENV).as_deref(),
        Ok("1"),
        "set OPENFLOW_LIVE_AI_SMOKE=1 to enable"
    );
    let context = load_smoke_context().expect("load saved provider");
    let model = std::env::var("OPENFLOW_LIVE_AI_PROBE_MODEL").unwrap_or(context.model);
    let dir = tempdir().expect("tempdir");
    let backend = AppBackend::new(
        AppBackendDeps {
            workflow_store: Box::new(FileWorkflowStore::new(dir.path().join("workflows.json"))),
            chat_store: Box::new(FileChatStore::new(dir.path().join("chats.json"))),
            project_workflow_store: Box::new(FileProjectWorkflowStore),
            agent_store: Box::new(FileAgentStore::new(dir.path().join("agents.json"))),
            project_store: Box::new(FileProjectStore::new(dir.path().join("projects.json"))),
            settings_store: Arc::new(FileSettingsStore::new(FileSettingsStore::default_path())),
            skill_catalog: Box::new(FileSkillCatalog),
            env: ProviderEnv::from_system(),
            runtime_handle: tokio::runtime::Handle::current(),
            attachment_store: Arc::new(FileRunAttachmentStore::default()),
        },
        None,
    );
    let project = backend
        .create_project_from_directory(dir.path().display().to_string())
        .expect("create project");
    let chat = backend.create_chat().expect("create chat");
    let chat = backend
        .update_chat_config(
            &chat.id,
            ChatConfig {
                model: Some(model),
                project_id: Some(project.id),
                ..ChatConfig::default()
            },
        )
        .expect("configure chat");
    let (_, initial_state, mut event_rx) = backend
        .start_chat(
            &chat.id,
            Some("Reply briefly: what is up?".to_string()),
            &context.settings,
            None,
        )
        .await
        .expect("start chat");

    assert!(initial_state.chat_logs.values().flatten().any(|message| {
        message.role == engine::ChatRole::User && message.content == "Reply briefly: what is up?"
    }));

    let paused = tokio::time::timeout(std::time::Duration::from_secs(45), async {
        while let Some(event) = event_rx.recv().await {
            let state = backend
                .apply_execution_event(event)
                .await
                .expect("apply execution event");
            if state.awaiting_node_id.is_some() {
                return state;
            }
            assert!(
                state.active,
                "direct chat stopped before awaiting input: {:?}",
                state.last_error
            );
        }
        panic!("direct chat event channel closed before awaiting input");
    })
    .await
    .expect("direct chat did not pause for input within 45 seconds");

    assert!(paused
        .chat_logs
        .values()
        .flatten()
        .any(|message| message.role == engine::ChatRole::Assistant));
    assert!(
        paused.structured_input_by_node.is_empty(),
        "direct chat must not expose structured multiple-choice questions: {:?}",
        paused.structured_input_by_node
    );
    backend.stop_run().await.expect("stop chat");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "manual only: probes image attachment replay through the saved provider"]
async fn saved_provider_attachment_replay_probe() {
    assert_eq!(
        std::env::var(ENABLE_ENV).as_deref(),
        Ok("1"),
        "set OPENFLOW_LIVE_AI_SMOKE=1 to enable"
    );
    let context = load_smoke_context().expect("load saved provider");
    println!(
        "Live attachment smoke: provider={} ({}) model={} transport={} settings={}",
        context.provider_label,
        context.provider_id,
        context.model,
        context.transport_label,
        FileSettingsStore::default_path().display()
    );

    let dir = tempdir().expect("tempdir");
    let image_path = dir.path().join("three-blue-squares.png");
    write_attachment_smoke_png(&image_path).expect("create image fixture");
    Box::pin(smoke_attachment_replay(
        &context,
        dir.path(),
        AttachmentSmokeCase {
            source_path: &image_path,
            initial_prompt: concat!(
                "Inspect the attached image. Identify the color and number of the repeated solid ",
                "shapes. Reply exactly as COLOR-COUNT. Example format: GREEN-2."
            ),
            initial_tokens: &["BLUE-3"],
            replay_prompt: concat!(
                "Using only the earlier image, which square is lowest: LEFT, MIDDLE, or RIGHT? ",
                "Reply with the position."
            ),
            replay_tokens: &["MIDDLE"],
            label: "PNG",
        },
    ))
    .await
    .expect("PNG attachment smoke");
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "manual only: probes PDF attachment replay through the saved provider"]
async fn saved_provider_pdf_attachment_replay_probe() {
    assert_eq!(
        std::env::var(ENABLE_ENV).as_deref(),
        Ok("1"),
        "set OPENFLOW_LIVE_AI_SMOKE=1 to enable"
    );
    let context = load_smoke_context().expect("load saved provider");
    println!(
        "Live attachment smoke: provider={} ({}) model={} transport={} settings={}",
        context.provider_label,
        context.provider_id,
        context.model,
        context.transport_label,
        FileSettingsStore::default_path().display()
    );

    let dir = tempdir().expect("tempdir");
    let pdf_path = dir.path().join("two-page-brief.pdf");
    write_attachment_smoke_pdf(&pdf_path).expect("create PDF fixture");
    Box::pin(smoke_attachment_replay(
        &context,
        dir.path(),
        AttachmentSmokeCase {
            source_path: &pdf_path,
            initial_prompt: concat!(
                "Read the attached two-page PDF. What code appears on page one, and what animal ",
                "appears on page two? Include both exact values."
            ),
            initial_tokens: &["EMBER-417", "AXOLOTL"],
            replay_prompt: concat!(
                "Using only the earlier PDF, what checksum appears on page two? ",
                "Reply with the exact digits."
            ),
            replay_tokens: &["7319"],
            label: "PDF",
        },
    ))
    .await
    .expect("PDF attachment smoke");
}

async fn smoke_fixed_workflow() -> Result<WorkflowRunSnapshot, String> {
    let context = load_smoke_context()?;
    let mut workflow = Workflow::new("Manual live AI smoke");
    workflow.nodes = vec![
        fixed_smoke_node(
            "extract",
            "Extract sentinel",
            "Read the entrypoint. Return project_code exactly as ORCHID-91 and a short summary.",
            &context.model,
        ),
        fixed_smoke_node(
            "verify",
            "Verify handoff",
            "Use the upstream JSON. Preserve project_code exactly and summarize the handoff.",
            &context.model,
        ),
    ];
    workflow.edges = vec![Edge::new("extract", "verify")];
    let provider_profile = context.settings.active_profile().clone();

    let snapshot = run_workflow_headless(
        workflow,
        Some(format!(
            "This is the fixed multi-node live smoke. project_code: {SENTINEL}"
        )),
        context.ai,
        vec![],
        vec![],
        BTreeMap::new(),
        None,
        Some(&provider_profile),
    )
    .await
    .map_err(|error| error.to_string())?;

    for node_id in ["extract", "verify"] {
        let output = snapshot
            .outputs
            .get(node_id)
            .ok_or_else(|| format!("node {node_id} produced no output"))?;
        if output["project_code"] != SENTINEL {
            return Err(format!("node {node_id} did not preserve {SENTINEL}"));
        }
        if output["summary"]
            .as_str()
            .is_none_or(|summary| summary.trim().is_empty())
        {
            return Err(format!("node {node_id} produced an empty summary"));
        }
    }

    Ok(snapshot)
}

fn record_result(name: &str, result: Result<(), String>, failures: &mut Vec<String>) {
    match result {
        Ok(()) => println!("PASS  {name}"),
        Err(error) => {
            eprintln!("FAIL  {name}: {error}");
            failures.push(format!("{name}: {error}"));
        }
    }
}

#[cfg_attr(miri, ignore)]
#[tokio::test]
#[ignore = "manual only: run ./scripts/smoke-live-ai.sh"]
async fn saved_provider_supports_every_live_ai_surface() {
    assert_eq!(
        std::env::var(ENABLE_ENV).as_deref(),
        Ok("1"),
        "run this manual smoke via ./scripts/smoke-live-ai.sh"
    );

    let preflight = load_smoke_context().unwrap_or_else(|error| {
        panic!(
            "live AI smoke preflight failed: {error}\n\
             Configure and select a ready provider in OpenFlow Settings, then retry."
        )
    });
    println!(
        "Live AI smoke: provider={} ({}) model={} settings={}",
        preflight.provider_label,
        preflight.provider_id,
        preflight.model,
        FileSettingsStore::default_path().display()
    );
    drop(preflight);

    let mut failures = Vec::new();
    record_result(
        "workflow Create with AI",
        smoke_workflow_authoring().await,
        &mut failures,
    );
    record_result(
        "agent Create with AI",
        smoke_agent_authoring().await,
        &mut failures,
    );

    match smoke_fixed_workflow().await {
        Ok(snapshot) => {
            println!("PASS  fixed two-node workflow");
            record_result(
                "post-run AI review",
                snapshot.report.suggestions_error.map_or(Ok(()), Err),
                &mut failures,
            );
        }
        Err(error) => {
            eprintln!("FAIL  fixed two-node workflow: {error}");
            eprintln!("SKIP  post-run AI review: workflow did not complete");
            failures.push(format!("fixed two-node workflow: {error}"));
        }
    }

    assert!(
        failures.is_empty(),
        "live AI smoke failed:\n- {}",
        failures.join("\n- ")
    );
}
