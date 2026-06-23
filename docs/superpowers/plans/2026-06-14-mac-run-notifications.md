# Mac Run Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show native macOS notifications when a workflow needs human attention or reaches a terminal outcome, instead of relying only on in-app UI updates.

**Architecture:** Keep OS-specific notification work in `crates/desktop`, because Desktop is the Tauri adapter and already bridges orchestration run events to the UI. Add a small pure classifier that maps existing `ExecutionEvent` values into notification copy, then call the Tauri notification plugin from the existing run-event bridge after each event is successfully applied to run state. Do not add UI imports or engine/orchestration state for this feature.

**Tech Stack:** Rust 2021, Tauri v2, `tauri-plugin-notification = "2.3.3"`, existing `engine::RunTelemetry`/`orchestration::run::execution::ExecutionEvent`, existing desktop tests via `cargo test -p desktop`.

---

## Source Notes

- Tauri v2 notification setup is Rust-compatible: add `tauri-plugin-notification`, initialize `.plugin(tauri_plugin_notification::init())`, and send via `NotificationExt` from Rust.
- The plugin is cross-platform, supports macOS, and `cargo info tauri-plugin-notification` currently resolves `2.3.3`.
- Because Desktop sends notifications from Rust, no `@tauri-apps/plugin-notification` package and no frontend capability permission are required for this slice. Add JS bindings only if a later slice lets users test/request notifications from Settings.

## File Structure

### Create

- `crates/desktop/src/run_notifications.rs`
  - Owns native run-notification classification and sending.
  - Exposes `RunNotification`, `RunNotificationKind`, `notification_for_event`, and `show_run_notification`.
  - Contains unit tests for event-to-notification mapping.

### Modify

- `crates/desktop/src/lib.rs`
  - Add `mod run_notifications;`.
  - Initialize `tauri_plugin_notification::init()` in the Tauri builder.
  - Pass workflow name into `spawn_run_event_bridge`.
  - Call notification classification/sending after each applied execution event.

- `crates/desktop/Cargo.toml`
  - Add `tauri-plugin-notification = "2.3.3"` to desktop dependencies.

- `Cargo.lock`
  - Update through Cargo after adding the new dependency.

### Not Modified

- `crates/engine/**`
  - Existing telemetry already exposes `NodeAwaitingInput`, `ToolApprovalRequested`, `Finished`, `Aborted`, and `Error`.

- `crates/orchestration/**`
  - Existing run projection remains authoritative for UI state.

- `crates/ui/**`
  - Existing in-app toasts/chat/status UI remain unchanged.

---

### Task 1: Add Pure Notification Classification

**Files:**
- Create: `crates/desktop/src/run_notifications.rs`
- Modify: `crates/desktop/src/lib.rs`
- Test: `crates/desktop/src/run_notifications.rs`

- [ ] **Step 1: Create the failing classifier tests**

Create `crates/desktop/src/run_notifications.rs` with this initial test-focused content:

```rust
use orchestration::run::execution::ExecutionEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunNotificationKind {
    NeedsInput,
    ToolApproval,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunNotification {
    pub kind: RunNotificationKind,
    pub title: String,
    pub body: String,
}

#[must_use]
pub fn notification_for_event(
    _event: &ExecutionEvent,
    _workflow_name: &str,
) -> Option<RunNotification> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{NodeId, PendingToolApproval, RunReport, ToolCall, ToolTier, WorkflowId};
    use serde_json::json;

    #[test]
    fn notifies_when_node_awaits_human_input() {
        let event = ExecutionEvent::NodeAwaitingInput {
            node_id: NodeId("node-1".to_string()),
            label: "Review plan".to_string(),
            context: "Please review the plan.".to_string(),
            is_initial: false,
        };

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::NeedsInput);
        assert_eq!(notification.title, "Workflow needs input");
        assert_eq!(
            notification.body,
            "Launch Flow is waiting for input at Review plan."
        );
    }

    #[test]
    fn notifies_when_tool_approval_is_requested() {
        let event = ExecutionEvent::ToolApprovalRequested {
            request: PendingToolApproval {
                approval_id: "approval-1".to_string(),
                node_id: NodeId("node-1".to_string()),
                node_label: "Implementer".to_string(),
                tool_call: ToolCall {
                    id: "tool-1".to_string(),
                    name: "bash".to_string(),
                    arguments: json!({ "cmd": "cargo test -p desktop" }),
                },
                tier: ToolTier::Exec,
            },
        };

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::ToolApproval);
        assert_eq!(notification.title, "Tool approval needed");
        assert_eq!(
            notification.body,
            "Implementer wants to run bash in Launch Flow."
        );
    }

    #[test]
    fn notifies_when_workflow_finishes() {
        let event = ExecutionEvent::Finished(RunReport {
            workflow_id: WorkflowId("workflow-1".to_string()),
            events: Vec::new(),
            outputs: vec![
                engine::NodeRunOutput {
                    node_id: NodeId("node-1".to_string()),
                    output: json!({ "ok": true }),
                },
                engine::NodeRunOutput {
                    node_id: NodeId("node-2".to_string()),
                    output: json!({ "ok": true }),
                },
            ],
        });

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::Completed);
        assert_eq!(notification.title, "Workflow complete");
        assert_eq!(notification.body, "Launch Flow completed 2 nodes.");
    }

    #[test]
    fn notifies_when_workflow_errors() {
        let event = ExecutionEvent::Error("provider request failed".to_string());

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::Failed);
        assert_eq!(notification.title, "Workflow stopped with an error");
        assert_eq!(
            notification.body,
            "Launch Flow stopped: provider request failed"
        );
    }

    #[test]
    fn notifies_when_workflow_aborts() {
        let event = ExecutionEvent::Aborted;

        let notification = notification_for_event(&event, "Launch Flow").expect("notification");

        assert_eq!(notification.kind, RunNotificationKind::Failed);
        assert_eq!(notification.title, "Workflow stopped");
        assert_eq!(notification.body, "Launch Flow stopped before completing.");
    }

    #[test]
    fn ignores_non_attention_events() {
        let event = ExecutionEvent::NodeStarted {
            node_id: NodeId("node-1".to_string()),
            label: "Implementer".to_string(),
        };

        assert_eq!(notification_for_event(&event, "Launch Flow"), None);
    }
}
```

- [ ] **Step 2: Wire the test module into the desktop crate**

Modify the top of `crates/desktop/src/lib.rs`:

```rust
mod run_notifications;
mod run_sleep_guard;
```

- [ ] **Step 3: Run the classifier tests and verify they fail**

Run:

```bash
cargo test -p desktop run_notifications -- --nocapture
```

Expected: FAIL. At least the five notification tests fail because `notification_for_event` returns `None`.

- [ ] **Step 4: Implement the classifier**

Replace `notification_for_event` in `crates/desktop/src/run_notifications.rs` with:

```rust
#[must_use]
pub fn notification_for_event(
    event: &ExecutionEvent,
    workflow_name: &str,
) -> Option<RunNotification> {
    let workflow_name = display_workflow_name(workflow_name);
    match event {
        ExecutionEvent::NodeAwaitingInput { label, .. } => Some(RunNotification {
            kind: RunNotificationKind::NeedsInput,
            title: "Workflow needs input".to_string(),
            body: format!("{workflow_name} is waiting for input at {label}."),
        }),
        ExecutionEvent::ToolApprovalRequested { request } => Some(RunNotification {
            kind: RunNotificationKind::ToolApproval,
            title: "Tool approval needed".to_string(),
            body: format!(
                "{} wants to run {} in {workflow_name}.",
                request.node_label, request.tool_call.name
            ),
        }),
        ExecutionEvent::Finished(report) => Some(RunNotification {
            kind: RunNotificationKind::Completed,
            title: "Workflow complete".to_string(),
            body: format!(
                "{workflow_name} completed {} {}.",
                report.outputs.len(),
                pluralize("node", report.outputs.len())
            ),
        }),
        ExecutionEvent::Error(message) => Some(RunNotification {
            kind: RunNotificationKind::Failed,
            title: "Workflow stopped with an error".to_string(),
            body: format!("{workflow_name} stopped: {message}"),
        }),
        ExecutionEvent::Aborted => Some(RunNotification {
            kind: RunNotificationKind::Failed,
            title: "Workflow stopped".to_string(),
            body: format!("{workflow_name} stopped before completing."),
        }),
        _ => None,
    }
}

fn display_workflow_name(workflow_name: &str) -> &str {
    let trimmed = workflow_name.trim();
    if trimmed.is_empty() {
        "Workflow"
    } else {
        trimmed
    }
}

fn pluralize(noun: &str, count: usize) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}
```

- [ ] **Step 5: Run the classifier tests and verify they pass**

Run:

```bash
cargo test -p desktop run_notifications -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/lib.rs crates/desktop/src/run_notifications.rs
git commit -m "test: classify workflow run notifications"
```

---

### Task 2: Add Tauri Notification Plugin and Sender

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/run_notifications.rs`
- Test: `crates/desktop/src/run_notifications.rs`

- [ ] **Step 1: Add the desktop dependency**

Modify `[dependencies]` in `crates/desktop/Cargo.toml` so it includes:

```toml
tauri.workspace = true
tauri-plugin-dialog = "2"
tauri-plugin-notification = "2.3.3"
tauri-plugin-shell = "2"
```

Keep the existing dependency list otherwise unchanged. If the current file still has accidental leading spaces before `tauri.workspace`, normalize those lines while touching this block.

- [ ] **Step 2: Update the lockfile**

Run:

```bash
cargo check -p desktop --quiet
```

Expected: This may compile for a while and should update `Cargo.lock`. If it fails because the notification plugin is not yet initialized or used, continue to the next step and rerun after code is complete.

- [ ] **Step 3: Add the notification sender function**

Append this non-test code to `crates/desktop/src/run_notifications.rs` after `notification_for_event` and its helper functions:

```rust
pub fn show_run_notification<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    notification: &RunNotification,
) {
    use tauri_plugin_notification::NotificationExt;

    if let Err(error) = app
        .notification()
        .builder()
        .title(notification.title.clone())
        .body(notification.body.clone())
        .show()
    {
        log::warn!(
            "failed to show {:?} run notification: {error}",
            notification.kind
        );
    }
}
```

- [ ] **Step 4: Add `log` if the desktop crate cannot already resolve it**

Run:

```bash
cargo check -p desktop --quiet
```

Expected: If it fails with `use of unresolved module or unlinked crate log`, add this line to `[dependencies]` in `crates/desktop/Cargo.toml`:

```toml
log = "0.4"
```

Then rerun:

```bash
cargo check -p desktop --quiet
```

Expected: PASS or only unrelated pre-existing failures. If adding `log` updates `Cargo.lock`, keep the lockfile update.

- [ ] **Step 5: Initialize the Tauri plugin**

Modify the builder plugin chain in `crates/desktop/src/lib.rs`:

```rust
    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
```

- [ ] **Step 6: Run the desktop crate check**

Run:

```bash
cargo check -p desktop --quiet
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.lock crates/desktop/Cargo.toml crates/desktop/src/lib.rs crates/desktop/src/run_notifications.rs
git commit -m "feat: add native run notification sender"
```

---

### Task 3: Emit Notifications from the Run Event Bridge

**Files:**
- Modify: `crates/desktop/src/lib.rs`
- Test: `cargo test -p desktop`

- [ ] **Step 1: Pass workflow names into the event bridge**

Replace the bridge signature in `crates/desktop/src/lib.rs`:

```rust
fn spawn_run_event_bridge(
    app: tauri::AppHandle,
    workflow_name: String,
    mut event_rx: UnboundedReceiver<ExecutionEvent>,
) {
```

- [ ] **Step 2: Notify after applying the first received event**

Inside `spawn_run_event_bridge`, replace the first event application block:

```rust
            let backend = app.state::<AppBackend>();
            let mut run_state = match backend.apply_execution_event(event).await {
                Ok(state) => state,
                Err(_) => break,
            };
```

with:

```rust
            let notification =
                run_notifications::notification_for_event(&event, workflow_name.as_str());
            let backend = app.state::<AppBackend>();
            let mut run_state = match backend.apply_execution_event(event).await {
                Ok(state) => state,
                Err(_) => break,
            };
            if let Some(notification) = notification.as_ref() {
                run_notifications::show_run_notification(&app, notification);
            }
```

- [ ] **Step 3: Notify after applying coalesced events**

Inside the `Some(event) => match backend.apply_execution_event(event).await { ... }` branch, replace it with:

```rust
                        Some(event) => {
                            let notification = run_notifications::notification_for_event(
                                &event,
                                workflow_name.as_str(),
                            );
                            match backend.apply_execution_event(event).await {
                                Ok(state) => {
                                    run_state = state;
                                    if let Some(notification) = notification.as_ref() {
                                        run_notifications::show_run_notification(&app, notification);
                                    }
                                }
                                Err(_) => {
                                    failed = true;
                                    break;
                                }
                            }
                        }
```

Keep the surrounding `tokio::select!` structure unchanged.

- [ ] **Step 4: Update start-run bridge call**

In `start_run`, capture the workflow name before moving `workflow` into `backend.start_run`:

```rust
    let workflow_name = workflow.name.clone();
    let (initial_state, event_rx) = backend
        .start_run(
            workflow,
            entrypoint,
            execution_cwd,
            &settings,
            transient_api_key.as_deref(),
        )
        .await?;
    spawn_run_event_bridge(app, workflow_name, event_rx);
```

- [ ] **Step 5: Update continue-run bridge call**

In `continue_run`, capture the workflow name before moving `workflow` into `backend.continue_run`:

```rust
    let workflow_name = workflow.name.clone();
    let (initial_state, event_rx) = backend
        .continue_run(workflow, None, &settings, transient_api_key.as_deref())
        .await?;
    spawn_run_event_bridge(app, workflow_name, event_rx);
```

- [ ] **Step 6: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: PASS.

- [ ] **Step 7: Run desktop tests**

Run:

```bash
cargo test -p desktop -- --nocapture
```

Expected: PASS. If this exposes unrelated failures from the current dirty worktree, capture the failing test names and rerun the focused notification tests:

```bash
cargo test -p desktop run_notifications -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/desktop/src/lib.rs crates/desktop/src/run_notifications.rs
git commit -m "feat: notify on workflow attention events"
```

---

### Task 4: Verify Integration and Manual macOS Behavior

**Files:**
- No source edits expected
- Verify: desktop crate, focused workflow run behavior, full project gate when practical

- [ ] **Step 1: Run the focused desktop checks**

Run:

```bash
cargo test -p desktop run_notifications -- --nocapture
cargo check -p desktop --quiet
```

Expected: PASS for both commands.

- [ ] **Step 2: Run the execution acceptance lane**

Run:

```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```

Expected: PASS. This proves the existing pause/finish events still drive workflow state correctly.

- [ ] **Step 3: Run the repo fast lane with desktop included**

Run:

```bash
./scripts/test-fast.sh --desktop
```

Expected: PASS. If failures pre-exist from unrelated dirty worktree changes, record the failing step and the focused commands from Step 1/2 as the scoped evidence.

- [ ] **Step 4: Run the full verification gate before handoff**

Run:

```bash
./scripts/verify.sh
```

Expected: PASS. If the full gate is too slow or fails in unrelated areas, report the exact failing step and include the focused pass commands.

- [ ] **Step 5: Manually verify macOS notifications in the running desktop app**

Run:

```bash
npm --prefix crates/desktop run start -- dev
```

Then:

1. Start a workflow that reaches a manual input pause.
2. Confirm macOS shows a native notification titled `Workflow needs input`.
3. Start or continue a workflow that requests tool approval.
4. Confirm macOS shows a native notification titled `Tool approval needed`.
5. Finish a workflow.
6. Confirm macOS shows a native notification titled `Workflow complete`.

Expected: Notifications appear in macOS Notification Center/banner UI. In development on macOS, Tauri’s plugin may associate notifications with Terminal because the app is launched by the dev process; this is acceptable for local verification as long as the notification body is correct.

- [ ] **Step 6: Commit verification-only doc note only if needed**

If manual verification exposes a durable macOS development caveat that should be documented, add it to `docs/contributing/testing-workflows.md` under Local Dev Loops. Otherwise do not edit docs.

When editing docs, use this exact text:

```markdown
Native macOS run notifications are emitted by the Tauri desktop shell. In `npm --prefix crates/desktop run start -- dev`, macOS may attribute development notifications to Terminal; packaged app builds should use the app bundle identifier.
```

Commit only if the doc edit is made:

```bash
git add docs/contributing/testing-workflows.md
git commit -m "docs: note mac notification dev behavior"
```

---

## Self-Review

**Spec coverage:** This plan covers notifications for required human input (`NodeAwaitingInput`), tool approval as another required-input pause (`ToolApprovalRequested`), successful workflow completion (`Finished`), and terminal failure/abort states (`Error`, `Aborted`) so users are not left watching the app for final state.

**Placeholder scan:** No forbidden placeholder language remains. Each code-changing step includes exact code or exact replacement snippets.

**Type consistency:** `RunNotificationKind`, `RunNotification`, `notification_for_event`, and `show_run_notification` are defined before use. `spawn_run_event_bridge` receives `workflow_name: String` and all call sites pass `workflow.name.clone()` before moving the workflow into backend calls.

**Boundary check:** Engine and orchestration stay unchanged. UI stays unchanged. Desktop owns native notification plugin setup and OS interaction, matching the architecture contract.
