use crate::aws_runtime::load_aws_sdk_config;
use aws_sdk_bedrock::config::ProvideCredentials;
use aws_sdk_bedrock::types::{
    FoundationModelLifecycle, FoundationModelLifecycleStatus, FoundationModelSummary,
    InferenceType, ModelModality,
};
use aws_sdk_bedrock::Client as BedrockControlClient;
use engine::AgentError;

/// Lists active on-demand Bedrock foundation models that support Converse text output.
///
/// # Errors
/// Returns an error when AWS configuration, credentials, or the Bedrock control-plane call fails.
pub async fn list_bedrock_foundation_models(
    region: &str,
    aws_profile: Option<&str>,
) -> Result<Vec<String>, AgentError> {
    let client = bedrock_control_client(region, aws_profile).await?;
    let response = client
        .list_foundation_models()
        .by_output_modality(ModelModality::Text)
        .by_inference_type(InferenceType::OnDemand)
        .send()
        .await
        .map_err(|error| map_bedrock_control_error(&error))?;
    Ok(filter_converse_model_ids(
        response.model_summaries.unwrap_or_default(),
    ))
}

/// Loads AWS credentials for Bedrock without calling the Bedrock API.
///
/// # Errors
/// Returns an error when the AWS credential chain cannot resolve credentials.
pub async fn verify_bedrock_credentials(
    region: &str,
    aws_profile: Option<&str>,
) -> Result<String, AgentError> {
    let trimmed_region = region.trim();
    if trimmed_region.is_empty() {
        return Err(AgentError::Permanent(
            "Amazon Bedrock AWS region missing".to_string(),
        ));
    }
    let config = load_aws_sdk_config(trimmed_region, aws_profile).await;
    let provider = config.credentials_provider().ok_or_else(|| {
        AgentError::Permanent("AWS credentials provider not configured".to_string())
    })?;
    let credentials = provider.provide_credentials().await.map_err(|error| {
        AgentError::Permanent(crate::bedrock_errors::humanize_bedrock_sdk_error(
            &error.to_string(),
        ))
    })?;
    let suffix: String = credentials
        .access_key_id()
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok(format!(
        "AWS credentials loaded (access key id ends with …{suffix})"
    ))
}

pub fn filter_converse_model_ids(summaries: Vec<FoundationModelSummary>) -> Vec<String> {
    let mut ids: Vec<String> = summaries
        .into_iter()
        .filter(is_converse_chat_model)
        .map(|summary| summary.model_id().to_string())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn is_converse_chat_model(summary: &FoundationModelSummary) -> bool {
    let active = summary
        .model_lifecycle()
        .map(FoundationModelLifecycle::status)
        == Some(&FoundationModelLifecycleStatus::Active);
    if !active {
        return false;
    }
    let model_id = summary.model_id();
    if model_id.is_empty() || model_id.contains("embed") || model_id.contains("titan-embed") {
        return false;
    }
    summary.output_modalities().contains(&ModelModality::Text)
}

async fn bedrock_control_client(
    region: &str,
    aws_profile: Option<&str>,
) -> Result<BedrockControlClient, AgentError> {
    let trimmed_region = region.trim();
    if trimmed_region.is_empty() {
        return Err(AgentError::Permanent(
            "Amazon Bedrock AWS region missing".to_string(),
        ));
    }
    let shared = load_aws_sdk_config(
        trimmed_region,
        aws_profile.map(str::trim).filter(|value| !value.is_empty()),
    )
    .await;
    Ok(BedrockControlClient::new(&shared))
}

fn map_bedrock_control_error<E>(error: &aws_sdk_bedrock::error::SdkError<E>) -> AgentError
where
    E: std::error::Error + Send + Sync + 'static,
{
    let message = crate::bedrock_errors::humanize_bedrock_sdk_error(
        &crate::bedrock_errors::format_aws_sdk_error(error),
    );
    match classify_sdk_error_code(&message) {
        SdkErrorClass::Transient => {
            AgentError::Transient(format!("Bedrock list models failed: {message}"))
        }
        SdkErrorClass::Permanent => {
            AgentError::Permanent(format!("Bedrock list models failed: {message}"))
        }
    }
}

enum SdkErrorClass {
    Transient,
    Permanent,
}

fn classify_sdk_error_code(message: &str) -> SdkErrorClass {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("throttl")
        || lowered.contains("timeout")
        || lowered.contains("serviceunavailable")
        || lowered.contains("internalserver")
    {
        SdkErrorClass::Transient
    } else {
        SdkErrorClass::Permanent
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn summary(
        model_id: &str,
        status: FoundationModelLifecycleStatus,
        text_output: bool,
    ) -> FoundationModelSummary {
        let mut builder = FoundationModelSummary::builder()
            .model_id(model_id)
            .model_arn(format!(
                "arn:aws:bedrock:us-east-1::foundation-model/{model_id}"
            ))
            .model_name(model_id)
            .model_lifecycle(
                FoundationModelLifecycle::builder()
                    .status(status)
                    .build()
                    .expect("lifecycle"),
            );
        if text_output {
            builder = builder.output_modalities(ModelModality::Text);
        }
        builder.build().expect("summary")
    }

    #[test]
    fn filters_active_text_on_demand_models() {
        let summaries = vec![
            summary(
                "amazon.titan-embed-text-v2:0",
                FoundationModelLifecycleStatus::Active,
                false,
            ),
            summary(
                "anthropic.claude-sonnet-4-20250514-v1:0",
                FoundationModelLifecycleStatus::Active,
                true,
            ),
        ];
        let ids = filter_converse_model_ids(summaries);
        assert_eq!(ids, vec!["anthropic.claude-sonnet-4-20250514-v1:0"]);
    }
}
