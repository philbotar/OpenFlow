use orchestration::workflow::authoring::WorkflowAuthoringService;
use orchestration::AppSettings;

#[tokio::test]
#[ignore = "requires STEP_WORKFLOW_LIVE_AI=1 and provider API key"]
async fn live_authoring_turn_produces_valid_dag() {
    if std::env::var("STEP_WORKFLOW_LIVE_AI").ok().as_deref() != Some("1") {
        return;
    }

    let mut service = WorkflowAuthoringService::new();
    let session_id = service.start_session(None);
    let settings = AppSettings::default();
    let provider_config = orchestration::settings::provider::resolve_provider_config(
        &settings,
        None,
        &orchestration::settings::provider::ProviderEnv::from_system(),
    )
    .expect("provider config");
    let ai = providers::create_provider(provider_config);
    let result = service
        .send_turn(
            &session_id,
            "Create a workflow that clarifies an idea, runs plan and risk in parallel, then writes a brief.".to_string(),
            &settings,
            &ai,
        )
        .await
        .expect("authoring turn");

    assert!(result.validation.valid, "{:?}", result.validation.errors);
    assert!(result.draft.as_ref().is_some_and(|draft| draft.nodes.len() >= 3));
}
