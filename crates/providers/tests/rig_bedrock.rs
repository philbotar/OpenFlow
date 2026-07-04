#![cfg(feature = "bedrock")]
#![allow(
    clippy::multiple_crate_versions,
    reason = "bedrock pulls duplicate transitive deps"
)]

use providers::{
    create_provider, AiClientConfig, AuthConfig, BedrockConfig, ProviderAdapterConfig, ProviderId,
};

fn bedrock_test_config() -> AiClientConfig {
    AiClientConfig {
        provider_id: ProviderId::from("bedrock"),
        provider_label: "Bedrock".into(),
        auth: AuthConfig::AwsCredentials {
            profile: Some("dev".into()),
            region: "eu-west-1".into(),
        },
        adapter: ProviderAdapterConfig::Bedrock(BedrockConfig {
            region: "eu-west-1".into(),
            aws_profile: Some("dev".into()),
        }),
    }
}

#[test]
fn bedrock_config_and_client_build_without_aws_invoke() {
    let config = bedrock_test_config();
    assert_eq!(config.provider_id.as_str(), "bedrock");
    assert!(matches!(
        config.adapter,
        ProviderAdapterConfig::Bedrock(ref bedrock) if bedrock.region == "eu-west-1"
    ));
    let _client = create_provider(config);
}
