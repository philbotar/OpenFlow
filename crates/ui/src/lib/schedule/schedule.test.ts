import { describe, expect, test, vi } from "vitest";
import type { ScheduleStatus, Workflow } from "../types";
import {
  ALL_WEEKDAYS,
  cronDayOfWeek,
  defaultWorkflowSchedule,
  describeScheduleStatus,
  describeWorkflowSchedule,
  scheduleFromPreset,
  scheduleForWorkflow,
  schedulePresetFromCron,
  statusForWorkflow,
  toggleWeekday,
  workflowsAddableToSchedule,
  workflowsWithSchedules,
} from "../schedule";
import { createEmptyToolConfig } from "../workflow";

function workflow(schedule: Workflow["settings"]["schedule"] = null): Workflow {
  return {
    id: "wf-1",
    name: "Daily workflow",
    nodes: [
      {
        id: "node-1",
        label: "Run",
        kind: "Agent",
        position: { x: 0, y: 0 },
        agent: {
          system_prompt: "system",
          task_prompt: "task",
          model: "gpt-4.1-mini",
          output_schema: { type: "object" },
          auto_start: true,
          tools: createEmptyToolConfig(),
          callable_agents: [],
          allow_all_callable_agents: false,
        },
      },
    ],
    edges: [],
    settings: {
      shared_context: "",
      retry_policy: { max_attempts: 3, backoff_ms: 1_000 },
      schedule,
    },
  };
}

describe("schedule helpers", () => {
  test("default schedule uses local timezone when available", () => {
    vi.spyOn(Intl, "DateTimeFormat").mockReturnValue({
      resolvedOptions: () => ({ timeZone: "Australia/Perth" }),
    } as Intl.DateTimeFormat);

    expect(defaultWorkflowSchedule()).toEqual({
      cron: "0 9 * * *",
      enabled: true,
      timezone: "Australia/Perth",
    });
  });

  test("scheduleForWorkflow returns persisted schedule", () => {
    const persisted = { cron: "*/15 * * * *", enabled: true, timezone: "UTC" };
    expect(scheduleForWorkflow(workflow(persisted))).toBe(persisted);
  });

  test("workflowsWithSchedules returns only explicitly scheduled workflows", () => {
    const scheduled = workflow({
      cron: "0 9 * * *",
      enabled: true,
      timezone: "Australia/Perth",
    });
    const unscheduled = { ...workflow(null), id: "wf-2", name: "Unscheduled" };

    expect(workflowsWithSchedules([scheduled, unscheduled]).map((item) => item.id)).toEqual([
      "wf-1",
    ]);
    expect(workflowsAddableToSchedule([scheduled, unscheduled]).map((item) => item.id)).toEqual([
      "wf-2",
    ]);
  });

  test("timed presets serialize selected weekdays to cron", () => {
    vi.spyOn(Intl, "DateTimeFormat").mockReturnValue({
      resolvedOptions: () => ({ timeZone: "Australia/Perth" }),
    } as Intl.DateTimeFormat);

    expect(
      scheduleFromPreset({
        preset: "timed",
        time: "08:30",
        weekdays: ["1", "2", "3", "4", "5"],
        intervalValue: "30",
        intervalUnit: "minutes",
        enabled: true,
      }),
    ).toEqual({
      cron: "30 8 * * 1-5",
      enabled: true,
      timezone: "Australia/Perth",
    });

    expect(
      scheduleFromPreset({
        preset: "timed",
        time: "08:30",
        weekdays: ["1", "3", "5"],
        intervalValue: "30",
        intervalUnit: "minutes",
        enabled: true,
      }),
    ).toEqual({
      cron: "30 8 * * 1,3,5",
      enabled: true,
      timezone: "Australia/Perth",
    });
  });

  test("interval presets serialize minute and hour cron", () => {
    vi.spyOn(Intl, "DateTimeFormat").mockReturnValue({
      resolvedOptions: () => ({ timeZone: "Australia/Perth" }),
    } as Intl.DateTimeFormat);

    expect(
      scheduleFromPreset({
        preset: "interval",
        time: "08:30",
        weekdays: [...ALL_WEEKDAYS],
        intervalValue: "45",
        intervalUnit: "minutes",
        enabled: true,
      }),
    ).toEqual({
      cron: "*/45 * * * *",
      enabled: true,
      timezone: "Australia/Perth",
    });

    expect(
      scheduleFromPreset({
        preset: "interval",
        time: "08:30",
        weekdays: [...ALL_WEEKDAYS],
        intervalValue: "2",
        intervalUnit: "hours",
        enabled: true,
      }),
    ).toEqual({
      cron: "0 */2 * * *",
      enabled: true,
      timezone: "Australia/Perth",
    });
  });

  test("interval presets serialize day cron", () => {
    vi.spyOn(Intl, "DateTimeFormat").mockReturnValue({
      resolvedOptions: () => ({ timeZone: "Australia/Perth" }),
    } as Intl.DateTimeFormat);

    expect(
      scheduleFromPreset({
        preset: "interval",
        time: "09:00",
        weekdays: [...ALL_WEEKDAYS],
        intervalValue: "2",
        intervalUnit: "days",
        enabled: true,
      }),
    ).toEqual({
      cron: "0 9 */2 * *",
      enabled: true,
      timezone: "Australia/Perth",
    });

    expect(
      scheduleFromPreset({
        preset: "interval",
        time: "09:00",
        weekdays: [...ALL_WEEKDAYS],
        intervalValue: "31",
        intervalUnit: "days",
        enabled: true,
      }),
    ).toEqual({
      cron: "0 9 */31 * *",
      enabled: true,
      timezone: "Australia/Perth",
    });
  });

  test("invalid day steps load as clamped day intervals", () => {
    vi.spyOn(Intl, "DateTimeFormat").mockReturnValue({
      resolvedOptions: () => ({ timeZone: "Australia/Perth" }),
    } as Intl.DateTimeFormat);

    expect(schedulePresetFromCron("0 9 */210 * *")).toMatchObject({
      preset: "interval",
      intervalValue: "31",
      intervalUnit: "days",
      time: "09:00",
    });

    expect(
      describeWorkflowSchedule(
        scheduleFromPreset({
          preset: "interval",
          time: "09:00",
          weekdays: [...ALL_WEEKDAYS],
          intervalValue: "31",
          intervalUnit: "days",
          enabled: true,
        }),
      ),
    ).toBe("Every 31 days at 09:00");
  });

  test("describeWorkflowSchedule creates compact row summaries", () => {
    expect(
      describeWorkflowSchedule({
        cron: "30 8 * * 1-5",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Weekdays at 08:30");
    expect(
      describeWorkflowSchedule({
        cron: "30 8 * * 1,3,5",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Mon, Wed, Fri at 08:30");
    expect(
      describeWorkflowSchedule({
        cron: "*/30 * * * *",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Every 30 minutes");
    expect(
      describeWorkflowSchedule({
        cron: "0 */2 * * *",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Every 2 hours");
    expect(
      describeWorkflowSchedule({
        cron: "0 9 */2 * *",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Every 2 days at 09:00");
    expect(
      describeWorkflowSchedule({
        cron: "0 9 */7 * *",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Every 7 days at 09:00");
  });

  test("schedulePresetFromCron recognizes common persisted schedules", () => {
    expect(schedulePresetFromCron("30 8 * * 1-5")).toMatchObject({
      preset: "timed",
      time: "08:30",
      weekdays: ["1", "2", "3", "4", "5"],
    });
    expect(schedulePresetFromCron("30 8 * * 1,3,5")).toMatchObject({
      preset: "timed",
      weekdays: ["1", "3", "5"],
    });
    expect(schedulePresetFromCron("*/15 * * * *")).toMatchObject({
      preset: "interval",
      intervalValue: "15",
      intervalUnit: "minutes",
    });
    expect(schedulePresetFromCron("0 * * * *")).toMatchObject({
      preset: "interval",
      intervalValue: "1",
      intervalUnit: "hours",
    });
    expect(schedulePresetFromCron("0 9 */2 * *")).toMatchObject({
      preset: "interval",
      intervalValue: "2",
      intervalUnit: "days",
      time: "09:00",
    });
    expect(schedulePresetFromCron("0 9 */7 * *")).toMatchObject({
      preset: "interval",
      intervalValue: "7",
      intervalUnit: "days",
      time: "09:00",
    });
    expect(schedulePresetFromCron("0 9 */14 * *")).toMatchObject({
      preset: "interval",
      intervalValue: "14",
      intervalUnit: "days",
      time: "09:00",
    });
  });

  test("toggleWeekday keeps at least one selected day", () => {
    expect(toggleWeekday(["1"], "1")).toEqual(["1"]);
    expect(toggleWeekday(["1", "3"], "1")).toEqual(["3"]);
    expect(toggleWeekday(["1"], "3")).toEqual(["1", "3"]);
  });

  test("cronDayOfWeek maps weekday sets to cron fields", () => {
    expect(cronDayOfWeek([...ALL_WEEKDAYS])).toBe("*");
    expect(cronDayOfWeek(["1", "2", "3", "4", "5"])).toBe("1-5");
    expect(cronDayOfWeek(["1", "3", "5"])).toBe("1,3,5");
  });

  test("statusForWorkflow finds matching status", () => {
    const statuses: ScheduleStatus[] = [
      {
        workflowId: "wf-1",
        workflowName: "Daily workflow",
        enabled: true,
        cron: "0 9 * * *",
        timezone: "UTC",
        nextRunAt: "2026-06-16T09:00:00Z",
        lastRunAt: null,
        lastSkippedAt: null,
        lastError: null,
      },
    ];
    expect(statusForWorkflow(statuses, "wf-1")?.workflowName).toBe("Daily workflow");
  });

  test("describeScheduleStatus prefers last error", () => {
    expect(
      describeScheduleStatus({
        workflowId: "wf-1",
        workflowName: "Daily workflow",
        enabled: true,
        cron: "0 9 * * *",
        timezone: "UTC",
        nextRunAt: null,
        lastRunAt: null,
        lastSkippedAt: null,
        lastError: "invalid cron expression",
      }),
    ).toBe("invalid cron expression");
  });
});
