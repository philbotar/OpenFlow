# Engine Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the defects found in the 2026-07-02 review of `crates/engine/src/{execution,conversation,graph}` and delete confirmed-dead state.

**Architecture:** The engine crate is a hexagonal core: `InteractiveEngine` is a pure synchronous state machine driven by an async host loop (`run`) that talks to `AiPort`/`ToolPort` adapters. Workflows are DAGs scheduled in Kahn layers. All fixes below stay inside that design — no new abstractions, mostly small guards and deletions.

**Tech Stack:** Rust, tokio, serde, thiserror. Tests are plain `#[test]`/`block_on` unit tests colocated with each module.

**Verification baseline:** Before starting, run `cargo test -p engine` and confirm it passes. Every task ends with the same command.

## Review context (why each task exists)

Findings from the review, ordered by severity:

1. **Interrupt during tool execution leaves dangling `ToolCall` items in the transcript.** `mark_node_interrupted` ([mod.rs:471](crates/engine/src/execution/interactive_engine/mod.rs)) drops the node's pending tool batches, and the `run` loop skips `on_tool_results` for interrupted nodes. The transcript then ends with `ToolCall` entries that have no matching `ToolResult`. On retry, the transcript is replayed to the provider, and e.g. the Anthropic adapter (`crates/providers/src/anthropic.rs`) maps each `ToolCall` to a `tool_use` block — a `tool_use` with no following `tool_result` is a 400 from the API. Every retry of a node interrupted mid-tool fails.
2. **`start_subagent_invoke` mutates status before validation.** It sets the subagent `Active` and writes it back into `declared_subagents` *before* checking that the parent node exists. If that check fails, the subagent is permanently stuck `Active` and can never be invoked again (only `Declared`/`Completed` are invocable).
3. **`handle_declare_subagents` swallows malformed arguments.** A parse failure becomes `unwrap_or_default()` → empty declarations → a *success* tool result saying "Subagents declared and ready for invocation." The model gets false confirmation and will then fail on every `call_subagent`.
4. **Saved agent with no model produces a request with an empty model string.** `build_saved_agent_request` copies `agent.model` (default `""`) with no guard; `build_agent_request` has a `NoModelConfigured` check but the subagent path bypasses it. Ad-hoc subagents inherit the parent model, saved ones don't.
5. **`retry_node` can overflow its `u8` counter.** Manual retries increment without bound (`*retry_count += 1`); 255 user retries panics in debug builds.
6. **Dead state.** `queued_nodes` and `started_invocations_by_node` are written but never read anywhere (engine or orchestration — external code only constructs them empty). `emit_started_for_current_attempt` updates a counter nobody reads. `pending_retry_delay_ms` in the checkpoint is always `None` because `prepare_stop_checkpoint` clears `pending_retry_delay` *before* serializing it. `from_checkpoint` duplicates the `WorkflowMismatch` check that `validate_checkpoint_against_workflow` already performs. The `messages.is_empty()` fallback in `build_system_messages` is unreachable.

Overall verdict: the modules are defensible — clean port boundaries, pure state machine, layered DAG validation is a correct Kahn's algorithm, and test coverage is genuinely good. The issues are edge cases, but #1 is user-visible.

## Deliberately NOT planned (reviewed and deferred)

- **Parallel model invocation within a layer.** `run` awaits `ai.invoke` serially per node even though layers exist precisely to expose parallelism. Fixing it is a product decision (interleaved streaming in the UI, provider rate limits), not a bug fix. Do it as its own plan if wanted.
- **`note_read_call` recomputes transitive upstream reads on every read call.** O(V+E) per read; fine until a profiler disagrees.
- **`ChatMessage` mixed serde casing** (`toolCallId`/`messageKind` camelCase, rest snake_case). It is the wire format the UI already reads; changing it breaks saved run logs for zero user value.
- **`is_clarifying_question` heuristics.** Imperfect by design; acceptable.

---

### Task 1: Close dangling tool calls when a node is interrupted

**Files:**
- Modify: `crates/engine/src/execution/interactive_engine/mod.rs` (mark_node_interrupted, ~line 471)
- Test: `crates/engine/src/execution/interactive_engine/tests.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/engine/src/execution/interactive_engine/tests.rs` (the `node()` helper, `PendingToolBatch`, and `json!` are already imported/in scope; private fields like `engine.transcripts` are accessible because `tests` is a child module):

```rust
#[test]
fn mark_node_interrupted_closes_dangling_tool_calls() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes.push(node("a"));
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    let call = ToolCall {
        id: "call-1".to_string(),
        name: "bash".to_string(),
        arguments: json!({}),
    };
    engine
        .transcripts
        .entry(NodeId("a".to_string()))
        .or_default()
        .push(AgentTranscriptItem::ToolCall { call: call.clone() });
    engine.test_insert_pending_batch(PendingToolBatch {
        approval_id: "ap-1".to_string(),
        node_id: NodeId("a".to_string()),
        tool_calls: vec![call],
        requires_approval: false,
    });

    engine.mark_node_interrupted(&NodeId("a".to_string()));

    let transcript = engine.transcript(&NodeId("a".to_string()));
    assert_eq!(transcript.len(), 2);
    match &transcript[1] {
        AgentTranscriptItem::ToolResult { result } => {
            assert_eq!(result.tool_call_id, "call-1");
            assert!(result.is_error);
        }
        other => panic!("expected tool result, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p engine mark_node_interrupted_closes_dangling_tool_calls`
Expected: FAIL — `assertion failed` (transcript has 1 item, no ToolResult appended).

- [ ] **Step 3: Implement the fix**

In `crates/engine/src/execution/interactive_engine/mod.rs`, add to the existing import block near the top:

```rust
use crate::execution::tool_results::error_tool_result;
```

Replace the body of `mark_node_interrupted`:

```rust
    /// Mark a node interrupted by the user while tools are executing.
    pub fn mark_node_interrupted(&mut self, node_id: &NodeId) {
        if self.interrupted_nodes.contains(node_id) {
            return;
        }
        // Close out any transcript ToolCall left without a result so replayed
        // transcripts stay valid for providers (tool_use requires tool_result).
        let dangling_calls: Vec<ToolCall> = self
            .pending_tool_batches
            .values()
            .filter(|batch| batch.node_id == *node_id)
            .flat_map(|batch| batch.tool_calls.iter().cloned())
            .collect();
        if !dangling_calls.is_empty() {
            let transcript = self.transcripts.entry(node_id.clone()).or_default();
            for call in &dangling_calls {
                transcript.push(AgentTranscriptItem::ToolResult {
                    result: error_tool_result(
                        call,
                        "tool execution did not complete (interrupted or cancelled)",
                    ),
                });
            }
        }
        self.pending_tool_batches
            .retain(|_, batch| batch.node_id != *node_id);
        self.interrupted_nodes.insert(node_id.clone());
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p engine`
Expected: PASS (all tests, including the new one).

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/interactive_engine/mod.rs crates/engine/src/execution/interactive_engine/tests.rs
git commit -m "fix: close dangling tool calls in transcript when node interrupted"
```

---

### Task 2: Don't mark a subagent Active before validating the parent node

**Files:**
- Modify: `crates/engine/src/execution/subagent_runtime.rs:143-162`
- Test: same file, `tests` module

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/engine/src/execution/subagent_runtime.rs`:

```rust
    #[test]
    fn failed_start_does_not_mark_subagent_active() {
        let workflow = Workflow::new("Test"); // no nodes: parent lookup will fail
        let mut declared = std::collections::BTreeMap::new();
        declared.insert(
            "sub-1".to_string(),
            SubagentSummary {
                id: "sub-1".to_string(),
                name: "Researcher".to_string(),
                purpose: "Find facts".to_string(),
                status: SubagentStatus::Declared,
            },
        );
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: CALL_SUBAGENT_TOOL.to_string(),
            arguments: json!({ "subagent_id": "sub-1", "input": "go" }),
        };

        match start_subagent_invoke(
            &workflow,
            &NodeId("missing-parent".to_string()),
            &tool_call,
            &mut declared,
            &std::collections::BTreeMap::new(),
            Vec::new(),
        ) {
            SubagentStartOutcome::Failed(result) => assert!(result.is_error),
            SubagentStartOutcome::Started(..) => panic!("expected failure"),
        }
        assert_eq!(declared["sub-1"].status, SubagentStatus::Declared);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p engine failed_start_does_not_mark_subagent_active`
Expected: FAIL — status is `Active`, not `Declared`.

- [ ] **Step 3: Reorder validation before mutation**

In `start_subagent_invoke`, move the parent-node lookup above the status mutation. The affected region currently reads:

```rust
    subagent.status = SubagentStatus::Active;
    declared_subagents.insert(subagent.id.clone(), subagent.clone());

    let Some(parent_node) = workflow.nodes.iter().find(|n| n.id == *parent_node_id) else {
        return SubagentStartOutcome::Failed(error_tool_result(
            tool_call,
            format!("Parent node '{parent_node_id}' not found in workflow"),
        ));
    };
```

Change it to:

```rust
    let Some(parent_node) = workflow.nodes.iter().find(|n| n.id == *parent_node_id) else {
        return SubagentStartOutcome::Failed(error_tool_result(
            tool_call,
            format!("Parent node '{parent_node_id}' not found in workflow"),
        ));
    };

    subagent.status = SubagentStatus::Active;
    declared_subagents.insert(subagent.id.clone(), subagent.clone());
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/subagent_runtime.rs
git commit -m "fix: validate parent node before marking subagent active"
```

---

### Task 3: Return an error tool result for malformed declare_subagents arguments

**Files:**
- Modify: `crates/engine/src/execution/subagent_runtime.rs:42-74` (handle_declare_subagents)
- Test: same file, `tests` module

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn declare_subagents_rejects_malformed_arguments() {
        let mut declared = std::collections::BTreeMap::new();
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: DECLARE_SUBAGENTS_TOOL.to_string(),
            arguments: json!({ "subagents": "not-an-array" }),
        };

        let outcome =
            handle_declare_subagents(&NodeId("n1".to_string()), &tool_call, &mut declared);

        assert!(outcome.tool_result.is_error);
        assert!(outcome.summaries.is_empty());
        assert!(declared.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p engine declare_subagents_rejects_malformed_arguments`
Expected: FAIL — `is_error` is false (current code returns a success result).

- [ ] **Step 3: Implement the fix**

Replace the start of `handle_declare_subagents`:

```rust
    let declarations =
        serde_json::from_value::<SubagentDeclarationBatch>(tool_call.arguments.clone())
            .map(|batch| batch.subagents)
            .unwrap_or_default();
```

with:

```rust
    let declarations =
        match serde_json::from_value::<SubagentDeclarationBatch>(tool_call.arguments.clone()) {
            Ok(batch) => batch.subagents,
            Err(err) => {
                return DeclareSubagentsOutcome {
                    summaries: Vec::new(),
                    tool_result: error_tool_result(
                        tool_call,
                        format!(
                            "Invalid arguments for {DECLARE_SUBAGENTS_TOOL}: {err}. \
                             Expected {{\"subagents\": [{{\"name\": \"...\", \"purpose\": \"...\"}}]}}."
                        ),
                    ),
                };
            }
        };
```

(`error_tool_result` is already imported in this file.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/subagent_runtime.rs
git commit -m "fix: surface malformed declare_subagents arguments as tool error"
```

---

### Task 4: Saved agents without a model inherit the parent node's model

**Files:**
- Modify: `crates/engine/src/execution/subagent_runtime.rs:164-181` (start_subagent_invoke request assembly)
- Test: same file, `tests` module

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn saved_agent_without_model_inherits_parent_model() {
        let mut workflow = Workflow::new("Test");
        let mut node = crate::Node::agent("Parent", 0.0, 0.0);
        node.id = NodeId("parent".to_string());
        node.agent.model = "parent-model".to_string();
        workflow.nodes.push(node);

        let mut agent = CallableAgent::new("Helper");
        agent.id = "agent-1".to_string(); // model stays "" (CallableAgent default)
        let mut snapshots = std::collections::BTreeMap::new();
        snapshots.insert("agent-1".to_string(), agent);

        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: CALL_SUBAGENT_TOOL.to_string(),
            arguments: json!({ "subagent_id": "agent-1", "input": "go" }),
        };

        match start_subagent_invoke(
            &workflow,
            &NodeId("parent".to_string()),
            &tool_call,
            &mut std::collections::BTreeMap::new(),
            &snapshots,
            Vec::new(),
        ) {
            SubagentStartOutcome::Started(session, _) => {
                assert_eq!(session.request.model, "parent-model");
            }
            SubagentStartOutcome::Failed(result) => {
                panic!("unexpected failure: {}", result.content)
            }
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p engine saved_agent_without_model_inherits_parent_model`
Expected: FAIL — `request.model` is `""`.

- [ ] **Step 3: Implement the fix**

In `start_subagent_invoke`, change the request assembly from `let sub_request = if let Some(agent_def) = ...` to a mutable binding with a fallback after it:

```rust
    let mut sub_request = if let Some(agent_def) = agent_snapshots.get(&call_args.subagent_id) {
        build_saved_agent_request(
            workflow,
            agent_def,
            &subagent,
            &call_args.input,
            available_tools,
        )
    } else {
        build_adhoc_agent_request(
            workflow,
            parent_node,
            &subagent,
            &call_args.input,
            available_tools,
        )
    };
    if sub_request.model.trim().is_empty() {
        sub_request.model = parent_node.agent.model.clone();
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/subagent_runtime.rs
git commit -m "fix: fall back to parent node model for saved agents without a model"
```

---

### Task 5: Saturate the manual retry counter

**Files:**
- Modify: `crates/engine/src/execution/interactive_engine/mod.rs:500-501` (retry_node)
- Test: `crates/engine/src/execution/interactive_engine/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn retry_node_saturates_retry_counter() {
    let mut workflow = Workflow::new("wf");
    workflow.nodes.push(node("a"));
    let mut engine = InteractiveEngine::new(workflow, None, None).unwrap();
    for _ in 0..300 {
        engine
            .failed_nodes
            .insert(NodeId("a".to_string()), "boom".to_string());
        engine.retry_node(&NodeId("a".to_string())).unwrap();
    }
    assert_eq!(engine.model_attempt_for_node(&NodeId("a".to_string())), u8::MAX);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p engine retry_node_saturates_retry_counter`
Expected: FAIL — panics with `attempt to add with overflow` (debug build) at retry 256.

- [ ] **Step 3: Implement the fix**

In `retry_node`, replace:

```rust
        let retry_count = self.retries_by_node.entry(node_id.clone()).or_default();
        *retry_count += 1;
```

with:

```rust
        let retry_count = self.retries_by_node.entry(node_id.clone()).or_default();
        *retry_count = retry_count.saturating_add(1);
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p engine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/interactive_engine/mod.rs crates/engine/src/execution/interactive_engine/tests.rs
git commit -m "fix: saturate manual retry counter to avoid u8 overflow"
```

---

### Task 6: Delete dead engine and checkpoint state

This is a pure deletion; no new tests. Existing suites in `engine` and `orchestration` are the safety net. Evidence of deadness: `grep -rn "queued_nodes\|started_invocations" crates/` shows only empty-set construction outside the engine; nothing ever reads either collection; `pending_retry_delay_ms` is always `None` because `prepare_stop_checkpoint` clears `pending_retry_delay` before reading it.

Serde compatibility: previously persisted checkpoints contain these JSON keys; serde ignores unknown keys by default (no `deny_unknown_fields` on `InteractiveEngineCheckpoint`), so old checkpoints still load.

**Files:**
- Modify: `crates/engine/src/execution/interactive_engine/mod.rs`
- Modify: `crates/engine/src/execution/interactive_engine/checkpoint.rs`
- Modify: `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` (test fixture)
- Modify: `crates/orchestration/src/run/coordinator/tests.rs` (test fixture)

- [ ] **Step 1: Remove dead fields from `InteractiveEngine`** (`mod.rs`)

Delete from the struct definition and from `new()`:
- `queued_nodes: BTreeSet<NodeId>,` / `queued_nodes: BTreeSet::new(),`
- `started_invocations_by_node: BTreeMap<NodeId, u8>,` / `started_invocations_by_node: BTreeMap::new(),`

Delete the whole method `emit_started_for_current_attempt` (~line 662).

In `schedule_manual_nodes_in_layer`, delete the line:
```rust
            self.queued_nodes.insert(node_id.clone());
```

In `gather_call_ai_actions`, delete the two lines:
```rust
            self.queued_nodes.insert(node_id.clone());
            self.emit_started_for_current_attempt(&node_id);
```

- [ ] **Step 2: Remove dead fields from the checkpoint** (`checkpoint.rs`)

From `InteractiveEngineCheckpoint`, delete the fields:
```rust
    pub queued_nodes: BTreeSet<NodeId>,
    pub started_invocations_by_node: BTreeMap<NodeId, u8>,
    pub pending_retry_delay_ms: Option<u64>,
```

From `collect_checkpoint_node_ids`, delete:
```rust
    ids.extend(checkpoint.started_invocations_by_node.keys().cloned());
    ids.extend(checkpoint.queued_nodes.iter().cloned());
```

From `prepare_stop_checkpoint`, delete:
```rust
            queued_nodes: self.queued_nodes.clone(),
            started_invocations_by_node: self.started_invocations_by_node.clone(),
            pending_retry_delay_ms: self
                .pending_retry_delay
                .and_then(|delay| u64::try_from(delay.as_millis()).ok()),
```

From `from_checkpoint`, delete:
```rust
            queued_nodes: checkpoint.queued_nodes,
            started_invocations_by_node: checkpoint.started_invocations_by_node,
```
and change:
```rust
            pending_retry_delay: checkpoint.pending_retry_delay_ms.map(Duration::from_millis),
```
to:
```rust
            pending_retry_delay: None,
```

Also in `from_checkpoint`, delete the duplicate mismatch check (the following block — `validate_checkpoint_against_workflow` on the next line performs the identical check):
```rust
        if workflow.id != checkpoint.workflow_id {
            return Err(CheckpointError::WorkflowMismatch {
                checkpoint: checkpoint.workflow_id,
                workflow: workflow.id,
            });
        }
```

Remove now-unused imports flagged by the compiler (`std::time::Duration` in checkpoint.rs becomes unused).

- [ ] **Step 3: Fix orchestration test fixtures**

In `crates/orchestration/src/adapters/storage/run_checkpoint_store.rs` (~lines 213-218) and `crates/orchestration/src/run/coordinator/tests.rs` (~lines 48-49), delete the fixture lines:
```rust
                queued_nodes: BTreeSet::new(),
                started_invocations_by_node: BTreeMap::new(),
```
and (run_checkpoint_store.rs only):
```rust
                pending_retry_delay_ms: None,
```
Remove any now-unused `BTreeSet` imports the compiler flags.

- [ ] **Step 4: Run the full test suite**

Run: `cargo test -p engine && cargo test -p orchestration`
Expected: PASS. Also run `cargo clippy -p engine -p orchestration` — expect no new warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/execution/interactive_engine/mod.rs crates/engine/src/execution/interactive_engine/checkpoint.rs crates/orchestration/src/adapters/storage/run_checkpoint_store.rs crates/orchestration/src/run/coordinator/tests.rs
git commit -m "refactor: remove dead queued/started/retry-delay engine state"
```

---

### Task 7: Remove unreachable fallback in build_system_messages

**Files:**
- Modify: `crates/engine/src/execution/node_invocation.rs:184-186`

- [ ] **Step 1: Delete the unreachable branch**

In `build_system_messages`, delete:

```rust
    if messages.is_empty() {
        messages.push(NODE_RUNTIME_PREAMBLE.to_string());
    }
```

Why unreachable: the preamble is pushed unless `node.agent.system_prompt` contains the `--- OpenFlow runtime ---` marker — and in that case the node prompt is non-empty and is itself pushed. `messages` can never be empty.

- [ ] **Step 2: Run tests to verify nothing regressed**

Run: `cargo test -p engine`
Expected: PASS (existing `build_system_messages_*` tests cover both branches).

- [ ] **Step 3: Commit**

```bash
git add crates/engine/src/execution/node_invocation.rs
git commit -m "refactor: drop unreachable empty-messages fallback"
```

---

## Final verification

- [ ] `cargo test -p engine && cargo test -p orchestration` — all green
- [ ] `cargo clippy --workspace` — no new warnings
