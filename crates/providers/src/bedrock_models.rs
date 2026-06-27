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
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(trimmed_region.to_string()));
    if let Some(profile) = aws_profile.map(str::trim).filter(|value| !value.is_empty()) {
        loader = loader.profile_name(profile);
    }
    let shared = loader.load().await;
    Ok(BedrockControlClient::new(&shared))
}

fn map_bedrock_control_error<E>(error: &aws_sdk_bedrock::error::SdkError<E>) -> AgentError
where
    E: std::error::Error + Send + Sync + 'static,
{
    let message = crate::bedrock::humanize_bedrock_sdk_error(&error.to_string());
    match classify_sdk_error_code(&message) {
        SdkErrorClass::Transient => AgentError::Transient(format!("Bedrock list models failed: {message}")),
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
