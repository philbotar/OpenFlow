import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createTauriTest } from "@srsholmes/tauri-playwright";
import { openflowIpcMocks } from "./ipcMocks.js";

const uiRoot = join(dirname(fileURLToPath(import.meta.url)), "../../ui");

export const { test, expect } = createTauriTest({
  devUrl: "http://localhost:1420",
  ipcMocks: openflowIpcMocks,
  mcpSocket: "/tmp/openflow-playwright.sock",
  tauriCommand: "npm run tauri -- dev",
  tauriCwd: uiRoot,
  tauriFeatures: ["e2e-testing"],
});
