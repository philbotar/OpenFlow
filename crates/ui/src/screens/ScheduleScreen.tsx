import { For, Show, createMemo, createSignal } from "solid-js";
import type { Workflow } from "@/lib/types";
import {
  PanelEmptyState,
  ScheduleTimePickerModal,
  ScheduleWorkflowPickerModal,
  SidebarIcon,
  TextSelect,
} from "@/components";
import { useAppContext } from "../context/AppContext";
import {
  defaultWorkflowSchedule,
  describeWorkflowSchedule,
  describeScheduleStatus,
  formatScheduleTimestamp,
  INTERVAL_UNIT_OPTIONS,
  intervalValueMax,
  parseIntervalValue,
  scheduleDraftFromSchedule,
  scheduleFromPreset,
  scheduleForWorkflow,
  statusForWorkflow,
  workflowsAddableToSchedule,
  workflowsWithSchedules,
} from "@/lib/schedule";
import type { IntervalUnit, ScheduleDraft, SchedulePreset } from "@/lib/schedule";

function cloneDraft(draft: ScheduleDraft): ScheduleDraft {
  return { ...draft, weekdays: [...draft.weekdays] };
}

function ScheduleRow(props: { workflow: Workflow }) {
  const ctx = useAppContext();
  const [draft, setDraft] = createSignal<ScheduleDraft>(
    cloneDraft(scheduleDraftFromSchedule(scheduleForWorkflow(props.workflow))),
  );
  const [timePickerOpen, setTimePickerOpen] = createSignal(false);
  const status = () => statusForWorkflow(ctx.scheduleStatuses(), props.workflow.id);
  const timedSummary = () => describeWorkflowSchedule(scheduleFromPreset(draft()));

  const save = () => {
    void ctx.handleSaveWorkflowSchedule(props.workflow.id, scheduleFromPreset(draft()));
  };

  const remove = () => {
    void ctx.handleSaveWorkflowSchedule(props.workflow.id, null);
  };

  const setPreset = (preset: SchedulePreset) => {
    setDraft((current) => ({ ...current, preset }));
  };

  const patchDraft = (patch: Partial<ScheduleDraft>) => {
    setDraft((current) => ({ ...current, ...patch }));
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
          classList={{ active: draft().preset === "timed" }}
          onClick={() => setPreset("timed")}
        >
          At time
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
          <>
            <button
              type="button"
              class="schedule-time-trigger"
              aria-haspopup="dialog"
              title={timedSummary()}
              onClick={() => setTimePickerOpen(true)}
            >
              {timedSummary()}
            </button>
            <ScheduleTimePickerModal
              open={timePickerOpen()}
              draft={draft()}
              onClose={() => setTimePickerOpen(false)}
              onChange={patchDraft}
            />
          </>
        }
      >
        <div class="schedule-interval-field" role="group" aria-label="Repeat interval">
          <span class="schedule-interval-label">Every</span>
          <input
            class="text-input schedule-interval-value"
            type="number"
            min={1}
            max={intervalValueMax(draft().intervalUnit)}
            value={draft().intervalValue}
            onInput={(event) =>
              setDraft((current) => ({
                ...current,
                intervalValue: event.currentTarget.value,
              }))
            }
          />
          <TextSelect
            class="schedule-interval-unit"
            value={draft().intervalUnit}
            options={INTERVAL_UNIT_OPTIONS.map(([value, label]) => ({ value, label }))}
            aria-label="Repeat interval unit"
            onChange={(event) =>
              setDraft((current) => {
                const intervalUnit = event.currentTarget.value as IntervalUnit;
                return {
                  ...current,
                  intervalUnit,
                  intervalValue: String(parseIntervalValue(current.intervalValue, intervalUnit)),
                };
              })
            }
          />
        </div>
      </Show>

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
      <div class="schedule-toolbar">
        <p class="schedule-toolbar-description">
          Run workflows automatically on a repeating schedule.
        </p>
        <button
          type="button"
          class="primary-button compact"
          onClick={() => setPickerOpen(true)}
          disabled={addableWorkflows().length === 0}
        >
          <SidebarIcon name="plus" />
          Add workflow
        </button>
      </div>

      <ScheduleWorkflowPickerModal
        open={pickerOpen()}
        workflows={addableWorkflows()}
        onClose={() => setPickerOpen(false)}
        onSelect={addWorkflow}
      />

      <Show
        when={scheduledWorkflows().length > 0}
        fallback={
          <PanelEmptyState
            title="No scheduled workflows yet"
            description="Add a workflow to run it automatically."
          />
        }
      >
        <div class="schedule-table">
          <div class="schedule-table-header">
            <span>Run</span>
            <span>Workflow</span>
            <span>Schedule</span>
            <span>Time / Every</span>
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
