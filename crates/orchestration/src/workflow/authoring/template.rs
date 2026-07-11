use super::draft::{materialize_authoring_draft, workflow_to_authoring_draft};
use super::layout::layout_workflow_by_layers;
use engine::Workflow;

const FEATURE_PLAN_TEMPLATE: &str =
    include_str!("../../../../../examples/feature_plan.workflow.json");

/// Preloaded skeleton for new Create-with-AI sessions: clarify → parallel plan/risk → brief.
#[must_use]
pub fn default_authoring_template_workflow(default_model: &str) -> Workflow {
    let example: Workflow =
        serde_json::from_str(FEATURE_PLAN_TEMPLATE).expect("feature_plan template json");
    let draft = workflow_to_authoring_draft(&example);
    let mut workflow = materialize_authoring_draft(draft, None, default_model);
    workflow.name = "Untitled workflow".to_string();
    layout_workflow_by_layers(&mut workflow).expect("feature_plan template layout");
    workflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_has_feature_plan_topology() {
        let workflow = default_authoring_template_workflow("gpt-5.5");
        assert_eq!(workflow.nodes.len(), 4);
        assert_eq!(workflow.edges.len(), 4);
        assert_eq!(workflow.name, "Untitled workflow");
        assert!(workflow.nodes.iter().any(|node| node.id == "idea"));
        assert!(workflow.nodes.iter().any(|node| node.id == "brief"));
    }
}
