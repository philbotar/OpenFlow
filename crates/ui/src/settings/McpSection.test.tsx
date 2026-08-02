// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import {
  applyMcpConfig,
  cancelMcpInstall,
  deleteMcpSecret,
  disconnectMcpOAuth,
  exportMcpConfig,
  importMcpConfig,
  installMcpPackage,
  listMcpRegistryVersions,
  mcpOAuthStatus,
  previewMcpRegistryInstall,
  previewMcpRegistryRemote,
  probeMcpServer,
  refreshMcpOAuth,
  rollbackMcpInstall,
  saveMcpSecret,
  searchMcpRegistry,
  startMcpOAuth,
} from "../api";
import { AppContext, type AppContextValue } from "../context/AppContext";
import type { AppSettings, McpDiscoveryRow, McpServerConfig } from "../lib/types";
import { McpSection } from "./McpSection";

const openExternalUrl = vi.hoisted(() => vi.fn(async () => {}));
const openLocalPath = vi.hoisted(() => vi.fn(async () => {}));

vi.mock("../api", () => ({
  applyMcpConfig: vi.fn(),
  cancelMcpInstall: vi.fn(),
  exportMcpConfig: vi.fn(),
  importMcpConfig: vi.fn(),
  probeMcpServer: vi.fn(),
  saveMcpSecret: vi.fn(),
  deleteMcpSecret: vi.fn(),
  disconnectMcpOAuth: vi.fn(),
  searchMcpRegistry: vi.fn(),
  listMcpRegistryVersions: vi.fn(),
  previewMcpRegistryInstall: vi.fn(),
  previewMcpRegistryRemote: vi.fn(),
  mcpOAuthStatus: vi.fn(),
  refreshMcpOAuth: vi.fn(),
  startMcpOAuth: vi.fn(),
  installMcpPackage: vi.fn(),
  rollbackMcpInstall: vi.fn(),
  openExternalUrl,
  openLocalPath,
}));

const defaultPolicy = {
  defaultToolAccess: "write" as const,
  defaultToolConcurrency: "exclusive" as const,
  allowRoots: false,
  allowSampling: false,
  allowElicitation: false,
};

function stdioServer(overrides: Partial<McpServerConfig> = {}): McpServerConfig {
  return {
    schemaVersion: 1,
    id: "gh",
    displayName: "GitHub",
    source: { type: "manual" },
    install: { type: "external" },
    connection: {
      type: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-github"],
      environment: {},
    },
    trust: {},
    policy: defaultPolicy,
    enabled: false,
    ...overrides,
  };
}

function appSettings(servers: McpServerConfig[] = []): AppSettings {
  return {
    active_provider: "openai",
    providers: {},
    mcp: { servers, discoverExternal: false },
  };
}

const discoveredMcp: McpDiscoveryRow[] = [
  {
    id: "linear",
    displayName: "Linear",
    command: "npx",
    args: ["-y", "linear-mcp"],
    envKeys: ["LINEAR_API_KEY"],
    enabled: false,
    source: "cursor",
    sourcePath: "/Users/me/.cursor/mcp.json",
  },
];

async function flushPromises() {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function setInput(input: HTMLInputElement | HTMLTextAreaElement, value: string) {
  input.value = value;
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
}

function chooseTextOption(scope: ParentNode, labelText: string, optionText: string) {
  const label = [...scope.querySelectorAll<HTMLLabelElement>("label")].find(
    (candidate) =>
      candidate.querySelector(":scope > span")?.textContent?.trim() === labelText,
  );
  const trigger = label?.querySelector<HTMLButtonElement>(
    'button[aria-haspopup="listbox"]',
  );
  expect(trigger).not.toBeNull();
  trigger?.click();
  const option = [...(label?.querySelectorAll<HTMLButtonElement>('[role="option"]') ?? [])].find(
    (candidate) => candidate.textContent?.trim() === optionText,
  );
  expect(option).not.toBeUndefined();
  option?.click();
}

describe("McpSection", () => {
  let mountPoint: HTMLDivElement;

  beforeEach(() => {
    mountPoint = document.createElement("div");
    document.body.appendChild(mountPoint);
    vi.mocked(importMcpConfig).mockReset();
    vi.mocked(applyMcpConfig).mockReset();
    vi.mocked(exportMcpConfig).mockReset();
    vi.mocked(searchMcpRegistry).mockReset();
    vi.mocked(listMcpRegistryVersions).mockReset();
    vi.mocked(previewMcpRegistryInstall).mockReset();
    vi.mocked(previewMcpRegistryRemote).mockReset();
    vi.mocked(installMcpPackage).mockReset();
    vi.mocked(cancelMcpInstall).mockReset();
    vi.mocked(rollbackMcpInstall).mockReset();
    vi.mocked(probeMcpServer).mockReset();
    vi.mocked(saveMcpSecret).mockReset();
    vi.mocked(deleteMcpSecret).mockReset();
    vi.mocked(disconnectMcpOAuth).mockReset();
    vi.mocked(mcpOAuthStatus).mockReset();
    vi.mocked(refreshMcpOAuth).mockReset();
    vi.mocked(startMcpOAuth).mockReset();
    openExternalUrl.mockClear();
    openLocalPath.mockClear();
    vi.mocked(mcpOAuthStatus).mockImplementation(async (serverId) => ({
      serverId,
      state: "disconnected",
      grantedScopes: [],
    }));
  });

  afterEach(() => {
    mountPoint.remove();
  });

  function renderSection(
    initialSettings = appSettings([stdioServer()]),
    discovered: McpDiscoveryRow[] = [],
  ) {
    const [settings, setSettings] = createSignal(structuredClone(initialSettings));
    const [discoveredRows] = createSignal(structuredClone(discovered));
    const updateSettings = vi.fn(async (mutator: (draft: AppSettings) => void) => {
      const draft = structuredClone(settings());
      mutator(draft);
      setSettings(draft);
    });
    const refreshDiscoveredMcp = vi.fn(async () => {});
    const context = {
      settings,
      discoveredMcp: discoveredRows,
      updateSettings,
      refreshDiscoveredMcp,
    } as unknown as AppContextValue;

    render(
      () => (
        <AppContext.Provider value={context}>
          <McpSection />
        </AppContext.Provider>
      ),
      mountPoint,
    );

    return { settings, updateSettings, refreshDiscoveredMcp };
  }

  test("previews an import without persistence, then applies it disabled and untrusted", async () => {
    const imported: Awaited<ReturnType<typeof importMcpConfig>> = {
      servers: [
        stdioServer({
          id: "massive",
          displayName: "Massive",
          source: {
            type: "imported",
            dialect: "claude",
            sourcePath: "/tmp/claude.json",
          },
          connection: {
            type: "stdio",
            command: "mcp-massive",
            args: ["--stdio"],
            environment: {
              MASSIVE_API_KEY: {
                type: "secret",
                secretRef: "keychain://openflow/mcp/massive",
              },
            },
          },
          trust: {
            approvedFingerprint: "must-be-cleared",
            approvedAt: "2026-08-01T00:00:00Z",
          },
          enabled: true,
        }),
      ],
      diagnostics: [{ serverId: "legacy", message: "Unsupported field ignored" }],
    };
    vi.mocked(importMcpConfig).mockResolvedValue(imported);
    vi.mocked(applyMcpConfig).mockResolvedValue(imported);
    const harness = renderSection(appSettings());

    mountPoint.querySelector<HTMLButtonElement>(".mcp-add-trigger")?.click();
    const textarea = mountPoint.querySelector<HTMLTextAreaElement>(".mcp-import-input");
    expect(textarea).not.toBeNull();
    if (!textarea) return;
    setInput(textarea, '{"mcpServers":{"massive":{"command":"mcp-massive"}}}');
    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Preview config")
      ?.click();
    await flushPromises();

    expect(harness.updateSettings).not.toHaveBeenCalled();
    expect(mountPoint.textContent).toContain("mcp-massive --stdio");
    expect(mountPoint.textContent).toContain("MASSIVE_API_KEY");
    expect(mountPoint.textContent).toContain("Unsupported field ignored");
    expect(mountPoint.textContent).not.toContain("keychain://openflow/mcp/massive");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Apply import")
      ?.click();
    await flushPromises();

    const applied = harness.settings().mcp?.servers[0];
    expect(applied).toMatchObject({
      id: "massive",
      enabled: false,
      trust: {},
      source: { type: "imported", dialect: "claude" },
    });
    expect(applyMcpConfig).toHaveBeenCalledWith(
      '{"mcpServers":{"massive":{"command":"mcp-massive"}}}',
    );
  });

  test("exports canonical config without exposing secret values", async () => {
    vi.mocked(exportMcpConfig).mockResolvedValue(
      '{"format":"openflow.mcp","servers":[{"secretRef":"mcp-secret:v1:opaque"}]}',
    );
    renderSection();

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Export config")
      ?.click();
    await flushPromises();

    const exported = mountPoint.querySelector<HTMLTextAreaElement>(
      'textarea[aria-label="Secret-free MCP export"]',
    );
    expect(exported?.value).toContain("openflow.mcp");
    expect(exported?.value).not.toContain("secret-value");
  });

  test("reviews and installs an exact Registry package before approval", async () => {
    const registryServer = {
      name: "io.example/massive",
      title: "Massive",
      description: "Large MCP server",
      version: "2.1.0",
      isLatest: true,
      packages: [
        {
          registryType: "npm",
          identifier: "massive-mcp",
          version: "2.1.0",
          transportType: "stdio",
          runtimeArguments: [],
          packageArguments: [],
          inputs: [],
        },
      ],
      remotes: [],
    };
    const installServer = stdioServer({
      id: "io-example-massive",
      displayName: "Massive",
      source: {
        type: "registry",
        catalogBaseUrl: "https://registry.modelcontextprotocol.io/v0.1",
        serverName: registryServer.name,
        version: "2.1.0",
      },
      install: { type: "npm", package: "massive-mcp", version: "2.1.0" },
    });
    vi.mocked(searchMcpRegistry).mockResolvedValue({
      catalogBaseUrl: "https://registry.modelcontextprotocol.io/v0.1",
      catalogLabel: "Preview",
      servers: [registryServer],
      count: 1,
    });
    vi.mocked(previewMcpRegistryInstall).mockResolvedValue({
      server: installServer,
      displayCommand: "npm install -- massive-mcp@2.1.0",
      catalogLabel: "Preview",
      warnings: ["Registry metadata does not establish safety."],
      requiresInstall: true,
    });
    vi.mocked(installMcpPackage).mockResolvedValue({
      operationId: "00000000-0000-4000-8000-000000000001",
      state: "succeeded",
      stdoutTail: "",
      stderrTail: "",
      outputTruncated: false,
      durationMs: 10,
      server: installServer,
    });
    vi.spyOn(globalThis.crypto, "randomUUID").mockReturnValue(
      "00000000-0000-4000-8000-000000000001",
    );
    const harness = renderSection(appSettings());

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Search Registry")
      ?.click();
    await flushPromises();
    expect(mountPoint.textContent).toContain("massive-mcp@2.1.0");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.includes("Review npm"))
      ?.click();
    await flushPromises();
    expect(mountPoint.textContent).toContain("npm install -- massive-mcp@2.1.0");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Install exact version")
      ?.click();
    await flushPromises();

    expect(installMcpPackage).toHaveBeenCalledWith(
      "00000000-0000-4000-8000-000000000001",
      installServer,
    );
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      id: "io-example-massive",
      enabled: false,
      trust: {},
    });
    expect(mountPoint.textContent).toContain("Run Approve & Test before enabling");
  });

  test("groups Registry versions into one compact server result", async () => {
    const versions = ["2.0.4", "2.0.5", "2.0.6"].map((version) => ({
      name: "io.example/massive",
      title: "Massive Context MCP",
      description: "Large-context search server",
      version,
      isLatest: version === "2.0.6",
      packages: [
        {
          registryType: "pypi",
          identifier: "massive-context-mcp",
          version,
          transportType: "stdio",
          runtimeArguments: [],
          packageArguments: [],
          inputs: [],
        },
      ],
      remotes: [],
    }));
    vi.mocked(searchMcpRegistry).mockResolvedValue({
      catalogBaseUrl: "https://registry.modelcontextprotocol.io/v0.1",
      catalogLabel: "Preview",
      servers: versions,
      count: versions.length,
    });
    renderSection(appSettings());

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Search Registry")
      ?.click();
    await flushPromises();

    expect(mountPoint.querySelectorAll(".mcp-registry-result")).toHaveLength(1);
    expect(mountPoint.textContent).toContain("3 exact versions");
    expect(mountPoint.textContent).toContain("massive-context-mcp@2.0.6");
    expect(mountPoint.querySelector(".mcp-registry-versions:not([open])")).not.toBeNull();
  });

  test("reviews a Registry remote without running a package installer", async () => {
    const registryServer = {
      name: "io.example/hosted",
      title: "Hosted MCP",
      description: "Hosted remote MCP server",
      version: "1.4.0",
      isLatest: true,
      packages: [],
      remotes: [
        {
          transportType: "streamable-http",
          url: "https://mcp.example.test/mcp",
          inputs: [{ name: "Authorization", required: true, secret: true }],
        },
      ],
    };
    const remoteServer: McpServerConfig = {
      schemaVersion: 1,
      id: "io-example-hosted",
      displayName: "Hosted MCP",
      source: {
        type: "registry",
        catalogBaseUrl: "https://registry.modelcontextprotocol.io/v0.1",
        serverName: registryServer.name,
        version: registryServer.version,
      },
      install: { type: "external" },
      connection: {
        type: "streamableHttp",
        url: "https://mcp.example.test/mcp",
        allowLocalhost: false,
        headers: {
          Authorization: { type: "secret", secretRef: "mcp-secret:v1:opaque" },
        },
        auth: { type: "none" },
      },
      trust: {},
      policy: defaultPolicy,
      enabled: false,
    };
    vi.mocked(searchMcpRegistry).mockResolvedValue({
      catalogBaseUrl: "https://registry.modelcontextprotocol.io/v0.1",
      catalogLabel: "Preview",
      servers: [registryServer],
      count: 1,
    });
    vi.mocked(previewMcpRegistryRemote).mockResolvedValue({
      server: remoteServer,
      displayCommand: "Connect https://mcp.example.test/mcp",
      catalogLabel: "Preview",
      warnings: ["Registry metadata does not establish safety."],
      requiresInstall: false,
    });
    const harness = renderSection(appSettings());

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Search Registry")
      ?.click();
    await flushPromises();
    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.includes("Review streamable-http remote"))
      ?.click();
    await flushPromises();

    expect(previewMcpRegistryRemote).toHaveBeenCalledWith(
      registryServer.name,
      registryServer.version,
      0,
    );
    expect(mountPoint.textContent).toContain("Connect https://mcp.example.test/mcp");
    expect(installMcpPackage).not.toHaveBeenCalled();
    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Add remote")
      ?.click();
    await flushPromises();

    expect(installMcpPackage).not.toHaveBeenCalled();
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      id: "io-example-hosted",
      install: { type: "external" },
      connection: { type: "streamableHttp", url: "https://mcp.example.test/mcp" },
      trust: {},
      enabled: false,
    });
  });

  test("keeps discovered servers inventory-only and customizes them disabled and untrusted", async () => {
    const harness = renderSection(
      {
        ...appSettings(),
        mcp: { servers: [], discoverExternal: true },
      },
      discoveredMcp,
    );
    const row = [...mountPoint.querySelectorAll<HTMLElement>(".mcp-connection-row")].find((item) =>
      item.textContent?.includes("Linear"),
    );

    expect(row?.textContent).toContain("npx -y linear-mcp");
    expect(row?.querySelector('input[type="checkbox"]')).toBeNull();
    expect(row?.textContent).not.toContain("Approve & Test");

    [...(row?.querySelectorAll("button") ?? [])]
      .find((button) => button.textContent?.trim() === "Customize")
      ?.click();
    await flushPromises();

    expect(probeMcpServer).not.toHaveBeenCalled();
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      schemaVersion: 1,
      id: "linear",
      source: {
        type: "imported",
        dialect: "cursor",
        sourcePath: "/Users/me/.cursor/mcp.json",
      },
      connection: { type: "stdio", command: "npx", args: ["-y", "linear-mcp"] },
      trust: {},
      enabled: false,
    });
  });

  test("manual connections start disabled and untrusted", async () => {
    const harness = renderSection(appSettings());
    mountPoint.querySelector<HTMLButtonElement>(".mcp-add-trigger")?.click();
    const inputs = mountPoint
      .querySelector("#mcp-add-heading")
      ?.closest("section")
      ?.querySelectorAll<HTMLInputElement>(".mcp-composer-fields input");
    expect(inputs?.length).toBe(5);
    if (!inputs) return;
    setInput(inputs[0], "filesystem");
    setInput(inputs[1], "Filesystem");
    setInput(inputs[2], "npx");
    setInput(inputs[3], "-y, @modelcontextprotocol/server-filesystem");
    setInput(inputs[4], "FILESYSTEM_ROOT");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Save connection")
      ?.click();
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toEqual(
      expect.objectContaining({
        schemaVersion: 1,
        id: "filesystem",
        enabled: false,
        trust: {},
        source: { type: "manual" },
        install: { type: "external" },
        connection: expect.objectContaining({
          type: "stdio",
          command: "npx",
          environment: { FILESYSTEM_ROOT: { type: "literal", value: "" } },
        }),
      }),
    );
  });

  test("manual remote connections capture transport, endpoint, inputs, and localhost opt-in", async () => {
    const harness = renderSection(appSettings());
    mountPoint.querySelector<HTMLButtonElement>(".mcp-add-trigger")?.click();
    const section = mountPoint.querySelector("#mcp-add-heading")?.closest("section");
    expect(section).not.toBeNull();
    if (!section) return;
    chooseTextOption(section, "Transport", "Streamable HTTP");
    const inputs = section?.querySelectorAll<HTMLInputElement>(".mcp-composer-fields input");
    expect(inputs?.length).toBe(5);
    if (!inputs) return;
    setInput(inputs[0], "local-remote");
    setInput(inputs[1], "Local Remote");
    setInput(inputs[2], "http://localhost:7777/mcp");
    setInput(inputs[3], "Authorization, X-Tenant");
    inputs[4].click();
    chooseTextOption(section, "Authentication", "OAuth 2.1");
    const clientId = [...section.querySelectorAll<HTMLLabelElement>("label")]
      .find((label) => label.textContent?.includes("OAuth client ID"))
      ?.querySelector<HTMLInputElement>("input");
    expect(clientId).not.toBeNull();
    if (clientId) setInput(clientId, "openflow-client");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Save connection")
      ?.click();
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      id: "local-remote",
      enabled: false,
      trust: {},
      connection: {
        type: "streamableHttp",
        url: "http://localhost:7777/mcp",
        allowLocalhost: true,
        headers: {
          Authorization: { type: "literal", value: "" },
          "X-Tenant": { type: "literal", value: "" },
        },
        auth: { type: "oauth", clientId: "openflow-client", scopes: [] },
      },
    });
  });

  test("OAuth connect stores only public metadata and disconnect clears the credential ref", async () => {
    const oauthServer: McpServerConfig = {
      ...stdioServer({ id: "hosted", displayName: "Hosted" }),
      connection: {
        type: "streamableHttp",
        url: "https://mcp.example.test/mcp",
        allowLocalhost: false,
        headers: {},
        auth: { type: "oauth", clientId: "", scopes: ["tools.read"] },
      },
    };
    vi.mocked(startMcpOAuth).mockResolvedValue({
      serverId: "hosted",
      state: "connected",
      clientId: "dynamic-openflow",
      issuer: "https://auth.example.test",
      credentialRef: "mcp-secret:v1:opaque",
      grantedScopes: ["tools.read"],
      expiresAt: "2026-08-02T00:00:00Z",
    });
    vi.mocked(disconnectMcpOAuth).mockResolvedValue({
      serverId: "hosted",
      state: "disconnected",
      grantedScopes: [],
    });
    const harness = renderSection(appSettings([oauthServer]));
    await flushPromises();

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Connect OAuth")
      ?.click();
    await flushPromises();

    expect(startMcpOAuth).toHaveBeenCalledWith("hosted", ["tools.read"]);
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      connection: {
        auth: {
          type: "oauth",
          clientId: "dynamic-openflow",
          issuer: "https://auth.example.test",
          credentialRef: "mcp-secret:v1:opaque",
          scopes: ["tools.read"],
        },
      },
    });
    expect(JSON.stringify(harness.settings())).not.toContain("access-token");
    expect(JSON.stringify(harness.settings())).not.toContain("refresh-token");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Disconnect OAuth")
      ?.click();
    await flushPromises();

    expect(disconnectMcpOAuth).toHaveBeenCalledWith("hosted");
    const auth = (
      harness.settings().mcp?.servers[0].connection as Extract<
        McpServerConfig["connection"],
        { type: "streamableHttp" }
      >
    ).auth;
    expect(auth.type === "oauth" ? auth.credentialRef : "not-oauth").toBeUndefined();
  });

  test("changes configured remote authentication through the shared dropdown", async () => {
    const remote = stdioServer({
      id: "hosted",
      displayName: "Hosted",
      enabled: true,
      trust: { approvedFingerprint: "sha256:approved" },
      connection: {
        type: "streamableHttp",
        url: "https://mcp.example.test/mcp",
        allowLocalhost: false,
        headers: {},
        auth: { type: "none" },
      },
    });
    const harness = renderSection(appSettings([remote]));

    chooseTextOption(mountPoint, "Authentication", "OAuth 2.1");
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      connection: {
        auth: { type: "oauth", clientId: "", scopes: [] },
      },
    });
  });

  test("changing a command clears trust and disables the server", async () => {
    const approved = stdioServer({
      enabled: true,
      trust: {
        approvedFingerprint: "sha256:approved",
        approvedAt: "2026-08-01T00:00:00Z",
      },
    });
    const harness = renderSection(appSettings([approved]));
    const command = mountPoint.querySelectorAll<HTMLInputElement>(
      ".mcp-configured-fields input",
    )[1];
    setInput(command, "node");
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      connection: { type: "stdio", command: "node" },
    });
  });

  test("masks credential-shaped args and moves them to secure input slots", async () => {
    const harness = renderSection(
      appSettings([
        stdioServer({
          id: "massive",
          displayName: "Massive",
          connection: {
            type: "stdio",
            command: "mcp_massive",
            args: ['"MASSIVE_API_KEY": "plain-text-secret"'],
            environment: {},
          },
        }),
      ]),
    );

    expect(mountPoint.textContent).toContain("[redacted credential argument]");
    const argumentInput = mountPoint.querySelector<HTMLInputElement>(
      '.mcp-configured-fields input[aria-invalid="true"]',
    );
    expect(argumentInput?.type).toBe("password");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Move to secure inputs")
      ?.click();
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      connection: {
        type: "stdio",
        args: [],
        environment: { MASSIVE_API_KEY: { type: "literal", value: "" } },
      },
    });
    expect(
      mountPoint.querySelector('input[aria-label="MASSIVE_API_KEY secret value"]'),
    ).not.toBeNull();
  });

  test("enabling a client capability clears trust and disables the server", async () => {
    const approved = stdioServer({
      enabled: true,
      trust: {
        approvedFingerprint: "sha256:approved",
        approvedAt: "2026-08-01T00:00:00Z",
      },
    });
    const harness = renderSection(appSettings([approved]));
    const samplingToggle = [...mountPoint.querySelectorAll<HTMLLabelElement>("label")]
      .find((label) => label.textContent?.includes("Allow approved sampling reqs"))
      ?.querySelector<HTMLInputElement>('input[type="checkbox"]');

    samplingToggle?.click();
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      policy: { allowSampling: true },
    });
  });

  test("changes tool access through the shared dropdown", async () => {
    const harness = renderSection(
      appSettings([
        stdioServer({
          enabled: true,
          trust: { approvedFingerprint: "sha256:approved" },
        }),
      ]),
    );

    chooseTextOption(mountPoint, "Tool access tier", "Read · user-classified server");
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      policy: { defaultToolAccess: "read" },
    });
  });

  test("changes tool concurrency through the shared dropdown", async () => {
    const harness = renderSection(
      appSettings([
        stdioServer({
          enabled: true,
          trust: { approvedFingerprint: "sha256:approved" },
        }),
      ]),
    );

    chooseTextOption(mountPoint, "Tool concurrency", "Shared · allow concurrent calls");
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      policy: { defaultToolConcurrency: "shared" },
    });
  });

  test("gates Enable on an approval fingerprint", () => {
    const approved = stdioServer({
      id: "approved",
      displayName: "Approved",
      trust: { approvedFingerprint: "sha256:approved" },
    });
    renderSection(appSettings([stdioServer(), approved]));
    const rows = [...mountPoint.querySelectorAll<HTMLElement>(".mcp-connection-row")];
    const untrustedRow = rows.find((row) => row.textContent?.includes("GitHub"));
    const approvedRow = rows.find((row) => row.textContent?.includes("Approved"));

    expect(
      untrustedRow?.querySelector<HTMLInputElement>(
        '.mcp-server-row-actions input[type="checkbox"]',
      )?.disabled,
    ).toBe(true);
    expect(
      approvedRow?.querySelector<HTMLInputElement>(
        '.mcp-server-row-actions input[type="checkbox"]',
      )?.disabled,
    ).toBe(false);
  });

  test("shows lifecycle actions and opens an imported source config", async () => {
    const approved = stdioServer({
      enabled: true,
      source: {
        type: "imported",
        dialect: "claude",
        sourcePath: "/tmp/claude.json",
      },
      trust: { approvedFingerprint: "sha256:approved" },
    });
    renderSection(appSettings([approved]));

    expect(mountPoint.querySelector('[data-state="ready"]')?.textContent).toContain("Ready");
    expect(mountPoint.textContent).toContain("Restart & Test");
    expect(mountPoint.textContent).toContain("Enable");
    expect(mountPoint.textContent).toContain("Copy diagnostics");
    const openSource = [...mountPoint.querySelectorAll("button")].find(
      (button) => button.textContent?.trim() === "Open source",
    );
    openSource?.click();
    await flushPromises();

    expect(openLocalPath).toHaveBeenCalledWith("/tmp/claude.json");
  });

  test("stores input values in the local secrets file and keeps only an opaque ref", async () => {
    vi.mocked(saveMcpSecret).mockResolvedValue("mcp-secret:v1:opaque");
    const harness = renderSection(
      appSettings([
        stdioServer({
          enabled: true,
          trust: { approvedFingerprint: "sha256:old" },
          connection: {
            type: "stdio",
            command: "npx",
            args: ["-y", "massive"],
            environment: { API_KEY: { type: "literal", value: "" } },
          },
        }),
      ]),
    );
    const secretInput = mountPoint.querySelector<HTMLInputElement>(
      'input[aria-label="API_KEY secret value"]',
    );
    expect(secretInput).not.toBeNull();
    if (!secretInput) return;
    setInput(secretInput, "do-not-return-to-ui");
    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Save secret")
      ?.click();
    await flushPromises();

    expect(saveMcpSecret).toHaveBeenCalledWith("gh", "env.API_KEY", "do-not-return-to-ui");
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      connection: {
        environment: {
          API_KEY: { type: "secret", secretRef: "mcp-secret:v1:opaque" },
        },
      },
    });
    expect(mountPoint.textContent).not.toContain("do-not-return-to-ui");
  });

  test("Approve & Test replaces the record with backend approval and shows health", async () => {
    const writeText = vi.fn(async (_value: string) => {});
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const approved = stdioServer({
      trust: {
        approvedFingerprint: "sha256:approved",
        approvedAt: "2026-08-01T00:00:00Z",
      },
    });
    vi.mocked(probeMcpServer).mockResolvedValue({
      server: { ...approved, enabled: true },
      report: {
        state: "ready",
        stage: "close",
        authRequired: false,
        durationMs: 37,
        transport: "stdio",
        protocolVersion: "2025-06-18",
        serverName: "Massive MCP",
        serverVersion: "1.2.3",
        capabilities: ["tools", "prompts"],
        toolNames: ["search", "fetch"],
      },
    });
    const harness = renderSection(appSettings([stdioServer()]));

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Approve & Test")
      ?.click();
    await flushPromises();

    expect(probeMcpServer).toHaveBeenCalledWith(
      expect.objectContaining({ id: "gh", enabled: false }),
      undefined,
    );
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      trust: { approvedFingerprint: "sha256:approved" },
      enabled: false,
    });
    expect(mountPoint.textContent).toContain("2025-06-18");
    expect(mountPoint.textContent).toContain("2 capabilities");
    expect(mountPoint.textContent).toContain("2 tools");
    expect(
      mountPoint.querySelector<HTMLInputElement>(
        '.mcp-server-row-actions input[type="checkbox"]',
      )?.disabled,
    ).toBe(false);
    expect(mountPoint.querySelector('[data-state="disabled"]')?.textContent).toContain(
      "Last test ready",
    );

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Copy diagnostics")
      ?.click();
    await flushPromises();
    expect(writeText).toHaveBeenCalledOnce();
    const copied = JSON.parse(writeText.mock.calls[0][0]);
    expect(copied).toMatchObject({
      serverId: "gh",
      state: "Disabled",
      attemptCount: 1,
      report: { protocolVersion: "2025-06-18" },
    });

    const fetchToggle = [...mountPoint.querySelectorAll<HTMLLabelElement>("label")]
      .find((label) => label.textContent?.trim() === "fetch")
      ?.querySelector<HTMLInputElement>('input[type="checkbox"]');
    fetchToggle?.click();
    await flushPromises();
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      policy: { enabledTools: ["search"] },
    });
  });

  test("failed probes leave the record disabled and untrusted with stage detail", async () => {
    vi.mocked(probeMcpServer).mockResolvedValue({
      server: stdioServer({ trust: { approvedFingerprint: "must-be-cleared" } }),
      report: {
        state: "failed",
        stage: "preflight",
        authRequired: false,
        durationMs: 3,
        transport: "stdio",
        capabilities: [],
        toolNames: [],
        error: "Executable was not found",
      },
    });
    const harness = renderSection();

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Approve & Test")
      ?.click();
    await flushPromises();

    expect(harness.settings().mcp?.servers[0]).toMatchObject({ enabled: false, trust: {} });
    expect(mountPoint.textContent).toContain("Preflight");
    expect(mountPoint.textContent).toContain("Executable was not found");
    expect(mountPoint.querySelector('[data-state="failed"]')).not.toBeNull();
    expect(mountPoint.textContent).toContain("Retry");
  });

  test("OAuth-required probes expose one-click browser authentication", async () => {
    const massive: McpServerConfig = {
      ...stdioServer({
        id: "massive",
        displayName: "Massive",
        source: { type: "imported", dialect: "mcpServers", sourcePath: "" },
      }),
      connection: {
        type: "streamableHttp",
        url: "https://mcp.massive.com/",
        allowLocalhost: false,
        headers: {},
        auth: { type: "none" },
      },
    };
    vi.mocked(probeMcpServer).mockResolvedValue({
      server: massive,
      report: {
        state: "failed",
        stage: "connect",
        authRequired: true,
        durationMs: 42,
        transport: "streamableHttp",
        capabilities: [],
        toolNames: [],
        error: "MCP server `massive` requires OAuth authorization",
      },
    });
    vi.mocked(startMcpOAuth).mockResolvedValue({
      serverId: "massive",
      state: "connecting",
      clientId: "dynamic-openflow",
      issuer: "https://auth.massive.com",
      credentialRef: "mcp-secret:v1:opaque",
      grantedScopes: ["openid", "offline_access", "account"],
    });
    const harness = renderSection(appSettings([massive]));

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Approve & Test")
      ?.click();
    await flushPromises();

    expect(mountPoint.querySelector('[data-state="auth-required"]')).not.toBeNull();
    expect(mountPoint.textContent).toContain("Authenticate OAuth");

    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Authenticate OAuth")
      ?.click();
    await flushPromises();

    expect(startMcpOAuth).toHaveBeenCalledWith("massive", []);
    expect(harness.settings().mcp?.servers[0]).toMatchObject({
      enabled: false,
      trust: {},
      connection: {
        type: "streamableHttp",
        auth: { type: "oauth", clientId: "dynamic-openflow" },
      },
    });
    expect(mountPoint.textContent).toContain("Complete OAuth sign-in in your browser.");
  });

  test("keeps section landmarks and disables every active source", async () => {
    const harness = renderSection(
      {
        ...appSettings([
          stdioServer({
            enabled: true,
            trust: { approvedFingerprint: "sha256:approved" },
          }),
        ]),
        mcp: {
          servers: [
            stdioServer({
              enabled: true,
              trust: { approvedFingerprint: "sha256:approved" },
            }),
          ],
          discoverExternal: true,
        },
      },
      discoveredMcp,
    );

    for (const id of ["mcp-connections-heading", "mcp-advanced-heading", "mcp-add-heading"]) {
      expect(mountPoint.querySelector(`section[aria-labelledby="${id}"]`)).not.toBeNull();
    }
    [...mountPoint.querySelectorAll("button")]
      .find((button) => button.textContent?.trim() === "Disable all")
      ?.click();
    await flushPromises();

    expect(harness.settings().mcp?.servers.every((server) => !server.enabled)).toBe(true);
    expect(harness.settings().mcp?.discoverExternal).toBe(false);
    expect(harness.refreshDiscoveredMcp).toHaveBeenCalledTimes(1);
  });
});
