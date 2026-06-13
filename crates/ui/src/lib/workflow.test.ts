import { describe, expect, test } from "vitest";
import type {
  AgentStatus,
  AppSettings,
  SubagentStatus,
  SubagentSummary,
  Workflow,
  WorkflowRunState,
} from "./types";
import {
  cloneSettings,
  cloneWorkflow,
  createEmptyToolConfig,
  canSendChat,
  executionLayers,
  isChatComposerBusy,
  pendingApprovalForNode,
  nodeChangedFiles,
  nodeEditBatches,
  normalizeRunState,
  nodeRunAppearanceOrder,
  projectChatLayout,
  sortTranscriptSegmentsByNodeOrder,
  projectWorkflowCanvasGraph,
  projectWorkflowCanvasStatusByNode,
  projectWorkflowCanvasSubagentsByNode,
} from "./workflow";

const workflow: Workflow = {
  id: "workflow-1",
  name: "Smoke workflow",
  nodes: [
    {
      id: "node-1",
      label: "Plan",
      kind: "Agent",
      position: { x: 96, y: 96 },
      agent: {
        system_prompt: "system",
        task_prompt: "task",
        model: "gpt-4o-mini",
        output_schema: { type: "object", properties: { title: { type: "string" } } },
        auto_start: true,
        tools: createEmptyToolConfig(),
        callable_agents: [],
        allow_all_callable_agents: false,
      },
    },
    {
      id: "node-2",
      label: "Draft",
      kind: "Agent",
      position: { x: 496, y: 96 },
      agent: {
        system_prompt: "system-2",
        task_prompt: "task-2",
        model: "gpt-4o-mini",
        output_schema: { type: "object" },
        auto_start: false,
        tools: createEmptyToolConfig(),
        callable_agents: [],
        allow_all_callable_agents: false,
      },
    },
  ],
  edges: [{ id: "edge-1", from: "node-1", to: "node-2" }],
  settings: {
    shared_context: "Use concise bullet summaries.",
  },
};

const settings: AppSettings = {
  active_provider: "openai",
  providers: {
    openai: {
      display_name: "OpenAI",
      base_url: "https://api.openai.com",
      transport: "responses",
      responses_path: "v1/responses",
      chat_completions_path: "v1/chat/completions",
      known_models: ["gpt-4o-mini"],
      default_model: "gpt-4o-mini",
      editable: false,
    },
    custom_openai_compatible: {
      display_name: "Compatible",
      base_url: "http://localhost:11434",
      transport: "chat_completions",
      responses_path: "v1/responses",
      chat_completions_path: "v1/chat/completions",
      known_models: ["llama3.1"],
      default_model: "llama3.1",
      editable: true,
    },
  },
};

const runState: WorkflowRunState = {
  active: true,
  awaitingNodeId: "node-2",
  activeManualNodeId: null,
  activeToolCallId: null,
  pendingApprovals: [],
  toolCallsByNode: {},
  toolArtifacts: {},
  execApprovalGranted: false,
  statusByNode: {
    "node-1": "completed",
    "node-2": "awaiting_input",
  },
  subagentsByNode: {},
  lastReport: null,
  lastError: null,
  chatLogs: {
    "node-1": [],
    "node-2": [],
  },
  runTrace: [
    {
      nodeId: "node-1",
      nodeLabel: "Plan",
      status: "completed",
      message: "done",
      output: null,
    },
  ],
  outputs: {},
  changedFiles: [],
  changedFilesByNode: {},
  editBatches: [],
};

describe("workflow helpers", () => {
  test("cloneWorkflow fills default retry policy when missing", () => {
    const cloned = cloneWorkflow(workflow);

    expect(cloned.settings.retry_policy).toEqual({
      max_attempts: 3,
      backoff_ms: 1_000,
    });
  });

  test("cloneWorkflow detaches nested workflow state", () => {
    const cloned = cloneWorkflow(workflow);

    cloned.nodes[0].position.x = 320;
    cloned.nodes[0].agent.model = "o3";
    (cloned.nodes[0].agent.output_schema as { properties: { title: { type: string } } }).properties.title.type = "number";
    cloned.edges[0].to = "node-9";
    cloned.settings.shared_context = "changed";

    expect(workflow.nodes[0].position.x).toBe(96);
    expect(workflow.settings.shared_context).toBe("Use concise bullet summaries.");
    expect(workflow.nodes[0].agent.model).toBe("gpt-4o-mini");
    expect((workflow.nodes[0].agent.output_schema as { properties: { title: { type: string } } }).properties.title.type).toBe(
      "string",
    );
    expect(workflow.edges[0].to).toBe("node-2");
  });

  test("cloneSettings detaches provider fields", () => {
    const cloned = cloneSettings(settings);

    cloned.providers.openai.known_models.push("o3");
    cloned.providers.openai.base_url = "https://changed.example";

    expect(settings.providers.openai.base_url).toBe("https://api.openai.com");
    expect(settings.providers.openai.known_models).toEqual(["gpt-4o-mini"]);
  });

  test("projectWorkflowCanvasGraph reuses the previous graph when only agent config changes", () => {
    const previous = projectWorkflowCanvasGraph(workflow);
    const edited = cloneWorkflow(workflow);
    edited.nodes[0].agent.system_prompt = "new system prompt";
    edited.nodes[1].agent.model = "gpt-4.1";

    const next = projectWorkflowCanvasGraph(edited, previous);

    expect(next).toEqual(previous);
  });

  test("projectWorkflowCanvasGraph emits a new graph when canvas-visible fields change", () => {
    const previous = projectWorkflowCanvasGraph(workflow);
    const edited = cloneWorkflow(workflow);
    edited.nodes[0].label = "Plan v2";

    const next = projectWorkflowCanvasGraph(edited, previous);

    expect(next?.nodes[0].label).toBe("Plan v2");
  });

  test("projectWorkflowCanvasStatusByNode reuses the previous snapshot when statuses are unchanged", () => {
    const previous = projectWorkflowCanvasStatusByNode(runState);
    const updated = {
      ...runState,
      runTrace: [
        ...runState.runTrace,
        {
          nodeId: "node-2",
          nodeLabel: "Draft",
          status: "paused",
          message: "waiting",
          output: null,
        },
      ],
      chatLogs: {
        ...runState.chatLogs,
        "node-2": [{ role: "User", content: "continue" }],
      },
    } satisfies WorkflowRunState;

    const next = projectWorkflowCanvasStatusByNode(updated, previous);

    expect(next).toEqual(previous);
  });

  test("canSendChat ignores approvals on other nodes", () => {
    const multiplexState: WorkflowRunState = {
      ...runState,
      awaitingNodeIds: ["node-2"],
      pendingApprovals: [
        {
          approvalId: "approval-1",
          nodeId: "node-1",
          nodeLabel: "Plan",
          toolCall: {
            id: "call-1",
            name: "write",
            arguments: { path: "out.txt", content: "hi" },
          },
          tier: "write",
        },
      ],
    };

    expect(pendingApprovalForNode(multiplexState, "node-2")).toBeUndefined();
    expect(canSendChat(multiplexState, "node-2", true, "continue")).toBe(true);
    expect(canSendChat(multiplexState, "node-1", true, "continue")).toBe(false);
  });

  test("isChatComposerBusy only returns true while the selected node is started or running a tool", () => {
    expect(isChatComposerBusy(runState, "node-2")).toBe(false);
    expect(
      isChatComposerBusy(
        {
          ...runState,
          statusByNode: {
            ...runState.statusByNode,
            "node-2": "started",
          },
        },
        "node-2",
      ),
    ).toBe(true);
    expect(
      isChatComposerBusy(
        {
          ...runState,
          statusByNode: {
            ...runState.statusByNode,
            "node-2": "running_tool",
          },
        },
        "node-2",
      ),
    ).toBe(true);
    expect(
      isChatComposerBusy(
        {
          ...runState,
          statusByNode: {
            ...runState.statusByNode,
            "node-2": "awaiting_input",
          },
        },
        "node-2",
      ),
    ).toBe(false);
    expect(
      isChatComposerBusy(
        {
          ...runState,
          statusByNode: {
            ...runState.statusByNode,
            "node-2": "awaiting_tool_approval",
          },
        },
        "node-2",
      ),
    ).toBe(false);
  });

  test("projectWorkflowCanvasSubagentsByNode returns null for null runState", () => {
    expect(projectWorkflowCanvasSubagentsByNode(null, null)).toBeNull();
  });

  test("projectWorkflowCanvasSubagentsByNode reads subagentsByNode", () => {
    const result = projectWorkflowCanvasSubagentsByNode(
      { ...runState, subagentsByNode: { "node-1": [{ id: "n1-sub-1", name: "Researcher", purpose: "Investigate", status: "declared" as SubagentStatus }] } },
      null,
    );
    expect(result).not.toBeNull();
    expect(result!["node-1"]).toEqual([
      { id: "n1-sub-1", name: "Researcher", purpose: "Investigate", status: "declared" as SubagentStatus },
    ]);
  });

  test("projectWorkflowCanvasSubagentsByNode reuses previous when unchanged", () => {
    const subagents: Record<string, SubagentSummary[]> = { "node-1": [{ id: "n1-sub-1", name: "Researcher", purpose: "Investigate", status: "declared" as SubagentStatus }] };
    const stateWithSubagents = { ...runState, subagentsByNode: subagents };
    const first = projectWorkflowCanvasSubagentsByNode(stateWithSubagents, null);
    const second = projectWorkflowCanvasSubagentsByNode(stateWithSubagents, first);
    expect(second).toBe(first);
  });

  test("projectWorkflowCanvasSubagentsByNode returns empty for nodes with no subagents", () => {
    const result = projectWorkflowCanvasSubagentsByNode(runState, null);
    expect(result).not.toBeNull();
    expect(Object.keys(result!)).toHaveLength(0);
  });

  test("nodeChangedFiles and nodeEditBatches scope to selected node", () => {
    const state: WorkflowRunState = {
      ...runState,
      changedFilesByNode: {
        "node-1": [
          {
            path: "a.ts",
            op: "update",
            timestampMs: 1,
          },
        ],
        "node-2": [
          {
            path: "b.ts",
            op: "create",
            timestampMs: 2,
          },
        ],
      },
      editBatches: [
        {
          batchId: "batch-1",
          nodeId: "node-1",
          toolCallId: "tc-1",
          toolName: "write",
          timestampMs: 1,
          snapshots: [],
        },
        {
          batchId: "batch-2",
          nodeId: "node-2",
          toolCallId: "tc-2",
          toolName: "edit",
          timestampMs: 2,
          snapshots: [],
        },
      ],
    };

    expect(nodeChangedFiles(state, "node-1").map((record) => record.path)).toEqual(["a.ts"]);
    expect(nodeChangedFiles(state, "node-2").map((record) => record.path)).toEqual(["b.ts"]);
    expect(nodeChangedFiles(state, null)).toEqual([]);
    expect(nodeEditBatches(state, "node-1").map((batch) => batch.batchId)).toEqual(["batch-1"]);
    expect(nodeEditBatches(state, "node-2").map((batch) => batch.batchId)).toEqual(["batch-2"]);
  });
});

function makeDiamondWorkflow(): Workflow {
  return {
    id: "diamond",
    name: "Diamond",
    nodes: [
      {
        id: "node-a",
        label: "A",
        kind: "Agent",
        position: { x: 0, y: 0 },
        agent: workflow.nodes[0].agent,
      },
      {
        id: "node-b",
        label: "B",
        kind: "Agent",
        position: { x: 0, y: 0 },
        agent: workflow.nodes[1].agent,
      },
      {
        id: "node-c",
        label: "C",
        kind: "Agent",
        position: { x: 0, y: 0 },
        agent: workflow.nodes[1].agent,
      },
      {
        id: "node-d",
        label: "D",
        kind: "Agent",
        position: { x: 0, y: 0 },
        agent: workflow.nodes[1].agent,
      },
    ],
    edges: [
      { id: "edge-ab", from: "node-a", to: "node-b" },
      { id: "edge-ac", from: "node-a", to: "node-c" },
      { id: "edge-bd", from: "node-b", to: "node-d" },
      { id: "edge-cd", from: "node-c", to: "node-d" },
    ],
    settings: workflow.settings,
  };
}

describe("executionLayers", () => {
  test("linear chain yields one node per layer", () => {
    expect(executionLayers(workflow)).toEqual([["node-1"], ["node-2"]]);
  });

  test("diamond workflow layers siblings in declaration order", () => {
    expect(executionLayers(makeDiamondWorkflow())).toEqual([
      ["node-a"],
      ["node-b", "node-c"],
      ["node-d"],
    ]);
  });

  test("appends cycle leftovers as a final layer", () => {
    const cyclic = cloneWorkflow(workflow);
    cyclic.edges.push({ id: "edge-cycle", from: "node-2", to: "node-1" });
    const layers = executionLayers(cyclic);
    expect(layers).toEqual([["node-1", "node-2"]]);
  });
});

describe("normalizeRunState", () => {
  test("maps legacy camelCase agent statuses to snake_case", () => {
    const state: WorkflowRunState = {
      ...runState,
      statusByNode: {
        "node-1": "completed" as AgentStatus,
        "node-2": "awaitingInput" as AgentStatus,
      },
    };

    const normalized = normalizeRunState(state);
    expect(normalized.statusByNode["node-2"]).toBe("awaiting_input");
    const layout = projectChatLayout(workflow, normalized);
    expect(layout.live).toEqual([]);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-2"]);
  });
});

describe("nodeRunAppearanceOrder", () => {
  test("uses run trace before DAG traversal for nodes with chat", () => {
    const state: WorkflowRunState = {
      ...runState,
      chatLogs: {
        "node-1": [{ role: "Assistant", content: "one" }],
        "node-2": [{ role: "Assistant", content: "two" }],
      },
      runTrace: [
        {
          nodeId: "node-2",
          nodeLabel: "Draft",
          status: "running",
          message: "started",
          output: null,
        },
        {
          nodeId: "node-1",
          nodeLabel: "Plan",
          status: "completed",
          message: "done",
          output: null,
        },
      ],
    };

    expect(nodeRunAppearanceOrder(state, ["node-1", "node-2"])).toEqual(["node-2", "node-1"]);
  });
});

describe("sortTranscriptSegmentsByNodeOrder", () => {
  test("reorders segments to match append ledger", () => {
    const segments = [
      { nodeId: "node-b", label: "B", messages: [], status: "completed" as const },
      { nodeId: "node-a", label: "A", messages: [], status: "completed" as const },
    ];
    expect(
      sortTranscriptSegmentsByNodeOrder(segments, ["node-a", "node-b"]).map(
        (segment) => segment.nodeId,
      ),
    ).toEqual(["node-a", "node-b"]);
  });
});

describe("projectChatLayout", () => {
  test("classifies live and settled nodes in layer order", () => {
    const diamond = makeDiamondWorkflow();
    const state: WorkflowRunState = {
      ...runState,
      active: true,
      awaitingNodeIds: ["node-b", "node-c"],
      awaitingNodeId: "node-b",
      statusByNode: {
        "node-a": "completed",
        "node-b": "awaiting_input",
        "node-c": "awaiting_input",
        "node-d": "idle",
      },
      chatLogs: {
        "node-a": [{ role: "Assistant", content: "upstream done" }],
        "node-b": [{ role: "User", content: "branch b" }],
        "node-c": [],
      },
    };

    const layout = projectChatLayout(diamond, state);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-a"]);
    expect(layout.live.map((segment) => segment.nodeId)).toEqual(["node-b", "node-c"]);
  });

  test("inactive run moves everything to settled", () => {
    const state: WorkflowRunState = {
      ...runState,
      active: false,
      statusByNode: {
        "node-1": "completed",
        "node-2": "awaiting_input",
      },
      chatLogs: {
        "node-1": [{ role: "Assistant", content: "one" }],
        "node-2": [{ role: "User", content: "two" }],
      },
    };

    const layout = projectChatLayout(workflow, state);
    expect(layout.live).toEqual([]);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-1", "node-2"]);
  });

  test("appends deleted-node logs after workflow nodes", () => {
    const state: WorkflowRunState = {
      ...runState,
      active: false,
      statusByNode: {
        "node-1": "completed",
        "node-2": "completed",
        "deleted-node": "completed",
      },
      chatLogs: {
        "node-1": [{ role: "Assistant", content: "one" }],
        "deleted-node": [{ role: "Assistant", content: "orphan" }],
      },
    };

    const layout = projectChatLayout(workflow, state);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual([
      "node-1",
      "deleted-node",
    ]);
    expect(layout.settled[1]?.label).toBe("deleted-node");
  });

  test("treats awaiting nodes as live before status catches up", () => {
    const state: WorkflowRunState = {
      ...runState,
      active: true,
      awaitingNodeIds: ["node-2"],
      awaitingNodeId: "node-2",
      statusByNode: {
        "node-1": "completed",
        "node-2": "idle",
      },
      chatLogs: {
        "node-1": [{ role: "Assistant", content: "done" }],
        "node-2": [],
      },
    };

    const layout = projectChatLayout(workflow, state);
    expect(layout.live).toEqual([]);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-1", "node-2"]);
    expect(layout.settled[1]?.status).toBe("awaiting_input");
  });

  test("folds a single live node into settled history", () => {
    const state: WorkflowRunState = {
      ...runState,
      active: true,
      statusByNode: {
        "node-1": "completed",
        "node-2": "started",
      },
      chatLogs: {
        "node-1": [{ role: "Assistant", content: "done" }],
        "node-2": [{ role: "Assistant", content: "working" }],
      },
    };

    const layout = projectChatLayout(workflow, state);
    expect(layout.live).toEqual([]);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-1", "node-2"]);
    expect(layout.settled[1]?.status).toBe("started");
  });

  test("keeps parallel live nodes out of settled until they finish", () => {
    const diamond = makeDiamondWorkflow();
    const state: WorkflowRunState = {
      ...runState,
      active: true,
      awaitingNodeIds: ["node-b", "node-c"],
      awaitingNodeId: "node-b",
      statusByNode: {
        "node-a": "completed",
        "node-b": "awaiting_input",
        "node-c": "awaiting_input",
        "node-d": "idle",
      },
      chatLogs: {
        "node-a": [{ role: "Assistant", content: "upstream done" }],
        "node-b": [{ role: "User", content: "branch b" }],
        "node-c": [{ role: "User", content: "branch c" }],
      },
    };

    const layout = projectChatLayout(diamond, state);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-a"]);
    expect(layout.live.map((segment) => segment.nodeId)).toEqual(["node-b", "node-c"]);
  });

  test("appends picked parallel node after prior settled segments", () => {
    const diamond = makeDiamondWorkflow();
    const state: WorkflowRunState = {
      ...runState,
      active: true,
      awaitingNodeIds: ["node-b", "node-c"],
      awaitingNodeId: "node-b",
      statusByNode: {
        "node-a": "completed",
        "node-b": "awaiting_input",
        "node-c": "awaiting_input",
        "node-d": "idle",
      },
      chatLogs: {
        "node-a": [{ role: "Assistant", content: "upstream done" }],
        "node-b": [{ role: "User", content: "branch b" }],
        "node-c": [{ role: "User", content: "branch c" }],
      },
      runTrace: [
        {
          nodeId: "node-a",
          nodeLabel: "Plan",
          status: "completed",
          message: "done",
          output: null,
        },
        {
          nodeId: "node-b",
          nodeLabel: "Branch B",
          status: "paused",
          message: "paused",
          output: null,
        },
        {
          nodeId: "node-c",
          nodeLabel: "Branch C",
          status: "paused",
          message: "paused",
          output: null,
        },
      ],
    };

    const layout = projectChatLayout(diamond, state, "node-c", ["node-a", "node-c"]);
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-a", "node-c"]);
    expect(layout.live.map((segment) => segment.nodeId)).toEqual(["node-b"]);
  });

  test("folds the picked parallel node inline, keeping siblings live", () => {
    const diamond = makeDiamondWorkflow();
    const state: WorkflowRunState = {
      ...runState,
      active: true,
      awaitingNodeIds: ["node-b", "node-c"],
      awaitingNodeId: "node-b",
      statusByNode: {
        "node-a": "completed",
        "node-b": "awaiting_input",
        "node-c": "awaiting_input",
        "node-d": "idle",
      },
      chatLogs: {
        "node-a": [{ role: "Assistant", content: "upstream done" }],
        "node-b": [{ role: "User", content: "branch b" }],
        "node-c": [{ role: "User", content: "branch c" }],
      },
    };

    const layout = projectChatLayout(diamond, state, "node-c");
    expect(layout.settled.map((segment) => segment.nodeId)).toEqual(["node-a", "node-c"]);
    expect(layout.live.map((segment) => segment.nodeId)).toEqual(["node-b"]);
  });

  test("preserves per-node message order", () => {
    const state: WorkflowRunState = {
      ...runState,
      active: false,
      statusByNode: {
        "node-1": "completed",
        "node-2": "completed",
      },
      chatLogs: {
        "node-1": [
          { role: "System", content: "first" },
          { role: "Assistant", content: "second" },
        ],
      },
    };

    const layout = projectChatLayout(workflow, state);
    expect(layout.settled[0]?.messages.map((message) => message.content)).toEqual([
      "first",
      "second",
    ]);
  });
});
