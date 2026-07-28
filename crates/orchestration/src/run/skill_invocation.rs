use crate::error::BackendError;
use crate::settings::model::SkillSummary;
use engine::{CallableAgent, Workflow};
use std::collections::{BTreeMap, BTreeSet};

const SKILL_BLOCK_HEADER: &str = "--- Invoked skills ---";

pub(crate) fn has_skill_invocations(
    workflow: &Workflow,
    agent_snapshots: &BTreeMap<String, CallableAgent>,
) -> bool {
    workflow
        .nodes
        .iter()
        .any(|node| !skill_invocation_candidates(&node.agent.task_prompt).is_empty())
        || agent_snapshots
            .values()
            .any(|agent| !skill_invocation_candidates(&agent.task_prompt).is_empty())
}

pub(crate) fn apply_skill_invocations(
    workflow: &mut Workflow,
    agent_snapshots: &mut BTreeMap<String, CallableAgent>,
    skills: &[SkillSummary],
) -> Result<(), BackendError> {
    let skills_by_id = skills
        .iter()
        .filter_map(|skill| {
            skill
                .path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(|path| (skill.id.as_str(), path))
        })
        .collect::<BTreeMap<_, _>>();

    for node in &mut workflow.nodes {
        append_invoked_skills(
            &mut node.agent.system_prompt,
            &node.agent.task_prompt,
            &format!("node {:?}", node.label),
            &skills_by_id,
        )?;
    }
    for agent in agent_snapshots.values_mut() {
        append_invoked_skills(
            &mut agent.system_prompt,
            &agent.task_prompt,
            &format!("callable agent {:?}", agent.name),
            &skills_by_id,
        )?;
    }
    Ok(())
}

fn append_invoked_skills(
    system_prompt: &mut String,
    task_prompt: &str,
    source: &str,
    skills_by_id: &BTreeMap<&str, &str>,
) -> Result<(), BackendError> {
    let skill_ids = skill_invocation_candidates(task_prompt);
    if skill_ids.is_empty() || system_prompt.contains(SKILL_BLOCK_HEADER) {
        return Ok(());
    }

    let required_skill_ids = leading_skill_invocations(task_prompt)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut paths = Vec::with_capacity(skill_ids.len());
    for skill_id in skill_ids {
        match skills_by_id.get(skill_id.as_str()) {
            Some(path) => paths.push((skill_id, *path)),
            None if required_skill_ids.contains(&skill_id) => {
                return Err(BackendError::SkillNotFound {
                    skill_id,
                    invoked_by: source.to_string(),
                });
            }
            None => {}
        }
    }
    if paths.is_empty() {
        return Ok(());
    }

    let mut block = String::from(
        "--- Invoked skills ---\n\
The task prompt explicitly invokes the installed skills below. Before any other work, use the \
read tool to read each SKILL.md completely, then follow its instructions. Resolve linked files \
relative to that SKILL.md. If a required file cannot be read, report the error; never invent \
skill instructions.",
    );
    for (skill_id, path) in paths {
        block.push_str(&format!("\n- /{skill_id}: {path}"));
    }

    if !system_prompt.trim().is_empty() {
        system_prompt.push_str("\n\n");
    }
    system_prompt.push_str(&block);
    Ok(())
}

fn skill_invocation_candidates(task_prompt: &str) -> Vec<String> {
    let mut invoked = Vec::new();
    let mut seen = BTreeSet::new();

    for token in task_prompt.split_whitespace() {
        let Some(skill_id) = token.strip_prefix('/') else {
            continue;
        };
        if valid_skill_id(skill_id) && seen.insert(skill_id.to_string()) {
            invoked.push(skill_id.to_string());
        }
    }

    invoked
}

fn leading_skill_invocations(task_prompt: &str) -> Vec<String> {
    let mut remaining = task_prompt.trim_start();
    let mut invoked = Vec::new();
    let mut seen = BTreeSet::new();

    while let Some(after_slash) = remaining.strip_prefix('/') {
        let token_end = after_slash
            .find(char::is_whitespace)
            .unwrap_or(after_slash.len());
        let skill_id = &after_slash[..token_end];
        if !valid_skill_id(skill_id) {
            break;
        }
        if seen.insert(skill_id.to_string()) {
            invoked.push(skill_id.to_string());
        }
        remaining = after_slash[token_end..].trim_start();
    }

    invoked
}

fn valid_skill_id(skill_id: &str) -> bool {
    !skill_id.is_empty()
        && skill_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.:".contains(character))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Node;

    fn skill(id: &str, path: &str) -> SkillSummary {
        SkillSummary {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            path: Some(path.to_string()),
        }
    }

    #[test]
    fn resolves_leading_task_prompt_skills_for_nodes_and_callable_agents() {
        let mut workflow = Workflow::new("Skill workflow");
        let mut node = Node::agent("Implement", 0.0, 0.0);
        node.agent.task_prompt = "/tdd /code-review Implement the ticket.".to_string();
        workflow.nodes.push(node);

        let mut saved_agent = CallableAgent::new("Reviewer");
        saved_agent.id = "reviewer".to_string();
        saved_agent.task_prompt = "Review the supplied diff with /code-review".to_string();
        let mut snapshots = BTreeMap::from([("reviewer".to_string(), saved_agent)]);

        apply_skill_invocations(
            &mut workflow,
            &mut snapshots,
            &[
                skill("tdd", "/skills/tdd/SKILL.md"),
                skill("code-review", "/skills/code-review/SKILL.md"),
            ],
        )
        .expect("resolve skills");

        let node_prompt = &workflow.nodes[0].agent.system_prompt;
        assert!(node_prompt.contains("--- Invoked skills ---"));
        assert!(node_prompt.contains("/tdd: /skills/tdd/SKILL.md"));
        assert!(node_prompt.contains("/code-review: /skills/code-review/SKILL.md"));

        let saved_prompt = &snapshots["reviewer"].system_prompt;
        assert!(saved_prompt.contains("--- Invoked skills ---"));
        assert!(!saved_prompt.contains("/tdd:"));
        assert!(saved_prompt.contains("/code-review: /skills/code-review/SKILL.md"));
    }

    #[test]
    fn rejects_unknown_leading_skill_with_source_context() {
        let mut workflow = Workflow::new("Skill workflow");
        let mut node = Node::agent("Implement", 0.0, 0.0);
        node.agent.task_prompt = "/missing Implement the ticket.".to_string();
        workflow.nodes.push(node);

        let error = apply_skill_invocations(&mut workflow, &mut BTreeMap::new(), &[])
            .expect_err("missing skill");

        assert_eq!(
            error.to_string(),
            "skill /missing invoked by node \"Implement\" is not installed"
        );
    }

    #[test]
    fn resolves_installed_skill_mentions_after_task_text() {
        let mut workflow = Workflow::new("Skill workflow");
        let mut prose_node = Node::agent("Prose", 0.0, 0.0);
        prose_node.agent.task_prompt = "Use /tdd for this task.".to_string();
        workflow.nodes.push(prose_node);

        apply_skill_invocations(
            &mut workflow,
            &mut BTreeMap::new(),
            &[skill("tdd", "/skills/tdd/SKILL.md")],
        )
        .expect("resolve installed skill");

        assert!(workflow.nodes[0]
            .agent
            .system_prompt
            .contains("/tdd: /skills/tdd/SKILL.md"));
    }

    #[test]
    fn ignores_unknown_inline_tokens_and_absolute_paths() {
        let mut workflow = Workflow::new("Skill workflow");
        let mut prose_node = Node::agent("Prose", 0.0, 0.0);
        prose_node.agent.task_prompt =
            "Keep /not-installed literal and read /tmp/input.md.".to_string();
        workflow.nodes.push(prose_node);

        apply_skill_invocations(&mut workflow, &mut BTreeMap::new(), &[])
            .expect("ignore non-invocations");

        assert!(!workflow.nodes[0]
            .agent
            .system_prompt
            .contains("--- Invoked skills ---"));
    }
}
