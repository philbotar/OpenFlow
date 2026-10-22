# Overseer output repair

When [deterministic provider repair](provider-adapters.md) cannot fix a malformed `openflow_submit_node_output` call, the runtime can make one bounded overseer AI call before the engine retries the worker. The [glossary](../glossary.md) defines `OutputRepairCandidate` and overseer output repair.

## Ownership

| Concern | Owner |
| --- | --- |
| Wire JSON decode / `jsonrepair-rs` | `crates/providers` (`mapping/`, Rig adapters) |
| Submit-output envelope + schema validation | `crates/engine` (`execution/completion_protocol.rs`) |
| Redacted `OutputRepairCandidate` | `crates/engine` (`ports/outbound.rs`) |
| Bounded overseer decorator | `crates/engine` (`execution/output_repair.rs` → `RepairingAiPort`) |
| Per-run wrap + telemetry projection | `crates/orchestration` (`run/execution/drive/setup.rs`, `ai_adapter.rs`, `events.rs`) |
| Workflow setting `outputRepairModel` | Engine `WorkflowSettings` + UI Workflow Settings panel |

The overseer uses the run's existing `AiPort` and the same provider. It is separate from workflow nodes and callable agents.

## Runtime sequence

1. Worker (node or subagent) invokes the provider through `AiInvocationAdapter` → `RepairingAiPort` → factory provider.
2. Providers decode and attempt deterministic repair; engine completion protocol validates against the node output schema.
3. On a repairable malformed final-output candidate, `RepairingAiPort` emits `OutputRepairStarted`, builds an isolated synthetic request, and calls the **inner** provider once (non-streaming).
4. Overseer output must be a completed turn with `repaired_arguments`; that value is revalidated through the same completion protocol.
5. Success → `OutputRepairSucceeded` and a normal `Completed` outcome for the worker. Failure / cancel / oversize / truncation → original primary error; existing retry caps apply.
6. Orchestration projects repair events to run-trace only (never chat, never intermediate `last_error`).

`wire_run` wraps the provider once, so nodes and subagents share the decorator. [End-to-end runtime](end-to-end-runtime.md) follows the host and event path.

## Model selection

- Workflow field: `outputRepairModel` (serde alias `output_repair_model`).
- Blank or absent → use the originating worker request's model.
- Nonblank → use that model on the **same** provider credentials as the run.
- A separate overseer provider is not supported.

## Safety guards

- At most one overseer call per primary invocation (`MAX_OUTPUT_REPAIR_ATTEMPTS_PER_INVOCATION = 1`).
- Candidate raw arguments capped at 64 KiB; truncated finish reasons are not repairable.
- Repair request excludes worker transcript, external tools, user-input permission, and repository context.
- Inputs: malformed arguments, sanitized validation detail, fixed tool name, expected schema.
- Telemetry and `Display`/`Debug` must not expose raw arguments or private reasoning.
- Cancellation on the node or run token wins over overseer work.

## Scope

Repair applies to malformed `openflow_submit_node_output` calls from primary nodes and callable subagents. It does not support separate overseer credentials, repair of `openflow_request_user_input` or external tool arguments, raw SSE recovery when Rig omits accumulated arguments, or cross-resume budgets beyond the existing checkpointed engine retry budgets.

Acceptance command: `cargo nextest run -p orchestration --test workflow_acceptance --no-capture output_repair`.
