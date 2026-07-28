import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createTauriTest } from "@srsholmes/tauri-playwright";
import { createOpenflowIpcMocks } from "./ipcMocks.js";
import { MULTI_SEGMENT_BOOTSTRAP } from "./fixtures/multiSegmentChat.js";

const uiRoot = join(dirname(fileURLToPath(import.meta.url)), "../../ui");
const inspectorBootstrap = {
  ...MULTI_SEGMENT_BOOTSTRAP,
  skills: [
    "code-review",
    "diagnose",
    "plan-execute",
    "ponytail",
    "research",
    "tdd",
    "ui-typography",
    "zoom-out",
  ].map((id) => ({
    id,
    name: id,
    description: `Use ${id} for this task.`,
    path: `/skills/${id}/SKILL.md`,
  })),
};

export const { test, expect } = createTauriTest({
  devUrl: "http://localhost:1420",
  ipcMocks: createOpenflowIpcMocks(inspectorBootstrap),
  mcpSocket: "/tmp/openflow-playwright-inspector.sock",
  tauriCommand: "npm run tauri -- dev",
  tauriCwd: uiRoot,
  tauriFeatures: ["e2e-testing"],
});
