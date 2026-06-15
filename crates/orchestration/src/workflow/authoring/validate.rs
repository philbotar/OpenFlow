use crate::api::{WorkflowAuthoringValidation, WorkflowValidationSummary};
use engine::{validate_workflow, Workflow, WorkflowValidationError};
use std::collections::HashSet;

#[must_use]
pub fn validate_authoring_workflow(workflow: &Workflow) -> WorkflowAuthoringValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for node in &workflow.nodes {
        if node.agent.system_prompt.trim().is_empty() {
            errors.push(format!("Node '{}' has an empty system prompt", node.label));
        }
        if node.agent.task_prompt.trim().is_empty() {
            errors.push(format!("Node '{}' has an empty task prompt", node.label));
        }
        if !node.agent.output_schema.is_object() {
            errors.push(format!(
                "Node '{}' output_schema must be a JSON object",
                node.label
            ));
        }
        if node.agent.model.trim().is_empty() {
            warnings.push(format!(
                "Node '{}' has no model; active provider default will be used at run time",
                node.label
            ));
        }
    }

    let node_ids: HashSet<_> = workflow.nodes.iter().map(|node| node.id.clone()).collect();
    for edge in &workflow.edges {
        if !node_ids.contains(&edge.from) {
            errors.push(format!(
                "Edge {} references missing from node {}",
                edge.id, edge.from
            ));
        }
        if !node_ids.contains(&edge.to) {
            errors.push(format!(
                "Edge {} references missing to node {}",
                edge.id, edge.to
            ));
        }
    }

    let dag = match validate_workflow(workflow) {
        Ok(()) => match engine::execution_layers(workflow) {
            Ok(layers) => Some(WorkflowValidationSummary {
                layer_count: layers.len(),
                layers: layers
                    .into_iter()
                    .map(|layer| layer.into_iter().map(|id| id.to_string()).collect())
                    .collect(),
            }),
            Err(error) => {
                errors.push(error.to_string());
                None
            }
        },
        Err(error) => {
            errors.push(dag_error_message(error));
            None
        }
    };

    let valid = errors.is_empty() && dag.is_some();
    WorkflowAuthoringValidation {
        valid,
        errors,
        warnings,
        dag,
    }
}

fn dag_error_message(error: WorkflowValidationError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::catalog::default_workflow;

    #[test]
    fn empty_prompts_are_semantic_errors() {
        let mut workflow = default_workflow("Bad");
        workflow.nodes[0].agent.system_prompt = "   ".to_string();
        let result = validate_authoring_workflow(&workflow);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("system prompt")));
    }
}
