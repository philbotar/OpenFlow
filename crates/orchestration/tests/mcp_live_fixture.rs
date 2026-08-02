use orchestration::adapters::mcp::McpRunClients;
use orchestration::mcp::model::{McpConnection, McpInstall, McpServerRecord, McpServerSource};
use orchestration::settings::model::McpSettings;
use serde_json::json;
use std::path::PathBuf;

#[tokio::test]
async fn local_stdio_server_negotiates_tools_resources_prompts_and_calls() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/fixtures/mcp_stdio_server.py");
    assert!(fixture.is_file(), "missing fixture: {}", fixture.display());
    let mut server = McpServerRecord::new(
        "live-fixture",
        "Live fixture",
        McpServerSource::Manual,
        McpInstall::External,
        McpConnection::Stdio {
            command: "python3".to_string(),
            args: vec![fixture.display().to_string()],
            environment: std::collections::BTreeMap::default(),
        },
    );
    orchestration::mcp::trust::approve_current(&mut server, chrono::Utc::now())
        .expect("approve fixture");
    server.enabled = true;
    let settings = McpSettings {
        servers: vec![server],
        discover_external: false,
        disabled_discovered_ids: Vec::new(),
        registry_base_url: McpSettings::default().registry_base_url,
    };

    let (clients, issues) = McpRunClients::connect(&settings).await;
    assert!(issues.is_empty(), "fixture connection issues: {issues:?}");
    let (tools, issues) = clients.list_all_tool_definitions().await;
    assert!(issues.is_empty(), "fixture tool issues: {issues:?}");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "mcp_12_live-fixture_echo");

    let catalog = clients
        .capability_catalog("live-fixture")
        .await
        .expect("capability catalog");
    assert_eq!(catalog.resources.len(), 1);
    assert_eq!(catalog.resources[0].uri, "fixture://status");
    assert_eq!(catalog.prompts.len(), 1);
    assert_eq!(catalog.prompts[0].name, "fixture_prompt");

    let outcome = clients
        .call_namespaced("mcp_12_live-fixture_echo", json!({"message": "hello"}))
        .await
        .expect("fixture tool call");
    assert!(!outcome.is_error);
    assert_eq!(outcome.content, "echo:hello");
    clients.close().await.expect("close fixture");
}
