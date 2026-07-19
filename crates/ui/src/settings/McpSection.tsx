import { createMemo, createSignal, For, Show } from "solid-js";
import { Button, SectionHeader, SettingsSection } from "@/components";
import { probeMcpServer } from "../api";
import { useAppContext } from "../context/AppContext";
import type { McpDiscoveryRow, McpServerConfig } from "../lib/types";

type McpConnection =
  | {
      kind: "configured";
      id: string;
      displayName: string;
      command: string;
      args: string[];
      enabled: boolean;
      sourceLabel: string;
      server: McpServerConfig;
      index: number;
    }
  | {
      kind: "discovered";
      id: string;
      displayName: string;
      command: string;
      args: string[];
      enabled: boolean;
      sourceLabel: string;
      sourcePath: string;
      row: McpDiscoveryRow;
    };

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

function sourceLabel(source: string): string {
  if (!source.trim()) return "External config";
  return `${source[0].toUpperCase()}${source.slice(1)} config`;
}

export function McpSection() {
  const ctx = useAppContext();
  const [draft, setDraft] = createSignal(emptyServer());
  const [showAddForm, setShowAddForm] = createSignal(false);
  const [probeResults, setProbeResults] = createSignal<Record<string, string>>({});

  const servers = () => ctx.settings().mcp?.servers ?? [];
  const discoverExternal = () => ctx.settings().mcp?.discoverExternal ?? true;
  const discoveredCount = () => ctx.discoveredMcp().length;
  const configuredCount = () => servers().length;
  const disabledDiscoveredIds = () => ctx.settings().mcp?.disabledDiscoveredIds ?? [];
  const isDiscoveredEnabled = (row: McpDiscoveryRow) =>
    row.enabled && !disabledDiscoveredIds().includes(row.id);
  const connections = createMemo<McpConnection[]>(() => {
    const byId = new Map<string, McpConnection>();

    for (const row of ctx.discoveredMcp()) {
      byId.set(row.id, {
        kind: "discovered",
        id: row.id,
        displayName: row.displayName,
        command: row.command,
        args: row.args,
        enabled: isDiscoveredEnabled(row),
        sourceLabel: sourceLabel(row.source),
        sourcePath: row.sourcePath,
        row,
      });
    }

    servers().forEach((server, index) => {
      byId.set(server.id, {
        kind: "configured",
        id: server.id,
        displayName: server.displayName,
        command: server.command,
        args: server.args,
        enabled: server.enabled,
        sourceLabel: "OpenFlow settings",
        server,
        index,
      });
    });

    return [...byId.values()];
  });

  const connectionCount = () => connections().length;

  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      const current = settings.mcp.servers[index];
      if (!current) return;
      settings.mcp.servers[index] = { ...current, ...patch };
    });
  };

  const removeServer = (index: number) => {
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.servers.splice(index, 1);
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

  const copyDiscoveredToSettings = (row: McpDiscoveryRow) => {
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      if (settings.mcp.servers.some((server) => server.id === row.id)) return;
      settings.mcp.servers.push({
        id: row.id,
        displayName: row.displayName,
        command: row.command,
        args: row.args,
        env: {},
        enabled: isDiscoveredEnabled(row),
      });
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
    setShowAddForm(false);
  };

  const configForProbe = (connection: McpConnection): McpServerConfig =>
    connection.kind === "configured"
      ? connection.server
      : {
          id: connection.id,
          displayName: connection.displayName,
          command: connection.command,
          args: connection.args,
          env: {},
          enabled: connection.enabled,
        };

  const probeServer = async (connection: McpConnection) => {
    setProbeResults((current) => ({ ...current, [connection.id]: "Testing…" }));
    try {
      const tools = await probeMcpServer(configForProbe(connection));
      setProbeResults((current) => ({
        ...current,
        [connection.id]: tools.length ? `${tools.length} tools: ${tools.join(", ")}` : "Connected",
      }));
    } catch (error) {
      setProbeResults((current) => ({
        ...current,
        [connection.id]: error instanceof Error ? error.message : String(error),
      }));
    }
  };

  return (
    <SettingsSection sectionClass="mcp-section">
      <SectionHeader
        eyebrow="MCP"
        title="MCP servers"
        description="Choose which MCP servers are available to workflow runs."
      />

      <div class="mcp-cards">
        <section class="mcp-card mcp-card--management" aria-labelledby="mcp-connections-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-connections-heading" class="mcp-card-title">
              Connections
            </h4>
            <p class="mcp-card-copy">
              {connectionCount()} available ·{" "}
              {connections().filter((connection) => connection.enabled).length} enabled
            </p>
          </div>
          <Show
            when={connectionCount() > 0}
            fallback={<div class="mcp-empty-state">No MCP servers yet.</div>}
          >
            <div class="mcp-server-list">
              <For each={connections()}>
                {(connection) => (
                  <div class="mcp-server-row mcp-connection-row">
                    <div class="mcp-server-row-main">
                      <strong class="mcp-server-name">{connection.displayName}</strong>
                      <p class="mcp-server-meta">
                        {connection.sourceLabel}
                        <Show when={connection.kind === "discovered"}>
                          {" "}
                          ·{" "}
                          {shortenPath(
                            (connection as Extract<McpConnection, { kind: "discovered" }>)
                              .sourcePath,
                          )}
                        </Show>
                      </p>
                      <Show when={connection.kind === "configured"}>
                        <div class="mcp-configured-fields">
                          <label>
                            <span>Name</span>
                            <input
                              class="text-input"
                              value={connection.displayName}
                              onInput={(event) =>
                                updateServer(
                                  (connection as Extract<McpConnection, { kind: "configured" }>)
                                    .index,
                                  { displayName: event.currentTarget.value },
                                )
                              }
                            />
                          </label>
                          <label>
                            <span>Command</span>
                            <input
                              class="text-input"
                              value={connection.command}
                              onInput={(event) =>
                                updateServer(
                                  (connection as Extract<McpConnection, { kind: "configured" }>)
                                    .index,
                                  { command: event.currentTarget.value },
                                )
                              }
                            />
                          </label>
                        </div>
                      </Show>
                      <Show when={probeResults()[connection.id]}>
                        <div class="mcp-probe-status" role="status" aria-live="polite">
                          {probeResults()[connection.id]}
                        </div>
                      </Show>
                    </div>
                    <div class="mcp-server-row-actions">
                      <label class="checkbox-row">
                        <input
                          type="checkbox"
                          checked={connection.enabled}
                          onChange={(event) => {
                            if (connection.kind === "configured") {
                              updateServer(connection.index, {
                                enabled: event.currentTarget.checked,
                              });
                            } else {
                              toggleDiscoveredEnabled(connection.row, event.currentTarget.checked);
                            }
                          }}
                        />
                        <span>Enabled</span>
                      </label>
                      <Button variant="secondary" onClick={() => void probeServer(connection)}>
                        Test
                      </Button>
                      <Show when={connection.kind === "discovered"}>
                        <Button
                          variant="secondary"
                          onClick={() =>
                            copyDiscoveredToSettings(
                              (connection as Extract<McpConnection, { kind: "discovered" }>).row,
                            )
                          }
                        >
                          Customize
                        </Button>
                      </Show>
                      <Show when={connection.kind === "configured"}>
                        <Button
                          variant="secondary"
                          ghost
                          onClick={() =>
                            removeServer(
                              (connection as Extract<McpConnection, { kind: "configured" }>).index,
                            )
                          }
                        >
                          Delete
                        </Button>
                      </Show>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </section>

        <section class="mcp-card mcp-card--discovery" aria-labelledby="mcp-advanced-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-advanced-heading" class="mcp-card-title">
              Advanced
            </h4>
            <p class="mcp-card-copy">
              {discoveredCount()} discovered · {configuredCount()} saved in OpenFlow
            </p>
          </div>
          <label class="checkbox-row mcp-discovery-toggle">
            <input
              type="checkbox"
              checked={discoverExternal()}
              onChange={(event) => void toggleDiscoverExternal(event.currentTarget.checked)}
            />
            <span>Use servers from external MCP configs</span>
          </label>
        </section>

        <section class="mcp-card mcp-card--composer" aria-labelledby="mcp-add-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-add-heading" class="mcp-card-title">
              Custom connection
            </h4>
            <p class="mcp-card-copy">Saved in OpenFlow settings and used at run start.</p>
          </div>
          <Show
            when={showAddForm()}
            fallback={
              <Button variant="primary" class="mcp-add-trigger" onClick={() => setShowAddForm(true)}>
                Add connection
              </Button>
            }
          >
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
                <span>Args</span>
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
              <Button variant="primary" onClick={addServer}>
                Save connection
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setDraft(emptyServer());
                  setShowAddForm(false);
                }}
              >
                Cancel
              </Button>
            </div>
          </Show>
        </section>
      </div>
    </SettingsSection>
  );
}
