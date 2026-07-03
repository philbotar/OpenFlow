# Completion Protocol Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fragile multi-path submit/output pipeline (tool calls + JSON salvage + string heuristics) with one explicit completion contract that fails fast at parse time and needs fewer LLM retries.

**Architecture:** Keep the engine hexagon (`AiPort` → `AgentTurnOutcome` → `InteractiveEngine::on_ai_complete`). Refactor *how* providers classify a turn, not the workflow DAG. Completion is either a **strict internal tool call** (Track A) or a **dedicated structured-output invoke** (Track B). Human pause stays a single internal tool. Delete salvage/recovery paths that paper over model mistakes.

**Tech Stack:** Rust, existing `providers` + `engine` crates, serde/json schema, provider-native strict tool schemas. **Not** [Rig](https://github.com/0xplaygrounds/rig) — see §Rig decision below.

---



## Scoping (read before Task 1)



### What hurts today


| Symptom                                                | Where                                      | Why it burns tokens / causes bugs                                                                   |
| ------------------------------------------------------ | ------------------------------------------ | --------------------------------------------------------------------------------------------------- |
| Submit is a function tool beside `read`/`write`/`bash` | `mapping.rs:all_tool_specs`                | Every turn sends full submit schema + preamble; models misfire shape or mix internal+external tools |
| 3 ways to “finish” a turn                              | `resolve_tool_turn_outcome`                | Internal tool → `parse_plain_json_completion` → plain text → `NeedsUserInput`                       |
| Flat-field salvage                                     | `normalize_submit_output_arguments`        | Models omit `output` wrapper; we guess and wrap in Rust instead of rejecting                        |
| Preamble vs question guess                             | `is_clarifying_question` + `completion.rs` | String heuristics gate human pause vs auto-retry                                                    |
| Tool markup in chat                                    | `strip_tool_call_markup`                   | Models echo `<tool_call>` / fences in assistant text                                                |
| Mixed internal+external batch                          | `resolve_tool_turn_outcome:497-500`        | Hard error → full turn wasted                                                                       |


Reference: `technical-overview.md` [§5.1](../architecture/technical-overview.md) documents the tool-driven control plane intentionally. This plan **keeps** explicit completion (no “plain prose advances the DAG”) but **narrows** how completion arrives.

### Track decision (pick one before Phase 2)


| Track                                         | Completion mechanism                                                                                                                                 | Diff size                         | Tradeoff                                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------- |
| **A — Strict tool-only**                      | Keep `openflow_submit_node_output`; delete salvage paths; validate at parse; fail → retry feedback                                                   | Small (~300 LOC touched)          | Still pays submit-tool token tax every turn                                               |
| **B — Structured final invoke** (recommended) | Work turns: external tools + `request_input` only. Finish turn: separate `AiPort` invoke with provider JSON-schema / structured output, **no tools** | Medium (~800 LOC)                 | Extra engine turn type; cleanest separation; best schema enforcement                      |
| **C — Adopt Rig**                             | Replace `providers` HTTP layer with Rig clients                                                                                                      | Large (new dep, rewrite adapters) | **Does not fix** submit semantics — you still define tools and completion contract on top |


**Default recommendation:** **Track B** in two phases — ship **Track A** quick wins first (Phase 1), then add structured final invoke (Phase 2). Skip Rig unless the goal is “rewrite all provider HTTP” as a separate project.

### Rig decision

**No, not for this refactor.**

- OpenFlow already has the right seam: `engine::AiPort` + `AgentTurnOutcome` (`[outbound.rs](../../crates/engine/src/ports/outbound.rs)`).
- [Rig](https://github.com/0xplaygrounds/rig) is a general LLM/agent transport library (unified providers, streaming, tool wiring). It replaces **how you call APIs**, not **what counts as node completion** in a workflow DAG.
- Your pain is domain-specific: internal control tools, `normalize_submit_output_arguments`, `is_clarifying_question`, mixed-tool invariants. Rig agents still need you to register `openflow_submit_node_output` the same way.
- Rig’s README warns of frequent breaking changes — high migration cost for marginal gain on submission semantics.
- **Revisit Rig only if** the goal is “delete `openai_compat.rs` / `anthropic.rs` HTTP boilerplate” — file that under a separate `providers-modernization` plan, not this one.

---



## File map


| File                                                           | Role after refactor                                                                                                  |
| -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `crates/engine/src/ports/outbound.rs`                          | `AgentRequest` gains `turn_kind: AgentTurnKind` (Track B); optional `MalformedRequestInput` error variant (Track A)  |
| `crates/providers/src/mapping.rs`                              | Single completion classifier; stricter `parse_internal_tool_outcome`; delete `parse_plain_json_completion` (Track A) |
| `crates/providers/src/openai_compat.rs`                        | Wire `response_format` / structured output for `AgentTurnKind::Finish` (Track B)                                     |
| `crates/providers/src/anthropic.rs`                            | Same for Anthropic `output_format` / tool-less turn (Track B)                                                        |
| `crates/providers/src/bedrock.rs`                              | Same (Track B)                                                                                                       |
| `crates/engine/src/execution/interactive_engine/completion.rs` | Drop `is_clarifying_question` gate; parse-time errors only                                                           |
| `crates/engine/src/execution/interactive_engine/mod.rs`        | Schedule `Finish` invoke when node has no pending tools and model stopped without submit (Track B)                   |
| `crates/engine/src/execution/node_invocation.rs`               | Update `NODE_RUNTIME_PREAMBLE` for chosen track                                                                      |
| `crates/engine/src/conversation/mod.rs`                        | Delete `is_clarifying_question` (Track A)                                                                            |
| `crates/orchestration/src/workflow/authoring/service.rs`       | Authoring path uses same strict parse rules                                                                          |


---



## Phase 1 — Track A quick wins (strict tool-only)



### Task 1: Inventory test — document current completion paths

**Files:**

- Test: `crates/providers/src/mapping.rs` (extend `#[cfg(test)]`)

- [ ] **Step 1: Add table-driven test listing all paths**

```rust
#[test]
fn completion_path_matrix_is_documented() {
    // Submit via internal tool
    let tool = parse_internal_tool_outcome(
        SUBMIT_OUTPUT_TOOL,
        r#"{"output":{"summary":"ok"},"assistant_message":null}"#,
        None,
        "test",
        None,
    )
    .unwrap();
    assert!(matches!(tool, AgentTurnOutcome::Completed(_)));

    // Plain JSON recovery (TO DELETE in Task 2)
    let json = parse_plain_json_completion(Some(r#"{"summary":"ok"}"#));
    assert!(json.is_some());

    // Plain text → NeedsUserInput (TO DELETE in Task 2)
    let params = ResolveToolTurnParams {
        tool_calls: vec![],
        assistant_message: Some("Which color?".into()),
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up: true,
            error: "err",
        },
        output_schema: None,
        provider_label: "test",
        usage: None,
        filter_assistant_on_external_batch: false,
    };
    let input = resolve_tool_turn_outcome(params).unwrap();
    assert!(matches!(input, AgentTurnOutcome::NeedsUserInput(_)));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p providers completion_path_matrix_is_documented -- --nocapture`  
Expected: PASS (baseline before deletions)

- [ ] **Step 3: Commit**

```bash
git add crates/providers/src/mapping.rs
git commit -m "test: document completion path matrix before refactor"
```

---



### Task 2: Delete plain-JSON and plain-text completion bypasses

**Files:**

- Modify: `crates/providers/src/mapping.rs`
- Modify: `crates/providers/src/anthropic.rs` (set `allow_plain_text_follow_up: false` at all call sites)
- Modify: `crates/providers/src/bedrock.rs` (same)
- Modify: `crates/providers/src/openai_compat.rs` (audit `NoToolCallsPolicy` call sites)

- [ ] **Step 1: Write failing test — plain JSON no longer completes**

```rust
#[test]
fn plain_json_in_assistant_text_does_not_complete_node() {
    let params = ResolveToolTurnParams {
        tool_calls: vec![],
        assistant_message: Some(r#"{"summary":"done"}"#.into()),
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up: false,
            error: "expected tool call",
        },
        output_schema: None,
        provider_label: "test",
        usage: None,
        filter_assistant_on_external_batch: false,
    };
    let err = resolve_tool_turn_outcome(params).unwrap_err();
    assert!(matches!(err, AgentError::Failed(_)));
}
```

- [ ] **Step 2: Run test — expect FAIL** (plain JSON still succeeds today)

Run: `cargo test -p providers plain_json_in_assistant_text_does_not_complete_node -- --nocapture`

- [ ] **Step 3: Remove bypasses**

In `mapping.rs`:

- Delete `parse_plain_json_completion` and its unit tests.
- In `resolve_tool_turn_outcome`, when `tool_calls.is_empty()`, only return `NoToolCallsPolicy::Error` or `Recover { allow_plain_text_follow_up: false, ... }` — remove the `parse_plain_json_completion` branch and the `allow_plain_text_follow_up` → `NeedsUserInput` branch.
- Delete `allow_plain_text_follow_up` field from `NoToolCallsPolicy::Recover` (collapse to `NoToolCallsPolicy::Error` if nothing else needs Recover).

- [ ] **Step 4: Run providers + engine tests**

Run: `cargo test -p providers && cargo test -p engine`  
Expected: PASS (fix any tests that relied on plain JSON completion)

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/mapping.rs crates/providers/src/anthropic.rs crates/providers/src/bedrock.rs crates/providers/src/openai_compat.rs
git commit -m "refactor: remove plain JSON and plain-text completion bypasses"
```

---



### Task 3: Strict request-input parse (delete `is_clarifying_question`)

**Files:**

- Modify: `crates/providers/src/mapping.rs`
- Modify: `crates/engine/src/ports/outbound.rs`
- Modify: `crates/engine/src/execution/interactive_engine/completion.rs`
- Delete: `is_clarifying_question` from `crates/engine/src/conversation/mod.rs`
- Modify: `crates/engine/src/lib.rs` (drop re-export)
- Test: `crates/providers/src/mapping.rs`, `crates/engine/src/conversation/mod.rs`

- [ ] **Step 1: Add** `AgentError::malformed_request_input`

In `outbound.rs` next to `MalformedSubmitOutput`:

```rust
#[error("{provider_label} human-input tool message was not a direct question: {detail}")]
MalformedRequestInput {
    provider_label: String,
    detail: String,
},
```

Add `is_malformed_request_input()` mirror of `is_malformed_submit_output()`.

- [ ] **Step 2: Write failing parse tests**

```rust
#[test]
fn request_input_rejects_preamble_without_question_mark() {
    let err = parse_internal_tool_outcome(
        REQUEST_INPUT_TOOL,
        r#"{"assistant_message":"Let me check the codebase first:"}"#,
        None,
        "test",
        None,
    )
    .unwrap_err();
    assert!(matches!(err, AgentError::MalformedRequestInput { .. }));
}

#[test]
fn request_input_accepts_direct_question() {
    let ok = parse_internal_tool_outcome(
        REQUEST_INPUT_TOOL,
        r#"{"assistant_message":"Which animation style do you prefer?"}"#,
        None,
        "test",
        None,
    )
    .unwrap();
    assert!(matches!(ok, AgentTurnOutcome::NeedsUserInput(_)));
}
```

Validation rule in `parse_internal_tool_outcome` for `REQUEST_INPUT_TOOL`:

```rust
fn validate_request_input_message(message: &str) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.len() < 10 {
        return Err("message too short".into());
    }
    if !trimmed.contains('?') {
        return Err("must be a direct question ending with ?".into());
    }
    const PREAMBLE_MARKERS: [&str; 3] = ["let me ", "i'll ", "first,"];
    let lower = trimmed.to_lowercase();
    if PREAMBLE_MARKERS.iter().any(|m| lower.starts_with(m)) {
        return Err("must not be preamble — ask the question directly".into());
    }
    Ok(())
}
```

- [ ] **Step 3: Run tests — expect FAIL**

Run: `cargo test -p providers request_input_rejects -- --nocapture`

- [ ] **Step 4: Wire engine retry to new error kind**

In `completion.rs`, replace `is_clarifying_question` check with:

```rust
fn handle_malformed_request_input_retry(
    &mut self,
    node_id: &NodeId,
    error: &AgentError,
) -> bool {
    if !error.is_malformed_request_input() {
        return false;
    }
    // ... same retry counter + MALFORMED_REQUEST_INPUT_FEEDBACK as today
}
```

Change `on_ai_complete` `NeedsUserInput` arm: only call `handle_malformed_request_input_retry` when coming from an error path, not before `apply_user_input_request`. **Delete** the pre-apply `is_clarifying_question` gate entirely.

Remove `is_clarifying_question` and its tests from `conversation/mod.rs`.

- [ ] **Step 5: Regenerate public API snapshot**

Run: `cargo test -p engine public_api` (or `./scripts/verify.sh public-api`)  
Update: `crates/engine/tests/snapshots/public_api.txt` if `is_clarifying_question` removal changes the snapshot.

- [ ] **Step 6: Run full gate**

Run: `./scripts/verify.sh test clippy`

- [ ] **Step 7: Commit**

```bash
git add crates/engine crates/providers
git commit -m "refactor: validate request-input at parse time, drop clarifying heuristic"
```

---



### Task 4: Tighten submit parse — reject unwrappable flat fields

**Files:**

- Modify: `crates/providers/src/mapping.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn submit_rejects_flat_schema_fields_without_output_wrapper() {
    let schema = json!({
        "type": "object",
        "properties": { "summary": { "type": "string" } },
        "required": ["summary"]
    });
    let err = parse_internal_tool_outcome(
        SUBMIT_OUTPUT_TOOL,
        r#"{"summary":"done","assistant_message":null}"#,
        None,
        "test",
        Some(&schema),
    )
    .unwrap_err();
    assert!(matches!(err, AgentError::MalformedSubmitOutput { .. }));
}
```

- [ ] **Step 2: Run — expect FAIL** (normalize currently wraps)

Run: `cargo test -p providers submit_rejects_flat_schema_fields -- --nocapture`

- [ ] **Step 3: Narrow** `normalize_submit_output_arguments`

Keep only the branch that nests *inside* an existing `output` object (`nest_flat_fields_into_object_properties`). Remove auto-wrap of top-level schema keys and `salvage_assistant_message_into_output`. Malformed shape → `MalformedSubmitOutput` → engine retry (already exists).

- [ ] **Step 4: Run tests**

Run: `cargo test -p providers -p engine && STEP_WORKFLOW_LIVE_AI=1 cargo test -p orchestration --test workflow_acceptance -- --nocapture` (live step optional but recommended before merge)

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/mapping.rs
git commit -m "refactor: reject flat submit args instead of silent normalize"
```

---



## Phase 2 — Track B structured final invoke (after Phase 1 ships)



### Task 5: Add `AgentTurnKind` to the port contract

**Files:**

- Modify: `crates/engine/src/ports/outbound.rs`
- Modify: `crates/engine/src/execution/node_invocation.rs`

- [ ] **Step 1: Write failing test in engine**

```rust
#[test]
fn agentic_turn_includes_submit_and_request_input_tools() {
    let request = sample_agent_request_with_kind(AgentTurnKind::Agentic);
    let names = tool_names_for_request(&request); // test helper mirroring mapping::all_tool_specs
    assert!(names.contains(&"openflow_submit_node_output".to_string()));
}

#[test]
fn finish_turn_omits_all_function_tools() {
    let request = sample_agent_request_with_kind(AgentTurnKind::Finish);
    let names = tool_names_for_request(&request);
    assert!(names.is_empty());
}
```

- [ ] **Step 2: Add enum**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTurnKind {
    /// External tools + optional request_input. Submit tool still available (Track A compat).
    Agentic,
    /// No tools; provider must return JSON matching output_schema.
    Finish,
}
```

Default `AgentTurnKind::Agentic` on `AgentRequest`.

- [ ] **Step 3: Implement and run tests**

Run: `cargo test -p engine agentic_turn_includes finish_turn_omits`

- [ ] **Step 4: Commit**

```bash
git add crates/engine/src/ports/outbound.rs crates/engine/src/execution/node_invocation.rs
git commit -m "feat: add AgentTurnKind to AgentRequest"
```

---



### Task 6: Provider structured-output for `Finish` turns

**Files:**

- Modify: `crates/providers/src/mapping.rs` (`all_tool_specs` respects `turn_kind`)
- Modify: `crates/providers/src/openai_compat.rs`
- Modify: `crates/providers/src/anthropic.rs`

- [ ] **Step 1: Write failing provider test (mock HTTP)**

When `request.turn_kind == Finish`, request body must include JSON schema for `output` and must **not** include `tools` array.

- [ ] **Step 2: Implement OpenAI path**

For Chat Completions / Responses: set `response_format: { type: "json_schema", json_schema: { name: "node_output", schema: effective_output_schema(...), strict: true } }`.

Parse assistant text as `AgentTurnOutcome::Completed` with `output` = parsed JSON, `assistant_message: None`.

- [ ] **Step 3: Implement Anthropic path**

Use Anthropic structured outputs (`output_format`) when `turn_kind == Finish`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p providers`

- [ ] **Step 5: Commit**

```bash
git add crates/providers
git commit -m "feat: structured output invoke for AgentTurnKind::Finish"
```

---



### Task 7: Engine schedules `Finish` invoke

**Files:**

- Modify: `crates/engine/src/execution/interactive_engine/mod.rs` (run loop)
- Modify: `crates/engine/src/execution/node_invocation.rs` (`build_agent_request`)

- [ ] **Step 1: Write failing engine test**

Scenario: node completes tool batch with no pending tools, model returns `Failed("expected tool call")` on agentic turn → engine automatically issues one `Finish` invoke before failing the node.

- [ ] **Step 2: Implement**

In the run loop, when agentic turn ends with no tool calls and no completion:

1. If `finish_attempted` flag false for this node, set `AgentTurnKind::Finish`, invoke once.
2. On second no-completion, fail node with actionable error.

- [ ] **Step 3: Remove submit tool from agentic turns** (optional sub-step once Finish works)

`all_tool_specs`: skip `submit_output_tool` when `turn_kind == Agentic` **only after** Finish path is green in acceptance tests.

- [ ] **Step 4: Update preamble**

`NODE_RUNTIME_PREAMBLE`: explain “finish via structured output on final turn” instead of “call submit tool”.

- [ ] **Step 5: Run acceptance**

Run: `cargo test -p orchestration --test workflow_acceptance -- --nocapture`  
Run: `./scripts/verify.sh`

- [ ] **Step 6: Commit**

```bash
git add crates/engine crates/providers
git commit -m "feat: engine Finish turn with structured output completion"
```

---



## Phase 3 — Cleanup



### Task 8: Deprecate `openflow_submit_node_output` tool (Track B only)

**Files:**

- Modify: `mapping.rs`, `node_invocation.rs`, UI tool labels, authoring prompts
- Modify: `crates/ui/src/components/conversation/toolBubbleState.ts`

- [ ] **Step 1: Feature-flag or workflow-setting** `completion_mode: tool | structured`
- [ ] **Step 2: Default new workflows to** `structured`**; keep tool path one release**
- [ ] **Step 3: Delete submit tool registration when flag removed**
- [ ] **Step 4: Commit per step**

---



## Self-review


| Requirement                               | Task                                |
| ----------------------------------------- | ----------------------------------- |
| Fewer completion paths                    | Task 2                              |
| Parse-time validation replaces heuristics | Task 3                              |
| Stop silent submit salvage                | Task 4                              |
| Structured output separation              | Tasks 5–7                           |
| Rig evaluated and rejected for this scope | Scoping §                           |
| Authoring path covered                    | Task 3 notes `authoring/service.rs` |
| Public API snapshot                       | Task 3 step 5                       |
| Acceptance workflows                      | Tasks 4, 7                          |


**Placeholder scan:** None.

---



## Open questions for product (decide before Phase 2)

1. **Keep submit tool as escape hatch during Finish rollout?** Recommended: yes, one release with both paths.
2. **Authoring workflow** (`workflow-authoring` id) — force structured draft only (already blocks `request_input`)? Yes, align with `should_allow_user_input`.
3. **Bedrock parity** in Task 6 — required for your branch or defer?

