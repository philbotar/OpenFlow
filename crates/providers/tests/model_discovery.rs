#![allow(
    clippy::unwrap_used,
    reason = "integration tests use unwrap for concise wire assertions"
)]

use providers::{
    list_remote_models, AiClientConfig, AuthConfig, OpenAiCompatibleConfig, ProviderAdapterConfig,
    ProviderId, WireApi,
};
use std::collections::BTreeMap;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config(base_url: String, auth: AuthConfig) -> AiClientConfig {
    AiClientConfig {
        provider_id: ProviderId::from("custom_openai_compatible"),
        provider_label: "Custom OpenAI-compatible API".to_string(),
        auth,
        adapter: ProviderAdapterConfig::OpenAiCompatible(OpenAiCompatibleConfig {
            base_url,
            wire_api: WireApi::ChatCompletions,
            responses_path: "v1/responses".to_string(),
            chat_completions_path: "v1/chat/completions".to_string(),
            model_transports: BTreeMap::new(),
            request_timeout: Duration::from_secs(5),
        }),
        debug_output: false,
    }
}

#[tokio::test]
async fn lists_and_sorts_openai_compatible_models_without_optional_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                { "id": "zeta-model", "object": "model" },
                { "id": "alpha-model", "object": "model" },
                { "id": "alpha-model", "object": "model" }
            ]
        })))
        .mount(&server)
        .await;

    let models = list_remote_models(&config(
        server.uri(),
        AuthConfig::Bearer {
            api_key: None,
            required: false,
        },
    ))
    .await
    .unwrap();

    assert_eq!(models, ["alpha-model", "zeta-model"]);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("authorization"));
}

#[tokio::test]
async fn sends_bearer_auth_when_model_listing_has_a_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{ "id": "secured-model" }]
        })))
        .mount(&server)
        .await;

    let models = list_remote_models(&config(
        server.uri(),
        AuthConfig::Bearer {
            api_key: Some("secret-key".to_string()),
            required: true,
        },
    ))
    .await
    .unwrap();

    assert_eq!(models, ["secured-model"]);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer secret-key"
    );
}
