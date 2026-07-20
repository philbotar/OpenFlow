# Slice 4: Configure, Wire, and Observe Run Repair

## Goal

- Persist and edit a workflow-level overseer model, wire `RepairingAiPort` once per run so nodes and subagents repair consistently, and expose repair lifecycle events as non-error run trace entries without duplicate failure toasts.

## Current Question

- Question: None.
- Recommended answer: Resolve `workflow.settings.output_repair_model` into `OutputRepairPolicy`, compose `RepairingAiPort` around the factory provider before constructing `AiInvocationAdapter`, then give the resulting invocation adapter to both `InteractiveEngine` and `ToolPortImpl`.
- Reason: This placement applies one persisted workflow choice to every run-time AI call and lets `AiInvocationAdapter` translate repair lifecycle stream events into existing run telemetry without provider branching.

## Codebase Findings

- `crates/orchestration/src/run/coordinator/session.rs::prepare_workflow_run` creates one factory `Box<dyn AiPort>` for fresh, continued, and durably resumed runs.
- `crates/orchestration/src/run/execution/drive/setup.rs::wire_run` wraps that port in `AiInvocationAdapter` and shares the adapter with `ToolPortImpl`.
- `crates/orchestration/src/run/execution/ai_adapter.rs` currently emits `AiInvokeFailed` immediately for any final error; recoverable intermediate failures must remain inside `RepairingAiPort` until repair is exhausted.
- `crates/orchestration/src/run/execution/events.rs` already projects telemetry into run trace and controls `last_error` behavior.
- `crates/ui/src/context/appProvider/useAppProviderState.ts` deduplicates unchanged error notifications; successful repair must never set an error in the first place.
- `WorkflowSettingsPanel` can access both the active workflow and `AppSettings`; its selector can resolve the workflow's provider override before falling back to the active provider.
- `AgentConfigForm` already demonstrates the required model-option behavior: use `known_models` and retain the current value when it is no longer in the catalog.
- Test command: `cargo test -p orchestration run::execution --lib -- --nocapture`

## Ownership

- Modify: `crates/orchestration/src/run/execution/drive/setup.rs` to compose `RepairingAiPort` before `AiInvocationAdapter` and update `RunWiring`/`ToolPortImpl` generic types.
- Modify: `crates/orchestration/src/run/execution/ai_adapter.rs` so `StreamSink` maps repair lifecycle events to run telemetry and never renders them as assistant/thinking chat.
- Modify: `crates/engine/src/execution/telemetry.rs` to add `OutputRepairStarted`, `OutputRepairSucceeded`, and `OutputRepairFailed` with node ID and sanitized metadata only.
- Modify: `crates/orchestration/src/run/execution/events.rs` to add trace entries without setting `last_error` during recoverable repair.
- Modify: `crates/orchestration/src/run/execution/tests.rs` for adapter composition and projection behavior.
- Modify: `crates/orchestration/tests/support/mock_ai_stack.rs` to add typed malformed-candidate builders and synthetic-request inspection used by the run-wiring tests.
- Modify: `crates/ui/src/lib/types/index.ts` to mirror canonical `outputRepairModel?: string | null` on `WorkflowSettings`.
- Modify: `crates/ui/src/lib/workflow/clone.ts` so cloned workflows retain the overseer-model choice.
- Modify: `crates/ui/src/lib/workflow/reasoning.ts` to add a small `workflowProviderProfile(settings, workflowSettings)` resolver shared by the selector and its tests.
- Modify: `crates/ui/src/panels/WorkflowSettingsPanel.tsx` to add an **Overseer model** `TextSelect` with **Use worker model** first, followed by the effective provider's known models and any currently saved custom value.
- Modify: `crates/ui/src/panels/WorkflowSettingsPanel.test.tsx` and `crates/ui/src/lib/workflow/workflow.test.ts` for selection, provider resolution, clone persistence, and removed/custom-model preservation.
- Test: `crates/engine/tests/snapshots/public_api.txt` if telemetry variants change the public snapshot.

## Telemetry Rules

- `OutputRepairStarted`: trace status running; message identifies output repair without raw content.
- `OutputRepairSucceeded`: trace status completed; include duration/model/usage only when available, without overwriting the worker node's context-window bubble.
- `OutputRepairFailed`: trace status completed or informational; record a sanitized reason, then let the original error drive existing retry/failure state.
- Do not add repair messages to normal node chat.
- Do not emit `AiInvokeFailed`, `NodeErrored`, or `last_error` for an intermediate primary failure that the decorator repairs successfully.
- Never include malformed arguments, response text, or private reasoning in telemetry output payloads.

## Configuration Rules

- Store the setting with each workflow as `outputRepairModel`; do not add an app-global setting or desktop IPC command.
- Resolve the selector catalog from `workflow.settings.provider_id` when present and valid, otherwise from `AppSettings.active_provider`.
- Blank means **Use worker model** and persists as `null`; it never copies a transient default model string into the workflow.
- Keep a nonblank saved value visible even if it is absent from `known_models`, so opening and saving an older workflow is lossless.
- A bad or unsupported configured model must not block run startup. If repair is needed and that model call fails, return the original malformed worker error and continue through the existing bounded retry path.
- V1 does not choose separate provider credentials for the overseer.

## Steps

- [x] **Step 1: Write failing orchestration tests**
  - Prove composition order is provider → `RepairingAiPort` → `AiInvocationAdapter`.
  - Prove a repaired node emits started/succeeded trace entries and no `AiInvokeFailed` or `last_error`.
  - Prove failed repair emits sanitized repair-failed telemetry and then preserves the original retryable node behavior.
  - Exercise one subagent request through the same wrapped port.
  - Prove `wire_run` passes the workflow's trimmed `output_repair_model` into `OutputRepairPolicy` and passes `None` when absent or blank.
- [x] **Step 2: Verify RED**
  - Run: `cargo test -p orchestration --lib run::execution -- --nocapture`
  - Expected: FAIL because run wiring and telemetry projection do not know the repair decorator/events.
- [x] **Step 3: Implement the run composition**
  - Resolve `OutputRepairPolicy` from the workflow settings and wrap the existing run-scoped provider in `RepairingAiPort` inside `wire_run`.
  - Keep `prepare_workflow_run` and provider factory behavior unchanged.
  - Update `AiInvocationAdapter::StreamSink` and event projection for the three lifecycle events.
  - Preserve cancellation-token and node-attempt behavior already owned by `AiInvocationAdapter`.
- [x] **Step 4: Verify GREEN**
  - Run: `cargo test -p orchestration --lib run::execution -- --nocapture`
  - Expected: PASS for node repair, fallback, telemetry, redaction, and subagent parity.
- [x] **Step 5: Add and verify the workflow setting UI**
  - Add the TypeScript field, clone support, effective-provider resolver, and Workflow Settings selector.
  - Run: `npm --prefix crates/ui run test -- src/panels/WorkflowSettingsPanel.test.tsx src/lib/workflow/workflow.test.ts`
  - Run: `npm --prefix crates/ui run typecheck`
  - Expected: PASS; selection persists, blank inherits the worker model, workflow provider overrides choose the correct catalog, and custom/removed model values remain visible.
- [x] **Step 6: Verify run coordinator and persistence compatibility**
  - Run: `cargo test -p orchestration --lib run::coordinator -- --nocapture`
  - Run: `cargo test -p orchestration --lib run::persistence -- --nocapture`
  - Expected: PASS; fresh, continued, and resumed runs all construct the same wrapper, workflow JSON retains `outputRepairModel`, and workflows without the field remain compatible.
- [x] **Step 7: Verify cross-crate execution architecture**
  - Run: `./scripts/check-architecture.sh`
  - Run: `./scripts/test-fast.sh --execution`
  - Expected: PASS; orchestration still imports only allowlisted provider factory/config types and engine construction remains in `run/execution`.

## Maintainability Gate

- [x] One composition point covers nodes and subagents.
- [x] Provider construction and credential resolution are unchanged.
- [x] The model choice is workflow data and is not duplicated in app settings, run state, or checkpoints.
- [x] Repair lifecycle events use the existing stream/telemetry seams.
- [x] Successful recovery does not enter normal error or chat paths.
- [x] No repair candidate is stored in run state or checkpoints.

## Self-Review

- [x] Generic wrapper types remain readable; add a local type alias if nesting obscures ownership.
- [x] Node cancellation and run cancellation both stop overseer work.
- [x] Context-window projection is not overwritten by repair usage.
- [x] Tests assert absence of raw secret sentinels in trace and error state.
- [x] UI copy makes the fallback explicit and does not imply that the overseer is a workflow node.
- [x] No vague implementation placeholders remain.

## Result

- Status: Complete.
- Verification: `run::execution` 67 pass; `run::coordinator` 32; `run::persistence` 2; UI panel+workflow 54; typecheck PASS; public-api + arch + `test-fast --execution` PASS.
- Notes: Subagent parity via shared `AiInvocationAdapter` wrap in `wire_run` (same port as nodes). Slice 5 covers acceptance docs and full gate.
