# Overseer output repair

When [deterministic provider repair](provider-adapters.md) cannot fix a malformed `openflow_submit_node_output` call, the runtime can make one bounded overseer AI call before the engine retries the worker. The overseer reuses the run's `AiPort` and provider; it is not a workflow node or callable agent. The [glossary](../glossary.md) defines `OutputRepairCandidate` and overseer output repair.

## Ownership

| Concern | Owner |
| --- | --- |
| Wire JSON decode / `jsonrepair-rs` | `crates/providers` (`mapping/`, Rig adapters) |
| Submit-output envelope + schema validation | `crates/engine` (`execution/completion_protocol.rs`) |
| Redacted `OutputRepairCandidate` | `crates/engine` (`ports/outbound.rs`) |
| Bounded overseer decorator | `crates/engine` (`execution/output_repair.rs` → `RepairingAiPort`) |
| Per-run wrap + telemetry projection | `crates/orchestration` (`run/execution/drive/setup.rs`, `ai_adapter.rs`, `events.rs`) |
| Workflow setting `outputRepairModel` | Engine `WorkflowSettings` + UI Workflow Settings panel |

## Runtime sequence

1. A worker (node or subagent) calls `AiInvocationAdapter` → `RepairingAiPort` → factory provider.
2. Providers decode and try deterministic repair; the engine completion protocol validates against the node output schema.
3. On a repairable malformed final-output candidate, `RepairingAiPort` emits `OutputRepairStarted`, builds an isolated synthetic request, and calls the inner provider once (non-streaming).
4. The overseer must return a completed turn with `repaired_arguments`, revalidated through the same completion protocol.
5. Success produces `OutputRepairSucceeded` and a normal `Completed` outcome for the worker. Failure, cancel, oversize, or truncation returns the original primary error, and existing retry caps apply.
6. Orchestration projects repair events to the run trace only, never to chat or intermediate `last_error`.

`wire_run` wraps the provider once, so nodes and subagents share the decorator. [End-to-end runtime](end-to-end-runtime.md) follows the host and event path.

## Model selection

| `outputRepairModel` (serde alias `output_repair_model`) | Overseer model |
| --- | --- |
| Blank or absent | The originating worker request's model |
| Nonblank | That model, on the same provider credentials as the run |

A separate overseer provider is not supported.

## Safety guards

- At most one overseer call per primary invocation (`MAX_OUTPUT_REPAIR_ATTEMPTS_PER_INVOCATION = 1`).
- Candidate raw arguments are capped at 64 KiB, and truncated finish reasons are not repairable.
- The repair request excludes worker transcript, external tools, user-input permission, and repository context. Its inputs are the malformed arguments, sanitized validation detail, fixed tool name, and expected schema.
- Telemetry and `Display`/`Debug` must not expose raw arguments or private reasoning.
- Cancellation of the node or run token wins over overseer work.

## Scope

Repair covers malformed `openflow_submit_node_output` calls from primary nodes and callable subagents. Not supported: separate overseer credentials, repair of `openflow_request_user_input` or external tool arguments, raw SSE recovery when Rig omits accumulated arguments, and cross-resume budgets beyond the existing checkpointed engine retry budgets.

Acceptance command: `cargo nextest run -p orchestration --test workflow_acceptance --no-capture output_repair`.
