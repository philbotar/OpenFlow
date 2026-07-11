//! Shared workflow normalization before execution handoff.
//!
// ponytail: at `run/` root (not `coordinator/`) so execution can import without coordinator→execution cycle.

use crate::settings::model::ProviderProfile;
use engine::Workflow;

/// Normalize a workflow before coordinator spawn or headless execution.
pub fn prepare_workflow_for_execution(workflow: &mut Workflow, profile: Option<&ProviderProfile>) {
    apply_workflow_reasoning_defaults(workflow);
    if let Some(profile) = profile {
        apply_provider_reasoning_defaults(workflow, profile);
    }
}

/// Apply workflow then provider reasoning defaults to unset nodes.
pub fn apply_reasoning_defaults(workflow: &mut Workflow, profile: &ProviderProfile) {
    prepare_workflow_for_execution(workflow, Some(profile));
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

    for node in &mut workflow.nodes {
        if node.agent.reasoning_effort.is_some() {
            continue;
        }
        node.agent.reasoning_effort = Some(default_effort.clone());
        if uses_budget && node.agent.reasoning_budget_tokens.is_none() {
            node.agent.reasoning_budget_tokens = default_budget;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::ProviderProfile;
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
        assert_eq!(
            workflow.nodes[0].agent.reasoning_budget_tokens,
            Some(10_240)
        );
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
