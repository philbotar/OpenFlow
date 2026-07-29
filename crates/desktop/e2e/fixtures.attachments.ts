import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createTauriTest } from "@srsholmes/tauri-playwright";
import {
  createOpenflowIpcMocks,
  EMPTY_BOOTSTRAP,
} from "./ipcMocks.js";

const uiRoot = join(dirname(fileURLToPath(import.meta.url)), "../../ui");

const pendingChat = {
  id: "chat-attachments",
  title: "Attachment chat",
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

const otherChat = {
  ...pendingChat,
  id: "chat-other",
  title: "Other chat",
  createdAtMs: 2,
  updatedAtMs: 2,
};

const acceptedChat = {
  ...pendingChat,
  title: "capture.png",
  runId: "run-attachments",
  updatedAtMs: 3,
};

const attachment = {
  id: "attachment-image-1",
  fileName: "capture.png",
  mediaType: "image/png",
  sizeBytes: 128,
  sha256: "fixture-sha256",
  kind: "image",
};

const acceptedRunState = {
  runId: "run-attachments",
  active: false,
  awaitingNodeId: null,
  awaitingNodeIds: [],
  activeManualNodeId: null,
  activeToolCallId: null,
  pendingApprovals: [],
  toolCallsByNode: {},
  toolArtifacts: {},
  execApprovalGranted: false,
  statusByNode: { "chat-attachments-node-1": "completed" },
  subagentsByNode: {},
  lastReport: null,
  lastError: null,
  chatLogs: {
    "chat-attachments-node-1": [
      {
        role: "User",
        content: "",
        attachments: [attachment],
      },
    ],
  },
  runTrace: [],
  outputs: {},
  changedFiles: [],
  changedFilesByNode: {},
  editBatches: [],
};

const bootstrap = {
  ...EMPTY_BOOTSTRAP,
  chats: [pendingChat, otherChat],
};

function ipcBody(body: string): (args?: Record<string, unknown>) => unknown {
  return new Function("args", body) as (args?: Record<string, unknown>) => unknown;
}

const acceptedChatJson = JSON.stringify(acceptedChat);
const acceptedRunStateJson = JSON.stringify(acceptedRunState);

const attachmentMocks: Record<
  string,
  (args?: Record<string, unknown>) => unknown
> = {
  "plugin:dialog|open": ipcBody(`
    return window.__openflowAttachmentScenario === "failure"
      ? ["/tmp/rejected.png"]
      : ["/tmp/capture.png"];
  `),
  start_chat: ipcBody(`
    window.__openflowE2e = window.__openflowE2e || { calls: [] };
    window.__openflowE2e.calls.push({ type: "start_chat", args });
    if (window.__openflowAttachmentScenario === "failure") {
      throw new Error("Attachment import failed: invalid image data.");
    }
    const runState = ${acceptedRunStateJson};
    window.__openflowAttachmentRunState = runState;
    return { chat: ${acceptedChatJson}, runState };
  `),
  get_run_state: ipcBody(`
    return window.__openflowAttachmentRunState || null;
  `),
  replay_run: ipcBody(`
    window.__openflowE2e = window.__openflowE2e || { calls: [] };
    window.__openflowE2e.calls.push({ type: "replay_run", args });
    return ${acceptedRunStateJson};
  `),
  load_chat_attachment_preview: ipcBody(`
    window.__openflowE2e = window.__openflowE2e || { calls: [] };
    window.__openflowE2e.calls.push({
      type: "load_chat_attachment_preview",
      args,
    });
    return {
      mediaType: "image/jpeg",
      dataBase64: "/9j/2Q==",
    };
  `),
};

export const { test, expect } = createTauriTest({
  devUrl: "http://localhost:1420",
  ipcMocks: {
    ...createOpenflowIpcMocks(bootstrap),
    ...attachmentMocks,
  },
  mcpSocket: "/tmp/openflow-playwright-chat-attachments.sock",
  tauriCommand: "npm run tauri -- dev",
  tauriCwd: uiRoot,
  tauriFeatures: ["e2e-testing"],
});
