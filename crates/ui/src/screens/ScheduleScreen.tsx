import { For, Show, createMemo, createSignal } from "solid-js";
import type { Workflow } from "../lib/types";
import { ScheduleWorkflowPickerModal } from "../components/ScheduleWorkflowPickerModal";
import { SidebarIcon } from "../components/SidebarIcon";
import { useAppContext } from "../context/AppContext";
import {
  defaultWorkflowSchedule,
  describeWorkflowSchedule,
  describeScheduleStatus,
  formatScheduleTimestamp,
  scheduleDraftFromSchedule,
  scheduleFromPreset,
  scheduleForWorkflow,
  statusForWorkflow,
  workflowsAddableToSchedule,
  workflowsWithSchedules,
} from "../lib/schedule";
import type { ScheduleDraft, SchedulePreset } from "../lib/schedule";

const WEEKDAYS = [
  ["0", "Sunday"],
  ["1", "Monday"],
  ["2", "Tuesday"],
  ["3", "Wednesday"],
  ["4", "Thursday"],
  ["5", "Friday"],
  ["6", "Saturday"],
];

const INTERVALS = [
  ["15", "15m"],
  ["30", "30m"],
  ["60", "1h"],
];

const COMMON_TIMEZONES = [
  "Australia/Perth",
  "Australia/Sydney",
  "Australia/Melbourne",
  "UTC",
  "America/New_York",
  "America/Los_Angeles",
  "Europe/London",
];

function timezoneOptions(current: string) {
  return COMMON_TIMEZONES.includes(current)
    ? COMMON_TIMEZONES
    : [current, ...COMMON_TIMEZONES];
}

function cloneDraft(draft: ScheduleDraft): ScheduleDraft {
  return { ...draft };
}

function ScheduleRow(props: { workflow: Workflow }) {
  const ctx = useAppContext();
  const [draft, setDraft] = createSignal<ScheduleDraft>(
    cloneDraft(scheduleDraftFromSchedule(scheduleForWorkflow(props.workflow))),
  );
  const status = () => statusForWorkflow(ctx.scheduleStatuses(), props.workflow.id);
  const timezoneItems = () => timezoneOptions(draft().timezone);

  const save = () => {
    void ctx.handleSaveWorkflowSchedule(props.workflow.id, scheduleFromPreset(draft()));
  };

  const remove = () => {
    void ctx.handleSaveWorkflowSchedule(props.workflow.id, null);
  };

  const setPreset = (preset: SchedulePreset) => {
    setDraft((current) => ({ ...current, preset }));
  };

  return (
    <div class="schedule-row">
      <label class="schedule-run-toggle" title={draft().enabled ? "Enabled" : "Disabled"}>
        <input
          type="checkbox"
          checked={draft().enabled}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              enabled: event.currentTarget.checked,
            }))
          }
        />
        <span>{draft().enabled ? "On" : "Off"}</span>
      </label>

      <div class="schedule-row-main">
        <div class="schedule-workflow-name">{props.workflow.name}</div>
        <div class="schedule-status-line">
          {describeScheduleStatus(status())} · {describeWorkflowSchedule(scheduleFromPreset(draft()))}
        </div>
      </div>

      <div class="schedule-segmented schedule-frequency-select" aria-label="Schedule cadence">
        <button
          type="button"
          classList={{ active: draft().preset === "daily" }}
          onClick={() => setPreset("daily")}
        >
          Daily
        </button>
        <button
          type="button"
          classList={{ active: draft().preset === "weekdays" }}
          onClick={() => setPreset("weekdays")}
        >
          Weekdays
        </button>
        <button
          type="button"
          classList={{ active: draft().preset === "weekly" }}
          onClick={() => setPreset("weekly")}
        >
          Weekly
        </button>
        <button
          type="button"
          classList={{ active: draft().preset === "interval" }}
          onClick={() => setPreset("interval")}
        >
          Repeat
        </button>
      </div>

      <Show
        when={draft().preset === "interval"}
        fallback={
          <div class="schedule-time-cell">
            <label class="schedule-field schedule-time-field">
              <input
                class="text-input"
                type="time"
                value={draft().time}
                onInput={(event) =>
                  setDraft((current) => ({
                    ...current,
                    time: event.currentTarget.value,
                  }))
                }
              />
            </label>
            <Show when={draft().preset === "weekly"}>
              <label class="schedule-field schedule-day-field">
                <select
                  class="text-input"
                  value={draft().weekday}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      weekday: event.currentTarget.value,
                    }))
                  }
                >
                  <For each={WEEKDAYS}>
                    {([value, label]) => <option value={value}>{label}</option>}
                  </For>
                </select>
              </label>
            </Show>
          </div>
        }
      >
        <div class="schedule-chip-group schedule-interval-select" aria-label="Repeat interval">
          <For each={INTERVALS}>
            {([value, label]) => (
              <button
                type="button"
                classList={{ active: draft().intervalMinutes === value }}
                onClick={() =>
                  setDraft((current) => ({
                    ...current,
                    intervalMinutes: value,
                  }))
                }
              >
                {label}
              </button>
            )}
          </For>
        </div>
      </Show>

      <label class="schedule-field schedule-timezone-field">
        <select
          class="text-input"
          value={draft().timezone}
          onChange={(event) =>
            setDraft((current) => ({
              ...current,
              timezone: event.currentTarget.value,
            }))
          }
        >
          <For each={timezoneItems()}>
            {(timezone) => <option value={timezone}>{timezone}</option>}
          </For>
        </select>
      </label>

      <div class="schedule-meta">
        <span>Next: {formatScheduleTimestamp(status()?.nextRunAt ?? null)}</span>
        <span>Last: {formatScheduleTimestamp(status()?.lastRunAt ?? null)}</span>
      </div>

      <div class="schedule-actions">
        <button class="schedule-icon-action" type="button" title="Save schedule" onClick={save}>
          <SidebarIcon name="save" />
        </button>
        <button class="schedule-icon-action danger" type="button" title="Remove schedule" onClick={remove}>
          <SidebarIcon name="trash" />
        </button>
      </div>
    </div>
  );
}

export function ScheduleScreen() {
  const ctx = useAppContext();
  const scheduledWorkflows = createMemo(() => workflowsWithSchedules(ctx.workflows()));
  const addableWorkflows = createMemo(() => workflowsAddableToSchedule(ctx.workflows()));
  const [pickerOpen, setPickerOpen] = createSignal(false);

  const addWorkflow = (workflowId: string) => {
    if (!workflowId) return;
    void ctx.handleSaveWorkflowSchedule(workflowId, defaultWorkflowSchedule());
    setPickerOpen(false);
  };

  return (
    <section class="schedule-screen">
      <div class="schedule-header">
        <div>
          <div class="eyebrow">Automation</div>
          <h2>Schedule</h2>
        </div>
        <div class="schedule-header-actions">
          <button
            type="button"
            class="secondary-button"
            onClick={() => void ctx.handleRefreshScheduleStatuses()}
          >
            Refresh
          </button>
          <button
            type="button"
            class="primary-button schedule-add-button"
            onClick={() => setPickerOpen(true)}
            disabled={addableWorkflows().length === 0}
          >
            <SidebarIcon name="plus" />
            Add workflow
          </button>
        </div>
      </div>

      <ScheduleWorkflowPickerModal
        open={pickerOpen()}
        workflows={addableWorkflows()}
        onClose={() => setPickerOpen(false)}
        onSelect={addWorkflow}
      />

      <Show
        when={scheduledWorkflows().length > 0}
        fallback={<div class="empty-panel">No scheduled workflows yet.</div>}
      >
        <div class="schedule-table">
          <div class="schedule-table-header">
            <span>Run</span>
            <span>Workflow</span>
            <span>Schedule</span>
            <span>Time / Every</span>
            <span>Timezone</span>
            <span>Runs</span>
            <span />
          </div>
          <For each={scheduledWorkflows()}>
            {(workflow) => <ScheduleRow workflow={workflow} />}
          </For>
        </div>
      </Show>
    </section>
  );
}
