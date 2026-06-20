import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createTauriTest } from "@srsholmes/tauri-playwright";
import { createOpenflowIpcMocks } from "./ipcMocks.js";
import { MULTI_SEGMENT_BOOTSTRAP } from "./fixtures/multiSegmentChat.js";

const uiRoot = join(dirname(fileURLToPath(import.meta.url)), "../../ui");

export const { test, expect } = createTauriTest({
  devUrl: "http://localhost:1420",
  ipcMocks: createOpenflowIpcMocks(MULTI_SEGMENT_BOOTSTRAP),
  mcpSocket: "/tmp/openflow-playwright-visual.sock",
  tauriCommand: "npm run tauri -- dev",
  tauriCwd: uiRoot,
  tauriFeatures: ["e2e-testing"],
});
