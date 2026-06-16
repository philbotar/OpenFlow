import type { ScheduleStatus, Workflow, WorkflowSchedule } from "./types";

export type SchedulePreset = "daily" | "weekdays" | "weekly" | "interval" | "custom";

export interface ScheduleDraft {
  preset: SchedulePreset;
  time: string;
  weekday: string;
  intervalMinutes: string;
  timezone: string;
  enabled: boolean;
  customCron?: string;
}

const DEFAULT_TIME = "09:00";
const DEFAULT_WEEKDAY = "1";
const DEFAULT_INTERVAL_MINUTES = "30";
const WEEKDAY_LABELS: Record<string, string> = {
  "0": "Sunday",
  "1": "Monday",
  "2": "Tuesday",
  "3": "Wednesday",
  "4": "Thursday",
  "5": "Friday",
  "6": "Saturday",
};

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

function withSchedule(
  cron: string,
  draft: Pick<ScheduleDraft, "enabled" | "timezone">,
): WorkflowSchedule {
  return {
    cron,
    enabled: draft.enabled,
    timezone: draft.timezone.trim() || localTimezone(),
  };
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

export function schedulePresetFromCron(
  cron: string,
  timezone = localTimezone(),
  enabled = true,
): ScheduleDraft {
  const parts = cron.trim().split(/\s+/);
  const base = {
    time: DEFAULT_TIME,
    weekday: DEFAULT_WEEKDAY,
    intervalMinutes: DEFAULT_INTERVAL_MINUTES,
    timezone,
    enabled,
  };

  if (parts.length === 5) {
    const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;
    const fixedTime =
      /^\d{1,2}$/.test(minute) && /^\d{1,2}$/.test(hour)
        ? timeFromCronParts(hour, minute)
        : DEFAULT_TIME;

    if (
      /^\*\/\d+$/.test(minute) &&
      hour === "*" &&
      dayOfMonth === "*" &&
      month === "*" &&
      dayOfWeek === "*"
    ) {
      return {
        ...base,
        preset: "interval",
        intervalMinutes: minute.slice(2),
      };
    }

    if (dayOfMonth === "*" && month === "*" && dayOfWeek === "1-5") {
      return { ...base, preset: "weekdays", time: fixedTime };
    }

    if (dayOfMonth === "*" && month === "*" && /^[0-6]$/.test(dayOfWeek)) {
      return { ...base, preset: "weekly", time: fixedTime, weekday: dayOfWeek };
    }

    if (dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
      return { ...base, preset: "daily", time: fixedTime };
    }
  }

  return { ...base, preset: "custom", customCron: cron };
}

export function scheduleDraftFromSchedule(schedule: WorkflowSchedule): ScheduleDraft {
  return schedulePresetFromCron(schedule.cron, schedule.timezone, schedule.enabled);
}

export function scheduleFromPreset(draft: ScheduleDraft): WorkflowSchedule {
  if (draft.preset === "interval") {
    const interval = draft.intervalMinutes || DEFAULT_INTERVAL_MINUTES;
    return withSchedule(`*/${interval} * * * *`, draft);
  }

  if (draft.preset === "custom") {
    return withSchedule(draft.customCron?.trim() || defaultWorkflowSchedule().cron, draft);
  }

  const { hour, minute } = timeParts(draft.time);
  if (draft.preset === "weekdays") {
    return withSchedule(`${minute} ${hour} * * 1-5`, draft);
  }
  if (draft.preset === "weekly") {
    return withSchedule(`${minute} ${hour} * * ${draft.weekday || DEFAULT_WEEKDAY}`, draft);
  }
  return withSchedule(`${minute} ${hour} * * *`, draft);
}

export function describeWorkflowSchedule(schedule: WorkflowSchedule): string {
  const draft = scheduleDraftFromSchedule(schedule);
  if (draft.preset === "daily") return `Daily at ${draft.time}`;
  if (draft.preset === "weekdays") return `Weekdays at ${draft.time}`;
  if (draft.preset === "weekly") {
    const weekday = WEEKDAY_LABELS[draft.weekday] ?? "Weekly";
    return `${weekday}s at ${draft.time}`;
  }
  if (draft.preset === "interval") {
    const minutes = draft.intervalMinutes;
    return `Every ${minutes} ${minutes === "1" ? "minute" : "minutes"}`;
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
