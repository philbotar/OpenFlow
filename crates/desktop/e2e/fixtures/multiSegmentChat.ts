/** Static 3-node settled run for chat segment visual regression. */

const WORKFLOW_ID = "pipeline-1";
const NODE_IDS = ["node-arch", "node-test", "node-impl"] as const;
const NODE_LABELS = ["Architecture Design", "Test Plan", "Implement Code"] as const;

const AGENT = {
  system_prompt: "You are a focused agent.",
  task_prompt: "Complete your step.",
  model: "gpt-4.1-mini",
  output_schema: { type: "object" },
  auto_start: true,
  tools: { approvalMode: "write" },
  callable_agents: [],
  allow_all_callable_agents: false,
};

const EMPTY_SETTINGS = {
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

const workflow = {
  id: WORKFLOW_ID,
  name: "Feature-to-Implementation Pipeline",
  nodes: NODE_IDS.map((id, index) => ({
    id,
    label: NODE_LABELS[index],
    kind: "Agent" as const,
    position: { x: 120, y: 140 + index * 120 },
    agent: AGENT,
  })),
  edges: [
    { id: "edge-1", from: NODE_IDS[0], to: NODE_IDS[1] },
    { id: "edge-2", from: NODE_IDS[1], to: NODE_IDS[2] },
  ],
  settings: { shared_context: "" },
};

function thinkingLine(text: string) {
  return { role: "Thinking", content: text };
}

function writeToolLine(path: string) {
  return {
    role: "Thinking",
    content: `Tool request: write\nArguments:\n{\n  "path": "${path}"\n}`,
  };
}

const runState = {
  active: false,
  awaitingNodeId: null,
  activeManualNodeId: null,
  activeToolCallId: null,
  pendingApprovals: [],
  toolCallsByNode: {},
  toolArtifacts: {},
  execApprovalGranted: false,
  statusByNode: Object.fromEntries(NODE_IDS.map((id) => [id, "completed"])),
  subagentsByNode: {},
  lastReport: null,
  lastError: null,
  chatLogs: {
    [NODE_IDS[0]]: [
      thinkingLine("Let me analyze the requirements and outline the architecture."),
      writeToolLine("docs/architecture.md"),
    ],
    [NODE_IDS[1]]: [
      thinkingLine("I'll draft a test plan covering unit and integration cases."),
      writeToolLine("docs/test-plan.md"),
    ],
    [NODE_IDS[2]]: [
      thinkingLine("Implementing the feature with file writes."),
      writeToolLine("package.json"),
      writeToolLine(".env"),
    ],
  },
  runTrace: [],
  outputs: {},
  changedFiles: [],
  changedFilesByNode: {},
  editBatches: [],
};

export const MULTI_SEGMENT_BOOTSTRAP = {
  workflows: [workflow],
  agents: [
    {
      id: "agent-1",
      name: "Pipeline Agent",
      system_prompt: AGENT.system_prompt,
      task_prompt: AGENT.task_prompt,
      model: AGENT.model,
      output_schema: AGENT.output_schema,
      auto_start: true,
      tools: AGENT.tools,
    },
  ],
  projects: [],
  skills: [],
  settings: EMPTY_SETTINGS,
  runState,
  runContinuable: false,
  scheduleStatuses: [],
};
