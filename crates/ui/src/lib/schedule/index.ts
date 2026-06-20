import type { ScheduleStatus, Workflow, WorkflowSchedule } from "../types";

export type SchedulePreset = "timed" | "interval" | "custom";
export type IntervalUnit = "minutes" | "hours" | "days";

export interface ScheduleDraft {
  preset: SchedulePreset;
  time: string;
  weekdays: string[];
  intervalValue: string;
  intervalUnit: IntervalUnit;
  enabled: boolean;
  customCron?: string;
}

const DEFAULT_TIME = "09:00";
const DEFAULT_INTERVAL_VALUE = "30";
const DEFAULT_INTERVAL_UNIT: IntervalUnit = "minutes";

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

const WEEKDAY_LABELS: Record<string, string> = {
  "0": "Sunday",
  "1": "Monday",
  "2": "Tuesday",
  "3": "Wednesday",
  "4": "Thursday",
  "5": "Friday",
  "6": "Saturday",
};

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

export function cronDayOfWeek(weekdays: string[]): string {
  const normalized = normalizeWeekdays(weekdays);
  if (normalized.length === 0 || normalized.length === ALL_WEEKDAYS.length) {
    return "*";
  }
  if (
    normalized.length === WEEKDAY_PRESET.length &&
    normalized.every((day, index) => day === WEEKDAY_PRESET[index])
  ) {
    return "1-5";
  }
  if (normalized.length === 1) {
    return normalized[0];
  }
  return normalized.join(",");
}

function weekdaysFromCronDayOfWeek(dayOfWeek: string): string[] | null {
  if (dayOfWeek === "*") {
    return [...ALL_WEEKDAYS];
  }

  const days = new Set<string>();
  for (const part of dayOfWeek.split(",")) {
    const trimmed = part.trim();
    const rangeMatch = /^([0-6])-([0-6])$/.exec(trimmed);
    if (rangeMatch) {
      const start = Number(rangeMatch[1]);
      const end = Number(rangeMatch[2]);
      if (start > end) {
        return null;
      }
      for (let day = start; day <= end; day += 1) {
        days.add(String(day));
      }
      continue;
    }

    if (/^[0-6]$/.test(trimmed)) {
      days.add(trimmed);
      continue;
    }

    return null;
  }

  return normalizeWeekdays([...days]);
}

function intervalDayStep(value: number): number {
  return Math.min(Math.max(value, 1), INTERVAL_VALUE_LIMITS.days);
}

function intervalCronFromDraft(value: number, unit: IntervalUnit, time: string): string {
  const { hour, minute } = timeParts(time);
  switch (unit) {
    case "minutes":
      return `*/${value} * * * *`;
    case "hours":
      return value === 1 ? "0 * * * *" : `0 */${value} * * *`;
    case "days":
      return `${minute} ${hour} */${intervalDayStep(value)} * *`;
  }
}

function intervalFromCronParts(
  parts: string[],
): { value: string; unit: IntervalUnit; time?: string } | null {
  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;
  if (
    /^\*\/\d+$/.test(minute) &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return { value: minute.slice(2), unit: "minutes" };
  }

  if (minute === "0" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    if (hour === "*") {
      return { value: "1", unit: "hours" };
    }
    if (/^\*\/\d+$/.test(hour)) {
      return { value: hour.slice(2), unit: "hours" };
    }
  }

  if (
    /^\d{1,2}$/.test(minute) &&
    /^\d{1,2}$/.test(hour) &&
    month === "*" &&
    dayOfWeek === "*" &&
    /^\*\/\d+$/.test(dayOfMonth)
  ) {
    const step = Number(dayOfMonth.slice(2));
    const time = timeFromCronParts(hour, minute);
    if (step < 1) {
      return null;
    }
    return {
      value: String(parseIntervalValue(String(Math.min(step, INTERVAL_VALUE_LIMITS.days)), "days")),
      unit: "days",
      time,
    };
  }

  return null;
}

function localTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

function padTimePart(value: string): string {
  return value.padStart(2, "0");
}

function timeFromCronParts(hour: string, minute: string): string {
  return `${padTimePart(hour)}:${padTimePart(minute)}`;
}

function timeParts(time: string): { hour: string; minute: string } {
  const match = /^(\d{1,2}):(\d{2})$/.exec(time);
  if (!match) return { hour: "9", minute: "0" };
  const hour = Math.min(Math.max(Number(match[1]), 0), 23);
  const minute = Math.min(Math.max(Number(match[2]), 0), 59);
  return { hour: String(hour), minute: String(minute) };
}

function withSchedule(cron: string, enabled: boolean): WorkflowSchedule {
  return {
    cron,
    enabled,
    timezone: localTimezone(),
  };
}

function describeTimedSchedule(time: string, weekdays: string[]): string {
  const normalized = normalizeWeekdays(weekdays);
  if (normalized.length === 0 || normalized.length === ALL_WEEKDAYS.length) {
    return `Daily at ${time}`;
  }
  if (
    normalized.length === WEEKDAY_PRESET.length &&
    normalized.every((day, index) => day === WEEKDAY_PRESET[index])
  ) {
    return `Weekdays at ${time}`;
  }
  if (normalized.length === 1) {
    const weekday = WEEKDAY_LABELS[normalized[0]] ?? "Weekly";
    return `${weekday}s at ${time}`;
  }
  const labels = normalized.map(
    (day) => WEEKDAY_OPTIONS.find(([value]) => value === day)?.[1] ?? day,
  );
  return `${labels.join(", ")} at ${time}`;
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

export function schedulePresetFromCron(cron: string, enabled = true): ScheduleDraft {
  const parts = cron.trim().split(/\s+/);
  const base = {
    time: DEFAULT_TIME,
    weekdays: [...ALL_WEEKDAYS],
    intervalValue: DEFAULT_INTERVAL_VALUE,
    intervalUnit: DEFAULT_INTERVAL_UNIT,
    enabled,
  };

  if (parts.length === 5) {
    const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;
    const fixedTime =
      /^\d{1,2}$/.test(minute) && /^\d{1,2}$/.test(hour)
        ? timeFromCronParts(hour, minute)
        : DEFAULT_TIME;

    if (dayOfMonth === "*" && month === "*") {
      const weekdays = weekdaysFromCronDayOfWeek(dayOfWeek);
      if (weekdays && /^\d{1,2}$/.test(minute) && /^\d{1,2}$/.test(hour)) {
        return { ...base, preset: "timed", time: fixedTime, weekdays };
      }
    }

    const interval = intervalFromCronParts(parts);
    if (interval) {
      const intervalUnit = interval.unit;
      const intervalValue = String(parseIntervalValue(interval.value, intervalUnit));
      return {
        ...base,
        preset: "interval",
        intervalValue,
        intervalUnit,
        time: interval.time ?? base.time,
      };
    }
  }

  return { ...base, preset: "custom", customCron: cron };
}

export function scheduleDraftFromSchedule(schedule: WorkflowSchedule): ScheduleDraft {
  return schedulePresetFromCron(schedule.cron, schedule.enabled);
}

export function scheduleFromPreset(draft: ScheduleDraft): WorkflowSchedule {
  if (draft.preset === "interval") {
    const intervalUnit = draft.intervalUnit;
    const value = parseIntervalValue(draft.intervalValue, intervalUnit);
    return withSchedule(intervalCronFromDraft(value, intervalUnit, draft.time), draft.enabled);
  }

  if (draft.preset === "custom") {
    return withSchedule(
      draft.customCron?.trim() || defaultWorkflowSchedule().cron,
      draft.enabled,
    );
  }

  const { hour, minute } = timeParts(draft.time);
  return withSchedule(
    `${minute} ${hour} * * ${cronDayOfWeek(draft.weekdays)}`,
    draft.enabled,
  );
}

export function describeWorkflowSchedule(schedule: WorkflowSchedule): string {
  const draft = scheduleDraftFromSchedule(schedule);
  if (draft.preset === "timed") {
    return describeTimedSchedule(draft.time, draft.weekdays);
  }
  if (draft.preset === "interval") {
    const intervalUnit = draft.intervalUnit;
    const value = parseIntervalValue(draft.intervalValue, intervalUnit);
    const unit = intervalUnit;
    const label = value === 1 ? unit.slice(0, -1) : unit;
    if (unit === "days") {
      return `Every ${value} ${label} at ${draft.time}`;
    }
    return `Every ${value} ${label}`;
  }
  return "Custom schedule";
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
