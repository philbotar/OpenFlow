import { createSignal, For, Show } from "solid-js";
import { probeMcpServer } from "../api";
import { useAppContext } from "../context/AppContext";
import type { McpDiscoveryRow, McpServerConfig } from "../lib/types";

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

function shortenPath(path: string): string {
  const home = path.replace(/^\/Users\/[^/]+/, "~");
  return home.length > 48 ? `…${home.slice(-45)}` : home;
}

export function McpSection() {
  const ctx = useAppContext();
  const [draft, setDraft] = createSignal(emptyServer());
  const [probeResult, setProbeResult] = createSignal<string | null>(null);

  const servers = () => ctx.settings().mcp?.servers ?? [];
  const discoverExternal = () => ctx.settings().mcp?.discoverExternal ?? true;
  const discoveredCount = () => ctx.discoveredMcp().length;
  const configuredCount = () => servers().length;

  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      const current = settings.mcp.servers[index];
      if (!current) return;
      settings.mcp.servers[index] = { ...current, ...patch };
    });
  };

  const toggleDiscoverExternal = async (enabled: boolean) => {
    await ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.discoverExternal = enabled;
    });
    await ctx.refreshDiscoveredMcp();
  };

  const toggleDiscoveredEnabled = (row: McpDiscoveryRow, enabled: boolean) => {
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      const disabled = new Set(settings.mcp.disabledDiscoveredIds ?? []);
      if (enabled) {
        disabled.delete(row.id);
      } else {
        disabled.add(row.id);
      }
      settings.mcp.disabledDiscoveredIds = [...disabled];
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

      <div class="mcp-cards">
        <section class="mcp-card mcp-card--discovery" aria-labelledby="mcp-discovery-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-discovery-heading" class="mcp-card-title">
              Discovery
            </h4>
            <p class="mcp-card-copy">Scan external config files for MCP servers on this machine.</p>
          </div>
          <label class="checkbox-row mcp-discovery-toggle">
            <input
              type="checkbox"
              checked={discoverExternal()}
              onChange={(event) => void toggleDiscoverExternal(event.currentTarget.checked)}
            />
            <span>Discover external MCP configs</span>
          </label>
          <p class="mcp-discovery-summary" aria-live="polite">
            {discoveredCount()} discovered · {configuredCount()} configured
          </p>
        </section>

        <section class="mcp-card mcp-card--management" aria-labelledby="mcp-discovered-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-discovered-heading" class="mcp-card-title">
              Discovered servers
            </h4>
            <p class="mcp-card-copy">Enable or disable servers found in external MCP configs.</p>
          </div>
          <Show
            when={discoveredCount() > 0}
            fallback={<div class="mcp-empty-state">No discovered MCP servers.</div>}
          >
            <div class="mcp-server-list">
              <For each={ctx.discoveredMcp()}>
                {(row) => (
                  <div class="mcp-server-row">
                    <div class="mcp-server-row-main">
                      <strong class="mcp-server-name">{row.displayName}</strong>
                      <p class="mcp-server-meta">
                        {row.source} · {shortenPath(row.sourcePath)}
                      </p>
                    </div>
                    <div class="mcp-server-row-actions">
                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          checked={
                            row.enabled &&
                            !(ctx.settings().mcp?.disabledDiscoveredIds ?? []).includes(row.id)
                          }
                          onChange={(event) =>
                            toggleDiscoveredEnabled(row, event.currentTarget.checked)
                          }
                        />
                        <span>Enabled</span>
                      </label>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </section>

        <section class="mcp-card mcp-card--management" aria-labelledby="mcp-servers-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-servers-heading" class="mcp-card-title">
              Configured servers
            </h4>
            <p class="mcp-card-copy">Edit saved MCP servers stored in OpenFlow settings.</p>
          </div>
          <Show
            when={configuredCount() > 0}
            fallback={<div class="mcp-empty-state">No MCP servers configured.</div>}
          >
            <div class="mcp-server-list">
              <For each={servers()}>
                {(server, index) => (
                  <div class="mcp-server-row mcp-server-row--configured">
                    <div class="mcp-server-row-main">
                      <div class="mcp-configured-fields">
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
                      </div>
                    </div>
                    <div class="mcp-server-row-actions">
                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          checked={server.enabled}
                          onChange={(event) =>
                            updateServer(index(), { enabled: event.currentTarget.checked })
                          }
                        />
                        <span>Enabled</span>
                      </label>
                      <button
                        type="button"
                        class="secondary-button"
                        onClick={() => void probeServer(server)}
                      >
                        Test
                      </button>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
          <Show when={probeResult()}>
            <div class="mcp-probe-status" role="status" aria-live="polite">
              {probeResult()}
            </div>
          </Show>
        </section>

        <section class="mcp-card mcp-card--composer" aria-labelledby="mcp-add-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-add-heading" class="mcp-card-title">
              Add custom server
            </h4>
            <p class="mcp-card-copy">Register a stdio MCP server by id and launch command.</p>
          </div>
          <div class="mcp-composer-fields">
            <label>
              <span>Id</span>
              <input
                class="text-input"
                value={draft().id}
                onInput={(event) =>
                  setDraft((current) => ({ ...current, id: event.currentTarget.value }))
                }
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
          </div>
          <div class="mcp-composer-actions">
            <button type="button" class="primary-button" onClick={addServer}>
              Add server
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
