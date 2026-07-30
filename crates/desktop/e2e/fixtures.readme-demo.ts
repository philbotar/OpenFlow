import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createTauriTest } from "@srsholmes/tauri-playwright";
import { createOpenflowIpcMocks, EMPTY_BOOTSTRAP } from "./ipcMocks.js";

const e2eRoot = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(e2eRoot, "../../..");
const sourceWorkflow = JSON.parse(
  readFileSync(join(repoRoot, "examples/feature_plan.workflow.json"), "utf8"),
);
const demoPositions: Record<string, { x: number; y: number }> = {
  idea: { x: 96, y: 146 },
  plan: { x: 476, y: 96 },
  risk: { x: 476, y: 196 },
  brief: { x: 856, y: 146 },
};
const workflow = {
  ...sourceWorkflow,
  nodes: sourceWorkflow.nodes.map(
    (node: {
      id: string;
      position: { x: number; y: number };
      agent: Record<string, unknown>;
    }) => ({
      ...node,
      position: demoPositions[node.id] ?? node.position,
      agent: {
        auto_start: true,
        tools: { approvalMode: "write" },
        callable_agents: [],
        allow_all_callable_agents: false,
        ...node.agent,
      },
    }),
  ),
  settings: sourceWorkflow.settings ?? { shared_context: "" },
};

export const README_DEMO_PROMPT =
  "Plan undo and redo for a visual workflow editor.";

const NODE_IDS = ["idea", "plan", "risk", "brief"] as const;

function markdownMessage(content: string) {
  return { role: "Assistant", content };
}

function thinkingMessage(content: string) {
  return { role: "Thinking", content, streaming: true };
}

function state(
  statusByNode: Record<(typeof NODE_IDS)[number], string>,
  chatLogs: Record<(typeof NODE_IDS)[number], unknown[]>,
  runTrace: unknown[],
  outputs: Record<string, unknown> = {},
  active = true,
) {
  return {
    runId: "readme-demo-run",
    active,
    awaitingNodeId: null,
    awaitingNodeIds: [],
    activeManualNodeId: null,
    activeToolCallId: null,
    pendingApprovals: [],
    toolCallsByNode: {},
    toolArtifacts: {},
    execApprovalGranted: false,
    statusByNode,
    subagentsByNode: {},
    lastReport: active
      ? null
      : {
          workflow_id: workflow.id,
          outputs: NODE_IDS.map((nodeId) => ({
            node_id: nodeId,
            output: outputs[nodeId],
          })),
        },
    lastError: null,
    chatLogs,
    runTrace,
    outputs,
    changedFiles: [],
    changedFilesByNode: {},
    editBatches: [],
  };
}

const userMessage = { role: "User", content: README_DEMO_PROMPT };
const clarifiedIdea = {
  target_user: "People editing visual workflows",
  pain: "Graph changes are hard to reverse safely",
  outcome: "Fast undo and redo with predictable state",
};
const implementationPlan = {
  slices: [
    "Model reversible graph commands",
    "Add bounded undo and redo stacks",
    "Wire shortcuts and disabled states",
  ],
};
const deliveryRisks = {
  risks: [
    "Coalescing drag events incorrectly",
    "Restoring stale node selections",
    "Persisting history across workflows",
  ],
};
const finalBrief = {
  brief:
    "Add command-based history around graph mutations, scoped per workflow.",
  next_action: "Ship node movement undo first, then cover edges and deletion.",
};
const clarifiedIdeaMarkdown = `## Product goal

Build reliable **undo and redo** for people editing visual workflows.

- **Problem:** Graph changes are hard to reverse safely.
- **Outcome:** Fast recovery with predictable state.`;
const implementationPlanMarkdown = `## Implementation plan

1. Model graph changes as reversible commands.
2. Add bounded undo and redo stacks.
3. Wire shortcuts and disabled states.`;
const deliveryRisksMarkdown = `## Delivery risks

- Coalescing drag events incorrectly
- Restoring stale node selections
- Persisting history across workflows`;
const finalBriefMarkdown = `## Recommended approach

Use **command-based history** around graph mutations, scoped per workflow.

### Next step

Ship node-movement undo first, then cover edges and deletion.`;

export const README_DEMO_STATES = {
  clarify: state(
    { idea: "started", plan: "idle", risk: "idle", brief: "idle" },
    {
      idea: [
        userMessage,
        thinkingMessage("Clarifying the user, pain, and desired outcome…"),
      ],
      plan: [],
      risk: [],
      brief: [],
    },
    [
      {
        nodeId: "idea",
        nodeLabel: "Clarify idea",
        status: "running",
        message: "Clarifying the product idea",
        output: null,
      },
    ],
  ),
  parallel: state(
    { idea: "completed", plan: "started", risk: "started", brief: "idle" },
    {
      idea: [userMessage, markdownMessage(clarifiedIdeaMarkdown)],
      plan: [thinkingMessage("Building the smallest implementation slices…")],
      risk: [thinkingMessage("Checking state, persistence, and interaction risks…")],
      brief: [],
    },
    [
      {
        nodeId: "idea",
        nodeLabel: "Clarify idea",
        status: "completed",
        message: "Clarified the product idea",
        output: clarifiedIdea,
      },
      {
        nodeId: "plan",
        nodeLabel: "Create plan",
        status: "running",
        message: "Creating implementation plan",
        output: null,
      },
      {
        nodeId: "risk",
        nodeLabel: "Find risks",
        status: "running",
        message: "Finding delivery risks",
        output: null,
      },
    ],
    { idea: clarifiedIdea },
  ),
  final: state(
    { idea: "completed", plan: "completed", risk: "completed", brief: "started" },
    {
      idea: [userMessage, markdownMessage(clarifiedIdeaMarkdown)],
      plan: [markdownMessage(implementationPlanMarkdown)],
      risk: [markdownMessage(deliveryRisksMarkdown)],
      brief: [thinkingMessage("Combining the plan and risks into one brief…")],
    },
    [
      {
        nodeId: "idea",
        nodeLabel: "Clarify idea",
        status: "completed",
        message: "Clarified the product idea",
        output: clarifiedIdea,
      },
      {
        nodeId: "plan",
        nodeLabel: "Create plan",
        status: "completed",
        message: "Created implementation plan",
        output: implementationPlan,
      },
      {
        nodeId: "risk",
        nodeLabel: "Find risks",
        status: "completed",
        message: "Found delivery risks",
        output: deliveryRisks,
      },
      {
        nodeId: "brief",
        nodeLabel: "Final brief",
        status: "running",
        message: "Synthesizing final brief",
        output: null,
      },
    ],
    {
      idea: clarifiedIdea,
      plan: implementationPlan,
      risk: deliveryRisks,
    },
  ),
  complete: state(
    {
      idea: "completed",
      plan: "completed",
      risk: "completed",
      brief: "completed",
    },
    {
      idea: [userMessage, markdownMessage(clarifiedIdeaMarkdown)],
      plan: [markdownMessage(implementationPlanMarkdown)],
      risk: [markdownMessage(deliveryRisksMarkdown)],
      brief: [markdownMessage(finalBriefMarkdown)],
    },
    [
      {
        nodeId: "idea",
        nodeLabel: "Clarify idea",
        status: "completed",
        message: "Clarified the product idea",
        output: clarifiedIdea,
      },
      {
        nodeId: "plan",
        nodeLabel: "Create plan",
        status: "completed",
        message: "Created implementation plan",
        output: implementationPlan,
      },
      {
        nodeId: "risk",
        nodeLabel: "Find risks",
        status: "completed",
        message: "Found delivery risks",
        output: deliveryRisks,
      },
      {
        nodeId: "brief",
        nodeLabel: "Final brief",
        status: "completed",
        message: "Completed final brief",
        output: finalBrief,
      },
    ],
    {
      idea: clarifiedIdea,
      plan: implementationPlan,
      risk: deliveryRisks,
      brief: finalBrief,
    },
    false,
  ),
};

const bootstrap = {
  ...EMPTY_BOOTSTRAP,
  workflows: [workflow],
  runState: null,
};

function ipcBody(body: string): (args?: Record<string, unknown>) => unknown {
  return new Function("args", body) as (
    args?: Record<string, unknown>,
  ) => unknown;
}

const startHandshakeStateJson = JSON.stringify({
  ...README_DEMO_STATES.clarify,
  statusByNode: {
    idea: "started",
    plan: "queued",
    risk: "queued",
    brief: "queued",
  },
});
const readmeDemoMocks = {
  start_run: ipcBody(`
    window.__openflowReadmeDemoRunState = ${startHandshakeStateJson};
    return window.__openflowReadmeDemoRunState;
  `),
  get_run_state: ipcBody(`
    return window.__openflowReadmeDemoRunState || null;
  `),
};

const uiRoot = join(e2eRoot, "../../ui");

export const { test, expect } = createTauriTest({
  devUrl: "http://localhost:1420",
  ipcMocks: {
    ...createOpenflowIpcMocks(bootstrap),
    ...readmeDemoMocks,
  },
  mcpSocket: "/tmp/openflow-playwright-readme-demo.sock",
  tauriCommand: "npm run tauri -- dev",
  tauriCwd: uiRoot,
  tauriFeatures: ["e2e-testing"],
});
