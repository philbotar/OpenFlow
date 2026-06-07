// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { AgentDefinition, AppSettings, BootstrapPayload, ProviderReadiness, SkillSummary, Workflow, WorkflowRunState } from "../lib/types";
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
  startRun: vi.fn(),
  submitToolApproval: vi.fn(),
  submitUserInput: vi.fn(),
  validateWorkflow: vi.fn(),
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
    startRun: apiMocks.startRun,
    submitUserInput: apiMocks.submitUserInput,
    validateWorkflow: apiMocks.validateWorkflow,
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
      key_ref: "provider:openai:api-key",
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
      key_ref: "provider:custom_openai_compatible:api-key",
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
        },
      },
    ],
    edges: [],
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
        }
      : {
          system_prompt: "",
          task_prompt: "",
          model: "",
          output_schema: { type: "object" },
          auto_start: false,
          tools: createEmptyToolConfig(),
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
}

function makeBootstrapPayload(
  workflows: Workflow[],
  agents: AgentDefinition[] = [makeAgent("agent-1", "Research Agent")],
  skills: SkillSummary[] = FIXTURE_SKILLS,
): BootstrapPayload {
  return {
    workflows,
    agents,
    skills,
    settings: SETTINGS,
    runState: null,
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
  };
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

  test("renders the macOS titlebar spacer inside the topbar", async () => {
    const restoreUserAgent = setUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)");
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const topbar = await waitForElement(() => container.querySelector(".topbar"), "topbar");
      expect(topbar.classList.contains("topbar-macos")).toBe(true);
      expect(topbar.querySelector(".topbar-window-controls-spacer")).not.toBeNull();
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
      expect(container.querySelector(".agent-list-row-title")?.textContent).toBe("Research Agent");
      expect((container.querySelector(".agent-list-row.active") as HTMLElement | null)).not.toBeNull();
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
        () => container.querySelector('button[aria-label="New agent"]') as HTMLButtonElement | null,
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
        (element) => (element.parentElement?.textContent ?? "").includes("Auto-start without pausing for human input"),
      ) as HTMLInputElement | undefined;
      expect(autoStartInput?.checked).toBe(true);

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
  test("shows a visible success toast after validation", async () => {
    apiMocks.validateWorkflow.mockResolvedValue({ layerCount: 1 });
    const workflow = makeWorkflow("workflow-1", "Workflow One");
    const { container, dispose } = await mountApp(makeBootstrapPayload([workflow]));

    try {
      const validateButton = await waitForElement(
        () => container.querySelector('button[aria-label="Validate workflow"]') as HTMLButtonElement | null,
        "validate workflow button",
      );
      validateButton.click();
      await flush();

      expect(apiMocks.validateWorkflow).toHaveBeenCalledWith(expect.objectContaining({ id: "workflow-1" }));

      const successToast = await waitForElement(
        () =>
          document.body.querySelector(
            '[data-sonner-toast][data-mounted="true"][data-visible="true"][data-type="success"] [data-title]',
          ) as HTMLElement | null,
        "validation success toast",
      );
      expect(successToast.textContent).toContain("Valid DAG · 1 layer");
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

      const showToolsButton = await waitForElement(
        () =>
          Array.from(container.querySelectorAll("button")).find(
            (element) => element.textContent === "Show tools",
          ) as HTMLButtonElement | null,
        "show tools button",
      );
      showToolsButton.click();
      await flush();

      expect(
        Array.from(container.querySelectorAll(".tool-config-option-title")).map((element) => element.textContent),
      ).toEqual(["read", "search", "find", "ast_grep"]);

      const checkboxes = Array.from(
        container.querySelectorAll('.tool-config-option input[type="checkbox"]'),
      ) as HTMLInputElement[];
      expect(checkboxes.every((element) => element.checked)).toBe(true);

      const hideToolsButton = Array.from(container.querySelectorAll("button")).find(
        (element) => element.textContent === "Hide tools",
      ) as HTMLButtonElement | undefined;
      hideToolsButton?.click();
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
      expect(savedWorkflows?.[0]?.nodes[0]?.agent.tools.catalog.tools).toEqual([
        { name: "read" },
        { name: "search" },
        { name: "find" },
        { name: "ast_grep" },
      ]);
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

  test("loads and saves provider API keys per provider", async () => {
    const { container, dispose } = await mountApp(
      makeBootstrapPayload([makeWorkflow("workflow-1", "Workflow One")]),
    );

    try {
      const settingsButton = await waitForElement(
        () => Array.from(container.querySelectorAll(".sidebar-nav-button")).find((element) => element.textContent?.includes("Settings")) as HTMLButtonElement | null,
        "settings button",
      );
      settingsButton.click();
      await flush();

      const apiKeyInput = await waitForElement(
        () => container.querySelector('input[type="password"]'),
        "provider api key input",
      ) as HTMLInputElement;
      expect(apiKeyInput.value).toBe("stored-openai-key");

      const providerSelect = Array.from(container.querySelectorAll("select")).find(
        (element) => Array.from(element.options).some((option) => option.value === "custom_openai_compatible"),
      ) as HTMLSelectElement | undefined;
      expect(providerSelect).toBeDefined();
      providerSelect!.value = "custom_openai_compatible";
      providerSelect!.dispatchEvent(new Event("change", { bubbles: true }));
      await flush();

      await waitForElement(
        () => (apiKeyInput.value === "stored-compatible-key" ? apiKeyInput : null),
        "compatible provider api key",
      );

      apiKeyInput.value = "updated-compatible-key";
      apiKeyInput.dispatchEvent(new Event("input", { bubbles: true }));
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
              key_ref: "provider:custom_openai_compatible:api-key",
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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();
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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();

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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();

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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();

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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();

    try {
      expect(container.querySelector(".composer-settings-button")).toBeNull();
      expect(container.querySelector(".composer-status-pill")).toBeNull();
      expect(container.querySelector('[aria-label="Send to paused node"]')).not.toBeNull();
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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();

    try {
      const labels = Array.from(container.querySelectorAll(".chat-role")).map((element) => element.textContent);
      expect(labels).toEqual(["System", "Agent 2", "Agent 2"]);
    } finally {
      dispose();
    }
  });

  test("renders tool request and tool result details in chat", async () => {
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
    const chatTab = await waitForElement(
      () => Array.from(container.querySelectorAll(".dock-tab-switcher button")).find((btn) => btn.textContent === "Chat") as HTMLButtonElement | null,
      "chat tab",
    );
    chatTab.click();
    await flush();

    try {
      const bubble = container.querySelector(".tool-bubble");
      expect(bubble).not.toBeNull();
      expect(bubble?.querySelector(".tool-bubble-header")?.textContent).toBe(
        "Tool Invocation: read",
      );
      expect(bubble?.querySelector(".tool-bubble-output")?.textContent).toContain(
        "¶README.md",
      );
    } finally {
      dispose();
    }
  });
});

describe("App bottom dock", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.clearAllMocks();
    window.localStorage.clear();
  });
  beforeEach(() => {
    installDefaultApiMocks();
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
