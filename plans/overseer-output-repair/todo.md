# Overseer Output Repair Implementation Plan

**Goal:** Recover malformed `openflow_submit_node_output` calls during node and subagent runs with one safe, schema-validated overseer AI pass before falling back to the existing model retry path.

**Architecture:** Keep wire decoding and deterministic `jsonrepair-rs` recovery in `providers`, move the OpenFlow completion-tool contract and output-schema validation into `engine`, and add an engine-owned `RepairingAiPort` decorator. Orchestration wraps the run's existing provider once, before `AiInvocationAdapter`, so primary nodes and subagents share the same behavior and telemetry without provider-specific branching. Each workflow may select an overseer model on that same provider; an unset selection inherits the originating worker request's model.

**Tech Stack:** Rust workspace (`engine`, `providers`, `orchestration`), SolidJS/TypeScript UI, Rig 0.39 provider adapters, `serde_json`, `jsonrepair-rs`, JSON Schema validation, Tokio cancellation, inline Rust unit tests, Vitest UI tests, Wiremock provider integration tests, and orchestration workflow acceptance tests.

---

## Fixed v1 decisions

- Repair only malformed final-output calls to `openflow_submit_node_output`.
- Run deterministic parsing and normalization first; invoke the overseer only when those paths fail.
- Reuse the run's existing provider with a fresh repair-only request. A workflow-level `outputRepairModel` may override the model used for repair; blank or absent means use the originating worker request's model.
- Allow one overseer call for each failed primary invocation. The existing engine malformed-submit retry budget remains the outer bound.
- Send only the size-capped malformed arguments, sanitized validation detail, expected tool name, and output schema. Do not send the originating transcript, repository context, tool results, or private reasoning.
- Preserve the original tool name and call ID. The overseer may repair arguments only.
- Revalidate the overseer candidate through the same completion protocol before it can advance a node.
- Skip overseer repair when the response is known to be truncated or the raw candidate exceeds 64 KiB; use the existing concise/file-backed resubmission guidance instead.
- Do not repair arbitrary external tool calls, request-input calls, tool execution results, or malformed streaming fragments in v1.
- Expose `outputRepairModel` as the workflow-level **Overseer model** selector. Offer models known to the effective workflow provider, preserve a saved custom/removed model in the selector, and use **Use worker model** as the default.

## Worktree baseline

- The repository is already heavily modified across engine, providers, orchestration, UI, docs, examples, and `Cargo.lock`.
- Treat the current working tree as the baseline. Do not revert, normalize, or overwrite unrelated edits.
- `docs/architecture/provider-adapters.md` is stale and still names the removed pre-Rig adapter files; slice 5 updates it after behavior is proven.

## Execution state

**Active slice:** complete.

- [x] 1. Establish one schema-validating completion protocol and a redacted repair candidate.
- [x] 2. Preserve malformed OpenAI arguments before Rig discards the raw value.
- [x] 3. Add the bounded engine-owned overseer `AiPort` decorator.
- [x] 4. Wire and configure repair in runs, then project non-error repair telemetry.
- [x] 5. Prove the full workflow, update architecture docs, and run the canonical gate.

## Deferred follow-ups

- A separately configured overseer provider and credentials; v1 model selection stays on the run's existing provider.
- Repair of malformed `openflow_request_user_input` calls.
- Repair of external tool arguments before execution; current invalid-argument `ToolResult` feedback remains authoritative.
- Raw malformed SSE fragment recovery when Rig does not expose the accumulated argument string.
- Cross-resume overseer budgets beyond the existing checkpointed engine retry budgets.
