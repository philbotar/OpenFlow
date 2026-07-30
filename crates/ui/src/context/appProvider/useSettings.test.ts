import { createRoot } from "solid-js";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { EMPTY_SETTINGS } from "../../constants/providers";
import { cloneSettings } from "../../lib/workflow";
import { useSettings } from "./useSettings";

const desktopMocks = vi.hoisted(() => ({
  deleteProviderApiKey: vi.fn(),
  loadProviderApiKey: vi.fn(),
  resolveProviderReadiness: vi.fn(),
  saveProviderApiKey: vi.fn(),
  saveSettings: vi.fn(),
}));

vi.mock("../../api", () => desktopMocks);

describe("useSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    desktopMocks.loadProviderApiKey.mockResolvedValue(null);
    desktopMocks.resolveProviderReadiness.mockResolvedValue({
      ready: true,
      provider: "OpenAI",
      message: "Ready",
      envVar: "OPENAI_API_KEY",
    });
    desktopMocks.saveSettings.mockResolvedValue(undefined);
  });

  test("autosaves settings mutations", async () => {
    await createRoot(async (dispose) => {
      const state = useSettings({
        showErrorToast: vi.fn(),
        showSuccessToast: vi.fn(),
      });
      const initial = cloneSettings(EMPTY_SETTINGS);
      initial.mcp = {
        servers: [
          {
            id: "gh",
            displayName: "GitHub",
            command: "npx",
            args: ["-y", "github-mcp"],
            env: {},
            enabled: true,
          },
        ],
      };
      state.setSettings(initial);

      await state.updateSettings((draft) => {
        if (draft.mcp) draft.mcp.servers[0].enabled = false;
      });

      expect(desktopMocks.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          mcp: expect.objectContaining({
            servers: [
              expect.objectContaining({
                id: "gh",
                enabled: false,
              }),
            ],
          }),
        }),
      );
      dispose();
    });
  });
});
