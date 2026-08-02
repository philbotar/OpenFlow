import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createTauriTest } from "@srsholmes/tauri-playwright";
import { createOpenflowIpcMocks, EMPTY_BOOTSTRAP } from "./ipcMocks.js";

const uiRoot = join(dirname(fileURLToPath(import.meta.url)), "../../ui");

const chat = {
  id: "chat-runtime",
  title: "Runtime chat",
  config: {
    model: null,
    approvalMode: "read_only",
    reasoningEffort: null,
    reasoningBudgetTokens: null,
    fastMode: false,
    projectId: null,
  },
  runId: null,
  createdAtMs: 1,
  updatedAtMs: 1,
};

const settings = {
  ...EMPTY_BOOTSTRAP.settings,
  providers: {
    ...EMPTY_BOOTSTRAP.settings.providers,
    openai: {
      ...EMPTY_BOOTSTRAP.settings.providers.openai,
      known_models: ["gpt-4.1-mini", "gpt-5"],
      default_model: "gpt-4.1-mini",
      reasoning_effort_options: [
        { value: "high", label: "High", uses_budget_tokens: false },
      ],
      fast_mode_available: true,
    },
  },
};

function ipcBody(body: string): (args?: Record<string, unknown>) => unknown {
  return new Function("args", body) as (args?: Record<string, unknown>) => unknown;
}

const chatJson = JSON.stringify(chat);

export const { test, expect } = createTauriTest({
  devUrl: "http://localhost:1420",
  ipcMocks: {
    ...createOpenflowIpcMocks({
      ...EMPTY_BOOTSTRAP,
      chats: [chat],
      settings,
    }),
    update_chat_config: ipcBody(`
      window.__openflowE2e = window.__openflowE2e || { calls: [] };
      window.__openflowE2e.calls.push({ type: "update_chat_config", args });
      return { ...${chatJson}, config: args.config };
    `),
  },
  mcpSocket: "/tmp/openflow-playwright-chat-runtime.sock",
  tauriCommand: "npm run tauri -- dev",
  tauriCwd: uiRoot,
  tauriFeatures: ["e2e-testing"],
});
