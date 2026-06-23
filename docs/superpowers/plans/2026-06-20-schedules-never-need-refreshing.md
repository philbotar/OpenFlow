# Schedules Never Need Refreshing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Schedule screen stays accurate without a manual Refresh button — schedule statuses push to the UI automatically whenever schedules change or time advances.

**Architecture:** Keep `ScheduleService` as the single source of runtime schedule state in orchestration. Add a lightweight `tick_at(now)` that recomputes `next_run_at` from in-memory entries (no workflow reload). The desktop shell already emits `schedule-event` every 30s from the schedule loop; extend it to tick before emit and emit after schedule-mutating IPC commands. The UI keeps only the existing `listenToScheduleStatuses` subscription and removes all user-facing and navigation-triggered `refreshSchedules` calls.

**Tech Stack:** Rust (`orchestration`, `desktop` IPC bridge), SolidJS (`crates/ui`), Tauri events (`schedule-event`), Vitest UI tests, existing `ScheduleService` / cron evaluator.

---

## Problem (current behavior)

| Layer | What happens today | Why user clicks Refresh |
| --- | --- | --- |
| `ScheduleService` | `next_run_at` computed only on `refresh()` (full workflow reload) or `claim_due_run()` | Stale "Next:" timestamps as clock moves |
| `desktop` schedule loop | Emits `list_schedule_statuses()` without ticking | Same stale timestamps between runs |
| `desktop` `save_workflow` | Backend refreshes schedules internally but **does not emit** `schedule-event` | UI must call `refreshSchedules` after save |
| `AppProvider` | Calls `refreshSchedules` when opening Schedule screen and after saving a schedule | Compensates for missing pushes |
| `ScheduleScreen` | Exposes a **Refresh** button | User-visible workaround |

## File map

| File | Responsibility after change |
| --- | --- |
| `crates/orchestration/src/schedule/service.rs` | Add `tick_at(now)` — recompute `next_run_at` / cron errors in place |
| `crates/orchestration/src/backend/mod.rs` | Add `tick_schedules_at(now)`; call from schedule loop path |
| `crates/orchestration/src/backend/tests.rs` | Backend integration test for tick without reload |
| `crates/desktop/src/lib.rs` | Tick + emit in loop; emit after schedule-mutating commands |
| `crates/ui/src/context/AppProvider.tsx` | Drop manual refresh on open/save; keep event listener only |
| `crates/ui/src/context/AppContext.tsx` | Remove `handleRefreshScheduleStatuses` from public context |
| `crates/ui/src/screens/ScheduleScreen.tsx` | Remove Refresh button |
| `crates/ui/src/app/App.test.tsx` | Assert event-driven updates, not `refreshSchedules` |
| `CHANGELOG.md` | Note removed Refresh UX + live schedule pushes |

**Note:** `crates/desktop` is not in the user scope tag but is required — it is the only path that pushes `schedule-event` to the UI. Tasks 3–4 touch desktop minimally (emit + tick wiring only).

---

### Task 1: Add `ScheduleService::tick_at`

**Files:**
- Modify: `crates/orchestration/src/schedule/service.rs`
- Test: `crates/orchestration/src/schedule/service.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the existing test module in `service.rs`:

```rust
#[test]
fn tick_at_recomputes_next_run_without_workflow_reload() {
    let service = ScheduleService::new();
    let workflow = workflow_with_schedule("wf-1", "0 9 * * *", true);
    service
        .refresh(&[workflow], utc("2026-06-16T08:00:00Z"))
        .expect("refresh schedules");

    assert_eq!(
        service.statuses()[0]
            .next_run_at
            .expect("next")
            .to_rfc3339(),
        "2026-06-16T09:00:00+00:00"
    );

    service.tick_at(utc("2026-06-16T10:00:00Z"));

    assert_eq!(
        service.statuses()[0]
            .next_run_at
            .expect("next")
            .to_rfc3339(),
        "2026-06-17T09:00:00+00:00"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test -p orchestration schedule::service::tests::tick_at_recomputes_next_run_without_workflow_reload -- --exact`

Expected: FAIL with `no method named tick_at`

- [ ] **Step 3: Write minimal implementation**

Add to `impl ScheduleService` in `service.rs`:

```rust
pub fn tick_at(&self, now: DateTime<Utc>) {
    let mut entries = self.entries.lock();
    for entry in entries.values_mut() {
        if !entry.schedule.enabled {
            entry.next_run_at = None;
            continue;
        }
        match next_run_after(&entry.schedule, now) {
            Ok(next) => {
                entry.next_run_at = Some(next);
            }
            Err(error) => {
                entry.next_run_at = None;
                entry.last_error = Some(error.to_string());
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk cargo test -p orchestration schedule::service::tests::tick_at_recomputes_next_run_without_workflow_reload -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
rtk git add crates/orchestration/src/schedule/service.rs
rtk git commit -m "feat(orchestration): tick schedule next-run times in memory"
```

---

### Task 2: Expose `AppBackend::tick_schedules_at`

**Files:**
- Modify: `crates/orchestration/src/backend/mod.rs`
- Test: `crates/orchestration/src/backend/tests.rs`

- [ ] **Step 1: Write the failing test**

Add to `backend/tests.rs`:

```rust
#[test]
fn tick_schedules_advances_next_run_without_reload() {
    let (backend, _dir) = backend();
    let mut workflow = backend
        .create_workflow("Scheduled".to_string())
        .expect("create workflow");
    workflow.settings.schedule = Some(engine::WorkflowSchedule {
        cron: "0 9 * * *".to_string(),
        enabled: true,
        timezone: "UTC".to_string(),
    });
    backend.save_workflow(workflow).expect("save workflow");
    backend
        .refresh_schedules_at("2026-06-16T08:00:00Z".parse().expect("timestamp"))
        .expect("refresh");

    backend.tick_schedules_at("2026-06-16T10:00:00Z".parse().expect("timestamp"));

    let statuses = backend.list_schedule_statuses();
    assert_eq!(
        statuses[0].next_run_at.expect("next").to_rfc3339(),
        "2026-06-17T09:00:00+00:00"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test -p orchestration tick_schedules_advances_next_run_without_reload -- --exact`

Expected: FAIL with `no method named tick_schedules_at`

- [ ] **Step 3: Write minimal implementation**

Add near `refresh_schedules_at` in `backend/mod.rs`:

```rust
pub fn tick_schedules_at(&self, now: DateTime<Utc>) {
    self.schedule.tick_at(now);
}

pub fn tick_schedules(&self) {
    self.tick_schedules_at(Utc::now());
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk cargo test -p orchestration tick_schedules_advances_next_run_without_reload -- --exact`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
rtk git add crates/orchestration/src/backend/mod.rs crates/orchestration/src/backend/tests.rs
rtk git commit -m "feat(orchestration): expose schedule tick on AppBackend"
```

---

### Task 3: Desktop — tick before emit and emit after schedule mutations

**Files:**
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Write the failing test**

There is no existing desktop unit test harness for emit wiring. Skip a new test file — verification is Task 6 (`verify.sh` + manual smoke). Document expected behavior here:

After `save_workflow`, UI listener receives updated statuses without calling `refresh_schedules`.

- [ ] **Step 2: Update `emit_schedule_statuses` to tick first**

Replace the body of `emit_schedule_statuses` in `lib.rs`:

```rust
fn emit_schedule_statuses(app: &tauri::AppHandle) {
    let backend = app.state::<AppBackend>();
    backend.tick_schedules();
    let _ = app.emit(SCHEDULE_EVENT, backend.list_schedule_statuses());
}
```

- [ ] **Step 3: Emit after schedule-mutating commands**

Add `app: tauri::AppHandle` parameter and emit call to these handlers (backend already refreshes on save):

```rust
#[tauri::command]
fn save_workflow(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflow: Workflow,
) -> Result<Workflow, CommandError> {
    let saved = backend.save_workflow(workflow)?;
    emit_schedule_statuses(&app);
    Ok(saved)
}

#[tauri::command]
fn save_workflows(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
    workflows: Vec<Workflow>,
) -> Result<(), CommandError> {
    backend.save_workflows(&workflows)?;
    emit_schedule_statuses(&app);
    Ok(())
}

#[tauri::command]
fn load_all_workflows(
    app: tauri::AppHandle,
    backend: tauri::State<AppBackend>,
) -> Result<Vec<Workflow>, CommandError> {
    let workflows = backend.load_all_workflows()?;
    emit_schedule_statuses(&app);
    Ok(workflows)
}
```

`bootstrap_app` already returns `schedule_statuses` from `refresh_schedules()` — no emit needed there (UI sets initial state from bootstrap payload).

- [ ] **Step 4: Build desktop crate**

Run: `rtk cargo build -p desktop`

Expected: PASS (no compile errors)

- [ ] **Step 5: Commit**

```bash
rtk git add crates/desktop/src/lib.rs
rtk git commit -m "feat(desktop): push schedule statuses on tick and workflow saves"
```

---

### Task 4: Remove manual refresh from UI state

**Files:**
- Modify: `crates/ui/src/context/AppProvider.tsx`
- Modify: `crates/ui/src/context/AppContext.tsx`

- [ ] **Step 1: Write the failing test**

In `crates/ui/src/app/App.test.tsx`, change the test `"opens schedule screen from sidebar"`:

Replace:

```typescript
expect(apiMocks.refreshSchedules).toHaveBeenCalled();
```

With:

```typescript
expect(apiMocks.refreshSchedules).not.toHaveBeenCalled();
```

Run: `npm --prefix crates/ui run test -- src/app/App.test.tsx -t "opens schedule screen from sidebar"`

Expected: FAIL (refresh still called on open)

- [ ] **Step 2: Remove refresh calls from `AppProvider.tsx`**

Delete `handleRefreshScheduleStatuses` entirely.

Change `handleOpenSchedule`:

```typescript
const handleOpenSchedule = () => {
  closeAddNodePicker();
  navigateToScreen("schedule", "nav-lateral");
};
```

Change `handleSaveWorkflowSchedule` — remove the `refreshSchedules` line; keep workflow save + toast:

```typescript
const handleSaveWorkflowSchedule = async (
  workflowId: string,
  schedule: WorkflowSchedule | null,
) => {
  const current = workflows().find((workflow) => workflow.id === workflowId);
  if (!current) return;
  const next = cloneWorkflow(current);
  next.settings.schedule = schedule;
  try {
    const saved = await desktop.saveWorkflow(next);
    setWorkflows(replaceWorkflow(workflows(), saved));
    setSuccess(`Saved schedule for "${saved.name}"`);
  } catch (error) {
    setError(normalizeError(error));
  }
};
```

Remove `handleRefreshScheduleStatuses` from the context value object.

- [ ] **Step 3: Remove from `AppContext.tsx`**

Delete the line:

```typescript
handleRefreshScheduleStatuses: () => Promise<void>;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix crates/ui run test -- src/app/App.test.tsx -t "opens schedule screen from sidebar"`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ui/src/context/AppProvider.tsx crates/ui/src/context/AppContext.tsx crates/ui/src/app/App.test.tsx
rtk git commit -m "refactor(ui): stop manual schedule refresh on navigation"
```

---

### Task 5: Remove Refresh button from Schedule screen

**Files:**
- Modify: `crates/ui/src/screens/ScheduleScreen.tsx`
- Test: `crates/ui/src/app/App.test.tsx`

- [ ] **Step 1: Write the failing test**

Add to `describe("App schedule screen")` in `App.test.tsx`:

```typescript
test("schedule screen has no manual refresh button", async () => {
  const workflow = makeWorkflow("workflow-1", "Workflow One");
  workflow.settings.schedule = {
    cron: "0 9 * * *",
    enabled: true,
    timezone: "Australia/Perth",
  };

  const { container, dispose } = await mountApp({
    workflows: [workflow],
    agents: [makeAgent("agent-1", "Research Agent")],
    skills: FIXTURE_SKILLS,
    settings: SETTINGS,
    runState: null,
    scheduleStatuses: [],
  });

  try {
    const scheduleNav = [...container.querySelectorAll(".sidebar-nav-button")].find((item) =>
      item.textContent?.includes("Schedule"),
    ) as HTMLButtonElement;
    scheduleNav.click();
    await flush();

    expect(
      [...container.querySelectorAll("button")].some((button) =>
        button.textContent?.includes("Refresh"),
      ),
    ).toBe(false);
  } finally {
    dispose();
  }
});
```

Run: `npm --prefix crates/ui run test -- src/app/App.test.tsx -t "schedule screen has no manual refresh button"`

Expected: FAIL (Refresh button still present)

- [ ] **Step 2: Remove Refresh button from `ScheduleScreen.tsx`**

Delete the Refresh `<button>` block (lines with `handleRefreshScheduleStatuses` / "Refresh" label). Keep only the "Add workflow" button in `.schedule-header-actions`.

- [ ] **Step 3: Run test to verify it passes**

Run: `npm --prefix crates/ui run test -- src/app/App.test.tsx -t "schedule screen has no manual refresh button"`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
rtk git add crates/ui/src/screens/ScheduleScreen.tsx crates/ui/src/app/App.test.tsx
rtk git commit -m "refactor(ui): remove schedule Refresh button"
```

---

### Task 6: Event-driven save test + verification gate

**Files:**
- Modify: `crates/ui/src/app/App.test.tsx`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add test that save updates statuses via event listener**

In `"saves workflow schedule from schedule screen"`, after mounting, simulate the desktop push:

```typescript
let scheduleHandler: ((statuses: ScheduleStatus[]) => void) | undefined;
apiMocks.listenToScheduleStatuses.mockImplementation(async (handler) => {
  scheduleHandler = handler;
  return () => {};
});
apiMocks.saveWorkflow.mockImplementation(async (workflow) => {
  scheduleHandler?.([
    {
      workflowId: workflow.id,
      workflowName: workflow.name,
      enabled: workflow.settings.schedule?.enabled ?? false,
      cron: workflow.settings.schedule?.cron ?? "",
      timezone: workflow.settings.schedule?.timezone ?? "UTC",
      nextRunAt: "2026-06-16T00:15:00Z",
      lastRunAt: null,
      lastSkippedAt: null,
      lastError: null,
    },
  ]);
  return workflow;
});
```

Assert `expect(apiMocks.refreshSchedules).not.toHaveBeenCalled()` after save click.

Run: `npm --prefix crates/ui run test -- src/app/App.test.tsx -t "App schedule screen"`

Expected: all schedule tests PASS

- [ ] **Step 2: Update CHANGELOG**

Under `## Unreleased` → `### Changed`:

```markdown
- **Schedule live updates:** remove the Schedule screen Refresh button; desktop pushes `schedule-event` after workflow schedule saves and on each schedule-loop tick so next/last run timestamps stay current without manual refresh.
```

- [ ] **Step 3: Run full verification**

Run: `./scripts/verify.sh`

Expected: all steps PASS

- [ ] **Step 4: Manual smoke (desktop dev)**

Run: `npm --prefix crates/desktop run start -- dev`

Checklist:
1. Open Schedule from sidebar — no Refresh button; "Next:" shows a time.
2. Save a schedule change — "Next:" updates within one event (no button click).
3. Wait ~30s — "Next:" still advances if the prior time passed (schedule loop tick).

- [ ] **Step 5: Commit**

```bash
rtk git add crates/ui/src/app/App.test.tsx CHANGELOG.md
rtk git commit -m "test(ui): schedule screen relies on pushed statuses"
```

---

## Self-review

### 1. Spec coverage

| Requirement | Task |
| --- | --- |
| Schedules never need manual refreshing | Tasks 4–5 remove Refresh UX and pull-to-refresh calls |
| Statuses stay current as time passes | Tasks 1–3 add `tick_at` + loop emit |
| Save immediately reflects in UI | Task 3 emit after `save_workflow` + Task 6 test |
| Orchestration owns schedule semantics | Tasks 1–2 stay in `ScheduleService` / `AppBackend` |
| UI stays thin (no cron math) | UI only listens to events; no new client-side scheduling |

### 2. Placeholder scan

No TBD/TODO/similar steps. All code blocks are complete.

### 3. Type consistency

- `tick_at` / `tick_schedules_at` use `DateTime<Utc>` matching existing `refresh_schedules_at`.
- UI test payload uses camelCase `ScheduleStatus` fields matching `lib/types.ts`.
- `listenToScheduleStatuses` handler signature unchanged.

### Gap note (out of scope)

`rename_workflow` does not refresh schedule entry names in `ScheduleService`. If renamed workflows appear on the Schedule screen with stale names until next full `refresh_schedules`, file a follow-up — not part of this plan.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-20-schedules-never-need-refreshing.md`. Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
