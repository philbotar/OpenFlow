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
      reasoning_effort_options: [
        { value: "fast", label: "Fast", uses_budget_tokens: false },
        { value: "low", label: "Low", uses_budget_tokens: false },
        { value: "medium", label: "Medium", uses_budget_tokens: false },
        { value: "high", label: "High", uses_budget_tokens: false },
      ],
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

function writeToolMarker(toolCallId: string) {
  return {
    role: "Thinking",
    content: "",
    toolCallId,
  };
}

function writeToolSummary(toolCallId: string, path: string, intent: string) {
  return {
    toolCallId,
    toolName: "write",
    status: "completed",
    arguments: { path },
    intent,
    lastOutput: `Wrote ${path}`,
    isError: false,
    streaming: false,
  };
}

const TOOL_CALLS = {
  [NODE_IDS[0]]: [
    writeToolSummary("write-architecture", "docs/architecture.md", "architecture doc"),
  ],
  [NODE_IDS[1]]: [
    writeToolSummary("write-test-plan", "docs/test-plan.md", "test plan"),
  ],
  [NODE_IDS[2]]: [
    writeToolSummary("write-package", "package.json", "package config"),
    writeToolSummary("write-env", ".env", "environment config"),
  ],
};

const runState = {
  active: false,
  awaitingNodeId: null,
  activeManualNodeId: null,
  activeToolCallId: null,
  pendingApprovals: [],
  toolCallsByNode: TOOL_CALLS,
  toolArtifacts: {},
  execApprovalGranted: false,
  statusByNode: Object.fromEntries(NODE_IDS.map((id) => [id, "completed"])),
  subagentsByNode: {},
  lastReport: null,
  lastError: null,
  chatLogs: {
    [NODE_IDS[0]]: [
      thinkingLine("Let me analyze the requirements and outline the architecture."),
      writeToolMarker(TOOL_CALLS[NODE_IDS[0]][0].toolCallId),
    ],
    [NODE_IDS[1]]: [
      thinkingLine("I'll draft a test plan covering unit and integration cases."),
      writeToolMarker(TOOL_CALLS[NODE_IDS[1]][0].toolCallId),
    ],
    [NODE_IDS[2]]: [
      thinkingLine("Implementing the feature with file writes."),
      writeToolMarker(TOOL_CALLS[NODE_IDS[2]][0].toolCallId),
      writeToolMarker(TOOL_CALLS[NODE_IDS[2]][1].toolCallId),
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
  discoveredMcp: [],
  settings: EMPTY_SETTINGS,
  runState,
  runContinuable: false,
  scheduleStatuses: [],
};
