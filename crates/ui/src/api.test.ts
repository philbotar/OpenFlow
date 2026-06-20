import { beforeEach, describe, expect, test, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import {
  RUN_STATE_EVENT,
  bootstrapApp,
  getRunState,
  listenToRunState,
  listScheduleStatuses,
  listRuns,
  replayRun,
  refreshSchedules,
  resumeDurableRun,
  startRun,
  submitToolApproval,
  workflowAuthoringTurn,
} from "./api";
import type { AppSettings, Workflow } from "./lib/types";
import { createEmptyToolConfig } from "./lib/workflow";

const settings: AppSettings = {
  active_provider: "openai",
  providers: {},
};

const workflow: Workflow = {
  id: "wf-1",
  name: "Test",
  nodes: [],
  edges: [],
  settings: { shared_context: "" },
};

describe("api desktop seam", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    invoke.mockResolvedValue(null);
    listen.mockResolvedValue(() => {});
  });

  test("bootstrapApp invokes bootstrap_app", async () => {
    await bootstrapApp();
    expect(invoke).toHaveBeenCalledWith("bootstrap_app");
  });

  test("startRun forwards workflow, settings, cwd, key, and entrypoint", async () => {
    await startRun(workflow, settings, "/tmp/project", "sk-test", "Kickoff text");

    expect(invoke).toHaveBeenCalledWith("start_run", {
      workflow,
      settings,
      executionCwd: "/tmp/project",
      transientApiKey: "sk-test",
      entrypoint: "Kickoff text",
    });
  });

  test("submitToolApproval passes null reason when omitted", async () => {
    await submitToolApproval("approval-1", true);

    expect(invoke).toHaveBeenCalledWith("submit_tool_approval", {
      approvalId: "approval-1",
      allow: true,
      reason: null,
    });
  });

  test("submitToolApproval forwards explicit denial reason", async () => {
    await submitToolApproval("approval-2", false, "Too risky");

    expect(invoke).toHaveBeenCalledWith("submit_tool_approval", {
      approvalId: "approval-2",
      allow: false,
      reason: "Too risky",
    });
  });

  test("getRunState invokes get_run_state", async () => {
    await getRunState();
    expect(invoke).toHaveBeenCalledWith("get_run_state");
  });

  test("listenToRunState subscribes to run-state events", async () => {
    const handler = vi.fn();
    await listenToRunState(handler);

    expect(listen).toHaveBeenCalledWith(RUN_STATE_EVENT, expect.any(Function));
    const callback = listen.mock.calls[0]?.[1] as (event: { payload: unknown }) => void;
    callback({ payload: { active: true } });
    expect(handler).toHaveBeenCalledWith({ active: true });
  });

  test("workflowAuthoringTurn invokes workflow_authoring_turn", async () => {
    invoke.mockResolvedValueOnce({
      sessionId: "s1",
      assistantMessage: "ok",
      validation: { valid: true, errors: [], warnings: [] },
      messages: [],
    });
    await workflowAuthoringTurn("s1", "hello", settings, null);
    expect(invoke).toHaveBeenCalledWith(
      "workflow_authoring_turn",
      expect.objectContaining({
        sessionId: "s1",
        message: "hello",
      }),
    );
  });

  test("listScheduleStatuses invokes list_schedule_statuses", async () => {
    await listScheduleStatuses();
    expect(invoke).toHaveBeenCalledWith("list_schedule_statuses");
  });

  test("refreshSchedules invokes refresh_schedules", async () => {
    await refreshSchedules();
    expect(invoke).toHaveBeenCalledWith("refresh_schedules");
  });

  test("passes workflowId to list_runs", async () => {
    invoke.mockResolvedValueOnce([]);
    await listRuns("wf-1");
    expect(invoke).toHaveBeenCalledWith("list_runs", { workflowId: "wf-1" });
  });

  test("passes runId to replay_run", async () => {
    invoke.mockResolvedValueOnce({ active: false });
    await replayRun("run-1");
    expect(invoke).toHaveBeenCalledWith("replay_run", { runId: "run-1" });
  });

  test("passes settings to resume_durable_run", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ active: true });
    await resumeDurableRun("run-1", settings, "key");
    expect(invoke).toHaveBeenCalledWith("resume_durable_run", {
      runId: "run-1",
      settings,
      transientApiKey: "key",
    });
  });
});
