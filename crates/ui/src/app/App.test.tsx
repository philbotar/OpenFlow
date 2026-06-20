// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { AgentDefinition, AppSettings, BootstrapPayload, Project, ProviderReadiness, SkillSummary, Workflow, WorkflowRunState } from "../lib/types";
import { createEmptyToolConfig } from "../lib/workflow";

const apiMocks = vi.hoisted(() => ({
  bootstrapApp: vi.fn(),
  listSkills: vi.fn(),
  listWorkflows: vi.fn(),
  clearRunTrace: vi.fn(),
  createAgentDefinition: vi.fn(),
  createAgentNode: vi.fn(),
  createWorkflow: vi.fn(),
  listenToRunState: vi.fn(),
  resolveProviderReadiness: vi.fn(),
  deleteProviderApiKey: vi.fn(),
  loadProviderApiKey: vi.fn(),
  saveAgents: vi.fn(),
  saveProviderApiKey: vi.fn(),
  saveSettings: vi.fn(),
  saveWorkflows: vi.fn(),
  saveWorkflow: vi.fn(),
  listScheduleStatuses: vi.fn(),
  refreshSchedules: vi.fn(),
  listenToScheduleStatuses: vi.fn(),
  startRun: vi.fn(),
  continueRun: vi.fn(),
  isRunContinuable: vi.fn(),
  submitToolApproval: vi.fn(),
  submitUserInput: vi.fn(),
  validateWorkflow: vi.fn(),
  startTerminal: vi.fn(),
  writeTerminal: vi.fn(),
  resizeTerminal: vi.fn(),
  stopTerminal: vi.fn(),
  listenToTerminalEvent: vi.fn(),
  createProjectFromDirectory: vi.fn(),
  assignWorkflowToProject: vi.fn(),
  copyWorkflowToProject: vi.fn(),
  unassignWorkflowFromProject: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(null),
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
    createAgentNode: apiMocks.createAgentNode,
    createWorkflow: apiMocks.createWorkflow,
    listenToRunState: apiMocks.listenToRunState,
    resolveProviderReadiness: apiMocks.resolveProviderReadiness,
    deleteProviderApiKey: apiMocks.deleteProviderApiKey,
    loadProviderApiKey: apiMocks.loadProviderApiKey,
    saveAgents: apiMocks.saveAgents,
    submitToolApproval: apiMocks.submitToolApproval,
    saveProviderApiKey: apiMocks.saveProviderApiKey,
    saveSettings: apiMocks.saveSettings,
    saveWorkflows: apiMocks.saveWorkflows,
    saveWorkflow: apiMocks.saveWorkflow,
    listScheduleStatuses: apiMocks.listScheduleStatuses,
    refreshSchedules: apiMocks.refreshSchedules,
    listenToScheduleStatuses: apiMocks.listenToScheduleStatuses,
    startRun: apiMocks.startRun,
    continueRun: apiMocks.continueRun,
    isRunContinuable: apiMocks.isRunContinuable,
    submitUserInput: apiMocks.submitUserInput,
    validateWorkflow: apiMocks.validateWorkflow,
    createProjectFromDirectory: apiMocks.createProjectFromDirectory,
    assignWorkflowToProject: apiMocks.assignWorkflowToProject,
    copyWorkflowToProject: apiMocks.copyWorkflowToProject,
    unassignWorkflowFromProject: apiMocks.unassignWorkflowFromProject,
    startTerminal: apiMocks.startTerminal,
    writeTerminal: apiMocks.writeTerminal,
    resizeTerminal: apiMocks.resizeTerminal,
    stopTerminal: apiMocks.stopTerminal,
    listenToTerminalEvent: apiMocks.listenToTerminalEvent,
  };
});

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {}),
  }),
}));

vi.mock("../canvas/WorkflowCanvasHost", () => ({
  default: (props: { onAddNode: () => void }) => (
    <button aria-label="Canvas add node" onClick={() => props.onAddNode()}>
      Canvas add node
    </button>
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

const SETTINGS: AppSettings = {
  active_provider: "openai",
  providers: {
    openai: {
      display_name: "OpenAI",
      base_url: "https://api.openai.com/v1",
      transport: "responses",
      responses_path: "responses",
      chat_completions_path: "chat/completions",
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
  apiMocks.isRunContinuable.mockResolvedValue(false);
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
  apiMocks.createWorkflow.mockImplementation(async (name: string) => makeWorkflow("created-workflow", name));
  apiMocks.createAgentDefinition.mockImplementation(async (name: string) => makeAgent("created-agent", name));
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
  apiMocks.saveWorkflow.mockImplementation(async (workflow) => workflow);
  apiMocks.refreshSchedules.mockResolvedValue([]);
  apiMocks.listScheduleStatuses.mockResolvedValue([]);
  apiMocks.listenToScheduleStatuses.mockResolvedValue(() => {});
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

function makeBootstrapPayload(
  workflows: Workflow[],
  agents: AgentDefinition[] = [makeAgent("agent-1", "Research Agent")],
  skills: SkillSummary[] = FIXTURE_SKILLS,
  projects: Project[] = [],
): BootstrapPayload {
  return {
    workflows,
    agents,
    projects,
    skills,
    settings: SETTINGS,
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
  const title = container.querySelector(".topbar-copy h2");
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

async function startWorkflowRename(container: HTMLElement, name: string) {
  const renameButton = await waitForElement(
    () => container.querySelector(`[aria-label="Rename ${name}"]`),
    `rename button for ${name}`,
  );
  (renameButton as HTMLButtonElement).click();
  await flush();
  return waitForElement(
    () => container.querySelector(`input[aria-label="Workflow name for ${name}"]`),
    `workflow rename input for ${name}`,
  ) as Promise<HTMLInputElement>;
}

describe("App workflow rename", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
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
      expect(labels).toEqual(["Workflows", "Projects"]);
      expect(workflowTitles(container)).toEqual(["Independent Flow"]);
      expect(container.querySelector(".project-folder-title")?.textContent).toBe("Syntech");
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
        '[aria-label="Add workflow to Project B"]',
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

      const modelInput = await waitForElement(
        () => Array.from(container.querySelectorAll("label span")).find((element) => element.textContent === "Model")?.parentElement?.querySelector("input") as HTMLInputElement | null,
        "agent model input",
      );
      expect(modelInput.value).toBe("gpt-4.1-mini");

      const autoStartInput = Array.from(container.querySelectorAll("label.checkbox-row input")).find(
        (element) => (element.parentElement?.textContent ?? "").includes("Request user input"),
      ) as HTMLInputElement | undefined;
      expect(autoStartInput?.checked).toBe(false);

      const systemPromptInput = Array.from(container.querySelectorAll("label span")).find(
        (element) => element.textContent === "System prompt",
      )?.parentElement?.querySelector("textarea") as HTMLTextAreaElement | null;
      expect(systemPromptInput?.value).toBe("You are a focused AI agent in a node workflow.");

      const taskPromptInput = Array.from(container.querySelectorAll("label span")).find(
        (element) => element.textContent === "Task prompt",
      )?.parentElement?.querySelector("textarea") as HTMLTextAreaElement | null;
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
      expect(container.querySelector(".panel-header-title-row h3")?.textContent).toBe("Writer Agent");
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

      const approvalSelect = container.querySelector(
        ".tool-config-body select.text-input",
      ) as HTMLSelectElement | null;
      expect(approvalSelect).not.toBeNull();
      expect(approvalSelect?.value).toBe("write");
      expect(Array.from(approvalSelect?.options ?? []).map((option) => option.value)).toEqual([
        "read_only",
        "write",
        "always_ask",
        "yolo",
      ]);

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

describe("App settings persistence", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });

  beforeEach(() => {
    installDefaultApiMocks();
  });

  test("renders full-page settings without sidebar or topbar", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);

      expect(container.querySelector(".sidebar")).toBeNull();
      expect(container.querySelector(".topbar")).toBeNull();
      expect(container.querySelector(".settings-shell")).not.toBeNull();
      expect(container.querySelector(".settings-nav")).not.toBeNull();
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

  test("loads and saves provider API keys per provider", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      await openSettingsScreen(container);

      settingsNavButton(container, "Authentication").click();
      await flush();

      const apiKeyInput = await waitForElement(
        () => container.querySelector('input[type="password"]'),
        "provider api key input",
      ) as HTMLInputElement;
      expect(apiKeyInput.value).toBe("stored-openai-key");

      settingsNavButton(container, "Provider").click();
      await flush();

      const providerSelect = Array.from(container.querySelectorAll("select")).find(
        (element) => Array.from(element.options).some((option) => option.value === "custom_openai_compatible"),
      ) as HTMLSelectElement | undefined;
      expect(providerSelect).toBeDefined();
      providerSelect!.value = "custom_openai_compatible";
      providerSelect!.dispatchEvent(new Event("change", { bubbles: true }));
      await flush();

      settingsNavButton(container, "Authentication").click();
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

      settingsNavButton(container, "Models").click();
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

  test("toggles chat focus mode from the chat dock", async () => {
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
        () => container.querySelector('[aria-label="Focus chat"]') as HTMLButtonElement | null,
        "focus chat button",
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
      expect(thinkingLine?.querySelector(".tool-line-name")?.textContent).toContain("thinking");
      expect(thinkingLine?.querySelector(".tool-line-target")?.textContent).toContain(
        "Agent prompt: You are a focused AI agent...",
      );
    } finally {
      dispose();
    }
  });

  test("renders compact tool line with invocation target in chat", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    workflow.nodes[0].label = "Idea";
    const runState = makeAwaitingRunState(workflow);
    runState.chatLogs[workflow.nodes[0].id] = [
      {
        role: "Thinking",
        content: "Tool request: read\nArguments:\n{\n  \"path\": \"README.md\"\n}",
      },
      {
        role: "Thinking",
        content: "Tool result: read\n¶README.md\n1:# OpenFlow",
      },
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
      const line = container.querySelector(".tool-line");
      expect(line).not.toBeNull();
      expect(line?.getAttribute("data-tool-name")).toBe("read");
      expect(line?.querySelector(".tool-line-name")?.textContent).toContain("Read File");
      expect(line?.querySelector(".tool-line-target")?.textContent).toBe("README.md");
      expect(line?.querySelector(".tool-line-output")).toBeNull();
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
      const branchCChip = [...container.querySelectorAll(".chat-filter-chip")].find((chip) =>
        chip.textContent?.includes("Branch C"),
      );
      expect(branchCChip).not.toBeUndefined();
      branchCChip!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flush();

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
      expect(container.querySelector(".overview-layout")).not.toBeNull();

      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 740, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("52px");
      expect(container.querySelector(".overview-layout")).toBeNull();

      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 460, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("192px");
      expect(container.querySelector(".overview-layout")).not.toBeNull();
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

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("188px");
      resizeZone.dispatchEvent(new MouseEvent("pointerdown", { clientY: 600, button: 0, bubbles: true }));
      window.dispatchEvent(new MouseEvent("pointermove", { clientY: 520, bubbles: true }));
      await flush();
      window.dispatchEvent(new MouseEvent("pointerup", { bubbles: true }));

      expect(editorScreen.style.getPropertyValue("--dock-height")).toBe("268px");
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
      expect(textarea?.placeholder).toContain("start the workflow");
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

  test("header run button still starts without entrypoint", async () => {
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    apiMocks.startRun.mockResolvedValue(makeAwaitingRunState(workflow));
    const { container, dispose } = await mountApp({
      workflows: [workflow],
      agents: [makeAgent("agent-1", "Research Agent")],
      skills: FIXTURE_SKILLS,
      settings: SETTINGS,
      runState: null,
    });
    try {
      const runButton = container.querySelector(
        'button[aria-label="Run workflow"]',
      ) as HTMLButtonElement;
      runButton.click();
      await flush();
      expect(apiMocks.startRun).toHaveBeenCalledWith(
        expect.objectContaining({ id: "workflow-1" }),
        expect.objectContaining({ active_provider: "openai" }),
        null,
        "stored-openai-key",
        null,
      );
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

describe("App schedule screen", () => {
  beforeEach(() => {
    installDefaultApiMocks();
    Object.defineProperty(window, "innerWidth", { value: 1280, configurable: true });
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
        '.schedule-row button[title="Save schedule"]',
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
        ".schedule-add-button",
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
              timezone: "Australia/Perth",
            }),
          }),
        }),
      );
    } finally {
      dispose();
    }
  });
});
