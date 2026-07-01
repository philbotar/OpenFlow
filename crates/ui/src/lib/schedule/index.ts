import type {
  IntervalUnit,
  ScheduleDraft,
  SchedulePreset,
  ScheduleStatus,
  Workflow,
  WorkflowSchedule,
} from "../types";

export type { IntervalUnit, ScheduleDraft, SchedulePreset };

const INTERVAL_VALUE_DEFAULTS: Record<IntervalUnit, number> = {
  minutes: 30,
  hours: 1,
  days: 1,
};

const INTERVAL_VALUE_LIMITS: Record<IntervalUnit, number> = {
  minutes: 59,
  hours: 23,
  days: 31,
};

export const INTERVAL_UNIT_OPTIONS: ReadonlyArray<readonly [IntervalUnit, string]> = [
  ["minutes", "min"],
  ["hours", "hrs"],
  ["days", "days"],
];

export const ALL_WEEKDAYS = ["0", "1", "2", "3", "4", "5", "6"] as const;
export const WEEKDAY_PRESET = ["1", "2", "3", "4", "5"] as const;

export const WEEKDAY_OPTIONS: ReadonlyArray<readonly [string, string]> = [
  ["0", "Sun"],
  ["1", "Mon"],
  ["2", "Tue"],
  ["3", "Wed"],
  ["4", "Thu"],
  ["5", "Fri"],
  ["6", "Sat"],
];

function localTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

export function intervalValueMax(unit: IntervalUnit): number {
  return INTERVAL_VALUE_LIMITS[unit];
}

export function parseIntervalValue(raw: string, unit: IntervalUnit): number {
  const parsed = Number.parseInt(raw.trim(), 10);
  if (!Number.isFinite(parsed) || parsed < 1) {
    return INTERVAL_VALUE_DEFAULTS[unit];
  }
  return Math.min(parsed, INTERVAL_VALUE_LIMITS[unit]);
}

export function normalizeWeekdays(weekdays: string[]): string[] {
  return [...new Set(weekdays.filter((day) => /^[0-6]$/.test(day)))].sort(
    (left, right) => Number(left) - Number(right),
  );
}

export function toggleWeekday(weekdays: string[], day: string): string[] {
  const normalized = normalizeWeekdays(weekdays);
  const next = new Set(normalized);
  if (next.has(day)) {
    next.delete(day);
  } else {
    next.add(day);
  }
  if (next.size === 0) {
    next.add(day);
  }
  return normalizeWeekdays([...next]);
}

export function defaultWorkflowSchedule(): WorkflowSchedule {
  return {
    cron: "0 9 * * *",
    enabled: true,
    timezone: localTimezone(),
  };
}

export function statusForWorkflow(
  statuses: ScheduleStatus[],
  workflowId: string,
): ScheduleStatus | null {
  return statuses.find((status) => status.workflowId === workflowId) ?? null;
}

export function scheduleForWorkflow(workflow: Workflow): WorkflowSchedule {
  return workflow.settings.schedule ?? defaultWorkflowSchedule();
}

export function workflowsWithSchedules(workflows: Workflow[]): Workflow[] {
  return workflows.filter(
    (workflow) => workflow.settings.schedule !== undefined && workflow.settings.schedule !== null,
  );
}

export function workflowsAddableToSchedule(workflows: Workflow[]): Workflow[] {
  return workflows.filter(
    (workflow) => workflow.settings.schedule === undefined || workflow.settings.schedule === null,
  );
}

export function formatScheduleTimestamp(value: string | null): string {
  if (!value) return "Never";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function describeScheduleStatus(status: ScheduleStatus | null): string {
  if (!status) return "Not scheduled";
  if (status.lastError) return status.lastError;
  if (!status.enabled) return "Disabled";
  if (status.nextRunAt) return `Next run ${formatScheduleTimestamp(status.nextRunAt)}`;
  return "Waiting for next run";
}
