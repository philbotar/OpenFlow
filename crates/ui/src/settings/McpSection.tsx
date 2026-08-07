import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import { Button, SectionHeader, SettingsSection, TextSelect } from "@/components";
import {
  applyMcpConfig,
  cancelMcpInstall,
  deleteMcpSecret,
  exportMcpConfig,
  importMcpConfig,
  installMcpPackage,
  listMcpRegistryVersions,
  mcpOAuthStatus,
  openExternalUrl,
  openLocalPath,
  previewMcpRegistryInstall,
  previewMcpRegistryRemote,
  probeMcpServer,
  refreshMcpOAuth,
  rollbackMcpInstall,
  saveMcpSecret,
  searchMcpRegistry,
  startMcpOAuth,
  disconnectMcpOAuth,
} from "../api";
import { useAppContext } from "../context/AppContext";
import type {
  McpConfigImport,
  McpCatalogPage,
  McpCatalogServer,
  McpConnection,
  McpDiscoveryRow,
  McpPersistedValue,
  McpProbeReport,
  McpInstallPreview,
  McpOAuthStatus,
  McpServerConfig,
} from "../lib/types";

function installLabel(install: McpServerConfig["install"]): string {
  if (install.type === "external") return "External";
  return `${install.package}@${install.version}`;
}

type McpConnectionRow =
  | {
      kind: "configured";
      id: string;
      server: McpServerConfig;
      index: number;
    }
  | {
      kind: "discovered";
      id: string;
      row: McpDiscoveryRow;
    };

type McpLifecycleState =
  | "Not installed"
  | "Needs config"
  | "Disabled"
  | "Connecting"
  | "Ready"
  | "Degraded"
  | "Auth required"
  | "Failed";

type McpLifecycleRecord = {
  state: McpLifecycleState;
  summary: string;
  report?: McpProbeReport;
  checkedAt?: string;
  attemptCount: number;
};

const MCP_TRANSPORT_OPTIONS = [
  { value: "stdio", label: "stdio" },
  { value: "streamableHttp", label: "Streamable HTTP" },
  { value: "legacySse", label: "Legacy HTTP + SSE" },
] as const;

const MCP_AUTH_OPTIONS = [
  { value: "none", label: "None" },
  { value: "oauth", label: "OAuth 2.1" },
] as const;

const MCP_AUTH_OPTIONS_WITH_STATIC = [
  ...MCP_AUTH_OPTIONS,
  { value: "static", label: "Static header" },
] as const;

const TOOL_ACCESS_OPTIONS = [
  { value: "write", label: "Write · approval policy applies" },
  { value: "read", label: "Read · user-classified server" },
] as const;

const TOOL_CONCURRENCY_OPTIONS = [
  { value: "exclusive", label: "Exclusive · serialize this server" },
  { value: "shared", label: "Shared · allow concurrent calls" },
] as const;

function defaultPolicy(): McpServerConfig["policy"] {
  return {
    defaultToolAccess: "write",
    defaultToolConcurrency: "exclusive",
    allowRoots: false,
    allowSampling: false,
    allowElicitation: false,
    samplingMaxRequestsPerRun: 4,
    samplingMaxTokensPerRequest: 4096,
    samplingMaxTotalTokensPerRun: 8192,
    elicitationMaxRequestsPerRun: 8,
  };
}

function emptyServer(): McpServerConfig {
  return {
    schemaVersion: 1,
    id: "",
    displayName: "",
    source: { type: "manual" },
    install: { type: "external" },
    connection: { type: "stdio", command: "", args: [], environment: {} },
    trust: {},
    policy: defaultPolicy(),
    enabled: false,
  };
}

function shortenPath(path: string): string {
  const home = path.replace(/^\/Users\/[^/]+/, "~");
  return home.length > 48 ? `…${home.slice(-45)}` : home;
}

function titleCase(value: string): string {
  return value ? `${value[0].toUpperCase()}${value.slice(1)}` : value;
}

function sourceLabel(server: McpServerConfig): string {
  switch (server.source.type) {
    case "manual":
      return "Manual";
    case "imported":
      return `${titleCase(server.source.dialect)} import · ${shortenPath(server.source.sourcePath)}`;
    case "registry":
      return `Registry · ${server.source.serverName}@${server.source.version}`;
  }
}

function transportLabel(connection: McpConnection): string {
  switch (connection.type) {
    case "stdio":
      return "stdio";
    case "streamableHttp":
      return "Streamable HTTP";
    case "legacySse":
      return "Legacy SSE";
  }
}

function connectionSummary(connection: McpConnection): string {
  if (connection.type === "stdio") {
    return [connection.command, ...connection.args.map(redactSensitiveArgument)]
      .filter(Boolean)
      .join(" ");
  }
  return connection.url;
}

const ENV_OBJECT_KEY_PATTERN = /["']?([A-Z][A-Z0-9_]{2,})["']?\s*:/g;
const SENSITIVE_NAME_PATTERN = /(?:API[_-]?KEY|TOKEN|SECRET|PASSWORD|AUTHORIZATION|CREDENTIAL)/i;

function environmentNamesInArgs(args: string[]): string[] {
  const names = new Set<string>();
  for (const argument of args) {
    for (const match of argument.matchAll(ENV_OBJECT_KEY_PATTERN)) {
      const name = match[1];
      if (SENSITIVE_NAME_PATTERN.test(name)) names.add(name);
    }
  }
  return [...names];
}

function redactSensitiveArgument(argument: string): string {
  if (environmentNamesInArgs([argument]).length > 0) return "[redacted credential argument]";
  if (SENSITIVE_NAME_PATTERN.test(argument) && /[:=]/.test(argument)) {
    return "[redacted credential argument]";
  }
  return argument;
}

function formatCheckedAt(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
}

type McpRegistryGroup = {
  name: string;
  title?: string;
  description: string;
  servers: McpCatalogServer[];
};

function groupRegistryServers(servers: McpCatalogServer[]): McpRegistryGroup[] {
  const groups = new Map<string, McpRegistryGroup>();
  for (const server of servers) {
    const current = groups.get(server.name);
    if (current) {
      current.servers.push(server);
      if (server.isLatest) {
        current.title = server.title ?? current.title;
        current.description = server.description || current.description;
      }
      continue;
    }
    groups.set(server.name, {
      name: server.name,
      title: server.title,
      description: server.description,
      servers: [server],
    });
  }
  return [...groups.values()];
}

function inputNames(connection: McpConnection): string[] {
  return Object.keys(connection.type === "stdio" ? connection.environment : connection.headers);
}

function inputEntries(
  connection: McpConnection,
): Array<{ name: string; slot: string; value: McpPersistedValue }> {
  const [prefix, values] =
    connection.type === "stdio"
      ? (["env", connection.environment] as const)
      : (["header", connection.headers] as const);
  return Object.entries(values).map(([name, value]) => ({
    name,
    slot: `${prefix}.${name}`,
    value,
  }));
}

function redactedValue(value: McpPersistedValue): McpPersistedValue {
  return value.type === "literal" ? { type: "literal", value: "" } : value;
}

function disabledUntrusted(server: McpServerConfig): McpServerConfig {
  const connection: McpConnection =
    server.connection.type === "stdio"
      ? {
          ...server.connection,
          environment: Object.fromEntries(
            Object.entries(server.connection.environment).map(([key, value]) => [
              key,
              redactedValue(value),
            ]),
          ),
        }
      : {
          ...server.connection,
          headers: Object.fromEntries(
            Object.entries(server.connection.headers).map(([key, value]) => [
              key,
              redactedValue(value),
            ]),
          ),
        };
  return { ...server, connection, trust: {}, enabled: false };
}

function discoveredRecord(row: McpDiscoveryRow): McpServerConfig {
  const values = Object.fromEntries(
    row.envKeys.map((key) => [key, { type: "literal", value: "" } satisfies McpPersistedValue]),
  );
  const connection: McpConnection = /^https?:\/\//i.test(row.command)
    ? {
        type: "streamableHttp",
        url: row.command,
        allowLocalhost: false,
        headers: values,
        auth: { type: "none" },
      }
    : {
        type: "stdio",
        command: row.command,
        args: row.args,
        environment: values,
      };
  return {
    schemaVersion: 1,
    id: row.id,
    displayName: row.displayName,
    source: { type: "imported", dialect: row.source, sourcePath: row.sourcePath },
    install: { type: "external" },
    connection,
    trust: {},
    policy: defaultPolicy(),
    enabled: false,
  };
}

function sourcePath(server: McpServerConfig): string | undefined {
  return server.source.type === "imported" ? server.source.sourcePath : undefined;
}

function reportSummary(report: McpProbeReport): string {
  if (report.state === "failed") {
    return `${titleCase(report.stage)} · ${report.error ?? "Probe failed"}`;
  }
  const server = report.serverName
    ? ` · ${report.serverName}${report.serverVersion ? ` ${report.serverVersion}` : ""}`
    : "";
  return [
    "Ready",
    report.protocolVersion ? `protocol ${report.protocolVersion}` : undefined,
    `${report.capabilities.length} capabilities`,
    `${report.toolNames.length} tools`,
    `${report.durationMs} ms${server}`,
  ]
    .filter(Boolean)
    .join(" · ");
}

function needsConfig(server: McpServerConfig): boolean {
  if (server.connection.type === "stdio" && !server.connection.command.trim()) return true;
  return inputEntries(server.connection).some(
    ({ value }) => value.type === "literal" && !value.value.trim(),
  );
}

function sourceTarget(server: McpServerConfig): { kind: "path" | "url"; value: string } | null {
  if (server.source.type === "imported") {
    return { kind: "path", value: server.source.sourcePath };
  }
  if (server.source.type === "registry") {
    return { kind: "url", value: server.source.catalogBaseUrl };
  }
  if (server.connection.type !== "stdio") {
    return { kind: "url", value: server.connection.url };
  }
  return null;
}

export function McpSection() {
  const ctx = useAppContext();
  const [draft, setDraft] = createSignal(emptyServer());
  const [showAddForm, setShowAddForm] = createSignal(false);
  const [importText, setImportText] = createSignal("");
  const [importPreview, setImportPreview] = createSignal<McpConfigImport | null>(null);
  const [previewContent, setPreviewContent] = createSignal("");
  const [importStatus, setImportStatus] = createSignal("");
  const [exportText, setExportText] = createSignal("");
  const [exportStatus, setExportStatus] = createSignal("");
  const [registryQuery, setRegistryQuery] = createSignal("");
  const [registryPage, setRegistryPage] = createSignal<McpCatalogPage | null>(null);
  const [registryVersions, setRegistryVersions] = createSignal<
    Record<string, McpCatalogServer[]>
  >({});
  const [registryStatus, setRegistryStatus] = createSignal("");
  const [installPreview, setInstallPreview] = createSignal<McpInstallPreview | null>(null);
  const [installOperationId, setInstallOperationId] = createSignal("");
  const [probeResults, setProbeResults] = createSignal<Record<string, string>>({});
  const [lifecycle, setLifecycle] = createSignal<Record<string, McpLifecycleRecord>>({});
  const [secretDrafts, setSecretDrafts] = createSignal<Record<string, string>>({});
  const [secretStatuses, setSecretStatuses] = createSignal<Record<string, string>>({});
  const [oauthStatuses, setOauthStatuses] = createSignal<Record<string, McpOAuthStatus>>({});
  const oauthPollTimers = new Map<string, number>();
  const loadedOauthStatuses = new Set<string>();

  const draftCommand = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? connection.command : "";
  };
  const draftArgs = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? connection.args.join(", ") : "";
  };
  const draftEnvironmentNames = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? Object.keys(connection.environment).join(", ") : "";
  };
  const draftRemoteHeaders = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? "" : Object.keys(connection.headers).join(", ");
  };
  const draftRemoteAuth = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? null : connection.auth;
  };
  const draftOauthClientId = () => {
    const auth = draftRemoteAuth();
    return auth?.type === "oauth" ? auth.clientId : "";
  };
  const draftOauthScopes = () => {
    const auth = draftRemoteAuth();
    return auth?.type === "oauth" ? auth.scopes.join(", ") : "";
  };
  const draftRemoteUrl = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? "" : connection.url;
  };
  const draftAllowsLocalhost = () => {
    const connection = draft().connection;
    return connection.type === "stdio" ? false : (connection.allowLocalhost ?? false);
  };
  const servers = () => ctx.settings().mcp?.servers ?? [];
  const discoverExternal = () => ctx.settings().mcp?.discoverExternal ?? false;
  const discoveredCount = () => (discoverExternal() ? ctx.discoveredMcp().length : 0);
  const configuredCount = () => servers().length;
  const connections = createMemo<McpConnectionRow[]>(() => {
    const byId = new Map<string, McpConnectionRow>();
    if (discoverExternal()) {
      for (const row of ctx.discoveredMcp()) {
        byId.set(row.id, { kind: "discovered", id: row.id, row });
      }
    }
    servers().forEach((server, index) => {
      byId.set(server.id, { kind: "configured", id: server.id, server, index });
    });
    return [...byId.values()];
  });
  const registryGroups = createMemo(() => groupRegistryServers(registryPage()?.servers ?? []));

  const registryVersionsFor = (group: McpRegistryGroup): McpCatalogServer[] => {
    const byVersion = new Map<string, McpCatalogServer>();
    for (const server of [...group.servers, ...(registryVersions()[group.name] ?? [])]) {
      byVersion.set(server.version, server);
    }
    return [...byVersion.values()];
  };

  const enabledConnectionCount = () => servers().filter((server) => server.enabled).length;

  const updateServer = (
    index: number,
    update: (server: McpServerConfig) => McpServerConfig,
  ) =>
    ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      const current = settings.mcp.servers[index];
      if (current) settings.mcp.servers[index] = update(current);
    });

  const updateConnection = (index: number, connection: McpConnection) =>
    updateServer(index, (server) => ({
      ...server,
      connection,
      trust: {},
      enabled: false,
    }));

  const updatePolicy = (
    index: number,
    patch: Partial<McpServerConfig["policy"]>,
  ) =>
    updateServer(index, (server) => ({
      ...server,
      policy: { ...server.policy, ...patch },
      trust: {},
      enabled: false,
    }));

  const updateStdioConnection = (
    index: number,
    connection: McpConnection,
    patch: Partial<
      Pick<Extract<McpConnection, { type: "stdio" }>, "command" | "args" | "environment">
    >,
  ) => {
    if (connection.type !== "stdio") return Promise.resolve();
    return updateConnection(index, { ...connection, ...patch });
  };

  const moveArgumentInputsToKeychain = (
    index: number,
    connection: Extract<McpConnection, { type: "stdio" }>,
  ) => {
    const names = environmentNamesInArgs(connection.args);
    if (names.length === 0) return Promise.resolve();
    const environment = { ...connection.environment };
    for (const name of names) {
      environment[name] ??= { type: "literal", value: "" };
    }
    return updateStdioConnection(index, connection, {
      args: connection.args.filter((argument) => environmentNamesInArgs([argument]).length === 0),
      environment,
    });
  };

  const updateHttpUrl = (index: number, connection: McpConnection, url: string) => {
    if (connection.type === "stdio") return Promise.resolve();
    return updateConnection(index, { ...connection, url });
  };

  const updateHttpAllowLocalhost = (
    index: number,
    connection: McpConnection,
    allowLocalhost: boolean,
  ) => {
    if (connection.type === "stdio") return Promise.resolve();
    return updateConnection(index, { ...connection, allowLocalhost });
  };

  const updateRemoteAuth = (
    index: number,
    connection: McpConnection,
    auth: Extract<McpConnection, { type: "streamableHttp" | "legacySse" }>["auth"],
  ) => {
    if (connection.type === "stdio") return Promise.resolve();
    return updateConnection(index, { ...connection, auth });
  };

  const updateOauthAuth = (
    index: number,
    connection: McpConnection,
    patch: Partial<Extract<NonNullable<ReturnType<typeof draftRemoteAuth>>, { type: "oauth" }>>,
  ) => {
    if (connection.type === "stdio" || connection.auth.type !== "oauth") {
      return Promise.resolve();
    }
    return updateRemoteAuth(index, connection, { ...connection.auth, ...patch });
  };

  const setOauthStatus = (status: McpOAuthStatus) =>
    setOauthStatuses((current) => ({ ...current, [status.serverId]: status }));

  const setOauthFailure = (serverId: string, error: unknown) =>
    setOauthStatuses((current) => ({
      ...current,
      [serverId]: {
        ...(current[serverId] ?? { serverId, grantedScopes: [] }),
        state: "failed",
        error: error instanceof Error ? error.message : String(error),
      },
    }));

  const syncOauthMetadata = async (status: McpOAuthStatus) => {
    await ctx.updateSettings((settings) => {
      const server = settings.mcp?.servers.find((candidate) => candidate.id === status.serverId);
      if (!server || server.connection.type === "stdio" || server.connection.auth.type !== "oauth") {
        return;
      }
      server.connection.auth = {
        ...server.connection.auth,
        clientId: status.clientId ?? server.connection.auth.clientId,
        issuer: status.issuer ?? server.connection.auth.issuer,
        credentialRef:
          status.state === "disconnected"
            ? undefined
            : (status.credentialRef ?? server.connection.auth.credentialRef),
        scopes:
          status.grantedScopes.length > 0
            ? status.grantedScopes
            : server.connection.auth.scopes,
      };
      server.trust = {};
      server.enabled = false;
    });
  };

  const pollOauth = (serverId: string) => {
    const existing = oauthPollTimers.get(serverId);
    if (existing !== undefined) window.clearTimeout(existing);
    const timer = window.setTimeout(async () => {
      oauthPollTimers.delete(serverId);
      try {
        const status = await mcpOAuthStatus(serverId);
        setOauthStatus(status);
        if (status.state === "connecting") {
          pollOauth(serverId);
        } else {
          await syncOauthMetadata(status);
        }
      } catch (error) {
        setOauthFailure(serverId, error);
      }
    }, 750);
    oauthPollTimers.set(serverId, timer);
  };

  const connectOauth = async (server: McpServerConfig) => {
    if (server.connection.type === "stdio" || server.connection.auth.type !== "oauth") return;
    try {
      await ctx.updateSettings(() => {});
      const status = await startMcpOAuth(server.id, server.connection.auth.scopes);
      setOauthStatus(status);
      await syncOauthMetadata(status);
      if (status.state === "connecting") pollOauth(server.id);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setOauthFailure(
        server.id,
        new Error(`OAuth setup failed before browser launch: ${message}`),
      );
    }
  };

  const authenticateOauth = async (
    connection: Extract<McpConnectionRow, { kind: "configured" }>,
  ) => {
    const remote = connection.server.connection;
    if (remote.type === "stdio") return;
    const oauthServer: McpServerConfig = {
      ...connection.server,
      connection: {
        ...remote,
        auth:
          remote.auth.type === "oauth"
            ? remote.auth
            : { type: "oauth", clientId: "", scopes: [] },
      },
      trust: {},
      enabled: false,
    };
    await updateServer(connection.index, () => oauthServer);
    await connectOauth(oauthServer);
  };

  const refreshOauth = async (serverId: string) => {
    try {
      const status = await refreshMcpOAuth(serverId);
      setOauthStatus(status);
      await syncOauthMetadata(status);
    } catch (error) {
      setOauthFailure(serverId, error);
    }
  };

  const disconnectOauth = async (serverId: string) => {
    const timer = oauthPollTimers.get(serverId);
    if (timer !== undefined) window.clearTimeout(timer);
    oauthPollTimers.delete(serverId);
    try {
      const status = await disconnectMcpOAuth(serverId);
      setOauthStatus(status);
      await syncOauthMetadata(status);
    } catch (error) {
      setOauthFailure(serverId, error);
    }
  };

  createEffect(() => {
    for (const server of servers()) {
      if (
        server.connection.type !== "stdio" &&
        server.connection.auth.type === "oauth" &&
        !loadedOauthStatuses.has(server.id)
      ) {
        loadedOauthStatuses.add(server.id);
        void mcpOAuthStatus(server.id).then(setOauthStatus).catch((error) => {
          setOauthFailure(server.id, error);
        });
      }
    }
  });

  onCleanup(() => {
    for (const timer of oauthPollTimers.values()) window.clearTimeout(timer);
    oauthPollTimers.clear();
  });

  const removeServer = (index: number) =>
    ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.servers.splice(index, 1);
    });

  const toggleDiscoverExternal = async (enabled: boolean) => {
    await ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.discoverExternal = enabled;
    });
    await ctx.refreshDiscoveredMcp();
  };

  const disableAll = async () => {
    await ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.servers = settings.mcp.servers.map((server) => ({
        ...server,
        enabled: false,
      }));
      settings.mcp.discoverExternal = false;
      settings.mcp.disabledDiscoveredIds = [
        ...new Set([
          ...(settings.mcp.disabledDiscoveredIds ?? []),
          ...ctx.discoveredMcp().map((row) => row.id),
        ]),
      ];
    });
    setProbeResults({});
    await ctx.refreshDiscoveredMcp();
  };

  const copyDiscoveredToSettings = (row: McpDiscoveryRow) =>
    ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      if (settings.mcp.servers.some((server) => server.id === row.id)) return;
      settings.mcp.servers.push(discoveredRecord(row));
    });

  const addServer = () => {
    const next = draft();
    const connection = next.connection;
    if (
      !next.id.trim() ||
      (connection.type === "stdio" ? !connection.command.trim() : !connection.url.trim())
    )
      return;
    void ctx.updateSettings((settings) => {
      settings.mcp ??= { servers: [] };
      settings.mcp.servers.push({
        ...next,
        id: next.id.trim(),
        displayName: next.displayName.trim() || next.id.trim(),
        connection:
          connection.type === "stdio"
            ? {
                ...connection,
                command: connection.command.trim(),
                args: connection.args.filter(Boolean),
              }
            : { ...connection, url: connection.url.trim() },
        trust: {},
        enabled: false,
      });
    });
    setDraft(emptyServer());
    setShowAddForm(false);
  };

  const previewImportConfig = async () => {
    const content = importText().trim();
    if (!content) {
      setImportStatus("Paste an MCP JSON config first.");
      return;
    }
    setImportStatus("Reading config…");
    setImportPreview(null);
    try {
      const imported = await importMcpConfig(content);
      setImportPreview(imported);
      setPreviewContent(content);
      const suffix = imported.diagnostics.length
        ? ` · ${imported.diagnostics.length} diagnostic${imported.diagnostics.length === 1 ? "" : "s"}`
        : "";
      setImportStatus(
        `Previewed ${imported.servers.length} MCP server${imported.servers.length === 1 ? "" : "s"}${suffix}. Nothing saved yet.`,
      );
    } catch (error) {
      setImportStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const applyImport = async () => {
    const preview = importPreview();
    if (!preview?.servers.length) return;
    setImportStatus("Applying config and securing inputs…");
    try {
      const applied = await applyMcpConfig(previewContent());
      await ctx.updateSettings((settings) => {
        settings.mcp ??= { servers: [] };
        for (const candidate of applied.servers.map(disabledUntrusted)) {
          const index = settings.mcp.servers.findIndex((server) => server.id === candidate.id);
          if (index >= 0) settings.mcp.servers[index] = candidate;
          else settings.mcp.servers.push(candidate);
        }
      });
      setImportStatus(
        `Applied ${applied.servers.length} MCP server${applied.servers.length === 1 ? "" : "s"} as disabled and untrusted. Inputs stored in local mcp-secrets.json.`,
      );
      setImportPreview(null);
      setPreviewContent("");
      setImportText("");
    } catch (error) {
      setImportStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const generateExport = async () => {
    setExportStatus("Generating secret-free config…");
    try {
      const content = await exportMcpConfig();
      setExportText(content);
      setExportStatus("Export ready. Secret refs remain; values are excluded.");
    } catch (error) {
      setExportStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const copyExport = async () => {
    const content = exportText();
    if (!content) return;
    try {
      await navigator.clipboard.writeText(content);
      setExportStatus("Copied secret-free MCP config.");
    } catch {
      setExportStatus("Copy failed. Select the export text manually.");
    }
  };

  const probeServer = async (connection: Extract<McpConnectionRow, { kind: "configured" }>) => {
    setProbeResults((current) => ({ ...current, [connection.id]: "Testing…" }));
    const attemptCount = (lifecycle()[connection.id]?.attemptCount ?? 0) + 1;
    setLifecycle((current) => ({
      ...current,
      [connection.id]: {
        state: "Connecting",
        summary: "Connecting, negotiating capabilities, and listing tools…",
        attemptCount,
      },
    }));
    try {
      const result = await probeMcpServer(connection.server, sourcePath(connection.server));
      const approved =
        result.report.state === "ready" && Boolean(result.server.trust.approvedFingerprint);
      await updateServer(connection.index, () =>
        approved ? { ...result.server, enabled: false } : disabledUntrusted(result.server),
      );
      setProbeResults((current) => ({
        ...current,
        [connection.id]: approved
          ? reportSummary(result.report)
          : result.report.state === "failed"
            ? reportSummary(result.report)
            : "Approval failed · backend returned no fingerprint",
      }));
      setLifecycle((current) => ({
        ...current,
        [connection.id]: {
          state: approved
            ? "Ready"
            : result.report.authRequired
              ? "Auth required"
              : "Failed",
          summary: reportSummary(result.report),
          report: result.report,
          checkedAt: new Date().toISOString(),
          attemptCount,
        },
      }));
    } catch (error) {
      await updateServer(connection.index, (server) => disabledUntrusted(server));
      setProbeResults((current) => ({
        ...current,
        [connection.id]: `Probe failed · ${error instanceof Error ? error.message : String(error)}`,
      }));
      setLifecycle((current) => ({
        ...current,
        [connection.id]: {
          state: "Failed",
          summary: `Probe failed · ${error instanceof Error ? error.message : String(error)}`,
          checkedAt: new Date().toISOString(),
          attemptCount,
        },
      }));
    }
  };

  const lifecycleFor = (server: McpServerConfig): McpLifecycleRecord => {
    const oauthStatus = oauthStatuses()[server.id];
    const current = lifecycle()[server.id];
    if (oauthStatus?.state === "connecting") {
      return {
        state: "Connecting",
        summary: "Complete OAuth sign-in in your browser.",
        report: current?.report,
        checkedAt: current?.checkedAt,
        attemptCount: current?.attemptCount ?? 0,
      };
    }
    if (oauthStatus?.state === "connected" && current?.report?.authRequired) {
      return {
        ...current,
        state: "Auth required",
        summary: "OAuth connected. Retry to approve and list tools.",
      };
    }
    if (oauthStatus?.state === "reauthorizationRequired") {
      return {
        state: "Auth required",
        summary: oauthStatus.error ?? "Reconnect OAuth before testing.",
        attemptCount: lifecycle()[server.id]?.attemptCount ?? 0,
      };
    }
    if (oauthStatus?.state === "failed") {
      return {
        state: "Auth required",
        summary: oauthStatus.error ?? "OAuth setup failed.",
        report: current?.report,
        checkedAt: current?.checkedAt,
        attemptCount: current?.attemptCount ?? 0,
      };
    }
    if (
      current?.state === "Connecting" ||
      current?.state === "Failed" ||
      current?.state === "Auth required"
    )
      return current;
    if (server.install.type !== "external" && !server.installHistory) {
      return { state: "Not installed", summary: "Install this exact package first.", attemptCount: 0 };
    }
    if (needsConfig(server)) {
      return { state: "Needs config", summary: "Store each required input before testing.", attemptCount: 0 };
    }
    if (!server.enabled) {
      return {
        state: "Disabled",
        summary:
          current?.state === "Ready"
            ? `Last test ready. Enable when needed. ${current.summary}`
            : "Server does not start with runs.",
        report: current?.report,
        checkedAt: current?.checkedAt,
        attemptCount: current?.attemptCount ?? 0,
      };
    }
    if (current) return current;
    return { state: "Ready", summary: "Enabled from an approved probe.", attemptCount: 0 };
  };

  const probeActionLabel = (server: McpServerConfig) => {
    if (oauthStatuses()[server.id]?.state === "connecting") return "Authenticating…";
    const state = lifecycleFor(server).state;
    if (state === "Failed" || state === "Degraded" || state === "Auth required") return "Retry";
    if (server.enabled) return "Restart & Test";
    return server.trust.approvedFingerprint ? "Test again" : "Approve & Test";
  };

  const copyDiagnostics = async (server: McpServerConfig) => {
    const record = lifecycleFor(server);
    const diagnostics = JSON.stringify(
      {
        serverId: server.id,
        displayName: server.displayName,
        state: record.state,
        summary: record.summary,
        checkedAt: record.checkedAt,
        attemptCount: record.attemptCount,
        report: record.report,
      },
      null,
      2,
    );
    try {
      await navigator.clipboard.writeText(diagnostics);
      setProbeResults((current) => ({ ...current, [server.id]: "Diagnostics copied." }));
    } catch {
      setProbeResults((current) => ({ ...current, [server.id]: "Copy diagnostics failed." }));
    }
  };

  const openSource = async (server: McpServerConfig) => {
    const target = sourceTarget(server);
    if (!target) return;
    if (target.kind === "path") await openLocalPath(target.value);
    else await openExternalUrl(target.value);
  };

  const saveInputSecret = async (
    row: Extract<McpConnectionRow, { kind: "configured" }>,
    name: string,
    slot: string,
  ) => {
    const draftKey = `${row.id}:${slot}`;
    const value = secretDrafts()[draftKey] ?? "";
    if (!value) {
      setSecretStatuses((current) => ({ ...current, [draftKey]: "Enter a value first." }));
      return;
    }
    setSecretStatuses((current) => ({ ...current, [draftKey]: "Saving securely…" }));
    try {
      const secretRef = await saveMcpSecret(row.id, slot, value);
      const connection = row.server.connection;
      const nextConnection: McpConnection =
        connection.type === "stdio"
          ? {
              ...connection,
              environment: {
                ...connection.environment,
                [name]: { type: "secret", secretRef },
              },
            }
          : {
              ...connection,
              headers: {
                ...connection.headers,
                [name]: { type: "secret", secretRef },
              },
            };
      await updateConnection(row.index, nextConnection);
      setSecretDrafts((current) => ({ ...current, [draftKey]: "" }));
      setSecretStatuses((current) => ({ ...current, [draftKey]: "Stored in mcp-secrets.json." }));
    } catch (error) {
      setSecretStatuses((current) => ({
        ...current,
        [draftKey]: error instanceof Error ? error.message : String(error),
      }));
    }
  };

  const clearInputSecret = async (
    row: Extract<McpConnectionRow, { kind: "configured" }>,
    name: string,
    slot: string,
    secretRef: string,
  ) => {
    const draftKey = `${row.id}:${slot}`;
    try {
      await deleteMcpSecret(secretRef);
      const connection = row.server.connection;
      const nextConnection: McpConnection =
        connection.type === "stdio"
          ? {
              ...connection,
              environment: {
                ...connection.environment,
                [name]: { type: "literal", value: "" },
              },
            }
          : {
              ...connection,
              headers: {
                ...connection.headers,
                [name]: { type: "literal", value: "" },
              },
            };
      await updateConnection(row.index, nextConnection);
      setSecretStatuses((current) => ({ ...current, [draftKey]: "Removed from mcp-secrets.json." }));
    } catch (error) {
      setSecretStatuses((current) => ({
        ...current,
        [draftKey]: error instanceof Error ? error.message : String(error),
      }));
    }
  };

  const searchRegistry = async (cursor?: string) => {
    setRegistryStatus("Searching Registry…");
    try {
      const page = await searchMcpRegistry(registryQuery().trim(), cursor);
      setRegistryPage((current) =>
        cursor && current
          ? { ...page, servers: [...current.servers, ...page.servers] }
          : page,
      );
      setRegistryStatus(
        `${page.catalogLabel} Registry · ${page.count ?? page.servers.length} result${(page.count ?? page.servers.length) === 1 ? "" : "s"}. Registry listing does not imply safety.`,
      );
    } catch (error) {
      setRegistryStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const loadRegistryVersions = async (serverName: string) => {
    setRegistryStatus("Loading exact versions…");
    try {
      const page = await listMcpRegistryVersions(serverName);
      setRegistryVersions((current) => ({ ...current, [serverName]: page.servers }));
      setRegistryStatus(`Loaded ${page.servers.length} exact version${page.servers.length === 1 ? "" : "s"}.`);
    } catch (error) {
      setRegistryStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const reviewRegistryPackage = async (server: McpCatalogServer, packageIndex: number) => {
    setRegistryStatus("Building exact install plan…");
    try {
      const preview = await previewMcpRegistryInstall(server.name, server.version, packageIndex);
      setInstallPreview(preview);
      setRegistryStatus("Review the exact command, provenance, inputs, and update diff.");
    } catch (error) {
      setRegistryStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const reviewRegistryRemote = async (server: McpCatalogServer, remoteIndex: number) => {
    setRegistryStatus("Building remote connection plan…");
    try {
      const preview = await previewMcpRegistryRemote(server.name, server.version, remoteIndex);
      setInstallPreview(preview);
      setRegistryStatus("Review the endpoint, provenance, inputs, and security defaults.");
    } catch (error) {
      setRegistryStatus(error instanceof Error ? error.message : String(error));
    }
  };

  const installReviewedPackage = async () => {
    const preview = installPreview();
    if (!preview) return;
    if (!preview.requiresInstall) {
      await ctx.updateSettings((settings) => {
        settings.mcp ??= { servers: [] };
        const index = settings.mcp.servers.findIndex((server) => server.id === preview.server.id);
        if (index >= 0) settings.mcp.servers[index] = preview.server;
        else settings.mcp.servers.push(preview.server);
      });
      setRegistryStatus("Remote added. Store required inputs, then run Approve & Test.");
      setInstallPreview(null);
      return;
    }
    const operationId = globalThis.crypto.randomUUID();
    setInstallOperationId(operationId);
    setRegistryStatus("Installing exact package…");
    try {
      const result = await installMcpPackage(operationId, preview.server);
      const installed = result.server;
      if (installed) {
        await ctx.updateSettings((settings) => {
          settings.mcp ??= { servers: [] };
          const index = settings.mcp.servers.findIndex((server) => server.id === installed.id);
          if (index >= 0) settings.mcp.servers[index] = installed;
          else settings.mcp.servers.push(installed);
        });
      }
      setRegistryStatus(
        result.state === "succeeded"
          ? "Installed. Run Approve & Test before enabling."
          : `Install ${result.state}. ${result.stderrTail || result.stdoutTail}`,
      );
      if (result.state === "succeeded") setInstallPreview(null);
    } catch (error) {
      setRegistryStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setInstallOperationId("");
    }
  };

  const cancelInstall = async () => {
    const operationId = installOperationId();
    if (!operationId) return;
    await cancelMcpInstall(operationId);
    setRegistryStatus("Cancelling install…");
  };

  const rollbackInstalled = async (
    connection: Extract<McpConnectionRow, { kind: "configured" }>,
  ) => {
    setProbeResults((current) => ({ ...current, [connection.id]: "Rolling back…" }));
    try {
      const rolledBack = await rollbackMcpInstall(connection.id);
      await updateServer(connection.index, () => rolledBack);
      setProbeResults((current) => ({
        ...current,
        [connection.id]: "Rolled back. Run Approve & Test before enabling.",
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
        description="Review, approve, and enable MCP servers. Inputs and OAuth tokens stay in app data/openflow/mcp-secrets.json."
      />

      <div class="mcp-cards">
        <section class="mcp-card mcp-card--management" aria-labelledby="mcp-connections-heading">
          <div class="mcp-card-header mcp-card-header--actions">
            <div>
              <h4 id="mcp-connections-heading" class="mcp-card-title">
                Connections
              </h4>
              <p class="mcp-card-copy">
                {connections().length} available · {enabledConnectionCount()} enabled
              </p>
            </div>
            <Show when={enabledConnectionCount() > 0 || discoverExternal()}>
              <Button variant="secondary" ghost onClick={() => void disableAll()}>
                Disable all
              </Button>
            </Show>
          </div>
          <Show
            when={connections().length > 0}
            fallback={<div class="mcp-empty-state">No MCP servers yet.</div>}
          >
            <div class="mcp-server-list">
              <For each={connections()}>
                {(connection) => (
                  <div class="mcp-server-row mcp-connection-row">
                    <div class="mcp-server-row-main">
                      <strong class="mcp-server-name">
                        {connection.kind === "configured"
                          ? connection.server.displayName
                          : connection.row.displayName}
                      </strong>
                      <Show when={connection.kind === "configured"}>
                        {connection.kind === "configured" ? (
                          <>
                            <p class="mcp-server-meta">
                              {sourceLabel(connection.server)} ·{" "}
                              {transportLabel(connection.server.connection)} ·{" "}
                              {connection.server.trust.approvedFingerprint
                                ? "Approved"
                                : "Untrusted"}
                            </p>
                            <p class="mcp-server-meta">
                              {connectionSummary(connection.server.connection)}
                            </p>
                            <div
                              class="mcp-lifecycle-status"
                              data-state={lifecycleFor(connection.server).state.toLowerCase().replace(/ /g, "-")}
                              role="status"
                            >
                              <strong>{lifecycleFor(connection.server).state}</strong>
                              <span>{lifecycleFor(connection.server).summary}</span>
                              <Show when={lifecycleFor(connection.server).checkedAt}>
                                <small>
                                  Checked {formatCheckedAt(lifecycleFor(connection.server).checkedAt ?? "")} ·
                                  attempt {lifecycleFor(connection.server).attemptCount}
                                </small>
                              </Show>
                            </div>
                            <Show when={inputNames(connection.server.connection).length > 0}>
                              <p class="mcp-server-meta">
                                Inputs: {inputNames(connection.server.connection).join(", ")}
                              </p>
                            </Show>
                            <details class="mcp-server-details">
                              <summary>Configuration &amp; permissions</summary>
                              <div class="mcp-configured-fields">
                              <label>
                                <span>Name</span>
                                <input
                                  class="text-input"
                                  value={connection.server.displayName}
                                  onInput={(event) =>
                                    void updateServer(connection.index, (server) => ({
                                      ...server,
                                      displayName: event.currentTarget.value,
                                    }))
                                  }
                                />
                              </label>
                              <Show when={connection.server.connection.type === "stdio"}>
                                {connection.server.connection.type === "stdio" ? (
                                  <>
                                    <label>
                                      <span>Command</span>
                                      <input
                                        class="text-input"
                                        value={connection.server.connection.command}
                                        onInput={(event) =>
                                          void updateStdioConnection(
                                            connection.index,
                                            connection.server.connection,
                                            {
                                              command: event.currentTarget.value,
                                            },
                                          )
                                        }
                                      />
                                    </label>
                                    <label>
                                      <span>Arguments (comma-separated)</span>
                                      <input
                                        class="text-input"
                                        type={
                                          environmentNamesInArgs(
                                            connection.server.connection.args,
                                          ).length > 0
                                            ? "password"
                                            : "text"
                                        }
                                        aria-invalid={
                                          environmentNamesInArgs(
                                            connection.server.connection.args,
                                          ).length > 0
                                        }
                                        value={connection.server.connection.args.join(", ")}
                                        onInput={(event) =>
                                          void updateStdioConnection(
                                            connection.index,
                                            connection.server.connection,
                                            {
                                              args: event.currentTarget.value
                                                .split(",")
                                                .map((part) => part.trim())
                                                .filter(Boolean),
                                            },
                                          )
                                        }
                                      />
                                    </label>
                                    <Show
                                      when={environmentNamesInArgs(
                                        connection.server.connection.args,
                                      )}
                                    >
                                      {(names) => (
                                        <Show when={names().length > 0}>
                                          <div class="mcp-argument-warning" role="alert">
                                            <div>
                                              <strong>Credentials are in Arguments</strong>
                                              <span>
                                                {names().join(", ")} must use secure input slots, not
                                                CLI args.
                                              </span>
                                            </div>
                                            <Button
                                              variant="secondary"
                                              size="small"
                                              onClick={() => {
                                                const stdio = connection.server.connection;
                                                if (stdio.type === "stdio") {
                                                  void moveArgumentInputsToKeychain(
                                                    connection.index,
                                                    stdio,
                                                  );
                                                }
                                              }}
                                            >
                                              Move to secure inputs
                                            </Button>
                                          </div>
                                        </Show>
                                      )}
                                    </Show>
                                  </>
                                ) : null}
                              </Show>
                              <Show when={connection.server.connection.type !== "stdio"}>
                                {connection.server.connection.type !== "stdio" ? (
                                  <>
                                    <label>
                                      <span>URL</span>
                                      <input
                                        class="text-input"
                                        value={connection.server.connection.url}
                                        onInput={(event) =>
                                          void updateHttpUrl(
                                            connection.index,
                                            connection.server.connection,
                                            event.currentTarget.value,
                                          )
                                        }
                                      />
                                    </label>
                                    <label class="checkbox-row">
                                      <input
                                        type="checkbox"
                                        checked={
                                          connection.server.connection.allowLocalhost ?? false
                                      }
                                      onChange={(event) =>
                                        void updateHttpAllowLocalhost(
                                          connection.index,
                                          connection.server.connection,
                                          event.currentTarget.checked,
                                        )
                                      }
                                      />
                                      <span>Allow explicit localhost development endpoint</span>
                                    </label>
                                    <label>
                                      <span>Authentication</span>
                                      <TextSelect
                                        value={connection.server.connection.auth.type}
                                        options={
                                          connection.server.connection.auth.type === "static"
                                            ? MCP_AUTH_OPTIONS_WITH_STATIC
                                            : MCP_AUTH_OPTIONS
                                        }
                                        onChange={(event) => {
                                          const type = event.currentTarget.value;
                                          if (type === "static") return;
                                          void updateRemoteAuth(
                                            connection.index,
                                            connection.server.connection,
                                            type === "oauth"
                                              ? {
                                                  type: "oauth",
                                                  clientId: "",
                                                  scopes: [],
                                                }
                                              : { type: "none" },
                                          );
                                        }}
                                      />
                                    </label>
                                    <Show when={connection.server.connection.auth.type === "oauth"}>
                                      {connection.server.connection.auth.type === "oauth" ? (
                                        <div class="mcp-import-panel">
                                          <label>
                                            <span>Client ID (optional)</span>
                                            <input
                                              class="text-input"
                                              value={connection.server.connection.auth.clientId}
                                              placeholder="Leave blank for dynamic registration"
                                              onInput={(event) =>
                                                void updateOauthAuth(
                                                  connection.index,
                                                  connection.server.connection,
                                                  {
                                                    clientId: event.currentTarget.value,
                                                    credentialRef: undefined,
                                                    issuer: undefined,
                                                  },
                                                )
                                              }
                                            />
                                          </label>
                                          <label>
                                            <span>Requested scopes</span>
                                            <input
                                              class="text-input"
                                              value={connection.server.connection.auth.scopes.join(", ")}
                                              placeholder="Discover from server"
                                              onInput={(event) =>
                                                void updateOauthAuth(
                                                  connection.index,
                                                  connection.server.connection,
                                                  {
                                                    scopes: event.currentTarget.value
                                                      .split(/[\s,]+/)
                                                      .map((scope) => scope.trim())
                                                      .filter(Boolean),
                                                    credentialRef: undefined,
                                                    issuer: undefined,
                                                  },
                                                )
                                              }
                                            />
                                          </label>
                                          <p class="mcp-probe-status" role="status" aria-live="polite">
                                            OAuth: {oauthStatuses()[connection.id]?.state ?? "disconnected"}
                                            {oauthStatuses()[connection.id]?.issuer
                                              ? ` · ${oauthStatuses()[connection.id]?.issuer}`
                                              : ""}
                                            {oauthStatuses()[connection.id]?.grantedScopes.length
                                              ? ` · ${oauthStatuses()[connection.id]?.grantedScopes.join(", ")}`
                                              : ""}
                                          </p>
                                          <Show when={oauthStatuses()[connection.id]?.error}>
                                            <p class="mcp-probe-status">
                                              {oauthStatuses()[connection.id]?.error}
                                            </p>
                                          </Show>
                                          <Button
                                            variant="secondary"
                                            disabled={oauthStatuses()[connection.id]?.state === "connecting"}
                                            onClick={() => void connectOauth(connection.server)}
                                          >
                                            {oauthStatuses()[connection.id]?.state === "connected"
                                              ? "Re-authenticate"
                                              : "Connect OAuth"}
                                          </Button>
                                          <Show when={oauthStatuses()[connection.id]?.state === "connected"}>
                                            <Button
                                              variant="secondary"
                                              onClick={() => void refreshOauth(connection.id)}
                                            >
                                              Refresh token
                                            </Button>
                                          </Show>
                                          <Show
                                            when={
                                              connection.server.connection.auth.credentialRef ||
                                              oauthStatuses()[connection.id]?.state === "connecting" ||
                                              oauthStatuses()[connection.id]?.state === "connected"
                                            }
                                          >
                                            <Button
                                              variant="secondary"
                                              ghost
                                              onClick={() => void disconnectOauth(connection.id)}
                                            >
                                              Disconnect OAuth
                                            </Button>
                                          </Show>
                                        </div>
                                      ) : null}
                                    </Show>
                                  </>
                                ) : null}
                              </Show>
                              <div class="mcp-import-panel">
                                <strong>Server policy</strong>
                                <p class="mcp-server-meta">
                                  Default deny. Any change revokes trust; run Approve &amp; Test again.
                                </p>
                                <label>
                                  <span>Tool access tier</span>
                                  <TextSelect
                                    value={connection.server.policy.defaultToolAccess}
                                    options={TOOL_ACCESS_OPTIONS}
                                    onChange={(event) =>
                                      void updatePolicy(connection.index, {
                                        defaultToolAccess: event.currentTarget.value as "read" | "write",
                                      })
                                    }
                                  />
                                </label>
                                <label>
                                  <span>Tool concurrency</span>
                                  <TextSelect
                                    value={connection.server.policy.defaultToolConcurrency}
                                    options={TOOL_CONCURRENCY_OPTIONS}
                                    onChange={(event) =>
                                      void updatePolicy(connection.index, {
                                        defaultToolConcurrency: event.currentTarget.value as "shared" | "exclusive",
                                      })
                                    }
                                  />
                                </label>
                                <Show
                                  when={(lifecycleFor(connection.server).report?.toolNames.length ?? 0) > 0}
                                >
                                  <div class="mcp-policy-tools">
                                    <span>Enabled tools</span>
                                    <For each={lifecycleFor(connection.server).report?.toolNames ?? []}>
                                      {(toolName) => {
                                        const checked = () =>
                                          connection.server.policy.enabledTools == null ||
                                          connection.server.policy.enabledTools.includes(toolName);
                                        return (
                                          <label class="checkbox-row">
                                            <input
                                              type="checkbox"
                                              checked={checked()}
                                              onChange={(event) => {
                                                const available =
                                                  lifecycleFor(connection.server).report?.toolNames ?? [];
                                                const enabled = new Set(
                                                  connection.server.policy.enabledTools ?? available,
                                                );
                                                if (event.currentTarget.checked) enabled.add(toolName);
                                                else enabled.delete(toolName);
                                                void updatePolicy(connection.index, {
                                                  enabledTools: [...enabled].sort(),
                                                });
                                              }}
                                            />
                                            <span>{toolName}</span>
                                          </label>
                                        );
                                      }}
                                    </For>
                                    <Button
                                      variant="secondary"
                                      ghost
                                      onClick={() =>
                                        void updatePolicy(connection.index, { enabledTools: null })
                                      }
                                    >
                                      Allow all discovered tools
                                    </Button>
                                  </div>
                                </Show>
                                <strong>Server-to-client capabilities</strong>
                                <label class="checkbox-row">
                                  <input
                                    type="checkbox"
                                    checked={connection.server.policy.allowRoots}
                                    onChange={(event) =>
                                      void updatePolicy(connection.index, {
                                        allowRoots: event.currentTarget.checked,
                                      })
                                    }
                                  />
                                  <span>Expose selected project root</span>
                                </label>
                                <label class="checkbox-row">
                                  <input
                                    type="checkbox"
                                    checked={connection.server.policy.allowSampling}
                                    onChange={(event) =>
                                      void updatePolicy(connection.index, {
                                        allowSampling: event.currentTarget.checked,
                                      })
                                    }
                                  />
                                  <span>Allow approved sampling reqs</span>
                                </label>
                                <Show when={connection.server.policy.allowSampling}>
                                  <label>
                                    <span>Sampling reqs / run</span>
                                    <input
                                      class="text-input"
                                      type="number"
                                      min="1"
                                      max="64"
                                      value={connection.server.policy.samplingMaxRequestsPerRun ?? 4}
                                      onChange={(event) =>
                                        void updatePolicy(connection.index, {
                                          samplingMaxRequestsPerRun: Math.min(
                                            64,
                                            Math.max(
                                              1,
                                              Number(event.currentTarget.value) || 4,
                                            ),
                                          ),
                                        })
                                      }
                                    />
                                  </label>
                                  <label>
                                    <span>Tokens / req</span>
                                    <input
                                      class="text-input"
                                      type="number"
                                      min="1"
                                      max="65536"
                                      value={connection.server.policy.samplingMaxTokensPerRequest ?? 4096}
                                      onChange={(event) =>
                                        void updatePolicy(connection.index, {
                                          samplingMaxTokensPerRequest: Math.min(
                                            65536,
                                            Math.max(
                                              1,
                                              Number(event.currentTarget.value) || 4096,
                                            ),
                                          ),
                                        })
                                      }
                                    />
                                  </label>
                                  <label>
                                    <span>Total requested tokens / run</span>
                                    <input
                                      class="text-input"
                                      type="number"
                                      min="1"
                                      max="262144"
                                      value={connection.server.policy.samplingMaxTotalTokensPerRun ?? 8192}
                                      onChange={(event) =>
                                        void updatePolicy(connection.index, {
                                          samplingMaxTotalTokensPerRun: Math.min(
                                            262144,
                                            Math.max(
                                              1,
                                              Number(event.currentTarget.value) || 8192,
                                            ),
                                          ),
                                        })
                                      }
                                    />
                                  </label>
                                </Show>
                                <label class="checkbox-row">
                                  <input
                                    type="checkbox"
                                    checked={connection.server.policy.allowElicitation}
                                    onChange={(event) =>
                                      void updatePolicy(connection.index, {
                                        allowElicitation: event.currentTarget.checked,
                                      })
                                    }
                                  />
                                  <span>Allow approved form/URL elicitation</span>
                                </label>
                                <Show when={connection.server.policy.allowElicitation}>
                                  <label>
                                    <span>Elicitation reqs / run</span>
                                    <input
                                      class="text-input"
                                      type="number"
                                      min="1"
                                      max="64"
                                      value={connection.server.policy.elicitationMaxRequestsPerRun ?? 8}
                                      onChange={(event) =>
                                        void updatePolicy(connection.index, {
                                          elicitationMaxRequestsPerRun: Math.min(
                                            64,
                                            Math.max(
                                              1,
                                              Number(event.currentTarget.value) || 8,
                                            ),
                                          ),
                                        })
                                      }
                                    />
                                  </label>
                                </Show>
                              </div>
                              <For each={inputEntries(connection.server.connection)}>
                                {(input) => {
                                  const draftKey = `${connection.id}:${input.slot}`;
                                  const storedSecretRef =
                                    input.value.type === "secret"
                                      ? input.value.secretRef
                                      : undefined;
                                  return (
                                    <div>
                                      <span>
                                        {input.name} · {input.value.type === "secret" ? "Secret ref" : "Missing"}
                                      </span>
                                      <input
                                        class="text-input"
                                        type="password"
                                        autocomplete="off"
                                        aria-label={`${input.name} secret value`}
                                        placeholder={
                                          input.value.type === "secret"
                                            ? "Enter replacement value"
                                            : "Enter secret value"
                                        }
                                        value={secretDrafts()[draftKey] ?? ""}
                                        onInput={(event) =>
                                          setSecretDrafts((current) => ({
                                            ...current,
                                            [draftKey]: event.currentTarget.value,
                                          }))
                                        }
                                      />
                                      <Button
                                        variant="secondary"
                                        onClick={() =>
                                          void saveInputSecret(
                                            connection,
                                            input.name,
                                            input.slot,
                                          )
                                        }
                                      >
                                        {input.value.type === "secret" ? "Replace secret" : "Save secret"}
                                      </Button>
                                      <Show when={storedSecretRef}>
                                        {storedSecretRef ? (
                                          <Button
                                            variant="secondary"
                                            ghost
                                            onClick={() =>
                                              void clearInputSecret(
                                                connection,
                                                input.name,
                                                input.slot,
                                                storedSecretRef,
                                              )
                                            }
                                          >
                                            Remove secret
                                          </Button>
                                        ) : null}
                                      </Show>
                                      <Show when={secretStatuses()[draftKey]}>
                                        <span role="status" aria-live="polite">
                                          {secretStatuses()[draftKey]}
                                        </span>
                                      </Show>
                                    </div>
                                  );
                                }}
                              </For>
                              </div>
                            </details>
                          </>
                        ) : null}
                      </Show>
                      <Show when={connection.kind === "discovered"}>
                        {connection.kind === "discovered" ? (
                          <>
                            <p class="mcp-server-meta">
                              {titleCase(connection.row.source)} inventory ·{" "}
                              {shortenPath(connection.row.sourcePath)} · Untrusted
                            </p>
                            <p class="mcp-server-meta">
                              {[connection.row.command, ...connection.row.args]
                                .filter(Boolean)
                                .join(" ")}
                            </p>
                            <Show when={connection.row.envKeys.length > 0}>
                              <p class="mcp-server-meta">
                                Inputs: {connection.row.envKeys.join(", ")}
                              </p>
                            </Show>
                          </>
                        ) : null}
                      </Show>
                      <Show
                        when={
                          probeResults()[connection.id] &&
                          (connection.kind === "discovered" ||
                            probeResults()[connection.id] !==
                              lifecycleFor(connection.server).summary)
                        }
                      >
                        <div class="mcp-probe-status" role="status" aria-live="polite">
                          {probeResults()[connection.id]}
                        </div>
                      </Show>
                    </div>
                    <div class="mcp-server-row-actions">
                      <Show when={connection.kind === "configured"}>
                        {connection.kind === "configured" ? (
                          <>
                            <label class="checkbox-row">
                              <input
                                type="checkbox"
                                checked={connection.server.enabled}
                                disabled={!connection.server.trust.approvedFingerprint}
                                onChange={(event) =>
                                  void updateServer(connection.index, (server) => ({
                                    ...server,
                                    enabled: event.currentTarget.checked,
                                  }))
                                }
                              />
                              <span>Enable</span>
                            </label>
                            <Show
                              when={
                                lifecycleFor(connection.server).state === "Auth required" &&
                                connection.server.connection.type !== "stdio" &&
                                oauthStatuses()[connection.id]?.state !== "connected"
                              }
                              fallback={
                                <Button
                                  variant="primary"
                                  size="small"
                                  disabled={lifecycleFor(connection.server).state === "Connecting"}
                                  onClick={() => void probeServer(connection)}
                                >
                                  {probeActionLabel(connection.server)}
                                </Button>
                              }
                            >
                              <Button
                                variant="primary"
                                size="small"
                                onClick={() => void authenticateOauth(connection)}
                              >
                                Authenticate OAuth
                              </Button>
                            </Show>
                            <Show
                              when={
                                lifecycleFor(connection.server).state === "Failed" &&
                                connection.server.connection.type !== "stdio" &&
                                oauthStatuses()[connection.id]?.state !== "connected"
                              }
                            >
                              <Button
                                variant="secondary"
                                size="small"
                                onClick={() => void authenticateOauth(connection)}
                              >
                                Authenticate OAuth
                              </Button>
                            </Show>
                            <Button
                              variant="secondary"
                              size="small"
                              ghost
                              onClick={() => void copyDiagnostics(connection.server)}
                            >
                              Copy diagnostics
                            </Button>
                            <Show when={sourceTarget(connection.server)}>
                              <Button
                                variant="secondary"
                                size="small"
                                ghost
                                onClick={() => void openSource(connection.server)}
                              >
                                Open source
                              </Button>
                            </Show>
                            <Show when={connection.server.installHistory?.previous}>
                              <Button
                                variant="secondary"
                                size="small"
                                ghost
                                onClick={() => void rollbackInstalled(connection)}
                              >
                                Roll back
                              </Button>
                            </Show>
                            <Button
                              variant="secondary"
                              size="small"
                              ghost
                              class="mcp-delete-action"
                              onClick={() => void removeServer(connection.index)}
                            >
                              Delete
                            </Button>
                          </>
                        ) : null}
                      </Show>
                      <Show when={connection.kind === "discovered"}>
                        {connection.kind === "discovered" ? (
                          <Button
                            variant="secondary"
                            size="small"
                            onClick={() => void copyDiscoveredToSettings(connection.row)}
                          >
                            Customize
                          </Button>
                        ) : null}
                      </Show>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </section>

        <section class="mcp-card mcp-card--composer" aria-labelledby="mcp-registry-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-registry-heading" class="mcp-card-title">
              Registry
            </h4>
            <p class="mcp-card-copy">
              Search Preview metadata. Review exact package + command before install.
            </p>
          </div>
          <div class="mcp-composer-fields mcp-registry-search">
            <label>
              <span>Server or capability</span>
              <input
                class="text-input"
                value={registryQuery()}
                onInput={(event) => setRegistryQuery(event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void searchRegistry();
                }}
              />
            </label>
            <Button variant="primary" size="small" onClick={() => void searchRegistry()}>
              Search Registry
            </Button>
          </div>
          <Show when={registryStatus()}>
            <p class="mcp-probe-status" role="status" aria-live="polite">
              {registryStatus()}
            </p>
          </Show>
          <Show when={registryPage()}>
            <div class="mcp-server-list">
              <For each={registryGroups()}>
                {(group) => {
                  const preferred =
                    group.servers.find((server) => server.isLatest) ?? group.servers[0];
                  const versions = () => registryVersionsFor(group);
                  return (
                    <div class="mcp-server-row mcp-registry-result">
                      <div class="mcp-server-row-main">
                        <strong class="mcp-server-name">{group.title ?? group.name}</strong>
                        <p class="mcp-server-meta">
                          {group.name} · {versions().length} exact version
                          {versions().length === 1 ? "" : "s"} · Registry metadata
                        </p>
                        <p class="mcp-server-meta">{group.description}</p>
                        <div class="mcp-registry-actions">
                          <For each={preferred.packages}>
                            {(pkg, packageIndex) => (
                              <Button
                                variant="secondary"
                                size="small"
                                onClick={() =>
                                  void reviewRegistryPackage(preferred, packageIndex())
                                }
                              >
                                Review {pkg.registryType} {pkg.identifier}@
                                {pkg.version ?? "unpinned"}
                              </Button>
                            )}
                          </For>
                          <For each={preferred.remotes}>
                            {(remote, remoteIndex) => (
                              <Button
                                variant="secondary"
                                size="small"
                                onClick={() =>
                                  void reviewRegistryRemote(preferred, remoteIndex())
                                }
                              >
                                Review {remote.transportType} remote
                              </Button>
                            )}
                          </For>
                        </div>
                        <details class="mcp-registry-versions">
                          <summary>Choose exact version ({versions().length})</summary>
                          <div class="mcp-version-list">
                            <For each={versions()}>
                              {(version) => (
                                <div class="mcp-version-row">
                                  <span>
                                    {version.version}
                                    {version.isLatest ? " · Latest" : ""}
                                  </span>
                                  <div class="mcp-version-actions">
                                    <For each={version.packages}>
                                      {(pkg, packageIndex) => (
                                        <Button
                                          variant="secondary"
                                          size="small"
                                          ghost
                                          onClick={() =>
                                            void reviewRegistryPackage(version, packageIndex())
                                          }
                                        >
                                          {pkg.registryType} {pkg.identifier}@
                                          {pkg.version ?? "unpinned"}
                                        </Button>
                                      )}
                                    </For>
                                    <For each={version.remotes}>
                                      {(remote, remoteIndex) => (
                                        <Button
                                          variant="secondary"
                                          size="small"
                                          ghost
                                          onClick={() =>
                                            void reviewRegistryRemote(version, remoteIndex())
                                          }
                                        >
                                          {remote.transportType} remote
                                        </Button>
                                      )}
                                    </For>
                                  </div>
                                </div>
                              )}
                            </For>
                            <Button
                              variant="secondary"
                              size="small"
                              ghost
                              onClick={() => void loadRegistryVersions(group.name)}
                            >
                              Load complete version history
                            </Button>
                          </div>
                        </details>
                      </div>
                    </div>
                  );
                }}
              </For>
              <Show when={registryPage()?.nextCursor}>
                <Button
                  variant="secondary"
                  onClick={() => void searchRegistry(registryPage()?.nextCursor)}
                >
                  Load more
                </Button>
              </Show>
            </div>
          </Show>
          <Show when={installPreview()}>
            {(selected) => (
              <div class="mcp-import-panel">
                <strong>{selected().server.displayName}</strong>
                <p class="mcp-server-meta">
                  {selected().catalogLabel} · {installLabel(selected().server.install)} · Disabled ·
                  Untrusted
                </p>
                <Show when={servers().find((server) => server.id === selected().server.id)}>
                  {(existing) => (
                    <p class="mcp-probe-status">
                      Update diff: {installLabel(existing().install)} → {installLabel(selected().server.install)}
                    </p>
                  )}
                </Show>
                <p class="mcp-server-meta">Exact install command:</p>
                <pre>{selected().displayCommand}</pre>
                <Show when={inputNames(selected().server.connection).length > 0}>
                  <p class="mcp-server-meta">
                    Inputs required after install: {inputNames(selected().server.connection).join(", ")}
                  </p>
                </Show>
                <For each={selected().warnings}>
                  {(warning) => <p class="mcp-probe-status">{warning}</p>}
                </For>
                <Show
                  when={installOperationId()}
                  fallback={
                    <Button variant="primary" onClick={() => void installReviewedPackage()}>
                      {selected().requiresInstall ? "Install exact version" : "Add remote"}
                    </Button>
                  }
                >
                  <Button variant="secondary" onClick={() => void cancelInstall()}>
                    Cancel install
                  </Button>
                </Show>
              </div>
            )}
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
            <span>Scan external MCP configs</span>
          </label>
          <p class="mcp-discovery-summary">
            Discovery is off by default. Results stay inventory-only until you customize, approve,
            and enable them.
          </p>
          <div class="mcp-import-panel">
            <Button variant="secondary" onClick={() => void generateExport()}>
              Export config
            </Button>
            <Show when={exportText()}>
              <textarea
                class="text-input mcp-import-input"
                rows={7}
                readOnly
                value={exportText()}
                aria-label="Secret-free MCP export"
              />
              <Button variant="secondary" onClick={() => void copyExport()}>
                Copy export
              </Button>
            </Show>
            <Show when={exportStatus()}>
              <p class="mcp-probe-status">{exportStatus()}</p>
            </Show>
          </div>
        </section>

        <section class="mcp-card mcp-card--composer" aria-labelledby="mcp-add-heading">
          <div class="mcp-card-header">
            <h4 id="mcp-add-heading" class="mcp-card-title">
              Custom connection
            </h4>
            <p class="mcp-card-copy">New connections stay disabled until approval succeeds.</p>
          </div>
          <Show
            when={showAddForm()}
            fallback={
              <Button variant="primary" class="mcp-add-trigger" onClick={() => setShowAddForm(true)}>
                Add connection
              </Button>
            }
          >
            <div class="mcp-import-panel">
              <label>
                <span>Paste MCP JSON</span>
                <textarea
                  class="text-input mcp-import-input"
                  rows={7}
                  placeholder={'{"mcpServers":{"example":{"command":"npx","args":["-y","package"]}}}'}
                  value={importText()}
                  onInput={(event) => setImportText(event.currentTarget.value)}
                />
              </label>
              <p class="mcp-card-copy">
                Preview commands, URLs, and input names. Secret values never appear here.
              </p>
              <Button variant="primary" onClick={() => void previewImportConfig()}>
                Preview config
              </Button>
              <Show when={importPreview()}>
                <div class="mcp-server-list mcp-import-preview">
                  <For each={importPreview()?.servers ?? []}>
                    {(server) => (
                      <div class="mcp-server-row mcp-import-preview-row">
                        <div class="mcp-server-row-main">
                          <strong class="mcp-server-name">{server.displayName}</strong>
                          <p class="mcp-server-meta">
                            {sourceLabel(server)} · {transportLabel(server.connection)} · Disabled ·
                            Untrusted
                          </p>
                          <p class="mcp-server-meta">{connectionSummary(server.connection)}</p>
                          <Show when={inputNames(server.connection).length > 0}>
                            <p class="mcp-server-meta">
                              Inputs: {inputNames(server.connection).join(", ")}
                            </p>
                          </Show>
                        </div>
                      </div>
                    )}
                  </For>
                  <For each={importPreview()?.diagnostics ?? []}>
                    {(diagnostic) => (
                      <p class="mcp-probe-status">
                        {diagnostic.serverId ? `${diagnostic.serverId}: ` : ""}
                        {diagnostic.message}
                      </p>
                    )}
                  </For>
                  <Show when={(importPreview()?.servers.length ?? 0) > 0}>
                    <Button variant="primary" onClick={() => void applyImport()}>
                      Apply import
                    </Button>
                  </Show>
                </div>
              </Show>
            </div>
            <div class="mcp-composer-divider" role="separator">
              Or configure manually
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
                    setDraft((current) => ({
                      ...current,
                      displayName: event.currentTarget.value,
                    }))
                  }
                />
              </label>
              <label>
                <span>Transport</span>
                <TextSelect
                  value={draft().connection.type}
                  options={MCP_TRANSPORT_OPTIONS}
                  onChange={(event) => {
                    const type = event.currentTarget.value;
                    setDraft((current) => ({
                      ...current,
                      connection:
                        type === "stdio"
                          ? { type: "stdio", command: "", args: [], environment: {} }
                          : {
                              type: type as "streamableHttp" | "legacySse",
                              url: "",
                              allowLocalhost: false,
                              headers: {},
                              auth: { type: "none" },
                        },
                    }));
                  }}
                />
              </label>
              <Show when={draft().connection.type === "stdio"}>
                <label>
                  <span>Command</span>
                  <input
                    class="text-input"
                    value={draftCommand()}
                    onInput={(event) =>
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? {
                              ...current,
                              connection: {
                                ...current.connection,
                                command: event.currentTarget.value,
                              },
                            }
                          : current,
                      )
                    }
                  />
                </label>
                <label>
                  <span>Args</span>
                  <input
                    class="text-input"
                    value={draftArgs()}
                    onInput={(event) =>
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? {
                              ...current,
                              connection: {
                                ...current.connection,
                                args: event.currentTarget.value
                                  .split(",")
                                  .map((part) => part.trim())
                                  .filter(Boolean),
                              },
                            }
                          : current,
                      )
                    }
                  />
                </label>
                <label>
                  <span>Secure env input names</span>
                  <input
                    class="text-input"
                    value={draftEnvironmentNames()}
                    placeholder="MASSIVE_API_KEY"
                    onInput={(event) => {
                      const names = event.currentTarget.value
                        .split(",")
                        .map((name) => name.trim())
                        .filter(Boolean);
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? {
                              ...current,
                              connection: {
                                ...current.connection,
                                environment: Object.fromEntries(
                                  names.map((name) => [
                                    name,
                                    current.connection.type === "stdio"
                                      ? (current.connection.environment[name] ?? {
                                          type: "literal",
                                          value: "",
                                        })
                                      : { type: "literal", value: "" },
                                  ]),
                                ),
                              },
                            }
                          : current,
                      );
                    }}
                  />
                </label>
              </Show>
              <Show when={draft().connection.type !== "stdio"}>
                <label>
                  <span>HTTPS endpoint</span>
                  <input
                    class="text-input"
                    value={draftRemoteUrl()}
                    onInput={(event) =>
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? current
                          : {
                              ...current,
                              connection: { ...current.connection, url: event.currentTarget.value },
                            },
                      )
                    }
                  />
                </label>
                <label>
                  <span>Header/input names</span>
                  <input
                    class="text-input"
                    value={draftRemoteHeaders()}
                    onInput={(event) => {
                      const headers = Object.fromEntries(
                        event.currentTarget.value
                          .split(",")
                          .map((name) => name.trim())
                          .filter(Boolean)
                          .map((name) => [name, { type: "literal", value: "" }]),
                      ) as Record<string, McpPersistedValue>;
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? current
                          : { ...current, connection: { ...current.connection, headers } },
                      );
                    }}
                  />
                </label>
                <label>
                  <span>Authentication</span>
                  <TextSelect
                    value={draftRemoteAuth()?.type ?? "none"}
                    options={MCP_AUTH_OPTIONS}
                    onChange={(event) => {
                      const oauth = event.currentTarget.value === "oauth";
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? current
                          : {
                              ...current,
                              connection: {
                                ...current.connection,
                                auth: oauth
                                  ? { type: "oauth", clientId: "", scopes: [] }
                                  : { type: "none" },
                              },
                        },
                      );
                    }}
                  />
                </label>
                <Show when={draftRemoteAuth()?.type === "oauth"}>
                  <label>
                    <span>OAuth client ID (optional)</span>
                    <input
                      class="text-input"
                      placeholder="Leave blank for dynamic registration"
                      value={draftOauthClientId()}
                      onInput={(event) =>
                        setDraft((current) =>
                          current.connection.type !== "stdio" &&
                          current.connection.auth.type === "oauth"
                            ? {
                                ...current,
                                connection: {
                                  ...current.connection,
                                  auth: {
                                    ...current.connection.auth,
                                    clientId: event.currentTarget.value,
                                  },
                                },
                              }
                            : current,
                        )
                      }
                    />
                  </label>
                  <label>
                    <span>Requested scopes</span>
                    <input
                      class="text-input"
                      placeholder="Discover from server"
                      value={draftOauthScopes()}
                      onInput={(event) =>
                        setDraft((current) =>
                          current.connection.type !== "stdio" &&
                          current.connection.auth.type === "oauth"
                            ? {
                                ...current,
                                connection: {
                                  ...current.connection,
                                  auth: {
                                    ...current.connection.auth,
                                    scopes: event.currentTarget.value
                                      .split(/[\s,]+/)
                                      .map((scope) => scope.trim())
                                      .filter(Boolean),
                                  },
                                },
                              }
                            : current,
                        )
                      }
                    />
                  </label>
                </Show>
                <label class="checkbox-row">
                  <input
                    type="checkbox"
                    checked={draftAllowsLocalhost()}
                    onChange={(event) =>
                      setDraft((current) =>
                        current.connection.type === "stdio"
                          ? current
                          : {
                              ...current,
                              connection: {
                                ...current.connection,
                                allowLocalhost: event.currentTarget.checked,
                              },
                            },
                      )
                    }
                  />
                  <span>Allow explicit localhost dev endpoint</span>
                </label>
              </Show>
            </div>
            <div class="mcp-composer-actions">
              <Button variant="primary" onClick={addServer}>
                Save connection
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setDraft(emptyServer());
                  setImportText("");
                  setImportPreview(null);
                  setImportStatus("");
                  setShowAddForm(false);
                }}
              >
                Cancel
              </Button>
            </div>
          </Show>
          <Show when={importStatus()}>
            <div class="mcp-probe-status" role="status" aria-live="polite">
              {importStatus()}
            </div>
          </Show>
        </section>
      </div>
    </SettingsSection>
  );
}
