// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { AgentDefinition, AppSettings, BootstrapPayload, Chat, Project, ProviderReadiness, ScheduleDraft, SkillSummary, Workflow, WorkflowRunState } from "../lib/types";
import { defaultWorkflowSchedule } from "../lib/schedule";
import { createEmptyToolConfig } from "../lib/workflow/testHelpers";

const apiMocks = vi.hoisted(() => ({
  bootstrapApp: vi.fn(),
  listSkills: vi.fn(),
  listWorkflows: vi.fn(),
  clearRunTrace: vi.fn(),
  createAgentDefinition: vi.fn(),
  createAgentDefinitionWithAi: vi.fn(),
  createAgentNode: vi.fn(),
  createWorkflow: vi.fn(),
  createChat: vi.fn(),
  deleteChat: vi.fn(),
  updateChatConfig: vi.fn(),
  listenToRunState: vi.fn(),
  listenToWorkflowAuthoringThinking: vi.fn(),
  listenToWorkflowAuthoringDraft: vi.fn(),
  resolveProviderReadiness: vi.fn(),
  deleteProviderApiKey: vi.fn(),
  debugLogPath: vi.fn(),
  appendDebugLog: vi.fn(),
  loadProviderApiKey: vi.fn(),
  saveAgents: vi.fn(),
  saveProviderApiKey: vi.fn(),
  saveSettings: vi.fn(),
  saveWorkflows: vi.fn(),
  saveWorkflow: vi.fn(),
  listScheduleStatuses: vi.fn(),
  refreshSchedules: vi.fn(),
  scheduleFromPreset: vi.fn(),
  scheduleDraftFromSchedule: vi.fn(),
  describeWorkflowSchedule: vi.fn(),
  listenToScheduleStatuses: vi.fn(),
  startRun: vi.fn(),
  startChat: vi.fn(),
  getRunState: vi.fn(),
  continueRun: vi.fn(),
  replayRun: vi.fn(),
  resumeDurableRun: vi.fn(),
  isRunContinuable: vi.fn(),
  submitToolApproval: vi.fn(),
  submitUserInput: vi.fn(),
  updateNodeRuntimeConfig: vi.fn(),
  validateWorkflow: vi.fn(),
  startTerminal: vi.fn(),
  writeTerminal: vi.fn(),
  resizeTerminal: vi.fn(),
  stopTerminal: vi.fn(),
  listenToTerminalEvent: vi.fn(),
  startWorkflowAuthoring: vi.fn(),
  endWorkflowAuthoring: vi.fn(),
  workflowAuthoringTurn: vi.fn(),
  loadAllWorkflows: vi.fn(),
  createProjectFromDirectory: vi.fn(),
  saveProjects: vi.fn(),
  assignWorkflowToProject: vi.fn(),
  copyWorkflowToProject: vi.fn(),
  unassignWorkflowFromProject: vi.fn(),
  deleteWorkflow: vi.fn(),
  gitIsRepo: vi.fn(),
  gitDiffRepo: vi.fn(),
  gitCurrentBranch: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(null),
  confirm: vi.fn().mockResolvedValue(false),
}));

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return {
    ...actual,
    bootstrapApp: apiMocks.bootstrapApp,
    listSkills: apiMocks.listSkills,
    listWorkflows: apiMocks.listWorkflows,
    clearRunTrace: apiMocks.clearRunTrace,
    createAgentDefinition: apiMocks.createAgentDefinition,
    createAgentDefinitionWithAi: apiMocks.createAgentDefinitionWithAi,
    createAgentNode: apiMocks.createAgentNode,
    createWorkflow: apiMocks.createWorkflow,
    createChat: apiMocks.createChat,
    deleteChat: apiMocks.deleteChat,
    updateChatConfig: apiMocks.updateChatConfig,
    listenToRunState: apiMocks.listenToRunState,
    listenToWorkflowAuthoringThinking: apiMocks.listenToWorkflowAuthoringThinking,
    listenToWorkflowAuthoringDraft: apiMocks.listenToWorkflowAuthoringDraft,
    resolveProviderReadiness: apiMocks.resolveProviderReadiness,
    deleteProviderApiKey: apiMocks.deleteProviderApiKey,
    debugLogPath: apiMocks.debugLogPath,
    appendDebugLog: apiMocks.appendDebugLog,
    loadProviderApiKey: apiMocks.loadProviderApiKey,
    saveAgents: apiMocks.saveAgents,
    submitToolApproval: apiMocks.submitToolApproval,
    saveProviderApiKey: apiMocks.saveProviderApiKey,
    saveSettings: apiMocks.saveSettings,
    saveWorkflows: apiMocks.saveWorkflows,
    saveWorkflow: apiMocks.saveWorkflow,
    listScheduleStatuses: apiMocks.listScheduleStatuses,
    refreshSchedules: apiMocks.refreshSchedules,
    scheduleFromPreset: apiMocks.scheduleFromPreset,
    scheduleDraftFromSchedule: apiMocks.scheduleDraftFromSchedule,
    describeWorkflowSchedule: apiMocks.describeWorkflowSchedule,
    listenToScheduleStatuses: apiMocks.listenToScheduleStatuses,
    startRun: apiMocks.startRun,
    startChat: apiMocks.startChat,
    getRunState: apiMocks.getRunState,
    continueRun: apiMocks.continueRun,
    replayRun: apiMocks.replayRun,
    resumeDurableRun: apiMocks.resumeDurableRun,
    isRunContinuable: apiMocks.isRunContinuable,
    submitUserInput: apiMocks.submitUserInput,
    updateNodeRuntimeConfig: apiMocks.updateNodeRuntimeConfig,
    validateWorkflow: apiMocks.validateWorkflow,
    loadAllWorkflows: apiMocks.loadAllWorkflows,
    createProjectFromDirectory: apiMocks.createProjectFromDirectory,
    saveProjects: apiMocks.saveProjects,
    assignWorkflowToProject: apiMocks.assignWorkflowToProject,
    copyWorkflowToProject: apiMocks.copyWorkflowToProject,
    unassignWorkflowFromProject: apiMocks.unassignWorkflowFromProject,
    deleteWorkflow: apiMocks.deleteWorkflow,
    startTerminal: apiMocks.startTerminal,
    writeTerminal: apiMocks.writeTerminal,
    resizeTerminal: apiMocks.resizeTerminal,
    stopTerminal: apiMocks.stopTerminal,
    listenToTerminalEvent: apiMocks.listenToTerminalEvent,
    startWorkflowAuthoring: apiMocks.startWorkflowAuthoring,
    endWorkflowAuthoring: apiMocks.endWorkflowAuthoring,
    workflowAuthoringTurn: apiMocks.workflowAuthoringTurn,
    gitIsRepo: apiMocks.gitIsRepo,
    gitDiffRepo: apiMocks.gitDiffRepo,
    gitCurrentBranch: apiMocks.gitCurrentBranch,
  };
});

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {}),
  }),
}));

vi.mock("../canvas/WorkflowCanvasHost", () => ({
  default: (props: {
    onAddNode: () => void;
    onSelectNode?: (nodeId: string) => void;
    onDeleteNode?: (nodeId: string) => void;
    onDeleteEdge?: (edgeId: string) => void;
    onCreateEdge?: (from: string, to: string) => void;
    graph?: { nodes: { id: string }[]; edges: { id: string }[] } | null;
  }) => (
    <>
      <button aria-label="Canvas add node" onClick={() => props.onAddNode()}>
        Canvas add node
      </button>
      {props.graph?.nodes.map((node) => (
        <>
          <button
            type="button"
            aria-label={`Select node ${node.id}`}
            onClick={() => props.onSelectNode?.(node.id)}
          />
          <button
            type="button"
            aria-label={`Canvas delete node ${node.id}`}
            onClick={() => props.onDeleteNode?.(node.id)}
          />
        </>
      ))}
      {props.graph?.edges.map((edge) => (
        <button
          type="button"
          aria-label={`Canvas delete edge ${edge.id}`}
          onClick={() => props.onDeleteEdge?.(edge.id)}
        />
      ))}
      {props.graph?.nodes.slice(1).map((node, index) => {
        const from = props.graph?.nodes[index]?.id;
        return (
          <button
            type="button"
            aria-label={`Canvas create edge ${from} ${node.id}`}
            onClick={() => from && props.onCreateEdge?.(from, node.id)}
          />
        );
      })}
    </>
  ),
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => ({
    cols: 80,
    rows: 24,
    options: { theme: {} },
    loadAddon: vi.fn(),
    open: vi.fn(),
    onData: vi.fn(),
    reset: vi.fn(),
    writeln: vi.fn(),
    write: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(() => ({
    fit: vi.fn(),
  })),
}));

import App from "../App";
import { confirm } from "@tauri-apps/plugin-dialog";

const SETTINGS: AppSettings = {
  active_provider: "openai",
  providers: {
    openai: {
      display_name: "OpenAI",
      base_url: "https://api.openai.com/v1",
      transport: "responses",
      responses_path: "responses",
      chat_completions_path: "chat/completions",
      request_timeout_secs: 300,
      known_models: ["gpt-4.1-mini"],
      default_model: "gpt-4.1-mini",
      editable: false,
    },
    custom_openai_compatible: {
      display_name: "Compatible",
      base_url: "https://example.invalid/v1",
      transport: "chat_completions",
      responses_path: "responses",
      chat_completions_path: "chat/completions",
      request_timeout_secs: 300,
      known_models: ["compatible-model"],
      default_model: "compatible-model",
      editable: true,
    },
  },
};

const READY: ProviderReadiness = {
  ready: true,
  provider: "OpenAI",
  message: "Ready",
  envVar: "OPENAI_API_KEY",
};

const FIXTURE_SKILLS: SkillSummary[] = [
  {
    id: "systematic-debugging",
    name: "Systematic Debugging",
    description: "Use when encountering bugs or test failures.",
  },
  {
    id: "brainstorming",
    name: "Brainstorming",
    description: "Explore ideas before building.",
  },
  {
    id: "documents",
    name: "Documents",
    description: "Work with project documents.",
  },
  {
    id: "browser",
    name: "Browser",
    description: "Inspect pages in the browser.",
  },
  {
    id: "requesting-code-review",
    name: "Requesting Code Review",
    description: "Ask for a structured code review.",
  },
];

function makeWorkflow(id: string, name: string): Workflow {
  return {
    id,
    name,
    nodes: [
      {
        id: `${id}-node-1`,
        label: `${name} node`,
        kind: "Agent",
        position: { x: 120, y: 140 },
        agent: {
          system_prompt: "",
          task_prompt: "",
          model: "gpt-4.1-mini",
          output_schema: { type: "object" },
          auto_start: false,
          tools: createEmptyToolConfig(),
          callable_agents: [],
          allow_all_callable_agents: false,
        },
      },
    ],
    edges: [],
    settings: {
      shared_context: "",
    },
  };
}

function makeAgent(id: string, name: string): AgentDefinition {
  return {
    id,
    name,
    system_prompt: "You are a focused AI agent in a node workflow.",
    task_prompt: "Return a concise JSON object for this node.",
    model: "",
    output_schema: {
      type: "object",
      additionalProperties: false,
      properties: {
        summary: { type: "string" },
      },
      required: ["summary"],
    },
    auto_start: true,
    tools: createEmptyToolConfig(),
  };
}

function makeNodeFromAgent(index: number, x: number, y: number, agent: AgentDefinition | null) {
  return {
    id: `created-node-${index + 1}`,
    label: agent?.name ?? `Agent ${index + 1}`,
    kind: "Agent" as const,
    position: { x, y },
    agent: agent
      ? {
          system_prompt: agent.system_prompt,
          task_prompt: agent.task_prompt,
          model: agent.model,
          output_schema: agent.output_schema,
          auto_start: agent.auto_start,
          tools: agent.tools,
          callable_agents: [],
          allow_all_callable_agents: false,
        }
      : {
          system_prompt: "",
          task_prompt: "",
          model: "",
          output_schema: { type: "object" },
          auto_start: false,
          tools: createEmptyToolConfig(),
          callable_agents: [],
          allow_all_callable_agents: false,
        },
  };
}

function parseMockTime(time: string): { hour: number; minute: number } {
  const match = /^(\d{1,2}):(\d{2})$/.exec(time);
  if (!match) return { hour: 9, minute: 0 };
  return {
    hour: Math.min(Math.max(Number(match[1]), 0), 23),
    minute: Math.min(Math.max(Number(match[2]), 0), 59),
  };
}

function cronDayOfWeekForMock(weekdays: string[]): string {
  const normalized = [...new Set(weekdays.filter((day) => /^[0-6]$/.test(day)))].sort(
    (left, right) => Number(left) - Number(right),
  );
  if (normalized.length === 0 || normalized.length === 7) {
    return "*";
  }
  if (normalized.join(",") === "1,2,3,4,5") {
    return "1-5";
  }
  if (normalized.length === 1) {
    return normalized[0];
  }
  return normalized.join(",");
}

function scheduleFromPresetMock(draft: ScheduleDraft) {
  if (draft.preset === "interval") {
    const parsed = Number.parseInt(draft.intervalValue.trim(), 10);
    const value = Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
    if (draft.intervalUnit === "minutes") {
      return { cron: `*/${value} * * * *`, enabled: draft.enabled, timezone: "UTC" };
    }
    if (draft.intervalUnit === "hours") {
      return {
        cron: value === 1 ? "0 * * * *" : `0 */${value} * * *`,
        enabled: draft.enabled,
        timezone: "UTC",
      };
    }
    const { hour, minute } = parseMockTime(draft.time);
    return {
      cron: `${minute} ${hour} */${Math.min(Math.max(value, 1), 31)} * *`,
      enabled: draft.enabled,
      timezone: "UTC",
    };
  }
  if (draft.preset === "custom") {
    return {
      cron: draft.customCron?.trim() || "0 9 * * *",
      enabled: draft.enabled,
      timezone: "UTC",
    };
  }
  const { hour, minute } = parseMockTime(draft.time);
  return {
    cron: `${minute} ${hour} * * ${cronDayOfWeekForMock(draft.weekdays)}`,
    enabled: draft.enabled,
    timezone: "UTC",
  };
}

function scheduleDraftFromScheduleMock(schedule: { cron: string; enabled: boolean }) {
  const parts = schedule.cron.trim().split(/\s+/);
  const base: ScheduleDraft = {
    preset: "timed",
    time: "09:00",
    weekdays: ["0", "1", "2", "3", "4", "5", "6"],
    intervalValue: "30",
    intervalUnit: "minutes",
    enabled: schedule.enabled,
  };
  if (parts.length === 5) {
    const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;
    if (
      /^\*\/\d+$/.test(minute) &&
      hour === "*" &&
      dayOfMonth === "*" &&
      month === "*" &&
      dayOfWeek === "*"
    ) {
      return { ...base, preset: "interval", intervalValue: minute.slice(2), intervalUnit: "minutes" };
    }
    if (minute === "0" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
      if (hour === "*") {
        return { ...base, preset: "interval", intervalValue: "1", intervalUnit: "hours" };
      }
      if (/^\*\/\d+$/.test(hour)) {
        return { ...base, preset: "interval", intervalValue: hour.slice(2), intervalUnit: "hours" };
      }
    }
    if (/^\d{1,2}$/.test(minute) && /^\d{1,2}$/.test(hour) && dayOfMonth === "*" && month === "*") {
      const weekdays =
        dayOfWeek === "*"
          ? [...base.weekdays]
          : dayOfWeek === "1-5"
            ? ["1", "2", "3", "4", "5"]
            : dayOfWeek.split(",").map((value) => value.trim()).filter((value) => /^[0-6]$/.test(value));
      return {
        ...base,
        preset: "timed",
        time: `${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`,
        weekdays: weekdays.length > 0 ? weekdays : base.weekdays,
      };
    }
  }
  return { ...base, preset: "custom", customCron: schedule.cron };
}

function installDefaultApiMocks() {
  if (!Element.prototype.scrollTo) {
    Element.prototype.scrollTo = vi.fn();
  }
  if (!globalThis.ResizeObserver) {
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver;
  }
  apiMocks.listenToRunState.mockResolvedValue(() => {});
  apiMocks.listenToWorkflowAuthoringThinking.mockResolvedValue(() => {});
  apiMocks.listenToWorkflowAuthoringDraft.mockResolvedValue(() => {});
  apiMocks.isRunContinuable.mockResolvedValue(false);
  apiMocks.getRunState.mockResolvedValue(null);
  apiMocks.resolveProviderReadiness.mockResolvedValue(READY);
  apiMocks.loadProviderApiKey.mockImplementation(async (providerId: string) => {
    if (providerId === "openai") {
      return "stored-openai-key";
    }
    if (providerId === "custom_openai_compatible") {
      return "stored-compatible-key";
    }
    return null;
  });
  apiMocks.saveProviderApiKey.mockResolvedValue(undefined);
  apiMocks.deleteProviderApiKey.mockResolvedValue(undefined);
  apiMocks.debugLogPath.mockResolvedValue("/tmp/openflow-debug-test.jsonl");
  apiMocks.appendDebugLog.mockResolvedValue({
    enabled: true,
    path: "/tmp/openflow-debug-test.jsonl",
  });
  apiMocks.createWorkflow.mockImplementation(async (name: string) => makeWorkflow("created-workflow", name));
  apiMocks.createAgentDefinition.mockImplementation(async (name: string) => makeAgent("created-agent", name));
  apiMocks.createAgentDefinitionWithAi.mockResolvedValue(
    makeAgent("ai-created-agent", "Research Reviewer"),
  );
  apiMocks.createAgentNode.mockImplementation(
    async (index: number, x: number, y: number, agentId: string | null = null) => {
      const agent = agentId ? makeAgent(agentId, agentId === "agent-2" ? "Writer Agent" : "Research Agent") : null;
      return makeNodeFromAgent(index, x, y, agent);
    },
  );
  apiMocks.listWorkflows.mockResolvedValue([]);
  apiMocks.listSkills.mockResolvedValue(FIXTURE_SKILLS);
  apiMocks.startTerminal.mockResolvedValue({ sessionId: "terminal-1", cwd: "/tmp/Repo" });
  apiMocks.writeTerminal.mockResolvedValue(undefined);
  apiMocks.resizeTerminal.mockResolvedValue(undefined);
  apiMocks.stopTerminal.mockResolvedValue(undefined);
  apiMocks.listenToTerminalEvent.mockResolvedValue(() => {});
  apiMocks.startWorkflowAuthoring.mockResolvedValue({
    sessionId: "authoring-session-1",
    draft: undefined,
  });
  apiMocks.workflowAuthoringTurn.mockResolvedValue({
    messages: [],
    validation: null,
    draft: null,
  });
  apiMocks.saveWorkflow.mockImplementation(async (workflow) => workflow);
  apiMocks.refreshSchedules.mockResolvedValue([]);
  apiMocks.listScheduleStatuses.mockResolvedValue([]);
  apiMocks.scheduleFromPreset.mockImplementation(async (draft: ScheduleDraft) =>
    scheduleFromPresetMock(draft),
  );
  apiMocks.scheduleDraftFromSchedule.mockImplementation(async (schedule) =>
    scheduleDraftFromScheduleMock(schedule),
  );
  apiMocks.describeWorkflowSchedule.mockResolvedValue("Mock schedule");
  apiMocks.listenToScheduleStatuses.mockResolvedValue(() => {});
  apiMocks.gitIsRepo.mockResolvedValue(false);
  apiMocks.gitDiffRepo.mockResolvedValue("");
  apiMocks.gitCurrentBranch.mockResolvedValue("main");
}

function makeProject(id: string, name: string, workflowIds: string[] = []): Project {
  return {
    id,
    path: `/tmp/${name}`,
    name,
    metadata: { description: "" },
    workflow_ids: workflowIds,
    default_execution_cwd: `/tmp/${name}`,
  };
}

function makeChat(id: string, title = "New chat"): Chat {
  return {
    id,
    title,
    config: {
      model: null,
      approvalMode: "read_only",
      reasoningEffort: null,
      reasoningBudgetTokens: null,
      projectId: null,
    },
    runId: null,
    createdAtMs: 1,
    updatedAtMs: 1,
  };
}

function makeBootstrapPayload(
  workflows: Workflow[],
  agents: AgentDefinition[] = [makeAgent("agent-1", "Research Agent")],
  skills: SkillSummary[] = FIXTURE_SKILLS,
  projects: Project[] = [],
): BootstrapPayload {
  return {
    workflows,
    chats: [],
    agents,
    projects,
    skills,
    settings: SETTINGS,
    discoveredMcp: [],
    runState: null,
    scheduleStatuses: [],
  };
}

function makeAwaitingRunState(workflow: Workflow): WorkflowRunState {
  const [node] = workflow.nodes;
  return {
    active: true,
    awaitingNodeId: node.id,
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode: {
      [node.id]: "awaiting_input",
    },
    subagentsByNode: {},
    lastReport: null,
    lastError: null,
    chatLogs: {
      [node.id]: [],
    },
    runTrace: [],
    outputs: {},
    changedFiles: [],
    changedFilesByNode: {},
    editBatches: [],
  };
}

function makeParallelWorkflow(): Workflow {
  const base = makeWorkflow("workflow-parallel", "Parallel");
  const agent = base.nodes[0].agent;
  return {
    ...base,
    nodes: [
      { ...base.nodes[0], id: "node-a", label: "Plan" },
      {
        id: "node-b",
        label: "Branch B",
        kind: "Agent",
        position: { x: 200, y: 80 },
        agent,
      },
      {
        id: "node-c",
        label: "Branch C",
        kind: "Agent",
        position: { x: 200, y: 200 },
        agent,
      },
      {
        id: "node-d",
        label: "Join",
        kind: "Agent",
        position: { x: 400, y: 140 },
        agent,
      },
    ],
    edges: [
      { id: "edge-ab", from: "node-a", to: "node-b" },
      { id: "edge-ac", from: "node-a", to: "node-c" },
      { id: "edge-bd", from: "node-b", to: "node-d" },
      { id: "edge-cd", from: "node-c", to: "node-d" },
    ],
  };
}

function makeParallelAwaitingRunState(workflow: Workflow): WorkflowRunState {
  const [a, b, c, d] = workflow.nodes;
  return {
    active: true,
    awaitingNodeIds: [b.id, c.id],
    awaitingNodeId: b.id,
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode: {
      [a.id]: "completed",
      [b.id]: "awaiting_input",
      [c.id]: "awaiting_input",
      [d.id]: "idle",
    },
    subagentsByNode: {},
    lastReport: null,
    lastError: null,
    chatLogs: {
      [a.id]: [{ role: "Assistant", content: "plan complete" }],
      [b.id]: [],
      [c.id]: [],
    },
    runTrace: [],
    outputs: {},
    changedFiles: [],
    changedFilesByNode: {},
    editBatches: [],
  };
}

async function openChatTab(container: HTMLElement) {
  const chatTab = await waitForElement(
    () =>
      Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
        (btn) => btn.textContent === "Chat",
      ) as HTMLButtonElement | null,
    "chat tab",
  );
  chatTab.click();
  await flush();
}

async function openRunTraceTab(container: HTMLElement) {
  const traceTab = await waitForElement(
    () =>
      Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
        (btn) => btn.textContent === "Run trace",
      ) as HTMLButtonElement | null,
    "run trace tab",
  );
  traceTab.click();
  await flush();
}

async function openInspector(container: HTMLElement) {
  const inspectorButton = await waitForElement(
    () => container.querySelector('button[aria-label="Inspector"]') as HTMLButtonElement | null,
    "inspector button",
  );
  inspectorButton.click();
  await flush();
}

function flush() {
  return new Promise<void>((resolve) => setTimeout(resolve, 0));
}

async function waitForElement<T extends Element>(read: () => T | null, label: string): Promise<T> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const value = read();
    if (value) {
      return value;
    }
    await flush();
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function workflowTitles(container: HTMLElement) {
  return Array.from(container.querySelectorAll(".workflow-row-title")).map((element) => element.textContent ?? "");
}

function topbarTitle(container: HTMLElement) {
  const title = container.querySelector(".topbar-title span");
  if (!title) {
    throw new Error("topbar title missing");
  }
  return title.textContent ?? "";
}

function settingsNavButton(container: HTMLElement, label: string) {
  const button = Array.from(container.querySelectorAll(".settings-nav-button")).find(
    (element) => element.textContent?.trim() === label,
  ) as HTMLButtonElement | undefined;
  if (!button) {
    throw new Error(`settings nav button missing: ${label}`);
  }
  return button;
}

async function openSettingsScreen(container: HTMLElement) {
  const settingsButton = await waitForElement(
    () =>
      Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) =>
        element.textContent?.includes("Settings"),
      ) as HTMLButtonElement | null,
    "settings button",
  );
  settingsButton.click();
  await flush();
}

function setUserAgent(userAgent: string) {
  const descriptor = Object.getOwnPropertyDescriptor(window.navigator, "userAgent");
  Object.defineProperty(window.navigator, "userAgent", {
    value: userAgent,
    configurable: true,
  });
  return () => {
    if (descriptor) {
      Object.defineProperty(window.navigator, "userAgent", descriptor);
      return;
    }
    Object.defineProperty(window.navigator, "userAgent", {
      value: undefined,
      configurable: true,
    });
  };
}


async function mountApp(payload: BootstrapPayload) {
  apiMocks.bootstrapApp.mockResolvedValue(payload);
  const container = document.createElement("div");
  document.body.append(container);
  const dispose = render(() => <App />, container);
  await waitForElement(() => container.querySelector(".workflow-row"), "workflow rows");
  await flush();
  return { container, dispose };
}

async function switchWorkflow(container: HTMLElement, name: string) {
  const row = [...container.querySelectorAll(".workflow-row-main")].find(
    (element) => element.querySelector(".workflow-row-title")?.textContent === name,
  ) as HTMLButtonElement | undefined;
  if (!row) {
    throw new Error(`workflow row missing: ${name}`);
  }
  row.click();
  await flush();
}

function onboardingDialog(container: HTMLElement) {
  return container.querySelector('[data-testid="first-run-onboarding"]') as HTMLElement | null;
}

async function dismissOnboardingIntro(container: HTMLElement) {
  const intro = container.querySelector('[data-testid="first-run-onboarding-intro"]');
  if (intro instanceof HTMLElement) {
    intro.click();
    await flush();
  }
}

async function clickOnboardingAction(container: HTMLElement, label: string) {
  const button = await waitForElement(
    () =>
      Array.from(container.querySelectorAll("button")).find((element) =>
        element.textContent?.trim().includes(label),
      ) as HTMLButtonElement | null,
    `onboarding action ${label}`,
  );
  button.click();
  await flush();
}

async function advanceOnboarding(container: HTMLElement, count: number) {
  for (let step = 0; step < count; step += 1) {
    await clickOnboardingAction(container, "Next");
  }
}

async function waitForOnboardingClosed(container: HTMLElement) {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (!onboardingDialog(container)) {
      return;
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 20));
  }
  throw new Error("Timed out waiting for first-run onboarding to close");
}

async function startWorkflowRename(container: HTMLElement, name: string) {
  const menuButton = await waitForElement(
    () => container.querySelector(`[aria-label="Workflow options for ${name}"]`),
    `workflow options button for ${name}`,
  );
  (menuButton as HTMLButtonElement).click();
  await flush();
  const renameButton = await waitForElement(
    () =>
      Array.from(container.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')).find(
        (button) => button.textContent === "Rename",
      ) ?? null,
    `rename menu item for ${name}`,
  );
  renameButton.click();
  await flush();
  return waitForElement(
    () => container.querySelector(`input[aria-label="Workflow name for ${name}"]`),
    `workflow rename input for ${name}`,
  ) as Promise<HTMLInputElement>;
}

describe("App first-run onboarding", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("shows full-screen slide onboarding and opens Build with AI", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const dialog = await waitForElement(
        () => onboardingDialog(container),
        "first-run onboarding",
      );
      await dismissOnboardingIntro(container);
      expect(dialog.getAttribute("aria-label")).toBe("Welcome to OpenFlow");
      expect(dialog.textContent).toContain("Build repeatable AI workflows.");

      await clickOnboardingAction(container, "Next");
      expect(dialog.textContent).toMatch(/Explain your\s*workflow/i);

      await advanceOnboarding(container, 2);
      expect(dialog.textContent).toMatch(/Just send\s*a message/i);

      await clickOnboardingAction(container, "Next");
      expect(dialog.textContent).toMatch(/One thing before\s*you start/i);
      expect(dialog.textContent).toContain("AI provider key");

      await clickOnboardingAction(container, "Build with AI");

      expect(apiMocks.startWorkflowAuthoring).toHaveBeenCalledWith(null, null);
      expect(topbarTitle(container)).toBe("Build workflow with AI");
      await waitForOnboardingClosed(container);
      expect(window.localStorage.getItem("openflow.firstRunOnboardingDismissed")).toBe("true");
    } finally {
      dispose();
    }
  });

  test("opens provider setup from onboarding", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await waitForElement(() => onboardingDialog(container), "first-run onboarding");
      await dismissOnboardingIntro(container);
      await advanceOnboarding(container, 4);

      await clickOnboardingAction(container, "Set up provider");

      expect(topbarTitle(container)).toBe("Settings");
      await waitForOnboardingClosed(container);
      expect(window.localStorage.getItem("openflow.firstRunOnboardingDismissed")).toBe("true");
    } finally {
      dispose();
    }
  });
});

describe("workflow authoring chat layout", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
    window.localStorage.setItem("openflow.firstRunOnboardingDismissed", "true");
  });

  async function openWorkflowAuthoring(container: HTMLElement) {
    const button = await waitForElement(
      () =>
        container.querySelector(
          'button[aria-label="New workflow"]',
        ) as HTMLButtonElement | null,
      "new workflow button",
    );
    button.click();
    await flush();
    const aiOption = await waitForElement(
      () =>
        Array.from(container.querySelectorAll('[role="menuitem"]')).find(
          (element) => element.textContent?.trim() === "Create with AI",
        ) as HTMLButtonElement | null,
      "create with ai option",
    );
    aiOption.click();
    await flush();
    await waitForElement(
      () => container.querySelector(".workflow-authoring-screen"),
      "workflow authoring screen",
    );
  }

  test("places New workflow above New chat and offers plain or AI creation", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const primaryActions = Array.from(
        container.querySelectorAll(".sidebar > .sidebar-list > .sidebar-nav-button, .sidebar > .sidebar-list > .sidebar-new-workflow-menu > .sidebar-nav-button"),
      ) as HTMLButtonElement[];
      expect(primaryActions.slice(0, 2).map((button) => button.textContent?.trim())).toEqual([
        "New workflow",
        "New chat",
      ]);

      const newWorkflowButton = primaryActions[0];
      expect(newWorkflowButton.getAttribute("aria-haspopup")).toBe("menu");
      expect(newWorkflowButton.getAttribute("aria-expanded")).toBe("false");
      newWorkflowButton.click();
      await flush();

      const menuItems = Array.from(
        container.querySelectorAll(".sidebar-new-workflow-menu-item"),
      ) as HTMLButtonElement[];
      expect(menuItems.map((item) => item.textContent?.trim())).toEqual([
        "Create new workflow",
        "Create with AI",
      ]);
      expect(newWorkflowButton.getAttribute("aria-expanded")).toBe("true");

      menuItems[0].click();
      await flush();
      expect(apiMocks.createWorkflow).toHaveBeenCalledWith("Workflow 2");
      expect(container.querySelector(".sidebar-new-workflow-popover")).toBeNull();

      newWorkflowButton.click();
      await flush();
      (
        Array.from(container.querySelectorAll(".sidebar-new-workflow-menu-item")).find(
          (item) => item.textContent?.trim() === "Create with AI",
        ) as HTMLButtonElement
      ).click();
      await flush();

      expect(apiMocks.startWorkflowAuthoring).toHaveBeenCalledWith(null, null);
      expect(container.querySelector(".workflow-authoring-screen")).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("uses dock chat shell with bubble composer and thinking indicator while busy", async () => {
    let resolveTurn!: (value: {
      messages: { role: string; content: string }[];
      validation: { valid: boolean; errors: string[]; warnings: string[] };
      draft: null;
    }) => void;
    const turnPromise = new Promise<{
      messages: { role: string; content: string }[];
      validation: { valid: boolean; errors: string[]; warnings: string[] };
      draft: null;
    }>((resolve) => {
      resolveTurn = resolve;
    });
    apiMocks.workflowAuthoringTurn.mockReturnValue(turnPromise);

    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openWorkflowAuthoring(container);

      expect(container.querySelector(".chat-layout")).not.toBeNull();
      expect(container.querySelector(".chat-composer-bar")).not.toBeNull();

      const pill = await waitForElement(
        () => container.querySelector(".chat-composer-pill") as HTMLElement | null,
        "authoring composer pill",
      );
      const pillRadius = window.getComputedStyle(pill).borderRadius;
      expect(pillRadius).not.toBe("0px");

      const textarea = pill.querySelector("textarea") as HTMLTextAreaElement;
      textarea.value = "Build a research workflow";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      await flush();

      pill.querySelector(".composer-send-button")?.dispatchEvent(
        new MouseEvent("click", { bubbles: true }),
      );
      await flush();

      const thinking = await waitForElement(
        () => container.querySelector(".tool-line--thinking") as HTMLElement | null,
        "authoring thinking bubble",
      );
      expect(thinking.querySelector(".tool-line-name-text")?.textContent).toBe("Thinking");
      expect(thinking.querySelector(".tool-line-preview-text--thinking")).toBeNull();

      resolveTurn({
        messages: [
          { role: "user", content: "Build a research workflow" },
          { role: "assistant", content: "Here is a draft." },
        ],
        validation: { valid: false, errors: [], warnings: [] },
        draft: null,
      });
      await flush();
      await flush();
    } finally {
      dispose();
    }
  });
});

describe("App workflow rename", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("deletes an inactive workflow from its row menu without changing selection", async () => {
    const active = makeWorkflow("workflow-1", "Workflow One");
    const deleted = makeWorkflow("workflow-2", "Workflow Two");
    vi.mocked(confirm).mockResolvedValueOnce(true);
    apiMocks.deleteWorkflow.mockResolvedValue([]);
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([active, deleted]),
    );

    try {
      const menuButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Workflow options for Workflow Two"]',
          ) as HTMLButtonElement | null,
        "workflow options button",
      );
      menuButton.click();
      (
        Array.from(container.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')).find(
          (button) => button.textContent === "Delete workflow",
        ) as HTMLButtonElement
      ).click();
      await flush();

      expect(apiMocks.deleteWorkflow).toHaveBeenCalledWith("workflow-2");
      expect(topbarTitle(container)).toBe("Workflow One");
      expect(
        container.querySelector(
          'button[aria-label="Workflow options for Workflow Two"]',
        ),
      ).toBeNull();
      expect(
        container.querySelector(
          'button[aria-label="Workflow options for Workflow One"]',
        ),
      ).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("focuses the rename input and does not switch workflows when it is clicked", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([
        makeWorkflow("workflow-1", "Workflow One"),
        makeWorkflow("workflow-2", "Workflow Two"),
      ]),
    );

    try {
      expect(topbarTitle(container)).toBe("Workflow One");

      const input = await startWorkflowRename(container, "Workflow Two");

      expect(document.activeElement).toBe(input);
      expect(input.selectionStart).toBe(0);
      expect(input.selectionEnd).toBe(input.value.length);

      input.click();
      await flush();

      expect(document.activeElement).toBe(input);
      expect(topbarTitle(container)).toBe("Workflow One");
    } finally {
      dispose();
    }
  });

  test("renders independent workflows above project folders", async () => {
    const independent = makeWorkflow("workflow-independent", "Independent Flow");
    const assigned = makeWorkflow("workflow-assigned", "Assigned Flow");
    const folderProject = makeProject("project-1", "Syntech", ["workflow-assigned"]);
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([independent, assigned], undefined, undefined, [folderProject]),
    );

    try {
      const labels = Array.from(container.querySelectorAll(".sidebar-section-label")).map(
        (element) => element.textContent ?? "",
      );
      expect(labels).toEqual(["Chats", "Workflows", "Projects"]);
      expect(workflowTitles(container)).toEqual(["Independent Flow"]);
      expect(container.querySelector(".project-folder-title")?.textContent).toBe("Syntech");
    } finally {
      dispose();
    }
  });

  test("removes a project from its options menu without deleting project files", async () => {
    const removedWorkflow = makeWorkflow("workflow-removed", "Removed Project Flow");
    const remainingWorkflow = makeWorkflow("workflow-remaining", "Remaining Project Flow");
    const independentWorkflow = makeWorkflow("workflow-independent", "Independent Flow");
    const removedProject = makeProject("project-remove", "Remove Me", ["workflow-removed"]);
    const remainingProject = makeProject("project-keep", "Keep Me", ["workflow-remaining"]);
    vi.mocked(confirm).mockResolvedValueOnce(true);
    apiMocks.saveProjects.mockResolvedValue(undefined);
    apiMocks.loadAllWorkflows.mockResolvedValue([remainingWorkflow, independentWorkflow]);

    const { container, dispose } = await mountApp(
      makeBootstrapPayload(
        [removedWorkflow, remainingWorkflow, independentWorkflow],
        undefined,
        undefined,
        [removedProject, remainingProject],
      ),
    );

    try {
      (
        container.querySelector(
          'button[aria-label="Project options for Remove Me"]',
        ) as HTMLButtonElement
      ).click();
      (
        Array.from(container.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')).find(
          (button) => button.textContent === "Remove project",
        ) as HTMLButtonElement
      ).click();
      await flush();
      await flush();

      expect(confirm).toHaveBeenCalledWith(
        'Remove "Remove Me" from OpenFlow? Its folder and workflow files stay on disk.',
        { title: "Remove project", kind: "warning" },
      );
      expect(apiMocks.saveProjects).toHaveBeenCalledWith([remainingProject]);
      expect(apiMocks.loadAllWorkflows).toHaveBeenCalledTimes(1);
      expect(
        container.querySelector('button[aria-label="Project options for Remove Me"]'),
      ).toBeNull();
      expect(
        container.querySelector('button[aria-label="Project options for Keep Me"]'),
      ).not.toBeNull();
      expect(topbarTitle(container)).toBe("Remaining Project Flow");
    } finally {
      dispose();
    }
  });

  test("copies a workflow from another project via the picker", async () => {
    const source = makeWorkflow("workflow-source", "Source Flow");
    const independent = makeWorkflow("workflow-independent", "Independent Flow");
    const projectA = makeProject("project-a", "Project A", ["workflow-source"]);
    const projectB = makeProject("project-b", "Project B", []);
    const copied = makeWorkflow("workflow-copy", "Source Flow copy");

    apiMocks.copyWorkflowToProject.mockResolvedValue({
      workflow: copied,
      projects: [projectA, { ...projectB, workflow_ids: ["workflow-copy"] }],
    });
    window.localStorage.setItem("openflow.expandedProjectIds", JSON.stringify(["project-b"]));

    const { container, dispose } = await mountApp(
      makeBootstrapPayload([source, independent], undefined, undefined, [projectA, projectB]),
    );

    try {
      const addButton = container.querySelector(
        '[aria-label="Project options for Project B"]',
      ) as HTMLButtonElement;
      addButton.click();
      await flush();

      const copyMenuItem = [...container.querySelectorAll(".project-folder-menu-item")].find(
        (item) => item.textContent === "Copy from…",
      ) as HTMLButtonElement;
      copyMenuItem.click();
      await flush();

      expect(
        container.querySelector('[role="dialog"][aria-label="Add workflow to project"]'),
      ).not.toBeNull();

      const option = [...container.querySelectorAll(".node-picker-option-title")].find(
        (item) => item.textContent === "Source Flow",
      )?.closest("button") as HTMLButtonElement;
      option.click();
      await flush();

      expect(apiMocks.copyWorkflowToProject).toHaveBeenCalledWith(
        "project-b",
        "workflow-source",
      );
      expect(topbarTitle(container)).toBe("Source Flow copy");
    } finally {
      dispose();
      window.localStorage.removeItem("openflow.expandedProjectIds");
    }
  });

  test("adds a project workflow with AI and applies it to that project", async () => {
    const independent = makeWorkflow("workflow-independent", "Independent Flow");
    const project = makeProject("project-b", "Project B", []);
    const generated = makeWorkflow("workflow-generated", "Generated Project Flow");

    apiMocks.workflowAuthoringTurn.mockResolvedValue({
      messages: [
        { role: "user", content: "Build a repo triage workflow" },
        { role: "assistant", content: "Built a project workflow." },
      ],
      validation: { valid: true, errors: [], warnings: [] },
      draft: generated,
    });
    apiMocks.assignWorkflowToProject.mockResolvedValue([
      {
        ...project,
        workflow_ids: ["workflow-generated"],
      },
    ]);
    window.localStorage.setItem("openflow.expandedProjectIds", JSON.stringify(["project-b"]));

    const { container, dispose } = await mountApp(
      makeBootstrapPayload([independent], undefined, undefined, [project]),
    );

    try {
      const addButton = container.querySelector(
        '[aria-label="Project options for Project B"]',
      ) as HTMLButtonElement;
      addButton.click();
      await flush();

      const aiMenuItem = [...container.querySelectorAll(".project-folder-menu-item")].find(
        (item) => item.textContent === "Create with AI",
      ) as HTMLButtonElement;
      aiMenuItem.click();
      await flush();

      expect(apiMocks.startWorkflowAuthoring).toHaveBeenCalledWith(null, "project-b");
      expect(topbarTitle(container)).toBe("Build workflow with AI");

      const textarea = await waitForElement(
        () => container.querySelector(".chat-composer-pill textarea"),
        "authoring textarea",
      );
      (textarea as HTMLTextAreaElement).value = "Build a repo triage workflow";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      await flush();

      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();
      await flush();

      const applyButton = await waitForElement(
        () =>
          [...container.querySelectorAll("button")].find(
            (button) => button.textContent?.trim() === "Create Workflow",
          ) as HTMLButtonElement | null,
        "create workflow button",
      );
      applyButton.click();
      await flush();
      await flush();

      expect(apiMocks.saveWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "workflow-generated",
          name: "Generated Project Flow",
        }),
      );
      expect(apiMocks.assignWorkflowToProject).toHaveBeenCalledWith(
        "project-b",
        "workflow-generated",
      );
      expect(topbarTitle(container)).toBe("Generated Project Flow");
    } finally {
      dispose();
      window.localStorage.removeItem("openflow.expandedProjectIds");
    }
  });

  test("renders the macOS titlebar spacer inside the topbar", async () => {
    const restoreUserAgent = setUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)");
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const topbar = await waitForElement(() => container.querySelector(".topbar"), "topbar");
      expect(topbar.classList.contains("topbar-macos")).toBe(true);
    } finally {
      dispose();
      restoreUserAgent();
    }
  });

  test("commits the edited workflow name on blur", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([
        makeWorkflow("workflow-1", "Workflow One"),
        makeWorkflow("workflow-2", "Workflow Two"),
      ]),
    );

    try {
      const input = await startWorkflowRename(container, "Workflow Two");

      input.value = "Workflow Two Renamed";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.blur();
      await flush();

      expect(container.querySelector(".workflow-row-input")).toBeNull();
      expect(workflowTitles(container)).toContain("Workflow Two Renamed");
    } finally {
      dispose();
    }
  });

  test("cancels the edited workflow name on escape", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([
        makeWorkflow("workflow-1", "Workflow One"),
        makeWorkflow("workflow-2", "Workflow Two"),
      ]),
    );

    try {
      const input = await startWorkflowRename(container, "Workflow Two");

      input.value = "Discarded Rename";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      await flush();

      expect(container.querySelector(".workflow-row-input")).toBeNull();
      expect(workflowTitles(container)).toContain("Workflow Two");
      expect(workflowTitles(container)).not.toContain("Discarded Rename");
    } finally {
      dispose();
    }
  });
});

describe("App agent dashboard", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("opens the agent dashboard from the sidebar", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")], [makeAgent("agent-1", "Research Agent")]),
    );

    try {
      const agentsButton = await waitForElement(
        () => Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) => element.textContent?.includes("Agents")) as HTMLButtonElement | null,
        "agents button",
      );
      agentsButton.click();
      await flush();

      expect(topbarTitle(container)).toBe("Agents");
      expect(
        container.querySelector(".agents-sidebar-panel .workflow-row-title")?.textContent,
      ).toBe("Research Agent");
      expect(
        container.querySelector(".agents-sidebar-panel .workflow-row.active") as HTMLElement | null,
      ).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("creates and saves reusable agents", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")], []),
    );

    try {
      const agentsButton = await waitForElement(
        () => Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) => element.textContent?.includes("Agents")) as HTMLButtonElement | null,
        "agents button",
      );
      agentsButton.click();
      await flush();

      const newAgentButton = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) =>
            element.textContent?.includes("New agent"),
          ) as HTMLButtonElement | null,
        "new agent button",
      );
      newAgentButton.click();
      await flush();

      expect(apiMocks.createAgentDefinition).toHaveBeenCalledWith("Agent 1");

      const modelSelect = await waitForElement(
        () =>
          Array.from(container.querySelectorAll("label span"))
            .find((element) => element.textContent === "Model")
            ?.parentElement?.querySelector(".text-select-trigger") as HTMLButtonElement | null,
        "agent model select",
      );
      expect(modelSelect?.querySelector(".text-select-value")?.textContent).toBe("gpt-4.1-mini");

      const autoStartInput = Array.from(container.querySelectorAll("label.checkbox-row input")).find(
        (element) => (element.parentElement?.textContent ?? "").includes("Start automatically"),
      ) as HTMLInputElement | undefined;
      expect(autoStartInput?.checked).toBe(true);

      const systemPromptInput = Array.from(container.querySelectorAll("label span")).find(
        (element) => element.textContent === "System prompt",
      )?.parentElement?.querySelector("textarea") as HTMLTextAreaElement | null;
      expect(systemPromptInput?.value).toBe("You are a focused AI agent in a node workflow.");

      const taskPromptInput = container.querySelector(
        'textarea[aria-label="Task prompt"]',
      ) as HTMLTextAreaElement | null;
      expect(taskPromptInput?.value).toBe("Return a concise JSON object for this node.");

      const schemaInput = Array.from(container.querySelectorAll("label span")).find(
        (element) => element.textContent === "JSON output schema",
      )?.parentElement?.querySelector("textarea") as HTMLTextAreaElement | null;
      expect(JSON.parse(schemaInput?.value ?? "")).toEqual({
        type: "object",
        additionalProperties: false,
        properties: {
          summary: { type: "string" },
        },
        required: ["summary"],
      });

      const nameInput = await waitForElement(
        () => Array.from(container.querySelectorAll("label span")).find((element) => element.textContent === "Name")?.parentElement?.querySelector("input") as HTMLInputElement | null,
        "agent name input",
      );
      nameInput.value = "Planner Agent";
      nameInput.dispatchEvent(new Event("input", { bubbles: true }));

      const saveButton = Array.from(container.querySelectorAll("button")).find(
        (element) => element.textContent === "Save",
      ) as HTMLButtonElement | undefined;
      expect(saveButton).toBeDefined();
      saveButton?.click();
      await flush();

      expect(apiMocks.saveAgents).toHaveBeenCalledWith(
        expect.arrayContaining([
          expect.objectContaining({
            id: "created-agent",
            name: "Planner Agent",
          }),
        ]),
      );
    } finally {
      dispose();
    }
  });

  test("creates an agent with AI from the row below New agent", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")], []),
    );

    try {
      const agentsButton = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) =>
            element.textContent?.includes("Agents"),
          ) as HTMLButtonElement | null,
        "agents button",
      );
      agentsButton.click();
      await flush();

      const createRows = Array.from(
        container.querySelectorAll(".agents-sidebar-panel .sidebar-nav-button"),
      ) as HTMLButtonElement[];
      expect(createRows.slice(0, 2).map((row) => row.textContent?.trim())).toEqual([
        "New agent",
        "Create with AI",
      ]);

      createRows[1].click();
      await flush();

      const description = await waitForElement(
        () =>
          Array.from(container.querySelectorAll("label span"))
            .find((element) => element.textContent === "What should this agent do?")
            ?.parentElement?.querySelector("textarea") as HTMLTextAreaElement | null,
        "agent description",
      );
      description.value = "Review research notes and identify unsupported claims.";
      description.dispatchEvent(new Event("input", { bubbles: true }));

      const createButton = Array.from(container.querySelectorAll("button")).find(
        (element) => element.textContent === "Create Agent",
      ) as HTMLButtonElement | undefined;
      expect(createButton?.disabled).toBe(false);
      createButton?.click();
      await flush();

      expect(apiMocks.createAgentDefinitionWithAi).toHaveBeenCalledWith(
        "Review research notes and identify unsupported claims.",
        expect.any(Object),
        "stored-openai-key",
      );
      expect(
        container.querySelector(".agents-sidebar-panel .workflow-row-title")?.textContent,
      ).toBe("Research Reviewer");
      const generatedNameInput = Array.from(container.querySelectorAll("label span"))
        .find((element) => element.textContent === "Name")
        ?.parentElement?.querySelector("input") as HTMLInputElement | null;
      expect(generatedNameInput?.value).toBe("Research Reviewer");
    } finally {
      dispose();
    }
  });

  test("lets you choose a saved agent when adding a node", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const savedAgents = [makeAgent("agent-1", "Research Agent"), makeAgent("agent-2", "Writer Agent")];
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([workflow], savedAgents),
    );

    try {
      const addNodeButton = await waitForElement(
        () => container.querySelector('button[aria-label="Canvas add node"]') as HTMLButtonElement | null,
        "add node button",
      );
      addNodeButton.click();
      await flush();

      expect(container.querySelector('[role="dialog"][aria-label="Add agent node"]')).not.toBeNull();

      const savedAgentButton = await waitForElement(
        () => Array.from(container.querySelectorAll(".node-picker-option-title")).find((element) => element.textContent === "Writer Agent")?.closest("button") as HTMLButtonElement | null,
        "saved agent option",
      );
      savedAgentButton.click();
      await flush();

      expect(apiMocks.createAgentNode).toHaveBeenCalledWith(1, 128, 116, "agent-2");
      await openInspector(container);
      expect(container.querySelector(".panel-header-title-row h3")?.textContent).toBe("Writer Agent");
      const requestUserInput = Array.from(container.querySelectorAll("label.checkbox-row input")).find(
        (element) => (element.parentElement?.textContent ?? "").includes("Allow follow-up questions"),
      ) as HTMLInputElement | undefined;
      expect(requestUserInput?.checked).toBe(false);
    } finally {
      dispose();
    }
  });
  test("validates the workflow after adding a node", async () => {
    apiMocks.validateWorkflow.mockResolvedValue({ layerCount: 1, layers: [["node-1"]] });
    apiMocks.createAgentNode.mockResolvedValue(
      makeNodeFromAgent(1, 128, 116, makeAgent("agent-2", "Writer Agent")),
    );
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const savedAgents = [makeAgent("agent-1", "Research Agent"), makeAgent("agent-2", "Writer Agent")];
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([workflow], savedAgents),
    );

    try {
      const addNodeButton = await waitForElement(
        () => container.querySelector('button[aria-label="Canvas add node"]') as HTMLButtonElement | null,
        "add node button",
      );
      addNodeButton.click();
      await flush();

      const savedAgentButton = await waitForElement(
        () => Array.from(container.querySelectorAll(".node-picker-option-title")).find((element) => element.textContent === "Writer Agent")?.closest("button") as HTMLButtonElement | null,
        "saved agent option",
      );
      savedAgentButton.click();
      await flush();

      expect(apiMocks.validateWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "workflow-1",
          nodes: expect.arrayContaining([
            expect.objectContaining({ id: "workflow-1-node-1" }),
            expect.objectContaining({ label: "Writer Agent" }),
          ]),
        }),
      );
    } finally {
      dispose();
    }
  });

  test("shows an error toast when validation fails after adding a node", async () => {
    apiMocks.validateWorkflow.mockRejectedValue(new Error("workflow contains a cycle"));
    apiMocks.createAgentNode.mockResolvedValue(
      makeNodeFromAgent(1, 128, 116, null),
    );
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      const addNodeButton = await waitForElement(
        () => container.querySelector('button[aria-label="Canvas add node"]') as HTMLButtonElement | null,
        "add node button",
      );
      addNodeButton.click();
      await flush();

      const blankNodeButton = await waitForElement(
        () => Array.from(container.querySelectorAll(".node-picker-option-title")).find((element) => element.textContent === "Blank agent node")?.closest("button") as HTMLButtonElement | null,
        "blank node option",
      );
      blankNodeButton.click();
      await flush();

      const errorToast = await waitForElement(
        () =>
          document.body.querySelector(
            '[data-sonner-toast][data-mounted="true"][data-visible="true"][data-type="error"] [data-title]',
          ) as HTMLElement | null,
        "validation error toast",
      );
      expect(errorToast.textContent).toContain("workflow contains a cycle");
    } finally {
      dispose();
    }
  });


  test("node tool access is hideable and saves enabled tools", async () => {
    apiMocks.saveWorkflows.mockResolvedValue(undefined);
    apiMocks.saveSettings.mockResolvedValue(undefined);

    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      await openInspector(container);
      expect(
        Array.from(container.querySelectorAll("span")).some((element) => element.textContent === "Max tool rounds"),
      ).toBe(false);

      const toolsSectionHeader = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".inspector-section-header")).find((element) =>
            element.textContent?.includes("Tools"),
          ) as HTMLButtonElement | null,
        "tools section header",
      );
      toolsSectionHeader.click();
      await flush();

      const approvalTrigger = container.querySelector(
        ".tool-config-body .text-select-trigger",
      ) as HTMLButtonElement | null;
      expect(approvalTrigger).not.toBeNull();
      expect(approvalTrigger?.textContent).toContain("Read auto-approve");
      approvalTrigger!.click();
      expect(
        [...container.querySelectorAll(".tool-config-body .text-select-option")].map(
          (option) => option.textContent,
        ),
      ).toEqual([
        "Read only",
        "Read auto-approve, write prompt",
        "Always ask",
        "Auto-approve all",
      ]);
      approvalTrigger!.click();

      toolsSectionHeader.click();
      await flush();

      expect(
        Array.from(container.querySelectorAll("span")).some((element) => element.textContent === "Max tool rounds"),
      ).toBe(false);

      const saveButton = container.querySelector('button[aria-label="Save workflow"]') as HTMLButtonElement | null;
      expect(saveButton).not.toBeNull();
      saveButton?.click();
      await flush();
      const saveCalls = apiMocks.saveWorkflows.mock.calls as [Workflow[]][];
      const savedWorkflows = saveCalls[saveCalls.length - 1]?.[0];
      expect(savedWorkflows?.[0]?.nodes[0]?.agent.tools).toEqual({
        approvalMode: "write",
      });
    } finally {
      dispose();
    }
  });
});

describe("App workflow structural editing", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("deleting a node removes its connections and persists immediately", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push({
      ...makeNodeFromAgent(1, 480, 140, null),
      id: "workflow-1-node-2",
      label: "Review",
    });
    workflow.edges.push({
      id: "edge-1",
      from: "workflow-1-node-1",
      to: "workflow-1-node-2",
    });
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      const deleteButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Canvas delete node workflow-1-node-1"]',
          ) as HTMLButtonElement | null,
        "canvas node delete button",
      );
      deleteButton.click();
      await flush();

      await vi.waitFor(() => expect(apiMocks.saveWorkflow).toHaveBeenCalledTimes(1));
      expect(apiMocks.saveWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "workflow-1",
          nodes: [expect.objectContaining({ id: "workflow-1-node-2" })],
          edges: [],
        }),
      );
    } finally {
      dispose();
    }
  });

  test("undo restores a deleted node and its connections", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push({
      ...makeNodeFromAgent(1, 480, 140, null),
      id: "workflow-1-node-2",
      label: "Review",
    });
    workflow.edges.push({
      id: "edge-1",
      from: "workflow-1-node-1",
      to: "workflow-1-node-2",
    });
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      const deleteButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Canvas delete node workflow-1-node-1"]',
          ) as HTMLButtonElement | null,
        "canvas node delete button",
      );
      deleteButton.click();
      await vi.waitFor(() => expect(apiMocks.saveWorkflow).toHaveBeenCalledTimes(1));

      const undoButton = await waitForElement(
        () =>
          Array.from(document.body.querySelectorAll("button")).find(
            (button) => button.textContent?.trim() === "Undo",
          ) as HTMLButtonElement | null,
        "delete undo button",
      );
      undoButton.click();

      await vi.waitFor(() => expect(apiMocks.saveWorkflow).toHaveBeenCalledTimes(2));
      const restored = apiMocks.saveWorkflow.mock.calls[1]?.[0] as Workflow;
      expect(restored.nodes.map((node) => node.id)).toEqual([
        "workflow-1-node-1",
        "workflow-1-node-2",
      ]);
      expect(restored.edges).toEqual([
        {
          id: "edge-1",
          from: "workflow-1-node-1",
          to: "workflow-1-node-2",
        },
      ]);
    } finally {
      dispose();
    }
  });

  test("deleting an edge persists immediately and can be undone", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push({
      ...makeNodeFromAgent(1, 480, 140, null),
      id: "workflow-1-node-2",
      label: "Review",
    });
    workflow.edges.push({
      id: "edge-1",
      from: "workflow-1-node-1",
      to: "workflow-1-node-2",
    });
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      const deleteButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Canvas delete edge edge-1"]',
          ) as HTMLButtonElement | null,
        "canvas edge delete button",
      );
      deleteButton.click();
      await vi.waitFor(() => expect(apiMocks.saveWorkflow).toHaveBeenCalledTimes(1));
      expect((apiMocks.saveWorkflow.mock.calls[0]?.[0] as Workflow).edges).toEqual([]);

      const undoButton = await waitForElement(
        () =>
          Array.from(document.body.querySelectorAll("button")).find(
            (button) => button.textContent?.trim() === "Undo",
          ) as HTMLButtonElement | null,
        "edge delete undo button",
      );
      undoButton.click();

      await vi.waitFor(() => expect(apiMocks.saveWorkflow).toHaveBeenCalledTimes(2));
      expect((apiMocks.saveWorkflow.mock.calls[1]?.[0] as Workflow).edges).toEqual([
        {
          id: "edge-1",
          from: "workflow-1-node-1",
          to: "workflow-1-node-2",
        },
      ]);
    } finally {
      dispose();
    }
  });

  test("serializes edge validation and saves snapshots in edit order", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push(
      {
        ...makeNodeFromAgent(1, 480, 140, null),
        id: "workflow-1-node-2",
      },
      {
        ...makeNodeFromAgent(2, 840, 140, null),
        id: "workflow-1-node-3",
      },
    );
    let resolveFirstValidation:
      | ((value: { layerCount: number; layers: string[][] }) => void)
      | undefined;
    apiMocks.validateWorkflow
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirstValidation = resolve;
          }),
      )
      .mockResolvedValue({ layerCount: 1, layers: [["node-1"]] });
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      const firstEdgeButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Canvas create edge workflow-1-node-1 workflow-1-node-2"]',
          ) as HTMLButtonElement | null,
        "first edge button",
      );
      firstEdgeButton.click();
      await flush();

      const secondEdgeButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Canvas create edge workflow-1-node-2 workflow-1-node-3"]',
          ) as HTMLButtonElement | null,
        "second edge button",
      );

      secondEdgeButton.click();
      await flush();

      expect(apiMocks.validateWorkflow).toHaveBeenCalledTimes(1);
      expect(apiMocks.saveWorkflow).not.toHaveBeenCalled();

      resolveFirstValidation?.({ layerCount: 1, layers: [["node-1"]] });
      await vi.waitFor(() => expect(apiMocks.validateWorkflow).toHaveBeenCalledTimes(2));
      await vi.waitFor(() => expect(apiMocks.saveWorkflow).toHaveBeenCalledTimes(2));

      expect((apiMocks.saveWorkflow.mock.calls[0]?.[0] as Workflow).edges).toHaveLength(1);
      expect((apiMocks.saveWorkflow.mock.calls[1]?.[0] as Workflow).edges).toHaveLength(2);
    } finally {
      dispose();
    }
  });

  test("structural edits are blocked while a run is active", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const payload = makeBootstrapPayload([workflow]);
    payload.runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp(payload);

    try {
      const deleteButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Canvas delete node workflow-1-node-1"]',
          ) as HTMLButtonElement | null,
        "canvas node delete button",
      );
      deleteButton.click();
      await flush();

      expect(apiMocks.saveWorkflow).not.toHaveBeenCalled();
      expect(
        container.querySelector('button[aria-label="Select node workflow-1-node-1"]'),
      ).not.toBeNull();
    } finally {
      dispose();
    }
  });
});

describe("App settings persistence", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("renders settings without sidebar but with unified topbar", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);

      expect(container.querySelector(".sidebar")).toBeNull();
      expect(container.querySelector(".topbar")).not.toBeNull();
      expect(container.querySelector('button[aria-label="Hide left sidebar"]')).toBeNull();
      expect(container.querySelector('button[aria-label="Open navigation"]')).toBeNull();
      expect(topbarTitle(container)).toBe("Settings");
      expect(container.querySelector(".settings-shell")).not.toBeNull();
      expect(container.querySelector(".settings-nav")).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("renders toast with the selected app theme", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);
      const darkThemeButton = Array.from(
        container.querySelectorAll('[aria-label="Theme preference"] button'),
      ).find((element) => element.textContent === "Dark") as HTMLButtonElement | undefined;
      expect(darkThemeButton).toBeDefined();
      darkThemeButton?.click();
      await flush();

      settingsNavButton(container, "Providers").click();
      await flush();
      const saveButton = Array.from(container.querySelectorAll("button")).find(
        (element) => element.textContent === "Save settings",
      ) as HTMLButtonElement | undefined;
      expect(saveButton).toBeDefined();
      saveButton?.click();
      await flush();

      const toaster = await waitForElement(
        () => document.body.querySelector("[data-sonner-toaster]"),
        "toast container",
      );
      expect(toaster.getAttribute("data-sonner-theme")).toBe("dark");
    } finally {
      dispose();
    }
  });

  test("returns to editor chrome from settings back button", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);
      const backButton = await waitForElement(
        () => container.querySelector(".settings-back-button") as HTMLButtonElement | null,
        "settings back button",
      );
      backButton.click();
      await flush();

      expect(container.querySelector(".sidebar")).not.toBeNull();
      expect(container.querySelector(".topbar")).not.toBeNull();
      expect(container.querySelector(".settings-shell")).toBeNull();
    } finally {
      dispose();
    }
  });

  test("settings nav exposes Appearance, Providers, and MCP Servers", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);
      const labels = [...container.querySelectorAll('.settings-nav-button')].map(
        (element) => element.textContent?.trim(),
      );
      expect(labels).toEqual([
        "Appearance",
        "Providers",
        "Search",
        "MCP Servers",
        "Diagnostics",
        "About",
      ]);
    } finally {
      dispose();
    }
  });

  test("provider fields are visible together on Providers page", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);
      settingsNavButton(container, "Providers").click();
      await flush();

      expect(container.querySelector('input[type="password"]')).not.toBeNull();
      expect(container.querySelector('.providers-section .text-select-trigger')).not.toBeNull();
      expect(
        [...container.querySelectorAll("button")].some(
          (element) => element.textContent === "Save settings",
        ),
      ).toBe(true);
    } finally {
      dispose();
    }
  });

  test("loads and saves provider API keys per provider", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);

      settingsNavButton(container, "Providers").click();
      await flush();

      const apiKeyInput = await waitForElement(
        () => container.querySelector('input[type="password"]'),
        "provider api key input",
      ) as HTMLInputElement;
      expect(apiKeyInput.value).toBe("stored-openai-key");

      const providerTrigger = container.querySelector(
        ".providers-section .text-select-trigger",
      ) as HTMLButtonElement;
      providerTrigger.click();
      const compatibleOption = [...container.querySelectorAll(".text-select-option")].find(
        (element) => element.textContent === "Compatible",
      ) as HTMLButtonElement;
      compatibleOption.click();
      await flush();

      const compatibleApiKeyInput = await waitForElement(
        () => {
          const input = container.querySelector('input[type="password"]') as HTMLInputElement | null;
          return input?.value === "stored-compatible-key" ? input : null;
        },
        "compatible provider api key",
      ) as HTMLInputElement;

      compatibleApiKeyInput.value = "updated-compatible-key";
      compatibleApiKeyInput.dispatchEvent(new Event("input", { bubbles: true }));
      await flush();

      const saveButton = Array.from(container.querySelectorAll("button")).find(
        (element) => element.textContent === "Save settings",
      ) as HTMLButtonElement | undefined;
      expect(saveButton).toBeDefined();
      saveButton?.click();
      await flush();
      const successToast = await waitForElement(
        () =>
          Array.from(document.body.querySelectorAll("*")).find(
            (element) => element.textContent?.includes("Settings saved successfully."),
          ) ?? null,
        "settings saved toast",
      );
      expect(successToast.textContent).toContain("Settings saved successfully.");

      expect(apiMocks.saveProviderApiKey).toHaveBeenCalledWith(
        "custom_openai_compatible",
        "updated-compatible-key",
      );
      expect(apiMocks.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          providers: expect.objectContaining({
            custom_openai_compatible: expect.objectContaining({
              base_url: "https://example.invalid/v1",
            }),
          }),
        }),
      );
      const lastSavedSettings = apiMocks.saveSettings.mock.calls[apiMocks.saveSettings.mock.calls.length - 1]?.[0];
      expect(JSON.stringify(lastSavedSettings)).not.toContain("api_key");
    } finally {
      dispose();
    }
  });
});

describe("App run trace presentation", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("shows human-readable tool names in trace rows and details", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    runState.runTrace = [
      {
        nodeId: workflow.nodes[0].id,
        nodeLabel: "openflow_call_subagent",
        status: "running",
        message: "running tool openflow_call_subagent",
        output: null,
      },
    ];
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });

    try {
      await openRunTraceTab(container);

      const traceRow = container.querySelector(".trace-row") as HTMLButtonElement | null;
      expect(traceRow?.querySelector("strong")?.textContent).toBe("Call Subagent");
      expect(traceRow?.textContent).toContain("Running Call Subagent");

      traceRow?.click();
      await flush();
      expect(container.querySelector(".trace-detail h3")?.textContent).toBe("Call Subagent");
      expect(container.querySelector(".trace-detail p")?.textContent).toBe(
        "Running Call Subagent",
      );
    } finally {
      dispose();
    }
  });
});

describe("App chat slash commands", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });
  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("expands known skill commands before submitting paused-node input", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    apiMocks.submitUserInput.mockResolvedValue(runState);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);
    try {
      const textarea = await waitForElement(
        () => container.querySelector(".chat-composer-pill textarea"),
        "chat textarea",
      );
      (textarea as HTMLTextAreaElement).value = "/systematic-debugging Investigate ORCHID-91";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      await flush();

      const sendButton = await waitForElement(
        () => container.querySelector(".chat-composer .primary-button"),
        "chat send button",
      );
      (sendButton as HTMLButtonElement).click();
      await flush();

      expect(apiMocks.submitUserInput).toHaveBeenCalledWith(
        workflow.nodes[0].id,
        "Skill invocation:\n- systematic-debugging\n\nUser message:\nInvestigate ORCHID-91",
      );
    } finally {
      dispose();
    }
  });

  test("renders skill description preview when typing a known slash command", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const textarea = await waitForElement(
        () => container.querySelector(".chat-composer-pill textarea"),
        "chat textarea",
      );
      (textarea as HTMLTextAreaElement).value = "/systematic-debugging Investigate ORCHID-91";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      await flush();

      const preview = await waitForElement(
        () => container.querySelector(".skill-description-preview"),
        "skill description preview",
      );
      expect(preview.textContent).toContain("Use when encountering bugs or test failures.");
      expect(preview.textContent).toContain("/systematic-debugging");
    } finally {
      dispose();
    }
  });

  test("shows skill combobox suggestions while typing a slash command", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const textarea = await waitForElement(
        () => container.querySelector(".chat-composer-pill textarea"),
        "chat textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "/sys";
      textarea.selectionStart = 4;
      textarea.selectionEnd = 4;
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      await flush();

      const combobox = await waitForElement(
        () => container.querySelector(".skill-command-combobox"),
        "skill command combobox",
      );
      expect(combobox.textContent).toContain("/systematic-debugging");
    } finally {
      dispose();
    }
  });

  test("submits paused-node input on enter from the compact composer", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    apiMocks.submitUserInput.mockResolvedValue(runState);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const textarea = await waitForElement(
        () => container.querySelector(".chat-composer-pill textarea"),
        "chat textarea",
      );
      (textarea as HTMLTextAreaElement).value = "Approved";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      await flush();

      expect(apiMocks.submitUserInput).toHaveBeenCalledWith(workflow.nodes[0].id, "Approved");
    } finally {
      dispose();
    }
  });

  test("keeps the compact composer free of provider controls", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelector(".composer-settings-button")).toBeNull();
      expect(container.querySelector(".composer-status-pill")).toBeNull();
      expect(container.querySelector('[aria-label="Send to paused node"]')).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("toggles dock focus mode from the dock tab bar", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const editor = container.querySelector(".editor-screen");
      expect(editor?.classList.contains("editor-screen--chat-focus")).toBe(false);

      const focusButton = await waitForElement(
        () => container.querySelector('[aria-label="Focus panel"]') as HTMLButtonElement | null,
        "focus panel button",
      );
      focusButton.click();
      await flush();

      expect(editor?.classList.contains("editor-screen--chat-focus")).toBe(true);
      expect(container.querySelector('[aria-label="Show canvas"]')).not.toBeNull();

      (container.querySelector('[aria-label="Show canvas"]') as HTMLButtonElement).click();
      await flush();

      expect(editor?.classList.contains("editor-screen--chat-focus")).toBe(false);
    } finally {
      dispose();
    }
  });

  test("shows node messages under the selected node label", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes[0].label = "Agent 2";
    const runState = makeAwaitingRunState(workflow);
    runState.chatLogs[workflow.nodes[0].id] = [
      { role: "System", content: "Node 'Agent 2' started" },
      { role: "Thinking", content: "Agent prompt: You are a focused AI agent..." },
      { role: "Assistant", content: "{\"summary\":\"Hello\"}" },
    ];
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const labels = Array.from(container.querySelectorAll(".chat-role")).map((element) => element.textContent);
      expect(labels).toEqual(["System"]);
      expect(
        container.querySelector('.chat-segment[data-node-id="' + workflow.nodes[0].id + '"] .eyebrow')
          ?.textContent,
      ).toBe("Agent 2");
      expect(container.querySelector(".thinking-bubble")).toBeNull();
      const thinkingLine = container.querySelector('.tool-line[data-tool-name="thinking"]');
      expect(thinkingLine).not.toBeNull();
      expect(thinkingLine?.querySelector(".tool-line-name")?.textContent).toContain(
        "Thought for a while",
      );
      expect(thinkingLine?.querySelector(".tool-line-target")).toBeNull();
    } finally {
      dispose();
    }
  });

  test("renders compact tool line with invocation target in chat", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes[0].label = "Idea";
    const nodeId = workflow.nodes[0].id;
    const runState = makeAwaitingRunState(workflow);
    runState.toolCallsByNode[nodeId] = [
      {
        toolCallId: "call-read-1",
        toolName: "read",
        status: "completed",
        arguments: { path: "README.md" },
        lastOutput: "¶README.md\n1:# OpenFlow",
        isError: false,
        streaming: false,
      },
    ];
    runState.chatLogs[nodeId] = [{ role: "Thinking", content: "", toolCallId: "call-read-1" }];
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const line = container.querySelector(".tool-line");
      expect(line).not.toBeNull();
      expect(line?.getAttribute("data-tool-name")).toBe("read");
      expect(line?.querySelector(".tool-line-name")?.textContent).toContain("Read README.md");
      expect(line?.querySelector(".tool-line-status")).toBeNull();
      expect(line?.querySelector(".tool-line-output")).toBeNull();
    } finally {
      dispose();
    }
  });

  test("does not insert blank rows between consecutive tool lines", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes[0].label = "Idea";
    const nodeId = workflow.nodes[0].id;
    const runState = makeAwaitingRunState(workflow);
    runState.toolCallsByNode[nodeId] = [
      {
        toolCallId: "call-read-1",
        toolName: "read",
        status: "completed",
        arguments: { path: "a.txt" },
        lastOutput: "ok",
        isError: false,
        streaming: false,
      },
      {
        toolCallId: "call-read-2",
        toolName: "read",
        status: "completed",
        arguments: { path: "b.txt" },
        lastOutput: "ok",
        isError: false,
        streaming: false,
      },
    ];
    runState.chatLogs[nodeId] = [
      { role: "Thinking", content: "", toolCallId: "call-read-1" },
      { role: "assistant", content: "" },
      { role: "Thinking", content: "", toolCallId: "call-read-2" },
    ];
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelectorAll(".tool-stack")).toHaveLength(1);
      expect(container.querySelectorAll(".chat-message-row--assistant").length).toBe(0);
      expect(container.textContent).toContain("Read 2 files");
    } finally {
      dispose();
    }
  });
});

describe("Global chat layout", () => {
  let runStateListener: ((state: WorkflowRunState) => void) | undefined;

  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
    runStateListener = undefined;
  });

  beforeEach(() => {
    installDefaultApiMocks();
    apiMocks.listenToRunState.mockImplementation(async (handler) => {
      runStateListener = handler;
      return () => {};
    });
  });

  test("shows pending strip while run is active before live nodes appear", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState: WorkflowRunState = {
      active: true,
      awaitingNodeId: null,
      awaitingNodeIds: [],
      activeManualNodeId: null,
      activeToolCallId: null,
      pendingApprovals: [],
      toolCallsByNode: {},
      toolArtifacts: {},
      execApprovalGranted: false,
      statusByNode: Object.fromEntries(workflow.nodes.map((node) => [node.id, "idle"])),
      subagentsByNode: {},
      lastReport: null,
      lastError: null,
      chatLogs: {},
      runTrace: [],
      outputs: {},
      changedFiles: [],
      changedFilesByNode: {},
      editBatches: [],
    };
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelector(".chat-live-strip--pending")).not.toBeNull();
      expect(container.querySelector(".chat-live-starting")?.textContent).toBe(
        "Starting workflow…",
      );
    } finally {
      dispose();
    }
  });

  test("logs an unchanged run failure once across streamed state deltas", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState: WorkflowRunState = {
      active: true,
      awaitingNodeId: null,
      awaitingNodeIds: [],
      activeManualNodeId: null,
      activeToolCallId: null,
      pendingApprovals: [],
      toolCallsByNode: {},
      toolArtifacts: {},
      execApprovalGranted: false,
      statusByNode: Object.fromEntries(workflow.nodes.map((node) => [node.id, "failed"])),
      subagentsByNode: {},
      lastReport: null,
      lastError: "provider returned no usable output",
      chatLogs: {},
      runTrace: [],
      outputs: {},
      changedFiles: [],
      changedFilesByNode: {},
      editBatches: [],
    };
    const settings: AppSettings = {
      ...SETTINGS,
      local_diagnostics: { debug_output: true },
    };
    const { dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings,
      runState,
    });

    try {
      expect(runStateListener).toBeDefined();
      runStateListener?.(runState);
      runStateListener?.({
        ...runState,
        chatLogs: {
          [workflow.nodes[0].id]: [{ role: "assistant", content: "partial", streaming: true }],
        },
      });
      runStateListener?.({
        ...runState,
        chatLogs: {
          [workflow.nodes[0].id]: [{ role: "assistant", content: "partial text", streaming: true }],
        },
      });
      await flush();

      expect(apiMocks.appendDebugLog).toHaveBeenCalledTimes(1);
      expect(apiMocks.appendDebugLog).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({ level: "error", message: runState.lastError }),
      );
    } finally {
      dispose();
    }
  });

  test("blocks chat behind a picker for parallel awaiting siblings", async () => {
    const workflow = makeParallelWorkflow();
    const runState = makeParallelAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const chips = container.querySelectorAll(".chat-filter-chip");
      const labels = [...chips].map((chip) => chip.textContent ?? "");
      expect(labels.some((text) => text.includes("Branch B"))).toBe(true);
      expect(labels.some((text) => text.includes("Branch C"))).toBe(true);
      // No composer until the user picks a node to talk to.
      expect(container.querySelectorAll(".chat-composer-pill textarea").length).toBe(0);
      const hint = container.querySelector(".chat-parallel-hint");
      expect(hint).not.toBeNull();
      expect(hint?.textContent).toContain("2");
      expect(hint?.textContent).toContain("agents are running in parallel");
      expect(hint?.textContent).toContain("Select a node above to view and reply");
    } finally {
      dispose();
    }
  });

  test("shows tool approval while parallel siblings are still unpicked", async () => {
    const workflow = makeParallelWorkflow();
    const runState = makeParallelAwaitingRunState(workflow);
    const [, b, c] = workflow.nodes;
    runState.awaitingNodeIds = [b.id];
    runState.awaitingNodeId = b.id;
    runState.statusByNode[b.id] = "awaiting_input";
    runState.statusByNode[c.id] = "awaiting_tool_approval";
    runState.pendingApprovals = [
      {
        approvalId: "approval-parallel",
        nodeId: c.id,
        nodeLabel: c.label,
        toolCall: {
          id: "call-bash",
          name: "bash",
          arguments: { command: "ls -la" },
        },
        tier: "read",
      },
    ];
    apiMocks.submitToolApproval.mockResolvedValue(runState);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelector(".chat-parallel-hint")).not.toBeNull();
      expect(container.querySelectorAll(".chat-composer-pill textarea").length).toBe(0);
      const card = container.querySelector(".chat-composer-bar .tool-approval-card");
      expect(card).not.toBeNull();
      card?.querySelector(".primary-button")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();
      expect(apiMocks.submitToolApproval).toHaveBeenCalledWith("approval-parallel", true);
    } finally {
      dispose();
    }
  });

  test("picking a parallel node streams it inline and routes the composer to it", async () => {
    const workflow = makeParallelWorkflow();
    const runState = makeParallelAwaitingRunState(workflow);
    apiMocks.submitUserInput.mockResolvedValue(runState);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelector(".chat-parallel-hint")).not.toBeNull();
      const branchCChip = [...container.querySelectorAll(".chat-filter-chip")].find((chip) =>
        chip.textContent?.includes("Branch C"),
      );
      expect(branchCChip).not.toBeUndefined();
      branchCChip!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

      expect(container.querySelector(".chat-parallel-hint")).toBeNull();
      expect(container.querySelectorAll(".chat-segment").length).toBe(1);
      expect(container.querySelector('.chat-segment[data-node-id="node-c"]')).not.toBeNull();
      const textarea = await waitForElement(
        () =>
          container.querySelector(
            ".chat-composer-bar .chat-composer-pill textarea",
          ) as HTMLTextAreaElement | null,
        "picked node composer",
      );
      textarea.value = "branch c reply";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea
        .closest(".chat-composer")
        ?.querySelector(".primary-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

      expect(apiMocks.submitUserInput).toHaveBeenCalledWith("node-c", "branch c reply");
      // The remaining live node stays visible and can be selected.
      const remaining = [...container.querySelectorAll(".chat-filter-chip")].filter((chip) =>
        chip.textContent?.includes("Branch B"),
      );
      expect(remaining.length).toBe(1);
    } finally {
      dispose();
    }
  });

  test("hides parallel live hint when filtering to a settled node chip", async () => {
    const workflow = makeParallelWorkflow();
    const runState = makeParallelAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelector(".chat-parallel-hint")).not.toBeNull();
      const planChip = [...container.querySelectorAll(".chat-filter-chip")].find((chip) =>
        chip.textContent?.includes("Plan"),
      );
      expect(planChip).not.toBeUndefined();
      planChip!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();
      expect(container.querySelector(".chat-parallel-hint")).toBeNull();
    } finally {
      dispose();
    }
  });

  test("keeps completed upstream messages in settled history", async () => {
    const workflow = makeParallelWorkflow();
    const runState = makeParallelAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const settledHeader = container.querySelector('.chat-segment[data-node-id="node-a"] .eyebrow');
      expect(settledHeader?.textContent).toBe("Plan");
      const chips = container.querySelectorAll(".chat-filter-chip");
      expect([...chips].some((chip) => chip.textContent?.includes("Branch B"))).toBe(true);
      expect([...chips].some((chip) => chip.textContent?.includes("Branch C"))).toBe(true);
    } finally {
      dispose();
    }
  });

  test("run-state awaiting update opens chat without changing canvas selection", async () => {
    const workflow = makeParallelWorkflow();
    const runState = makeParallelAwaitingRunState(workflow);
    runState.awaitingNodeIds = [];
    runState.awaitingNodeId = null;
    runState.statusByNode["node-b"] = "idle";
    runState.statusByNode["node-c"] = "idle";
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });

    try {
      await openInspector(container);
      const inspectorTitle = () =>
        container.querySelector(".inspector-panel .panel-header-title-row")?.textContent;
      expect(inspectorTitle()).toContain("Plan");
      runStateListener?.({
        ...runState,
        awaitingNodeIds: ["node-b"],
        awaitingNodeId: "node-b",
        statusByNode: {
          ...runState.statusByNode,
          "node-b": "awaiting_input",
        },
      });
      await flush();
      const chatTab = Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
        (button) => button.textContent === "Chat",
      );
      expect(chatTab?.classList.contains("active")).toBe(true);
      expect(inspectorTitle()).toContain("Plan");
    } finally {
      dispose();
    }
  });

  test("moves completed node from live strip into settled history", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push({
      id: "workflow-1-node-2",
      label: "Downstream",
      kind: "Agent",
      position: { x: 320, y: 140 },
      agent: workflow.nodes[0].agent,
    });
    workflow.edges.push({
      id: "edge-2",
      from: workflow.nodes[0].id,
      to: "workflow-1-node-2",
    });
    const runState = makeAwaitingRunState(workflow);
    runState.statusByNode[workflow.nodes[0].id] = "completed";
    runState.statusByNode["workflow-1-node-2"] = "awaiting_input";
    runState.awaitingNodeId = "workflow-1-node-2";
    runState.awaitingNodeIds = ["workflow-1-node-2"];
    runState.chatLogs[workflow.nodes[0].id] = [{ role: "Assistant", content: "upstream done" }];
    runState.chatLogs["workflow-1-node-2"] = [];
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      expect(container.querySelectorAll(".chat-live-column").length).toBe(0);
      expect(container.querySelectorAll(".chat-segment").length).toBe(2);
      expect(container.querySelector('.chat-segment[data-node-id="' + workflow.nodes[0].id + '"]')).not.toBeNull();
      expect(
        container.querySelector(".chat-composer-bar .chat-composer-pill textarea"),
      ).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("renders approval card for a single live node in segment footer", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    runState.statusByNode[workflow.nodes[0].id] = "awaiting_tool_approval";
    runState.awaitingNodeId = null;
    runState.pendingApprovals = [
      {
        approvalId: "approval-1",
        nodeId: workflow.nodes[0].id,
        nodeLabel: workflow.nodes[0].label,
        toolCall: {
          id: "call-1",
          name: "grep",
          arguments: { pattern: "todo" },
        },
        tier: "read",
      },
    ];
    apiMocks.submitToolApproval.mockResolvedValue(runState);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const card = container.querySelector(".chat-composer-bar .tool-approval-card");
      expect(card).not.toBeNull();
      card?.querySelector(".primary-button")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();
      expect(apiMocks.submitToolApproval).toHaveBeenCalledWith("approval-1", true);
    } finally {
      dispose();
    }
  });

  test("filter chips narrow settled history", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push({
      id: "workflow-1-node-2",
      label: "Second",
      kind: "Agent",
      position: { x: 320, y: 140 },
      agent: workflow.nodes[0].agent,
    });
    const runState = makeAwaitingRunState(workflow);
    runState.active = false;
    runState.awaitingNodeId = null;
    runState.statusByNode = {
      [workflow.nodes[0].id]: "completed",
      "workflow-1-node-2": "completed",
    };
    runState.chatLogs = {
      [workflow.nodes[0].id]: [{ role: "Assistant", content: "first" }],
      "workflow-1-node-2": [{ role: "Assistant", content: "second" }],
    };
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const chips = container.querySelectorAll(".chat-filter-chip");
      expect(chips.length).toBeGreaterThan(1);
      (chips[1] as HTMLButtonElement).click();
      await flush();
      expect(container.querySelectorAll(".chat-segment").length).toBe(1);
      (chips[0] as HTMLButtonElement).click();
      await flush();
      expect(container.querySelectorAll(".chat-segment").length).toBe(2);
    } finally {
      dispose();
    }
  });

  test("restores chat history after switching workflows", async () => {
    const workflowA = makeWorkflow("workflow-1", "Workflow One");
    workflowA.nodes.push({
      id: "workflow-1-node-2",
      label: "Second",
      kind: "Agent",
      position: { x: 320, y: 140 },
      agent: workflowA.nodes[0].agent,
    });
    const workflowB = makeWorkflow("workflow-2", "Workflow Two");
    const runState = makeAwaitingRunState(workflowA);
    runState.active = false;
    runState.awaitingNodeId = null;
    runState.statusByNode = {
      [workflowA.nodes[0].id]: "completed",
      "workflow-1-node-2": "completed",
    };
    runState.chatLogs = {
      [workflowA.nodes[0].id]: [{ role: "Assistant", content: "first" }],
      "workflow-1-node-2": [{ role: "Assistant", content: "second" }],
    };
    const { container, dispose } = await mountApp({
      ...makeBootstrapPayload([workflowA, workflowB]),
      runState,
    });

    try {
      await switchWorkflow(container, "Workflow Two");
      expect(topbarTitle(container)).toBe("Workflow Two");
      await switchWorkflow(container, "Workflow One");
      expect(topbarTitle(container)).toBe("Workflow One");
      await openChatTab(container);

      expect(container.querySelector(".conversation-empty-state")).toBeNull();
      expect(container.querySelectorAll(".chat-segment").length).toBe(2);

      const secondNodeButton = container.querySelector(
        '[aria-label="Select node workflow-1-node-2"]',
      ) as HTMLButtonElement | null;
      expect(secondNodeButton).not.toBeNull();
      secondNodeButton!.click();
      await flush();

      expect(container.querySelectorAll(".chat-segment").length).toBe(1);
      expect(
        container.querySelector('.chat-segment[data-node-id="workflow-1-node-2"]'),
      ).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("settled segments expose header status classes for styling hooks", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes.push({
      id: "workflow-1-node-2",
      label: "Second",
      kind: "Agent",
      position: { x: 320, y: 140 },
      agent: workflow.nodes[0].agent,
    });
    const runState = makeAwaitingRunState(workflow);
    runState.active = false;
    runState.awaitingNodeId = null;
    runState.statusByNode = {
      [workflow.nodes[0].id]: "completed",
      "workflow-1-node-2": "completed",
    };
    runState.chatLogs = {
      [workflow.nodes[0].id]: [{ role: "Assistant", content: "first" }],
      "workflow-1-node-2": [{ role: "Assistant", content: "second" }],
    };
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });
    await openChatTab(container);

    try {
      const firstId = workflow.nodes[0].id;
      const secondId = "workflow-1-node-2";
      const first = container.querySelector(`.chat-segment[data-node-id="${firstId}"]`);
      const second = container.querySelector(`.chat-segment[data-node-id="${secondId}"]`);
      expect(container.querySelectorAll(".chat-segment").length).toBe(2);
      expect(
        first?.querySelector(".chat-segment-status")?.classList.contains("status-completed"),
      ).toBe(true);
      expect(second?.querySelector(".chat-segment-header")).not.toBeNull();
    } finally {
      dispose();
    }
  });
});

describe("App bottom dock", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    window.localStorage.clear();
  });
  beforeEach(() => {
    installDefaultApiMocks();
    vi.stubGlobal(
      "ResizeObserver",
      vi.fn().mockImplementation(() => ({
        observe: vi.fn(),
        disconnect: vi.fn(),
      })),
    );
  });

  test("selecting a canvas node does not automatically open the inspector", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    apiMocks.bootstrapApp.mockResolvedValue({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      projects: [],
      runState: null,
    });

    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(() => <App />, container);

    try {
      await waitForElement(() => container.querySelector(".editor-screen"), "editor screen");
      const nodeButton = await waitForElement(
        () =>
          container.querySelector(
            `button[aria-label="Select node ${workflow.nodes[0].id}"]`,
          ) as HTMLButtonElement | null,
        "canvas node button",
      );

      nodeButton.click();
      await flush();

      expect(container.querySelector(".inspector-panel")).toBeNull();
      const chatTab = Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
        (button) => button.textContent === "Chat",
      );
      expect(chatTab?.classList.contains("active")).toBe(true);
    } finally {
      dispose();
    }
  });

  test("opens terminal tab and starts terminal in active workflow cwd", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const project = makeProject("p1", "Repo", ["workflow-1"]);
    apiMocks.bootstrapApp.mockResolvedValue({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      projects: [project],
      runState: null,
    });
    window.localStorage.setItem("openflow.expandedProjectIds", JSON.stringify(["p1"]));

    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(() => <App />, container);

    try {
      await waitForElement(() => container.querySelector(".editor-screen"), "editor screen");
      await flush();

      const terminalTab = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
            (button) => button.textContent === "Terminal",
          ) as HTMLButtonElement | null,
        "terminal tab",
      );
      terminalTab.click();
      await flush();

      expect(apiMocks.startTerminal).toHaveBeenCalledWith("/tmp/Repo", 80, 24);
      expect(container.querySelector(".terminal-host")).not.toBeNull();
      expect(container.querySelector(".terminal-tab-label")?.textContent).toBe("Repo");
      expect(container.querySelector(".terminal-tab-select")?.getAttribute("title")).toBe("/tmp/Repo");
    } finally {
      dispose();
      window.localStorage.removeItem("openflow.expandedProjectIds");
    }
  });

  test("opens another terminal session when the new-terminal control is clicked", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const project = makeProject("p1", "Repo", ["workflow-1"]);
    apiMocks.bootstrapApp.mockResolvedValue({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      projects: [project],
      runState: null,
    });
    apiMocks.startTerminal
      .mockResolvedValueOnce({ sessionId: "terminal-1", cwd: "/tmp/Repo" })
      .mockResolvedValueOnce({ sessionId: "terminal-2", cwd: "/tmp/Repo" });
    window.localStorage.setItem("openflow.expandedProjectIds", JSON.stringify(["p1"]));

    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(() => <App />, container);

    try {
      await waitForElement(() => container.querySelector(".editor-screen"), "editor screen");
      await flush();

      const terminalTab = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
            (button) => button.textContent === "Terminal",
          ) as HTMLButtonElement | null,
        "terminal tab",
      );
      terminalTab.click();
      await flush();

      const addButton = await waitForElement(
        () => container.querySelector(".terminal-tab-add") as HTMLButtonElement | null,
        "terminal add button",
      );
      addButton.click();
      await flush();

      expect(apiMocks.startTerminal).toHaveBeenCalledTimes(2);
      expect(container.querySelectorAll(".terminal-tab")).toHaveLength(2);
    } finally {
      dispose();
      window.localStorage.removeItem("openflow.expandedProjectIds");
    }
  });

  test("collapses and restores the bottom dock by dragging the seam", async () => {
    Object.defineProperty(window, "innerHeight", { value: 1000, configurable: true });
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });

    try {
      const editorScreen = await waitForElement(
        () => container.querySelector(".editor-screen"),
        "editor screen",
      ) as HTMLDivElement;
      const resizeZone = await waitForElement(
        () => container.querySelector(".dock-resize-zone"),
        "dock resize zone",
      );

      expect(container.querySelector(".dock-visibility-action")).toBeNull();
      expect(container.querySelector(".dock-resize-handle")).toBeNull();
      expect(container.querySelector(".chat-layout")).not.toBeNull();

      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 1300, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("52px");
      expect(container.querySelector(".chat-layout")).toBeNull();

      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 460, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("192px");
      expect(container.querySelector(".chat-layout")).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("opens chat at seventy-five percent height after the dock was collapsed", async () => {
    Object.defineProperty(window, "innerHeight", { value: 1000, configurable: true });
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });

    try {
      const editorScreen = await waitForElement(
        () => container.querySelector(".editor-screen"),
        "editor screen",
      ) as HTMLDivElement;
      const resizeZone = await waitForElement(
        () => container.querySelector(".dock-resize-zone"),
        "dock resize zone",
      );
      const chatTab = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
            (button) => button.textContent === "Chat",
          ) as HTMLButtonElement | null,
        "chat tab",
      );

      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 1300, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("52px");

      chatTab.click();
      await flush();

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("750px");
      expect(container.querySelector(".chat-layout")).not.toBeNull();
    } finally {
      dispose();
    }
  });

  test("restores chat to seventy-five percent height after leaving focus mode", async () => {
    Object.defineProperty(window, "innerHeight", { value: 1000, configurable: true });
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });

    try {
      const editorScreen = await waitForElement(
        () => container.querySelector(".editor-screen"),
        "editor screen",
      ) as HTMLDivElement;
      await openChatTab(container);

      const focusButton = await waitForElement(
        () => container.querySelector('[aria-label="Focus panel"]') as HTMLButtonElement | null,
        "focus panel button",
      );
      focusButton.click();
      await flush();

      (container.querySelector('[aria-label="Show canvas"]') as HTMLButtonElement).click();
      await flush();

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("750px");
    } finally {
      dispose();
    }
  });

  test("resizes the bottom dock from the seam", async () => {
    Object.defineProperty(window, "innerHeight", { value: 1000, configurable: true });
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const runState = makeAwaitingRunState(workflow);
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState,
    });

    try {
      const editorScreen = await waitForElement(
        () => container.querySelector(".editor-screen"),
        "editor screen",
      ) as HTMLDivElement;
      const resizeZone = await waitForElement(
        () => container.querySelector(".dock-resize-zone"),
        "dock resize zone",
      );

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("750px");
      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 520, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("830px");
    } finally {
      dispose();
    }
  });
});

describe("Idle global chat kickoff", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
    apiMocks.listenToRunState.mockResolvedValue(() => {});
  });

  test("shows enabled composer when no run is active", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: null,
    });
    await openChatTab(container);
    try {
      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      expect(textarea?.disabled).toBe(false);
      expect(textarea?.getAttribute("aria-label")).toContain("Run in the top bar");
    } finally {
      dispose();
    }
  });

  test("starts run from idle global chat with entrypoint", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes[0].agent.auto_start = true;
    const idleRunState = { ...makeAwaitingRunState(workflow), active: false };
    apiMocks.startRun.mockResolvedValue(makeAwaitingRunState(workflow));
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: idleRunState,
    });
    await openChatTab(container);
    try {
      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "Plan project ORCHID-91";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();
      expect(apiMocks.startRun).toHaveBeenCalledWith(
        expect.objectContaining({ id: "workflow-1" }),
        expect.objectContaining({ active_provider: "openai" }),
        null,
        "stored-openai-key",
        "Plan project ORCHID-91",
      );
    } finally {
      dispose();
    }
  });

  test("auto-flushes kickoff to single awaiting manual root", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes[0].agent.auto_start = false;
    const started = makeAwaitingRunState(workflow);
    started.active = true;
    started.awaitingNodeId = workflow.nodes[0].id;
    started.awaitingNodeIds = [workflow.nodes[0].id];
    started.statusByNode[workflow.nodes[0].id] = "awaiting_input";
    apiMocks.startRun.mockResolvedValue(started);
    apiMocks.submitUserInput.mockResolvedValue(started);
    const idleRunState = { ...makeAwaitingRunState(workflow), active: false };
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: idleRunState,
    });
    await openChatTab(container);
    try {
      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "Manual kickoff";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();
      expect(apiMocks.startRun).toHaveBeenCalledWith(
        expect.objectContaining({ id: "workflow-1" }),
        expect.objectContaining({ active_provider: "openai" }),
        null,
        "stored-openai-key",
        "Manual kickoff",
      );
      expect(apiMocks.submitUserInput).toHaveBeenCalledWith(
        workflow.nodes[0].id,
        "Manual kickoff",
      );
    } finally {
      dispose();
    }
  });

});

describe("Ad-hoc chats", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
    apiMocks.listenToRunState.mockResolvedValue(() => {});
  });

  test("hides chats and persists the collapsed section", async () => {
    const payload = {
      ...makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
      chats: [makeChat("chat-1", "Project notes")],
    };
    const { container, dispose } = await mountApp(payload);

    try {
      const toggle = container.querySelector(
        'button[aria-label="Toggle chats section"]',
      ) as HTMLButtonElement;
      const section = toggle.closest(".sidebar-section-group");
      const collapsible = section?.querySelector(".collapsible-section");

      expect(toggle.getAttribute("aria-expanded")).toBe("true");
      expect(collapsible?.classList.contains("collapsible-section--open")).toBe(true);

      toggle.click();
      await flush();

      expect(toggle.getAttribute("aria-expanded")).toBe("false");
      expect(collapsible?.classList.contains("collapsible-section--open")).toBe(false);
      expect(window.localStorage.getItem("openflow.chatsSectionHidden")).toBe("true");
    } finally {
      dispose();
    }
  });

  test("deletes a chat from its history menu after confirmation", async () => {
    const deletedChat = makeChat("chat-1", "Delete me");
    const survivor = makeChat("chat-2", "Keep me");
    vi.mocked(confirm).mockResolvedValueOnce(true);
    apiMocks.deleteChat.mockResolvedValue(undefined);
    const payload = {
      ...makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
      chats: [deletedChat, survivor],
    };
    const { container, dispose } = await mountApp(payload);

    try {
      const menuButton = await waitForElement(
        () =>
          container.querySelector(
            'button[aria-label="Chat options for Delete me"]',
          ) as HTMLButtonElement | null,
        "chat options button",
      );
      menuButton.click();
      (
        Array.from(container.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')).find(
          (button) => button.textContent === "Delete chat",
        ) as HTMLButtonElement
      ).click();
      await flush();

      expect(confirm).toHaveBeenCalledWith(
        'Delete "Delete me"? This removes it from chat history.',
        { title: "Delete chat", kind: "warning" },
      );
      expect(apiMocks.deleteChat).toHaveBeenCalledWith("chat-1");
      expect(
        container.querySelector('button[aria-label="Chat options for Delete me"]'),
      ).toBeNull();
      expect(container.textContent).toContain("Keep me");
    } finally {
      dispose();
    }
  });

  test("shows approval, effort, and project controls for a chat", async () => {
    const chat = makeChat("chat-1");
    const project = makeProject("project-1", "OpenFlow");
    const chatSettings: AppSettings = {
      ...SETTINGS,
      providers: {
        ...SETTINGS.providers,
        openai: {
          ...SETTINGS.providers.openai,
          known_models: ["gpt-4.1-mini", "gpt-5"],
          reasoning_effort_options: [
            { value: "high", label: "High", uses_budget_tokens: false },
          ],
        },
      },
    };
    apiMocks.updateChatConfig.mockImplementation(async (_chatId, config) => ({
      ...chat,
      config,
    }));
    const payload = {
      ...makeBootstrapPayload(
        [makeWorkflow("workflow-1", "Workflow One")],
        undefined,
        undefined,
        [project],
      ),
      chats: [chat],
      settings: chatSettings,
    };
    const { container, dispose } = await mountApp(payload);

    try {
      const chatRow = Array.from(container.querySelectorAll(".workflow-row-main")).find(
        (element) => element.querySelector(".workflow-row-title")?.textContent === "New chat",
      ) as HTMLButtonElement;
      chatRow.click();
      await flush();

      expect(
        container.querySelector('[aria-label="Chat project"]')?.textContent,
      ).toBe("Project: None");
      expect(
        container.querySelector('[aria-label="Chat model"]')?.textContent,
      ).toBe("Model: Default (gpt-4.1-mini)");
      expect(
        container.querySelector('[aria-label="Chat tool approval mode"]')?.textContent,
      ).toBe("Approval: Read only");
      expect(
        container.querySelector('[aria-label="Chat reasoning effort"]')?.textContent,
      ).toMatch(/^Effort: /);

      (container.querySelector('[aria-label="Chat model"]') as HTMLButtonElement).click();
      await flush();
      const modelOption = Array.from(
        container.querySelectorAll(".text-select-option"),
      ).find((element) => element.textContent === "gpt-5") as HTMLButtonElement;
      modelOption.click();
      await flush();

      expect(
        container.querySelector('[aria-label="Chat model"]')?.textContent,
      ).toBe("Model: gpt-5");
      expect(apiMocks.updateChatConfig).toHaveBeenCalledWith(
        "chat-1",
        expect.objectContaining({ model: "gpt-5" }),
      );

      (container.querySelector('[aria-label="Chat model"]') as HTMLButtonElement).click();
      await flush();
      const defaultModelOption = Array.from(
        container.querySelectorAll(".text-select-option"),
      ).find(
        (element) => element.textContent === "Default (gpt-4.1-mini)",
      ) as HTMLButtonElement;
      defaultModelOption.click();
      await flush();

      expect(
        container.querySelector('[aria-label="Chat model"]')?.textContent,
      ).toBe("Model: Default (gpt-4.1-mini)");
      expect(apiMocks.updateChatConfig).toHaveBeenCalledWith(
        "chat-1",
        expect.objectContaining({ model: null }),
      );

      (container.querySelector('[aria-label="Chat project"]') as HTMLButtonElement).click();
      await flush();
      const projectOption = Array.from(
        container.querySelectorAll(".text-select-option"),
      ).find((element) => element.textContent === "OpenFlow") as HTMLButtonElement;
      projectOption.click();
      await flush();

      expect(
        container.querySelector('[aria-label="Chat project"]')?.textContent,
      ).toBe("Project: OpenFlow");
      expect(apiMocks.updateChatConfig).toHaveBeenCalledWith(
        "chat-1",
        expect.objectContaining({ projectId: "project-1" }),
      );

      (
        container.querySelector(
          '[aria-label="Chat tool approval mode"]',
        ) as HTMLButtonElement
      ).click();
      await flush();
      const approvalOption = Array.from(
        container.querySelectorAll(".text-select-option"),
      ).find((element) => element.textContent === "Always ask") as HTMLButtonElement;
      approvalOption.click();
      await flush();

      expect(
        container.querySelector('[aria-label="Chat tool approval mode"]')?.textContent,
      ).toBe("Approval: Always ask");
      expect(apiMocks.updateChatConfig).toHaveBeenLastCalledWith(
        "chat-1",
        expect.objectContaining({ approvalMode: "always_ask" }),
      );

      (
        container.querySelector(
          '[aria-label="Chat reasoning effort"]',
        ) as HTMLButtonElement
      ).click();
      await flush();
      const effortOption = Array.from(
        container.querySelectorAll(".text-select-option"),
      ).find((element) => element.textContent === "High") as HTMLButtonElement;
      effortOption.click();
      await flush();

      expect(
        container.querySelector('[aria-label="Chat reasoning effort"]')?.textContent,
      ).toBe("Effort: High");
      expect(apiMocks.updateChatConfig).toHaveBeenLastCalledWith(
        "chat-1",
        expect.objectContaining({ reasoningEffort: "high" }),
      );
    } finally {
      dispose();
    }
  });

  test("creates a full-page chat and starts it without creating a workflow", async () => {
    const chat = makeChat("chat-1");
    const startedChat = { ...chat, title: "Explain durable execution", runId: "run-1" };
    const startedState = makeAwaitingRunState(makeWorkflow(chat.id, chat.title));
    startedState.runId = "run-1";
    apiMocks.createChat.mockResolvedValue(chat);
    apiMocks.startChat.mockResolvedValue({ chat: startedChat, runState: startedState });
    apiMocks.submitUserInput.mockResolvedValue(startedState);
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      (container.querySelector('button[aria-label="New chat"]') as HTMLButtonElement).click();
      await flush();

      expect(container.querySelector(".chat-screen")).not.toBeNull();
      expect(container.querySelector(".canvas-panel")).toBeNull();
      expect(container.querySelector(".chat-segment-header")).toBeNull();
      expect(container.querySelector('[aria-label="Filter conversation by node"]')).toBeNull();

      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "Explain durable execution";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

      expect(apiMocks.startChat).toHaveBeenCalledWith(
        "chat-1",
        expect.objectContaining({ active_provider: "openai" }),
        "stored-openai-key",
        "Explain durable execution",
      );
      expect(apiMocks.createWorkflow).not.toHaveBeenCalled();
      expect(apiMocks.saveWorkflows).not.toHaveBeenCalled();
      expect(container.querySelector(".topbar-title")?.textContent).toContain(
        "Explain durable execution",
      );
    } finally {
      dispose();
    }
  });

  test("applies a model change while the active chat is thinking", async () => {
    const chat = { ...makeChat("chat-1", "Active chat"), runId: "run-1" };
    const executionWorkflow = makeWorkflow(chat.id, chat.title);
    const activeState = makeAwaitingRunState(executionWorkflow);
    activeState.active = true;
    activeState.runId = "run-1";
    activeState.awaitingNodeId = null;
    activeState.awaitingNodeIds = [];
    activeState.statusByNode[executionWorkflow.nodes[0].id] = "started";
    apiMocks.replayRun.mockResolvedValue(activeState);
    apiMocks.updateChatConfig.mockImplementation(async (_chatId, config) => ({
      ...chat,
      config,
    }));
    apiMocks.updateNodeRuntimeConfig.mockResolvedValue(activeState);
    const settings: AppSettings = {
      ...SETTINGS,
      providers: {
        ...SETTINGS.providers,
        openai: {
          ...SETTINGS.providers.openai,
          known_models: ["gpt-4.1-mini", "gpt-5"],
        },
      },
    };
    const payload = {
      ...makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
      chats: [chat],
      settings,
    };
    const { container, dispose } = await mountApp(payload);

    try {
      const chatRow = Array.from(container.querySelectorAll(".workflow-row-main")).find(
        (element) =>
          element.querySelector(".workflow-row-title")?.textContent === "Active chat",
      ) as HTMLButtonElement;
      chatRow.click();
      await flush();

      const modelControl = container.querySelector(
        '[aria-label="Chat model"]',
      ) as HTMLButtonElement;
      expect(container.querySelector(".direct-chat-generating")?.textContent).toContain(
        "Thinking",
      );
      expect(modelControl.disabled).toBe(false);
      modelControl.click();
      await flush();
      (
        Array.from(container.querySelectorAll(".text-select-option")).find(
          (element) => element.textContent === "gpt-5",
        ) as HTMLButtonElement
      ).click();
      await flush();

      expect(apiMocks.updateNodeRuntimeConfig).toHaveBeenCalledWith(
        executionWorkflow.nodes[0].id,
        {
          model: "gpt-5",
          approvalMode: "read_only",
          reasoningEffort: null,
          reasoningBudgetTokens: null,
        },
      );
    } finally {
      dispose();
    }
  });

  test("renders the first message from the atomic chat start", async () => {
    const chat = makeChat("chat-1");
    const executionWorkflow = makeWorkflow(chat.id, chat.title);
    const initialState = makeAwaitingRunState(executionWorkflow);
    initialState.active = true;
    initialState.runId = "run-1";
    initialState.awaitingNodeId = null;
    initialState.awaitingNodeIds = [];
    initialState.statusByNode[executionWorkflow.nodes[0].id] = "started";
    initialState.chatLogs[executionWorkflow.nodes[0].id] = [
      { role: "user", content: "Explain durable execution" },
    ];
    apiMocks.createChat.mockResolvedValue(chat);
    apiMocks.startChat.mockResolvedValue({
      chat: { ...chat, runId: "run-1" },
      runState: initialState,
    });
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      (container.querySelector('button[aria-label="New chat"]') as HTMLButtonElement).click();
      await flush();

      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "Explain durable execution";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

      expect(apiMocks.getRunState).toHaveBeenCalled();
      expect(apiMocks.submitUserInput).not.toHaveBeenCalled();
      expect(container.querySelector(".direct-chat-transcript")?.textContent).toContain(
        "Explain durable execution",
      );
    } finally {
      dispose();
    }
  });

  test("reopens a saved chat and resumes its durable run before sending", async () => {
    const chat = { ...makeChat("chat-1", "Durable execution"), runId: "run-1" };
    const executionWorkflow = makeWorkflow(chat.id, chat.title);
    const awaiting = makeAwaitingRunState(executionWorkflow);
    awaiting.active = false;
    awaiting.runId = "run-1";
    awaiting.chatLogs[executionWorkflow.nodes[0].id] = [
      { role: "user", content: "How do durable runs work?" },
      { role: "assistant", content: "They persist checkpoints between app sessions." },
    ];
    apiMocks.replayRun.mockResolvedValue(awaiting);
    apiMocks.resumeDurableRun.mockResolvedValue(awaiting);
    apiMocks.updateNodeRuntimeConfig.mockResolvedValue(awaiting);
    apiMocks.submitUserInput.mockResolvedValue(awaiting);
    const payload = {
      ...makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
      chats: [chat],
    };
    const { container, dispose } = await mountApp(payload);

    try {
      const chatRow = Array.from(container.querySelectorAll(".workflow-row-main")).find(
        (element) =>
          element.querySelector(".workflow-row-title")?.textContent === "Durable execution",
      ) as HTMLButtonElement;
      chatRow.click();
      await flush();

      expect(container.querySelector(".direct-chat-transcript")?.textContent).toContain(
        "How do durable runs work?",
      );
      expect(container.querySelector(".direct-chat-transcript")?.textContent).toContain(
        "They persist checkpoints between app sessions.",
      );
      expect(container.querySelector(".chat-segment-header")).toBeNull();

      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "Continue from there";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

      expect(apiMocks.replayRun).toHaveBeenCalledWith("run-1");
      expect(apiMocks.resumeDurableRun).toHaveBeenCalledWith(
        "run-1",
        expect.objectContaining({ active_provider: "openai" }),
        "stored-openai-key",
      );
      expect(apiMocks.updateNodeRuntimeConfig).toHaveBeenCalledWith(
        executionWorkflow.nodes[0].id,
        {
          model: "gpt-4.1-mini",
          approvalMode: "read_only",
          reasoningEffort: null,
          reasoningBudgetTokens: null,
        },
      );
      expect(apiMocks.submitUserInput).toHaveBeenCalledWith(
        executionWorkflow.nodes[0].id,
        "Continue from there",
      );
    } finally {
      dispose();
    }
  });

  test("restarts a saved chat whose old run has no checkpoint", async () => {
    const chat = { ...makeChat("chat-1", "Broken first message"), runId: "run-broken" };
    const executionWorkflow = makeWorkflow(chat.id, chat.title);
    const restartedState = makeAwaitingRunState(executionWorkflow);
    restartedState.active = true;
    restartedState.runId = "run-2";
    restartedState.awaitingNodeId = null;
    restartedState.awaitingNodeIds = [];
    restartedState.chatLogs[executionWorkflow.nodes[0].id] = [
      { role: "user", content: "Try again" },
    ];
    apiMocks.replayRun.mockRejectedValue(
      new Error("run run-broken has no checkpoints"),
    );
    apiMocks.startChat.mockResolvedValue({
      chat: { ...chat, runId: "run-2" },
      runState: restartedState,
    });
    const payload = {
      ...makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
      chats: [chat],
    };
    const { container, dispose } = await mountApp(payload);

    try {
      const chatRow = Array.from(container.querySelectorAll(".workflow-row-main")).find(
        (element) =>
          element.querySelector(".workflow-row-title")?.textContent ===
          "Broken first message",
      ) as HTMLButtonElement;
      chatRow.click();
      await flush();

      const textarea = container.querySelector(
        ".chat-composer-pill textarea",
      ) as HTMLTextAreaElement;
      textarea.value = "Try again";
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      container
        .querySelector(".composer-send-button")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

      expect(apiMocks.replayRun).toHaveBeenCalledWith("run-broken");
      expect(apiMocks.resumeDurableRun).not.toHaveBeenCalled();
      expect(apiMocks.startChat).toHaveBeenCalledWith(
        "chat-1",
        expect.objectContaining({ active_provider: "openai" }),
        "stored-openai-key",
        "Try again",
      );
      expect(container.querySelector(".direct-chat-transcript")?.textContent).toContain(
        "Try again",
      );
    } finally {
      dispose();
    }
  });
});

describe("App sidebar visibility", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
    Object.defineProperty(window, "innerWidth", { value: 1280, configurable: true });
    window.dispatchEvent(new Event("resize"));
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("applies sidebar-hidden at medium desktop widths when toggled", async () => {
    Object.defineProperty(window, "innerWidth", { value: 1200, configurable: true });
    window.dispatchEvent(new Event("resize"));

    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const hideButton = container.querySelector(
        'button[aria-label="Hide left sidebar"]',
      ) as HTMLButtonElement;
      expect(hideButton).not.toBeNull();
      hideButton.click();
      await flush();

      const shell = container.querySelector(".app-shell");
      expect(shell?.classList.contains("app-shell--sidebar-hidden")).toBe(true);
      expect(window.localStorage.getItem("openflow.leftPanelHidden")).toBe("true");
    } finally {
      dispose();
    }
  });
});

describe("App compact shell", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("opens and closes the sidebar drawer from the compact nav trigger", async () => {
    Object.defineProperty(window, "innerWidth", { value: 390, configurable: true });
    window.dispatchEvent(new Event("resize"));
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const shell = container.querySelector(".app-shell");
      expect(shell?.classList.contains("app-shell--compact")).toBe(true);
      expect(container.querySelector(".editor-screen")).not.toBeNull();

      const navButton = await waitForElement(
        () => container.querySelector('button[aria-label="Open navigation"]') as HTMLButtonElement | null,
        "compact nav button",
      );
      navButton.click();
      await flush();
      expect(shell?.classList.contains("app-shell--sidebar-drawer-open")).toBe(true);

      const scrim = container.querySelector(".sidebar-drawer-scrim") as HTMLButtonElement;
      scrim.click();
      await flush();
      expect(shell?.classList.contains("app-shell--sidebar-drawer-open")).toBe(false);
    } finally {
      dispose();
      Object.defineProperty(window, "innerWidth", { value: 1280, configurable: true });
      window.dispatchEvent(new Event("resize"));
    }
  });

  test("closes the drawer after selecting a sidebar destination", async () => {
    Object.defineProperty(window, "innerWidth", { value: 390, configurable: true });
    window.dispatchEvent(new Event("resize"));
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([
        makeWorkflow("workflow-1", "Workflow One"),
        makeWorkflow("workflow-2", "Workflow Two"),
      ]),
    );

    try {
      const shell = container.querySelector(".app-shell");
      const navButton = await waitForElement(
        () => container.querySelector('button[aria-label="Open navigation"]') as HTMLButtonElement | null,
        "compact nav button",
      );
      navButton.click();
      await flush();

      const agentsButton = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) =>
            element.textContent?.includes("Agents"),
          ) as HTMLButtonElement | null,
        "agents button",
      );
      agentsButton.click();
      await flush();

      expect(shell?.classList.contains("app-shell--sidebar-drawer-open")).toBe(false);
      expect(topbarTitle(container)).toBe("Agents");
    } finally {
      dispose();
      Object.defineProperty(window, "innerWidth", { value: 1280, configurable: true });
      window.dispatchEvent(new Event("resize"));
    }
  });
});

function mockLocalTimezone(timezone: string) {
  vi.spyOn(Intl, "DateTimeFormat").mockImplementation(
    () =>
      ({
        resolvedOptions: () => ({ timeZone: timezone }),
      }) as Intl.DateTimeFormat,
  );
}

describe("App schedule screen", () => {
  beforeEach(() => {
    installDefaultApiMocks();
    Object.defineProperty(window, "innerWidth", { value: 1280, configurable: true });
    mockLocalTimezone("Australia/Perth");
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  test("opens schedule screen from sidebar", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: null,
      scheduleStatuses: [],
    });

    try {
      const button = await waitForElement(
        () =>
          Array.from(container.querySelectorAll(".sidebar-nav-button")).find((item) =>
            item.textContent?.includes("Schedule"),
          ) as HTMLButtonElement | null,
        "schedule button",
      );
      button.click();
      await flush();

      await waitForElement(
        () => container.querySelector(".schedule-screen"),
        "schedule screen",
      );
      expect(topbarTitle(container)).toBe("Schedule");
      expect(apiMocks.refreshSchedules).not.toHaveBeenCalled();
    } finally {
      dispose();
    }
  });

  test("schedule screen has no manual refresh button", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.settings.schedule = {
      cron: "0 9 * * *",
      enabled: true,
      timezone: "Australia/Perth",
    };

    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: null,
      scheduleStatuses: [],
    });

    try {
      const scheduleNav = [...container.querySelectorAll(".sidebar-nav-button")].find((item) =>
        item.textContent?.includes("Schedule"),
      ) as HTMLButtonElement;
      scheduleNav.click();
      await flush();

      expect(
        [...container.querySelectorAll("button")].some((button) =>
          button.textContent?.includes("Refresh"),
        ),
      ).toBe(false);
    } finally {
      dispose();
    }
  });

  test("saves workflow schedule from schedule screen", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.settings.schedule = {
      cron: "0 9 * * *",
      enabled: true,
      timezone: "Australia/Perth",
    };
    apiMocks.saveWorkflows.mockResolvedValue(undefined);
    let scheduleHandler: ((statuses: unknown[]) => void) | undefined;
    apiMocks.listenToScheduleStatuses.mockImplementation(async (handler) => {
      scheduleHandler = handler;
      return () => {};
    });
    apiMocks.saveWorkflow.mockImplementation(async (workflow: Workflow) => {
      scheduleHandler?.([
        {
          workflowId: workflow.id,
          workflowName: workflow.name,
          enabled: workflow.settings.schedule?.enabled ?? false,
          cron: workflow.settings.schedule?.cron ?? "",
          timezone: workflow.settings.schedule?.timezone ?? "UTC",
          nextRunAt: "2026-06-16T00:15:00Z",
          lastRunAt: null,
          lastSkippedAt: null,
          lastError: null,
        },
      ]);
      return workflow;
    });

    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: null,
      scheduleStatuses: [],
    });

    try {
      const button = [...container.querySelectorAll("button")].find((item) =>
        item.textContent?.includes("Schedule"),
      ) as HTMLButtonElement;
      button.click();
      await flush();

      expect(container.querySelector('input[placeholder="0 9 * * *"]')).toBeNull();

      const repeatButton = [...container.querySelectorAll(".schedule-frequency-select button")].find(
        (item) => item.textContent?.includes("Repeat"),
      ) as HTMLButtonElement;
      repeatButton.click();

      const intervalInput = container.querySelector(
        ".schedule-interval-field input[type='number']",
      ) as HTMLInputElement;
      intervalInput.value = "15";
      intervalInput.dispatchEvent(new Event("input", { bubbles: true }));

      const saveButton = container.querySelector(
        '.schedule-row button[aria-label="Save schedule"]',
      ) as HTMLButtonElement;
      saveButton.click();
      await flush();

      expect(apiMocks.saveWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "workflow-1",
          settings: expect.objectContaining({
            schedule: expect.objectContaining({
              cron: "*/15 * * * *",
              enabled: true,
            }),
          }),
        }),
      );
      expect(apiMocks.refreshSchedules).not.toHaveBeenCalled();
    } finally {
      dispose();
    }
  });

  test("shows only workflows added to the schedule page", async () => {
    const scheduled = makeWorkflow("workflow-1", "Scheduled Workflow");
    scheduled.settings.schedule = {
      cron: "0 9 * * *",
      enabled: true,
      timezone: "Australia/Perth",
    };
    const unscheduled = makeWorkflow("workflow-2", "Unscheduled Workflow");
    apiMocks.saveWorkflow.mockImplementation(async (workflow: Workflow) => workflow);
    apiMocks.refreshSchedules.mockResolvedValue([]);

    const { container, dispose } = await mountApp({
      workflows: [scheduled, unscheduled],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: null,
      scheduleStatuses: [],
    });

    try {
      const button = [...container.querySelectorAll("button")].find((item) =>
        item.textContent?.includes("Schedule"),
      ) as HTMLButtonElement;
      button.click();
      await flush();

      expect(container.querySelector(".schedule-table")?.textContent).toContain(
        "Scheduled Workflow",
      );
      expect(container.querySelector(".schedule-table")?.textContent).not.toContain(
        "Unscheduled Workflow",
      );

      const addWorkflowButton = container.querySelector(
        ".schedule-toolbar .primary-button.compact",
      ) as HTMLButtonElement;
      addWorkflowButton.click();
      await flush();

      expect(
        container.querySelector('[role="dialog"][aria-label="Add workflow to schedule"]'),
      ).not.toBeNull();

      const addOption = [...container.querySelectorAll(".node-picker-option-title")].find(
        (item) => item.textContent === "Unscheduled Workflow",
      )?.closest("button") as HTMLButtonElement;
      addOption.click();
      await flush();

      expect(apiMocks.saveWorkflow).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "workflow-2",
          settings: expect.objectContaining({
            schedule: expect.objectContaining({
              cron: "0 9 * * *",
              enabled: true,
              timezone: defaultWorkflowSchedule().timezone,
            }),
          }),
        }),
      );
    } finally {
      dispose();
    }
  });
});
