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
  startRun,
  submitToolApproval,
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
});
