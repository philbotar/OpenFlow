//! Shared workflow normalization before execution handoff.
//!
// ponytail: at `run/` root (not `coordinator/`) so execution can import without coordinator→execution cycle.

use crate::settings::model::ProviderProfile;
use crate::settings::provider::ProviderConfigError;
use engine::{AgentNodeConfig, Workflow};
use providers::ProviderId;
use std::collections::BTreeMap;

/// Normalize a workflow before coordinator spawn or headless execution.
pub fn prepare_workflow_for_execution(workflow: &mut Workflow, profile: Option<&ProviderProfile>) {
    apply_workflow_reasoning_defaults(workflow);
    if let Some(profile) = profile {
        apply_provider_model_default(workflow, profile);
        apply_provider_reasoning_defaults(workflow, profile);
    }
}

/// Normalize each node from its override provider, then the shared workflow provider.
///
/// # Errors
/// Returns an unsupported-provider error when a referenced profile is unavailable.
pub fn prepare_workflow_for_execution_with_profiles(
    workflow: &mut Workflow,
    default_provider_id: &ProviderId,
    profiles: &BTreeMap<ProviderId, ProviderProfile>,
) -> Result<(), ProviderConfigError> {
    workflow.settings.provider_id = Some(default_provider_id.to_string());
    apply_workflow_reasoning_defaults(workflow);

    for node in &mut workflow.nodes {
        let provider_id = node
            .agent
            .provider_id
            .as_deref()
            .map(str::trim)
            .filter(|provider_id| !provider_id.is_empty())
            .map(ProviderId::from)
            .unwrap_or_else(|| default_provider_id.clone());
        let profile =
            profiles
                .get(&provider_id)
                .ok_or_else(|| ProviderConfigError::UnsupportedProvider {
                    provider: provider_id.to_string(),
                })?;
        apply_provider_model_default_to_agent(&mut node.agent, profile);
        apply_provider_reasoning_default_to_agent(&mut node.agent, profile);
    }
    Ok(())
}

fn apply_provider_model_default(workflow: &mut Workflow, profile: &ProviderProfile) {
    for node in &mut workflow.nodes {
        apply_provider_model_default_to_agent(&mut node.agent, profile);
    }
}

fn apply_provider_model_default_to_agent(agent: &mut AgentNodeConfig, profile: &ProviderProfile) {
    let Some(default_model) = profile
        .default_model
        .as_ref()
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
    else {
        return;
    };

    if agent.model.trim().is_empty() {
        agent.model = default_model.to_string();
    }
}

/// Apply workflow then provider reasoning defaults to unset nodes.
pub fn apply_reasoning_defaults(workflow: &mut Workflow, profile: &ProviderProfile) {
    apply_workflow_reasoning_defaults(workflow);
    apply_provider_reasoning_defaults(workflow, profile);
}

/// Apply workflow-level reasoning defaults to nodes that have no per-node override.
pub fn apply_workflow_reasoning_defaults(workflow: &mut Workflow) {
    let Some(default_effort) = workflow
        .settings
        .reasoning_effort
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let default_effort = default_effort.to_string();
    let default_budget = workflow.settings.reasoning_budget_tokens;

    for node in &mut workflow.nodes {
        if node.agent.reasoning_effort.is_some() {
            continue;
        }
        node.agent.reasoning_effort = Some(default_effort.clone());
        if node.agent.reasoning_budget_tokens.is_none() {
            node.agent.reasoning_budget_tokens = default_budget;
        }
    }
}

/// Provider reasoning settings for a one-off request (e.g. workflow authoring).
#[must_use]
pub fn provider_reasoning_for_profile(profile: &ProviderProfile) -> (Option<String>, Option<u32>) {
    let Some(effort) = profile
        .default_reasoning_effort
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            profile
                .reasoning_effort_options
                .first()
                .map(|option| option.value.clone())
        })
    else {
        return (None, None);
    };
    let uses_budget = profile
        .reasoning_effort_options
        .iter()
        .find(|option| option.value == effort)
        .is_some_and(|option| option.uses_budget_tokens);
    let budget = if uses_budget {
        profile
            .default_reasoning_budget_tokens
            .get(&effort)
            .copied()
    } else {
        None
    };
    (Some(effort), budget)
}

/// Apply provider-level reasoning defaults to nodes that have no per-node override.
pub fn apply_provider_reasoning_defaults(workflow: &mut Workflow, profile: &ProviderProfile) {
    for node in &mut workflow.nodes {
        apply_provider_reasoning_default_to_agent(&mut node.agent, profile);
    }
}

fn apply_provider_reasoning_default_to_agent(
    agent: &mut AgentNodeConfig,
    profile: &ProviderProfile,
) {
    let Some(default_effort) = profile
        .default_reasoning_effort
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let default_effort = default_effort.to_string();
    let default_budget = profile
        .default_reasoning_budget_tokens
        .get(&default_effort)
        .copied();
    let uses_budget = profile
        .reasoning_effort_options
        .iter()
        .find(|option| option.value == default_effort)
        .is_some_and(|option| option.uses_budget_tokens);

    if agent.reasoning_effort.is_some() {
        return;
    }
    agent.reasoning_effort = Some(default_effort);
    if uses_budget && agent.reasoning_budget_tokens.is_none() {
        agent.reasoning_budget_tokens = default_budget;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::{AppSettings, ProviderProfile};
    use engine::{AgentNodeConfig, Node, NodeId, NodeKind, NodePosition, Workflow};
    use providers::{provider_spec, ProviderId};

    fn sample_workflow() -> Workflow {
        let mut workflow = Workflow::new("test");
        workflow.nodes.push(Node {
            id: NodeId::from("node-1"),
            label: "Agent".to_string(),
            kind: NodeKind::Agent,
            position: NodePosition { x: 0.0, y: 0.0 },
            agent: AgentNodeConfig::default(),
        });
        workflow
    }

    #[test]
    fn prepare_workflow_for_execution_applies_workflow_defaults_without_profile() {
        let mut workflow = sample_workflow();
        workflow.settings.reasoning_effort = Some("medium".to_string());

        prepare_workflow_for_execution(&mut workflow, None);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("medium".to_string())
        );
    }

    #[test]
    fn prepare_workflow_for_execution_applies_provider_default_model() {
        let mut workflow = sample_workflow();
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());
        profile.default_model = Some("profile-default-model".to_string());

        prepare_workflow_for_execution(&mut workflow, Some(&profile));

        assert_eq!(workflow.nodes[0].agent.model, "profile-default-model");
    }

    #[test]
    fn prepare_workflow_for_execution_preserves_node_model_override() {
        let mut workflow = sample_workflow();
        workflow.nodes[0].agent.model = "node-model".to_string();
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());
        profile.default_model = Some("profile-default-model".to_string());

        prepare_workflow_for_execution(&mut workflow, Some(&profile));

        assert_eq!(workflow.nodes[0].agent.model, "node-model");
    }

    #[test]
    fn prepare_workflow_for_execution_uses_each_nodes_effective_provider_defaults() {
        let mut workflow = sample_workflow();
        workflow
            .nodes
            .push(Node::agent("Anthropic node", 100.0, 0.0));
        workflow.nodes[1].agent.provider_id = Some("anthropic".to_string());

        let mut settings = AppSettings::default();
        let openai = settings
            .providers
            .get_mut(&ProviderId::from("openai"))
            .expect("openai profile");
        openai.default_model = Some("openai-default".to_string());
        openai.default_reasoning_effort = Some("medium".to_string());
        let anthropic = settings
            .providers
            .get_mut(&ProviderId::from("anthropic"))
            .expect("anthropic profile");
        anthropic.default_model = Some("anthropic-default".to_string());
        anthropic.default_reasoning_effort = Some("adaptive".to_string());

        prepare_workflow_for_execution_with_profiles(
            &mut workflow,
            &ProviderId::from("openai"),
            &settings.providers,
        )
        .expect("known providers");

        assert_eq!(workflow.settings.provider_id.as_deref(), Some("openai"));
        assert_eq!(workflow.nodes[0].agent.model, "openai-default");
        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort.as_deref(),
            Some("medium")
        );
        assert_eq!(workflow.nodes[1].agent.model, "anthropic-default");
        assert_eq!(
            workflow.nodes[1].agent.reasoning_effort.as_deref(),
            Some("adaptive")
        );
    }

    #[test]
    fn apply_provider_reasoning_defaults_sets_effort_and_budget() {
        let mut workflow = sample_workflow();
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());
        profile.default_reasoning_effort = Some("low".to_string());

        apply_provider_reasoning_defaults(&mut workflow, &profile);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("low".to_string())
        );
        assert!(workflow.nodes[0].agent.reasoning_budget_tokens.is_none());
    }

    #[test]
    fn apply_provider_reasoning_defaults_preserves_node_override() {
        let mut workflow = sample_workflow();
        workflow.nodes[0].agent.reasoning_effort = Some("high".to_string());
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());
        profile.default_reasoning_effort = Some("low".to_string());

        apply_provider_reasoning_defaults(&mut workflow, &profile);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("high".to_string())
        );
    }

    #[test]
    fn apply_provider_reasoning_defaults_skips_when_unset() {
        let mut workflow = sample_workflow();
        let profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("anthropic")).unwrap());

        apply_provider_reasoning_defaults(&mut workflow, &profile);

        assert!(workflow.nodes[0].agent.reasoning_effort.is_none());
    }

    #[test]
    fn apply_workflow_reasoning_defaults_sets_effort_and_budget() {
        let mut workflow = sample_workflow();
        workflow.settings.reasoning_effort = Some("medium".to_string());
        workflow.settings.reasoning_budget_tokens = Some(8_192);

        apply_workflow_reasoning_defaults(&mut workflow);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("medium".to_string())
        );
        assert_eq!(workflow.nodes[0].agent.reasoning_budget_tokens, Some(8_192));
    }

    #[test]
    fn apply_workflow_reasoning_defaults_preserves_node_override() {
        let mut workflow = sample_workflow();
        workflow.nodes[0].agent.reasoning_effort = Some("high".to_string());
        workflow.settings.reasoning_effort = Some("low".to_string());

        apply_workflow_reasoning_defaults(&mut workflow);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("high".to_string())
        );
    }

    #[test]
    fn apply_reasoning_defaults_prefers_workflow_over_provider() {
        let mut workflow = sample_workflow();
        workflow.settings.reasoning_effort = Some("medium".to_string());
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("openai")).unwrap());
        profile.default_reasoning_effort = Some("low".to_string());

        apply_reasoning_defaults(&mut workflow, &profile);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("medium".to_string())
        );
    }

    #[test]
    fn apply_reasoning_defaults_falls_back_to_provider_when_workflow_unset() {
        let mut workflow = sample_workflow();
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("openai")).unwrap());
        profile.default_reasoning_effort = Some("low".to_string());

        apply_reasoning_defaults(&mut workflow, &profile);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("low".to_string())
        );
    }

    #[test]
    fn apply_provider_reasoning_defaults_openai_effort_without_budget() {
        let mut workflow = sample_workflow();
        let mut profile =
            ProviderProfile::from_spec(provider_spec(&ProviderId::from("openai")).unwrap());
        profile.default_reasoning_effort = Some("medium".to_string());
        assert!(!profile
            .reasoning_effort_options
            .iter()
            .any(|option| option.value == "medium" && option.uses_budget_tokens));

        apply_provider_reasoning_defaults(&mut workflow, &profile);

        assert_eq!(
            workflow.nodes[0].agent.reasoning_effort,
            Some("medium".to_string())
        );
        assert!(workflow.nodes[0].agent.reasoning_budget_tokens.is_none());
    }
}
