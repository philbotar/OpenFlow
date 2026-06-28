//! Shared node input assembly and [`AgentRequest`] construction for execution engines.

use crate::conversation::AgentTranscriptItem;
use crate::execution::RunError;
use crate::graph::{Node, NodeId, Workflow};
use crate::ports::AgentRequest;
use crate::tools::{
    merge_file_change_record, merge_read_record, FileChangeRecord, ReadRecord, ToolDefinition,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Resolved upstream adjacency for a workflow graph.
#[must_use]
pub fn build_upstream_map(workflow: &Workflow) -> HashMap<NodeId, Vec<NodeId>> {
    let mut upstream_map: HashMap<NodeId, Vec<NodeId>> = workflow
        .nodes
        .iter()
        .map(|node| (node.id.clone(), Vec::new()))
        .collect();
    for edge in &workflow.edges {
        upstream_map
            .entry(edge.to.clone())
            .or_default()
            .push(edge.from.clone());
    }
    for ids in upstream_map.values_mut() {
        ids.sort();
    }
    upstream_map
}

/// Append workflow shared context to an arbitrary system prompt base.
#[must_use]
pub fn merge_shared_context(workflow: &Workflow, base: &str) -> String {
    let shared = workflow.settings.shared_context.trim();
    if shared.is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\n--- Workflow context ---\n{shared}")
    }
}

/// Pre-system runtime contract prepended to every workflow agent's system prompt.
///
/// Keep the `## Available tools` section in sync with `orchestration/src/tool/registry.rs`
/// whenever builtins or harness tools are added or materially changed.
pub const NODE_RUNTIME_PREAMBLE: &str = "\
--- OpenFlow runtime ---\n\
You are one agent node in a workflow graph. Downstream nodes start only after you \
successfully submit this node's output.\n\
\n\
## When this node is done\n\
- The node is incomplete until you call openflow_submit_node_output exactly once.\n\
- Plain assistant text does not finish the node and does not advance the workflow.\n\
- Call submit only when: (1) the task prompt is satisfied, (2) output matches the node \
output schema, (3) you do not need more human input.\n\
\n\
## How to finish and advance to the next node\n\
Call openflow_submit_node_output with:\n\
{\"output\": <object matching the output schema>, \"assistant_message\": <string or null>}\n\
Put every schema field under \"output\", not at the top level. After a successful submit, \
the host stores your output and may run downstream nodes that depend on this one.\n\
\n\
## When to pause for a human\n\
Call openflow_request_user_input with {\"assistant_message\": \"<one direct question>\"} \
when you cannot complete the task without human clarification. assistant_message must be \
the question itself (usually ending with ?), not preamble or narration about asking. After \
the human replies, continue working toward submit.\n\
\n\
## Operating rules\n\
- Follow the node task prompt and any workflow context directly. Be concise in human-facing \
assistant messages; keep detailed reasoning in your private work.\n\
- Gather enough context before acting. Use input.upstream, input.changed_files, and input.reads \
first, then read/search/find only the files needed to make a correct change or answer.\n\
- Read before you edit. For existing files, inspect the relevant contents, preserve indentation \
and local style, and prefer the smallest edit that satisfies the task.\n\
- Recover from failed tool calls. Tool errors, failed edits, empty searches, and truncated output \
are feedback: narrow the query, read the artifact, adjust the edit, or choose another available \
tool instead of stopping immediately.\n\
- Preserve user work. Treat the execution folder as a real checkout; do not revert, delete, or \
overwrite unrelated files unless the task explicitly asks for it.\n\
- Use the available tool schema on the current request as the parameter source of truth. Do not \
invent parameters from this preamble when the live schema differs.\n\
\n\
## Available tools\n\
Your node's tool catalog controls which builtins are callable on each turn; runtime harness \
tools (submit, request_user_input, subagent tools) are always available. Tool schemas on each \
request are authoritative for parameters.\n\
\n\
### Read and search\n\
- read — read a local file, directory listing, HTTP(S) URL, or spilled tool artifact. Default \
output is numbered lines (3000-line cap). Append :start-end for a line range (e.g. src/lib.rs:10-20) \
or :raw for full unnumbered content. Truncated tool output is readable via artifact:{id} \
(same selectors apply).\n\
- search — search file contents by regex (ripgrep/Rust regex; no backrefs or lookaround). \
Gitignore-aware by default. Results cap at 500 matches — narrow the pattern or paths if you hit \
the limit.\n\
- find — find files and directories by glob (e.g. **/*.rs). Results cap at 200 paths — narrow \
the pattern if you hit the limit.\n\
- ast_grep — search code structurally with ast-grep patterns ($VAR metavariables). Prefer over \
search when matching syntax trees rather than raw text.\n\
\n\
### Write and edit\n\
- write — create or overwrite a file under the execution folder. Prefer edit for existing files.\n\
- edit — edit files two ways: (1) replace-mode — path + edits[] where old_text must match \
exactly and uniquely unless all:true; (2) hashline-mode — input string with ¶path#TAG sections \
copied from read output.\n\
- apply_patch — apply a Codex-style *** Begin Patch / *** End Patch envelope. Usually prefer \
edit for targeted changes.\n\
\n\
### Execute\n\
- bash — run a command in the execution folder. Use cwd for the working directory (not \
cd dir && …). Prefer read/search/find/edit/write when they suffice. Output over 50KB is \
truncated to an artifact (read via artifact:{id}). Returns merged stdout/stderr, wall time, \
and exit code.\n\
- Run shell commands non-interactive: pass flags that avoid prompts, avoid pagers, and do not \
start long-running foreground servers or watchers unless the task requires them.\n\
\n\
### Subagents\n\
- openflow_declare_subagents — declare subagents (name + purpose) available during this run.\n\
- openflow_call_subagent — invoke a declared subagent by id with a task instruction. The \
openflow_call_subagent schema lists currently available subagents for this node.\n\
\n\
### Tool usage\n\
- Use catalog tools when they improve correctness. Tool errors are returned to you; recover \
and keep working toward submit unless the task is impossible.\n\
- Batch independent read/search/find calls when you can.\n\
- search and find skip paths matched by .gitignore (including .flow/ when ignored).\n\
- Your input JSON includes changed_files from upstream nodes — use it to avoid redundant reads.\n\
- Your input JSON includes reads (paths already read upstream + structural outline) — use it to orient; only read a listed path when you need its actual contents.\n\
- Write-tier and exec-tier tools may require human approval before running.\n\
\n\
## Project workflows\n\
When this workflow is assigned to a project, the execution folder is that project's \
repository checkout on disk. You are working inside a real codebase — not an isolated \
sandbox. Use read/search/find (and bash for git or other CLI tasks) with \
repository-relative paths. Workflow definitions for this project live under \
`.flow/workflows/` in that repo; do not confuse them with application source unless \
the task targets them. A follow-on system block may include the exact repository path.\n\
\n\
## Do not\n\
- Stop with prose only and expect the workflow to continue.\n\
- Call submit before the task is actually complete.\n\
- Ask the human for information you can discover with the available context or tools.\n\
- Use bash for file reads, searches, or edits when read/search/find/edit/write are sufficient.\n\
- Assume downstream nodes have started before submit succeeds.";

/// Assemble ordered system messages for a workflow agent node (engine-owned; providers do not edit).
#[must_use]
pub fn build_system_messages(
    workflow: &Workflow,
    node: &Node,
    project_repository_root: Option<&str>,
) -> Vec<String> {
    let mut messages = Vec::new();
    if !node
        .agent
        .system_prompt
        .contains("--- OpenFlow runtime ---")
    {
        messages.push(NODE_RUNTIME_PREAMBLE.to_string());
    }
    if let Some(root) = project_repository_root
        .map(str::trim)
        .filter(|root| !root.is_empty())
    {
        messages.push(format!(
            "--- Project repository ---\n\
This workflow is assigned to a project. You are working in the repository at:\n\
{root}\n\
All read/write/bash tools resolve paths relative to this checkout unless an absolute \
path is given."
        ));
    }
    let node_prompt = node.agent.system_prompt.trim();
    if !node_prompt.is_empty() {
        messages.push(node_prompt.to_string());
    }
    let shared = workflow.settings.shared_context.trim();
    if !shared.is_empty() {
        messages.push(format!("--- Workflow context ---\n{shared}"));
    }
    if messages.is_empty() {
        messages.push(NODE_RUNTIME_PREAMBLE.to_string());
    }
    messages
}

/// Collect file-change records from all transitive upstream nodes (deduped by path, latest timestamp wins).
#[must_use]
pub fn upstream_changed_files<S: std::hash::BuildHasher>(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>, S>,
    changed_files_by_node: &BTreeMap<NodeId, Vec<FileChangeRecord>>,
) -> Vec<FileChangeRecord> {
    let mut by_path: BTreeMap<String, FileChangeRecord> = BTreeMap::new();
    for upstream_id in transitive_upstream_ids(node_id, upstream_by_node) {
        if let Some(records) = changed_files_by_node.get(&upstream_id) {
            for record in records {
                merge_file_change_record(&mut by_path, record.clone());
            }
        }
    }
    by_path.into_values().collect()
}

/// Collect read records from all transitive upstream nodes (deduped by path, latest outline wins).
#[must_use]
pub fn upstream_reads<S: std::hash::BuildHasher>(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>, S>,
    reads_by_node: &BTreeMap<NodeId, Vec<ReadRecord>>,
) -> Vec<ReadRecord> {
    let mut by_path: BTreeMap<String, ReadRecord> = BTreeMap::new();
    for upstream_id in transitive_upstream_ids(node_id, upstream_by_node) {
        if let Some(records) = reads_by_node.get(&upstream_id) {
            for record in records {
                merge_read_record(&mut by_path, record.clone());
            }
        }
    }
    by_path.into_values().collect()
}

fn transitive_upstream_ids<S: std::hash::BuildHasher>(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>, S>,
) -> Vec<NodeId> {
    let mut visited = BTreeSet::new();
    let mut stack: Vec<NodeId> = upstream_by_node.get(node_id).cloned().unwrap_or_default();
    let mut collected = Vec::new();
    while let Some(id) = stack.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        collected.push(id.clone());
        if let Some(parents) = upstream_by_node.get(&id) {
            stack.extend(parents.iter().cloned());
        }
    }
    collected.sort();
    collected
}

/// Build the JSON input payload for a node from upstream outputs and optional entrypoint text.
#[must_use]
pub fn build_node_input<S: std::hash::BuildHasher>(
    node_id: &str,
    upstream_by_node: &HashMap<NodeId, Vec<NodeId>, S>,
    outputs_by_node: &BTreeMap<NodeId, Value>,
    entrypoint_text: Option<&str>,
    changed_files_by_node: &BTreeMap<NodeId, Vec<FileChangeRecord>>,
    reads_by_node: &BTreeMap<NodeId, Vec<ReadRecord>>,
    forward_upstream_reads: bool,
) -> Value {
    let upstream = upstream_by_node
        .get(node_id)
        .into_iter()
        .flat_map(|ids| ids.iter())
        .filter_map(|id| {
            outputs_by_node.get(id).map(|output| {
                json!({
                    "node_id": id,
                    "output": output
                })
            })
        })
        .collect::<Vec<_>>();
    let changed_files = upstream_changed_files(node_id, upstream_by_node, changed_files_by_node);
    let reads = if forward_upstream_reads {
        upstream_reads(node_id, upstream_by_node, reads_by_node)
    } else {
        Vec::new()
    };

    if upstream.is_empty() {
        if let Some(text) = entrypoint_text.filter(|text| !text.trim().is_empty()) {
            let mut payload = json!({
                "entrypoint": { "text": text },
                "upstream": []
            });
            if !changed_files.is_empty() {
                payload["changed_files"] = json!(changed_files);
            }
            if !reads.is_empty() {
                payload["reads"] = json!(reads);
            }
            return payload;
        }
    }

    let mut payload = json!({ "upstream": upstream });
    if !changed_files.is_empty() {
        payload["changed_files"] = json!(changed_files);
    }
    if !reads.is_empty() {
        payload["reads"] = json!(reads);
    }
    payload
}

/// Snapshot of runtime state needed to build an [`AgentRequest`].
pub struct NodeInvocationContext<'a> {
    pub workflow: &'a Workflow,
    pub upstream_map: &'a HashMap<NodeId, Vec<NodeId>>,
    pub outputs: &'a BTreeMap<NodeId, Value>,
    pub changed_files_by_node: &'a BTreeMap<NodeId, Vec<FileChangeRecord>>,
    pub reads_by_node: &'a BTreeMap<NodeId, Vec<ReadRecord>>,
    pub entrypoint_text: Option<&'a str>,
    pub transcript: &'a [AgentTranscriptItem],
    pub available_tools: &'a [ToolDefinition],
    pub project_repository_root: Option<&'a str>,
}

/// # Errors
/// Returns [`RunError::NodeFailed`] when the node has no model configured.
pub fn build_agent_request(
    ctx: &NodeInvocationContext<'_>,
    node: &Node,
    require_model: bool,
) -> Result<AgentRequest, RunError> {
    if require_model && node.agent.model.trim().is_empty() {
        return Err(RunError::NodeFailed {
            node_id: node.id.clone(),
            kind: crate::execution::NodeFailureKind::NoModelConfigured {
                label: node.label.clone(),
            },
        });
    }

    Ok(AgentRequest {
        workflow_id: ctx.workflow.id.clone(),
        node_id: node.id.clone(),
        node_label: node.label.clone(),
        model: node.agent.model.clone(),
        system_messages: build_system_messages(ctx.workflow, node, ctx.project_repository_root),
        task_prompt: node.agent.task_prompt.clone(),
        input: build_node_input(
            &node.id,
            ctx.upstream_map,
            ctx.outputs,
            ctx.entrypoint_text,
            ctx.changed_files_by_node,
            ctx.reads_by_node,
            ctx.workflow.settings.forward_upstream_reads,
        ),
        output_schema: node.agent.output_schema.clone(),
        tool_config: node.agent.tools.clone(),
        available_tools: ctx.available_tools.to_vec(),
        transcript: ctx.transcript.to_vec(),
        model_attempt: 1,
        reasoning_effort: node.agent.reasoning_effort.clone(),
        reasoning_budget_tokens: node.agent.reasoning_budget_tokens,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test fixtures use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::graph::{Edge, Workflow};

    #[test]
    fn build_system_messages_prepends_runtime_preamble() {
        let workflow = Workflow::new("completion");
        let mut node = crate::graph::Node::agent("idea", 0.0, 0.0);
        node.agent.system_prompt = "You are a planner.".to_string();
        let messages = build_system_messages(&workflow, &node, None);
        assert!(messages[0].contains("--- OpenFlow runtime ---"));
        assert!(messages[1].contains("You are a planner."));
        assert!(messages[0].contains("openflow_submit_node_output"));
        assert!(messages[0].contains("ast_grep"));
        assert!(messages[0].contains("artifact:{id}"));
    }

    #[test]
    fn runtime_preamble_includes_cursor_grade_operating_rules() {
        assert!(NODE_RUNTIME_PREAMBLE.contains("Gather enough context"));
        assert!(NODE_RUNTIME_PREAMBLE.contains("Read before you edit"));
        assert!(NODE_RUNTIME_PREAMBLE.contains("non-interactive"));
        assert!(NODE_RUNTIME_PREAMBLE.contains("Recover from failed tool calls"));
        assert!(NODE_RUNTIME_PREAMBLE.contains("Preserve user work"));
        assert!(NODE_RUNTIME_PREAMBLE.contains("available tool schema"));
    }

    #[test]
    fn build_system_messages_appends_shared_context_last() {
        let mut workflow = Workflow::new("shared");
        workflow.settings.shared_context = "Use the style guide.".to_string();
        let node = crate::graph::Node::agent("idea", 0.0, 0.0);
        let messages = build_system_messages(&workflow, &node, None);
        assert!(messages[1].contains("focused AI agent"));
        assert!(messages[2].contains("--- Workflow context ---"));
        assert!(messages[2].contains("Use the style guide."));
    }

    #[test]
    fn build_system_messages_includes_project_repository_root() {
        let workflow = Workflow::new("repo");
        let node = crate::graph::Node::agent("idea", 0.0, 0.0);
        let messages = build_system_messages(&workflow, &node, Some("/tmp/my-repo"));
        assert!(messages[0].contains("--- OpenFlow runtime ---"));
        assert!(messages[1].contains("--- Project repository ---"));
        assert!(messages[1].contains("/tmp/my-repo"));
    }

    #[test]
    fn blank_entrypoint_is_not_injected_into_root_input() {
        let input = build_node_input(
            "idea",
            &HashMap::from([(NodeId("idea".to_string()), Vec::new())]),
            &BTreeMap::new(),
            Some("   "),
            &BTreeMap::new(),
            &BTreeMap::new(),
            true,
        );
        assert_eq!(input, json!({"upstream": []}));
    }

    #[test]
    fn downstream_input_receives_sorted_upstream_outputs() {
        let mut workflow = Workflow::new("join");
        workflow.nodes = vec![
            crate::graph::Node::agent("root", 0.0, 0.0),
            crate::graph::Node::agent("alpha", 0.0, 0.0),
            crate::graph::Node::agent("beta", 0.0, 0.0),
            crate::graph::Node::agent("join", 0.0, 0.0),
        ];
        workflow.nodes[0].id = NodeId("root".into());
        workflow.nodes[1].id = NodeId("alpha".into());
        workflow.nodes[2].id = NodeId("beta".into());
        workflow.nodes[3].id = NodeId("join".into());
        workflow.edges = vec![
            Edge::new("root", "beta"),
            Edge::new("root", "alpha"),
            Edge::new("beta", "join"),
            Edge::new("alpha", "join"),
        ];

        let upstream_map = build_upstream_map(&workflow);
        let mut outputs = BTreeMap::new();
        outputs.insert(NodeId("alpha".into()), json!({"summary": "from alpha"}));
        outputs.insert(NodeId("beta".into()), json!({"summary": "from beta"}));

        let input = build_node_input(
            "join",
            &upstream_map,
            &outputs,
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
            true,
        );
        assert_eq!(
            input,
            json!({
                "upstream": [
                    { "node_id": "alpha", "output": { "summary": "from alpha" } },
                    { "node_id": "beta", "output": { "summary": "from beta" } }
                ]
            })
        );
    }

    #[test]
    fn downstream_input_includes_upstream_changed_files() {
        let upstream_map =
            HashMap::from([(NodeId("join".to_string()), vec![NodeId("alpha".into())])]);
        let mut outputs = BTreeMap::new();
        outputs.insert(NodeId("alpha".into()), json!({"summary": "done"}));
        let mut changed_files_by_node = BTreeMap::new();
        changed_files_by_node.insert(
            NodeId("alpha".into()),
            vec![FileChangeRecord {
                path: "src/main.rs".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: Some("+1|fn main()".to_string()),
                batch_id: None,
                timestamp_ms: 1,
            }],
        );

        let input = build_node_input(
            "join",
            &upstream_map,
            &outputs,
            None,
            &changed_files_by_node,
            &BTreeMap::new(),
            true,
        );

        assert_eq!(
            input["changed_files"],
            json!([{
                "path": "src/main.rs",
                "op": "update",
                "diffSummary": "+1|fn main()",
                "timestampMs": 1
            }])
        );
    }

    #[test]
    fn upstream_changed_files_dedupes_renames_by_destination() {
        let upstream_map =
            HashMap::from([(NodeId("join".to_string()), vec![NodeId("alpha".into())])]);
        let mut changed_files_by_node = BTreeMap::new();
        changed_files_by_node.insert(
            NodeId("alpha".into()),
            vec![
                FileChangeRecord {
                    path: "old.rs".to_string(),
                    op: crate::tools::FileChangeOp::Rename,
                    rename_to: Some("new.rs".to_string()),
                    diff_summary: None,
                    batch_id: None,
                    timestamp_ms: 1,
                },
                FileChangeRecord {
                    path: "new.rs".to_string(),
                    op: crate::tools::FileChangeOp::Update,
                    rename_to: None,
                    diff_summary: None,
                    batch_id: None,
                    timestamp_ms: 2,
                },
            ],
        );

        let files = upstream_changed_files("join", &upstream_map, &changed_files_by_node);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new.rs");
    }

    #[test]
    fn transitive_upstream_changed_files_reach_multi_hop_downstream() {
        let upstream_map = HashMap::from([
            (NodeId("beta".to_string()), vec![NodeId("alpha".into())]),
            (NodeId("gamma".to_string()), vec![NodeId("beta".into())]),
        ]);
        let mut changed_files_by_node = BTreeMap::new();
        changed_files_by_node.insert(
            NodeId("alpha".into()),
            vec![FileChangeRecord {
                path: "src/main.rs".to_string(),
                op: crate::tools::FileChangeOp::Update,
                rename_to: None,
                diff_summary: None,
                batch_id: None,
                timestamp_ms: 1,
            }],
        );

        let input = build_node_input(
            "gamma",
            &upstream_map,
            &BTreeMap::new(),
            None,
            &changed_files_by_node,
            &BTreeMap::new(),
            true,
        );

        assert_eq!(input["changed_files"].as_array().map(Vec::len), Some(1));
        assert_eq!(input["changed_files"][0]["path"], "src/main.rs");
    }

    #[test]
    fn downstream_input_includes_upstream_reads_when_forwarding_enabled() {
        let upstream_map =
            HashMap::from([(NodeId("join".to_string()), vec![NodeId("alpha".into())])]);
        let mut outputs = BTreeMap::new();
        outputs.insert(NodeId("alpha".into()), json!({"summary": "done"}));
        let mut reads_by_node = BTreeMap::new();
        reads_by_node.insert(
            NodeId("alpha".into()),
            vec![ReadRecord {
                path: "src/lib.rs".to_string(),
                outline: Some("fn main".to_string()),
            }],
        );

        let input = build_node_input(
            "join",
            &upstream_map,
            &outputs,
            None,
            &BTreeMap::new(),
            &reads_by_node,
            true,
        );

        assert_eq!(input["reads"][0]["path"], "src/lib.rs");
        assert_eq!(input["reads"][0]["outline"], "fn main");
    }

    #[test]
    fn upstream_reads_omitted_when_forwarding_disabled() {
        let upstream_map =
            HashMap::from([(NodeId("join".to_string()), vec![NodeId("alpha".into())])]);
        let mut reads_by_node = BTreeMap::new();
        reads_by_node.insert(
            NodeId("alpha".into()),
            vec![ReadRecord {
                path: "src/lib.rs".to_string(),
                outline: Some("fn main".to_string()),
            }],
        );

        let input = build_node_input(
            "join",
            &upstream_map,
            &BTreeMap::new(),
            None,
            &BTreeMap::new(),
            &reads_by_node,
            false,
        );

        assert!(input.get("reads").is_none());
    }

    #[test]
    fn build_agent_request_copies_reasoning_effort_fields() {
        let mut workflow = Workflow::new("test");
        let mut node = crate::graph::Node::agent("idea", 0.0, 0.0);
        node.agent.model = "gpt-4o".to_string();
        node.agent.reasoning_effort = Some("adaptive".to_string());
        node.agent.reasoning_budget_tokens = Some(40960);
        workflow.nodes.push(node.clone());
        let upstream_map = build_upstream_map(&workflow);
        let ctx = NodeInvocationContext {
            workflow: &workflow,
            upstream_map: &upstream_map,
            outputs: &BTreeMap::new(),
            changed_files_by_node: &BTreeMap::new(),
            reads_by_node: &BTreeMap::new(),
            entrypoint_text: None,
            transcript: &[],
            available_tools: &[],
            project_repository_root: None,
        };
        let request = build_agent_request(&ctx, &node, true).unwrap();
        assert_eq!(request.reasoning_effort, Some("adaptive".to_string()));
        assert_eq!(request.reasoning_budget_tokens, Some(40960));
    }
}
