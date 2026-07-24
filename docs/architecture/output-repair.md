# Overseer output repair

Decision record for recovering malformed `openflow_submit_node_output` calls with one bounded overseer AI pass before the engine's existing malformed-submit retry path.

## Ownership

| Concern | Owner |
| --- | --- |
| Wire JSON decode / `jsonrepair-rs` | `crates/providers` (`mapping/`, Rig adapters) |
| Submit-output envelope + schema validation | `crates/engine` (`execution/completion_protocol.rs`) |
| Redacted `OutputRepairCandidate` | `crates/engine` (`ports/outbound.rs`) |
| Bounded overseer decorator | `crates/engine` (`execution/output_repair.rs` → `RepairingAiPort`) |
| Per-run wrap + telemetry projection | `crates/orchestration` (`run/execution/drive/setup.rs`, `ai_adapter.rs`, `events.rs`) |
| Workflow setting `outputRepairModel` | Engine `WorkflowSettings` + UI Workflow Settings panel |

The overseer is **not** a workflow node or CallableAgent. It is a same-provider repair pass on the run's existing `AiPort`.

## Runtime sequence

1. Worker (node or subagent) invokes the provider through `AiInvocationAdapter` → `RepairingAiPort` → factory provider.
2. Providers decode and attempt deterministic repair; engine completion protocol validates against the node output schema.
3. On a repairable malformed final-output candidate, `RepairingAiPort` emits `OutputRepairStarted`, builds an isolated synthetic request, and calls the **inner** provider once (non-streaming).
4. Overseer output must be a completed turn with `repaired_arguments`; that value is revalidated through the same completion protocol.
5. Success → `OutputRepairSucceeded` and a normal `Completed` outcome for the worker. Failure / cancel / oversize / truncation → original primary error; existing retry caps apply.
6. Orchestration projects repair events to run-trace only (never chat, never intermediate `last_error`).

Composition point: `wire_run` wraps once so nodes and subagents share the decorator.

## Model selection

- Workflow field: `outputRepairModel` (serde alias `output_repair_model`).
- Blank or absent → use the originating worker request's model.
- Nonblank → use that model on the **same** provider credentials as the run.
- V1 does not support a separate overseer provider.

## Safety guards

- At most one overseer call per primary invocation (`MAX_OUTPUT_REPAIR_ATTEMPTS_PER_INVOCATION = 1`).
- Candidate raw arguments capped at 64 KiB; truncated finish reasons are not repairable.
- Repair request excludes worker transcript, external tools, user-input permission, and repository context.
- Inputs: malformed arguments, sanitized validation detail, fixed tool name, expected schema.
- Telemetry and `Display`/`Debug` must not expose raw arguments or private reasoning.
- Cancellation on the node or run token wins over overseer work.

## V1 scope

**In scope:** malformed `openflow_submit_node_output` for primary nodes and callable subagents.

**Out of scope (deferred):** separate overseer provider/credentials; repairing `openflow_request_user_input`; repairing external tool arguments before execution; raw SSE fragment recovery when Rig does not expose accumulated arguments; cross-resume overseer budgets beyond existing checkpointed engine retry budgets.

## Related

- [`provider-adapters.md`](provider-adapters.md) — Rig transport and deterministic recovery boundary
- [`end-to-end-runtime.md`](end-to-end-runtime.md) — run host and event path
- [`../glossary.md`](../glossary.md) — `OutputRepairCandidate`, overseer output repair
- Acceptance: `cargo nextest run -p orchestration --test workflow_acceptance --no-capture output_repair`
