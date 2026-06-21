import { createSignal, For, Show } from "solid-js";
import { probeMcpServer } from "../api";
import { useAppContext } from "../context/AppContext";
import type { McpServerConfig } from "../lib/types";

function emptyServer(): McpServerConfig {
  return {
    id: "",
    displayName: "",
    command: "",
    args: [],
    env: {},
    enabled: true,
  };
}

export function McpSection() {
  const ctx = useAppContext();
  const [draft, setDraft] = createSignal(emptyServer());
  const [probeResult, setProbeResult] = createSignal<string | null>(null);

  const servers = () => ctx.settings().mcp?.servers ?? [];

  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      const current = settings.mcp.servers[index];
      if (!current) return;
      settings.mcp.servers[index] = { ...current, ...patch };
    });
  };

  const addServer = () => {
    const next = draft();
    if (!next.id.trim() || !next.command.trim()) return;
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.servers.push({
        ...next,
        id: next.id.trim(),
        displayName: next.displayName.trim() || next.id.trim(),
        command: next.command.trim(),
        args: next.args.filter((arg) => arg.length > 0),
      });
    });
    setDraft(emptyServer());
  };

  const probeServer = async (config: McpServerConfig) => {
    setProbeResult("Probing…");
    try {
      const tools = await probeMcpServer(config);
      setProbeResult(tools.length ? tools.join(", ") : "No tools reported");
    } catch (error) {
      setProbeResult(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <div class="settings-section mcp-section">
      <header class="providers-section-header">
        <div class="providers-section-intro">
          <div class="eyebrow">MCP</div>
          <h3>External tool servers</h3>
          <p>Stdio MCP servers merge into the tool catalog at run start when enabled.</p>
        </div>
      </header>

      <section class="settings-subsection" aria-labelledby="mcp-servers-heading">
        <h3 id="mcp-servers-heading" class="settings-subheading">
          Configured servers
        </h3>
        <Show when={servers().length === 0}>
          <p>No MCP servers configured.</p>
        </Show>
        <For each={servers()}>
          {(server, index) => (
            <div class="settings-row">
              <label>
                <span>Name</span>
                <input
                  class="text-input"
                  value={server.displayName}
                  onInput={(event) =>
                    updateServer(index(), { displayName: event.currentTarget.value })
                  }
                />
              </label>
              <label>
                <span>Command</span>
                <input
                  class="text-input"
                  value={server.command}
                  onInput={(event) =>
                    updateServer(index(), { command: event.currentTarget.value })
                  }
                />
              </label>
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={server.enabled}
                  onChange={(event) =>
                    updateServer(index(), { enabled: event.currentTarget.checked })
                  }
                />
                <span>Enabled</span>
              </label>
              <button type="button" class="btn-secondary" onClick={() => void probeServer(server)}>
                Test
              </button>
            </div>
          )}
        </For>
        <Show when={probeResult()}>
          <p class="settings-hint">{probeResult()}</p>
        </Show>
      </section>

      <section class="settings-subsection" aria-labelledby="mcp-add-heading">
        <h3 id="mcp-add-heading" class="settings-subheading">
          Add server
        </h3>
        <label>
          <span>Id</span>
          <input
            class="text-input"
            value={draft().id}
            onInput={(event) => setDraft((current) => ({ ...current, id: event.currentTarget.value }))}
          />
        </label>
        <label>
          <span>Display name</span>
          <input
            class="text-input"
            value={draft().displayName}
            onInput={(event) =>
              setDraft((current) => ({ ...current, displayName: event.currentTarget.value }))
            }
          />
        </label>
        <label>
          <span>Command</span>
          <input
            class="text-input"
            value={draft().command}
            onInput={(event) =>
              setDraft((current) => ({ ...current, command: event.currentTarget.value }))
            }
          />
        </label>
        <label>
          <span>Args (comma-separated)</span>
          <input
            class="text-input"
            value={draft().args.join(", ")}
            onInput={(event) =>
              setDraft((current) => ({
                ...current,
                args: event.currentTarget.value
                  .split(",")
                  .map((part) => part.trim())
                  .filter(Boolean),
              }))
            }
          />
        </label>
        <button type="button" class="btn-primary" onClick={addServer}>
          Add server
        </button>
      </section>
    </div>
  );
}
