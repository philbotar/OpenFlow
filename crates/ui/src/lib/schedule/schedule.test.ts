import { describe, expect, test, vi } from "vitest";
import type { ScheduleStatus, Workflow } from "../types";
import {
  defaultWorkflowSchedule,
  describeScheduleStatus,
  intervalValueMax,
  parseIntervalValue,
  scheduleForWorkflow,
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

  test("parseIntervalValue clamps to valid per-unit ranges", () => {
    expect(parseIntervalValue("0", "minutes")).toBe(30);
    expect(parseIntervalValue("999", "minutes")).toBe(intervalValueMax("minutes"));
    expect(parseIntervalValue("999", "hours")).toBe(intervalValueMax("hours"));
    expect(parseIntervalValue("999", "days")).toBe(intervalValueMax("days"));
    expect(parseIntervalValue("2", "hours")).toBe(2);
  });

  test("toggleWeekday keeps at least one selected day", () => {
    expect(toggleWeekday(["1"], "1")).toEqual(["1"]);
    expect(toggleWeekday(["1", "3"], "1")).toEqual(["3"]);
    expect(toggleWeekday(["1"], "3")).toEqual(["1", "3"]);
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
