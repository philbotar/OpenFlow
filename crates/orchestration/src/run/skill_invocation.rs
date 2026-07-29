use crate::error::BackendError;
use crate::settings::model::SkillSummary;
use engine::{CallableAgent, Workflow};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const SKILL_BLOCK_HEADER: &str = "--- Invoked skills ---";
const ADDITIONAL_SKILL_BLOCK_HEADER: &str = "--- Additional invoked skills ---";

pub(crate) type SkillPaths = BTreeMap<String, String>;

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
                .map(|path| (skill.id.clone(), path.to_string()))
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

pub(crate) fn skill_paths(skills: &[SkillSummary]) -> SkillPaths {
    skills
        .iter()
        .filter_map(|skill| {
            skill
                .path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(|path| (skill.id.clone(), path.to_string()))
        })
        .collect()
}

pub(crate) fn apply_explicit_skill_invocations(
    workflow: &mut Workflow,
    skill_ids: &[String],
    paths: &SkillPaths,
) -> Result<(), BackendError> {
    if skill_ids.is_empty() {
        return Ok(());
    }
    let root_ids = workflow
        .nodes
        .iter()
        .filter(|node| !workflow.edges.iter().any(|edge| edge.to == node.id))
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    for node in &mut workflow.nodes {
        if root_ids.contains(&node.id) {
            append_skill_ids(
                &mut node.agent.system_prompt,
                skill_ids,
                paths,
                "chat entrypoint",
            )?;
        }
    }
    Ok(())
}

pub(crate) fn skill_prompt_for_ids(
    skill_ids: &[String],
    paths: &SkillPaths,
    source: &str,
) -> Result<Option<String>, BackendError> {
    if skill_ids.is_empty() {
        return Ok(None);
    }
    let entries = load_skill_entries(skill_ids, paths, source)?;
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(render_skill_block(
        ADDITIONAL_SKILL_BLOCK_HEADER,
        &entries,
    )))
}

fn append_invoked_skills(
    system_prompt: &mut String,
    task_prompt: &str,
    source: &str,
    skills_by_id: &BTreeMap<String, String>,
) -> Result<(), BackendError> {
    let skill_ids = skill_invocation_candidates(task_prompt);
    if skill_ids.is_empty() {
        return Ok(());
    }

    let required_skill_ids = leading_skill_invocations(task_prompt)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut paths = Vec::with_capacity(skill_ids.len());
    for skill_id in skill_ids {
        match skills_by_id.get(skill_id.as_str()) {
            Some(path) => paths.push((skill_id, path.clone())),
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

    append_skill_entries(
        system_prompt,
        &load_skill_entries_from_paths(paths, source)?,
    );
    Ok(())
}

fn append_skill_ids(
    system_prompt: &mut String,
    skill_ids: &[String],
    paths: &SkillPaths,
    source: &str,
) -> Result<(), BackendError> {
    append_skill_entries(
        system_prompt,
        &load_skill_entries(skill_ids, paths, source)?,
    );
    Ok(())
}

fn append_skill_entries(system_prompt: &mut String, entries: &[(String, String, String)]) {
    let entries = entries
        .iter()
        .filter(|(skill_id, _, _)| !system_prompt.contains(&format!("- /{skill_id}:")))
        .cloned()
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }
    let header = if system_prompt.contains(SKILL_BLOCK_HEADER) {
        ADDITIONAL_SKILL_BLOCK_HEADER
    } else {
        SKILL_BLOCK_HEADER
    };
    if !system_prompt.trim().is_empty() {
        system_prompt.push_str("\n\n");
    }
    system_prompt.push_str(&render_skill_block(header, &entries));
}

fn load_skill_entries(
    skill_ids: &[String],
    paths: &SkillPaths,
    source: &str,
) -> Result<Vec<(String, String, String)>, BackendError> {
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for skill_id in skill_ids {
        if !seen.insert(skill_id.clone()) {
            continue;
        }
        let Some(path) = paths.get(skill_id) else {
            return Err(BackendError::SkillNotFound {
                skill_id: skill_id.clone(),
                invoked_by: source.to_string(),
            });
        };
        entries.push((
            skill_id.clone(),
            path.clone(),
            read_skill_file(skill_id, path)?,
        ));
    }
    Ok(entries)
}

fn load_skill_entries_from_paths(
    paths: Vec<(String, String)>,
    source: &str,
) -> Result<Vec<(String, String, String)>, BackendError> {
    let skill_ids = paths
        .iter()
        .map(|(skill_id, _)| skill_id.clone())
        .collect::<Vec<_>>();
    let path_map = paths.into_iter().collect::<SkillPaths>();
    load_skill_entries(&skill_ids, &path_map, source)
}

fn read_skill_file(skill_id: &str, path: &str) -> Result<String, BackendError> {
    fs::read_to_string(path).map_err(|error| BackendError::SkillReadFailed {
        skill_id: skill_id.to_string(),
        path: path.to_string(),
        error: error.to_string(),
    })
}

fn render_skill_block(header: &str, entries: &[(String, String, String)]) -> String {
    let mut block = format!(
        "{header}\nThe host read each exact SKILL.md below before this turn. Follow its instructions before other work. Resolve linked files relative to that SKILL.md. If a linked file cannot be read, report the error; never invent skill instructions."
    );
    for (skill_id, path, content) in entries {
        block.push_str(&format!(
            "\n\n- /{skill_id}: {path}\n<SKILL.md /{skill_id}>\n{content}\n</SKILL.md /{skill_id}>"
        ));
    }
    block
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

    fn write_skill(root: &tempfile::TempDir, id: &str) -> String {
        let path = root.path().join(id).join("SKILL.md");
        std::fs::create_dir_all(path.parent().expect("skill parent")).expect("create skill");
        std::fs::write(&path, format!("# {id}\n\nFollow {id}.")).expect("write skill");
        path.display().to_string()
    }

    #[test]
    fn resolves_leading_task_prompt_skills_for_nodes_and_callable_agents() {
        let skill_root = tempfile::tempdir().expect("skill root");
        let tdd_path = write_skill(&skill_root, "tdd");
        let review_path = write_skill(&skill_root, "code-review");
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
            &[skill("tdd", &tdd_path), skill("code-review", &review_path)],
        )
        .expect("resolve skills");

        let node_prompt = &workflow.nodes[0].agent.system_prompt;
        assert!(node_prompt.contains("--- Invoked skills ---"));
        assert!(node_prompt.contains(&format!("/tdd: {tdd_path}")));
        assert!(node_prompt.contains(&format!("/code-review: {review_path}")));

        let saved_prompt = &snapshots["reviewer"].system_prompt;
        assert!(saved_prompt.contains("--- Invoked skills ---"));
        assert!(!saved_prompt.contains("/tdd:"));
        assert!(saved_prompt.contains(&format!("/code-review: {review_path}")));
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
        let skill_root = tempfile::tempdir().expect("skill root");
        let tdd_path = write_skill(&skill_root, "tdd");
        let mut workflow = Workflow::new("Skill workflow");
        let mut prose_node = Node::agent("Prose", 0.0, 0.0);
        prose_node.agent.task_prompt = "Use /tdd for this task.".to_string();
        workflow.nodes.push(prose_node);

        apply_skill_invocations(
            &mut workflow,
            &mut BTreeMap::new(),
            &[skill("tdd", &tdd_path)],
        )
        .expect("resolve installed skill");

        assert!(workflow.nodes[0]
            .agent
            .system_prompt
            .contains(&format!("/tdd: {tdd_path}")));
    }

    #[test]
    fn includes_skill_file_contents_in_runtime_prompt() {
        let root = tempfile::tempdir().expect("skill root");
        let skill_path = root.path().join("tdd").join("SKILL.md");
        std::fs::create_dir_all(skill_path.parent().expect("skill parent"))
            .expect("create skill parent");
        std::fs::write(&skill_path, "# TDD\n\nWrite the test first.").expect("write skill");

        let mut workflow = Workflow::new("Skill workflow");
        let mut node = Node::agent("Prose", 0.0, 0.0);
        node.agent.task_prompt = "/tdd for this task".to_string();
        workflow.nodes.push(node);

        apply_skill_invocations(
            &mut workflow,
            &mut BTreeMap::new(),
            &[skill("tdd", skill_path.to_str().expect("skill path"))],
        )
        .expect("resolve installed skill");

        assert!(workflow.nodes[0].agent.system_prompt.contains("# TDD"));
        assert!(workflow.nodes[0]
            .agent
            .system_prompt
            .contains("Write the test first."));
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
