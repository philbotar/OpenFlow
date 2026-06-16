import { describe, expect, test, vi } from "vitest";
import type { ScheduleStatus, Workflow } from "./types";
import {
  defaultWorkflowSchedule,
  describeScheduleStatus,
  describeWorkflowSchedule,
  scheduleFromPreset,
  scheduleForWorkflow,
  schedulePresetFromCron,
  statusForWorkflow,
  workflowsAddableToSchedule,
  workflowsWithSchedules,
} from "./schedule";
import { createEmptyToolConfig } from "./workflow";

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

  test("schedule presets serialize to cron without exposing cron editing", () => {
    expect(
      scheduleFromPreset({
        preset: "weekdays",
        time: "08:30",
        weekday: "1",
        intervalMinutes: "30",
        timezone: "Australia/Perth",
        enabled: true,
      }),
    ).toEqual({
      cron: "30 8 * * 1-5",
      enabled: true,
      timezone: "Australia/Perth",
    });
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
        cron: "*/30 * * * *",
        enabled: true,
        timezone: "Australia/Perth",
      }),
    ).toBe("Every 30 minutes");
  });

  test("schedulePresetFromCron recognizes common persisted schedules", () => {
    expect(schedulePresetFromCron("30 8 * * 1-5")).toMatchObject({
      preset: "weekdays",
      time: "08:30",
    });
    expect(schedulePresetFromCron("*/15 * * * *")).toMatchObject({
      preset: "interval",
      intervalMinutes: "15",
    });
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
