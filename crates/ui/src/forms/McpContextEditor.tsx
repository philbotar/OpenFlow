import { createMemo, createSignal, For, Show } from "solid-js";
import { Button, TextSelect } from "@/components";
import { listMcpCapabilities, previewMcpPrompt, previewMcpResource } from "../api";
import type {
  McpCapabilityCatalog,
  McpContextSnapshot,
  McpPromptSelection,
  McpResourceSelection,
  McpServerConfig,
} from "../lib/types";

const DEFAULT_MAX_BYTES = 65_536;
const MAX_CONTEXT_BYTES = 1_048_576;

export function McpContextEditor(props: {
  servers: readonly McpServerConfig[];
  resources: readonly McpResourceSelection[];
  prompts: readonly McpPromptSelection[];
  onChange: (resources: McpResourceSelection[], prompts: McpPromptSelection[]) => void;
}) {
  const availableServers = createMemo(() => props.servers.filter((server) => server.enabled));
  const [serverId, setServerId] = createSignal(availableServers()[0]?.id ?? "");
  const [catalog, setCatalog] = createSignal<McpCapabilityCatalog>();
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal("");
  const [preview, setPreview] = createSignal<McpContextSnapshot>();
  const totalBytes = createMemo(() =>
    [...props.resources, ...props.prompts].reduce((sum, selection) => sum + selection.maxBytes, 0),
  );

  const loadCatalog = async () => {
    const id = serverId();
    if (!id) return;
    setLoading(true);
    setError("");
    setPreview();
    try {
      setCatalog(await listMcpCapabilities(id));
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setLoading(false);
    }
  };

  const resourceSelection = (uri: string) =>
    props.resources.find((selection) => selection.serverId === serverId() && selection.uri === uri);
  const promptSelection = (name: string) =>
    props.prompts.find((selection) => selection.serverId === serverId() && selection.name === name);

  const updateResource = (uri: string, patch?: Partial<McpResourceSelection>) => {
    const existing = resourceSelection(uri);
    const resources = existing
      ? patch
        ? props.resources.map((selection) =>
            selection === existing ? { ...selection, ...patch } : { ...selection },
          )
        : props.resources.filter((selection) => selection !== existing).map((selection) => ({ ...selection }))
      : [
          ...props.resources.map((selection) => ({ ...selection })),
          { serverId: serverId(), uri, maxBytes: DEFAULT_MAX_BYTES, subscribe: false },
        ];
    props.onChange(resources, props.prompts.map((selection) => ({ ...selection, arguments: { ...selection.arguments } })));
  };

  const updatePrompt = (
    name: string,
    patch?: Partial<McpPromptSelection>,
    defaults: Record<string, string> = {},
  ) => {
    const existing = promptSelection(name);
    const prompts = existing
      ? patch
        ? props.prompts.map((selection) =>
            selection === existing
              ? { ...selection, ...patch, arguments: patch.arguments ?? { ...selection.arguments } }
              : { ...selection, arguments: { ...selection.arguments } },
          )
        : props.prompts
            .filter((selection) => selection !== existing)
            .map((selection) => ({ ...selection, arguments: { ...selection.arguments } }))
      : [
          ...props.prompts.map((selection) => ({ ...selection, arguments: { ...selection.arguments } })),
          { serverId: serverId(), name, arguments: defaults, maxBytes: DEFAULT_MAX_BYTES },
        ];
    props.onChange(props.resources.map((selection) => ({ ...selection })), prompts);
  };

  const boundedBytes = (value: string) => {
    const parsed = Number.parseInt(value, 10);
    return Number.isFinite(parsed) ? Math.max(1, Math.min(MAX_CONTEXT_BYTES, parsed)) : DEFAULT_MAX_BYTES;
  };

  const showResourcePreview = async (uri: string, maxBytes: number) => {
    setError("");
    try {
      setPreview(await previewMcpResource(serverId(), uri, maxBytes));
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const showPromptPreview = async (selection: McpPromptSelection) => {
    setError("");
    try {
      setPreview(
        await previewMcpPrompt(
          selection.serverId,
          selection.name,
          selection.arguments,
          selection.maxBytes,
        ),
      );
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  return (
    <div class="mcp-context-editor">
      <p class="field-help">
        Only checked items enter model context. OpenFlow freezes content at run start with server
        provenance. Server content remains untrusted data.
      </p>
      <Show
        when={availableServers().length > 0}
        fallback={<p class="field-help">Enable and approve an MCP server first.</p>}
      >
        <label>
          <span>Server</span>
          <TextSelect
            value={serverId()}
            options={availableServers().map((server) => ({ value: server.id, label: server.displayName }))}
            onChange={(event) => {
              setServerId(event.currentTarget.value);
              setCatalog();
              setPreview();
            }}
          />
        </label>
        <Button variant="secondary" onClick={() => void loadCatalog()} disabled={loading()}>
          {loading() ? "Loading…" : catalog() ? "Refresh catalog" : "Load resources & prompts"}
        </Button>
      </Show>

      <Show when={totalBytes() > 0}>
        <p class={totalBytes() > MAX_CONTEXT_BYTES ? "field-error" : "field-help"}>
          Context budget: {totalBytes().toLocaleString()} / {MAX_CONTEXT_BYTES.toLocaleString()} bytes
        </p>
      </Show>
      <Show when={error()}>
        <p class="field-error" role="alert">{error()}</p>
      </Show>

      <Show when={catalog()}>
        {(loaded) => (
          <>
            <div class="mcp-context-group">
              <h4>Resources</h4>
              <Show when={loaded().resources.length > 0} fallback={<p class="field-help">None advertised.</p>}>
                <For each={loaded().resources}>
                  {(resource) => {
                    const selected = () => resourceSelection(resource.uri);
                    return (
                      <div class="mcp-context-item">
                        <label class="checkbox-row">
                          <input
                            type="checkbox"
                            checked={Boolean(selected())}
                            onChange={() => updateResource(resource.uri)}
                          />
                          <span>{resource.title ?? resource.name}</span>
                        </label>
                        <code>{resource.uri}</code>
                        <Show when={resource.description}><p>{resource.description}</p></Show>
                        <Show when={resource.sizeBytes != null}>
                          <p class="field-help">Advertised size: {resource.sizeBytes?.toLocaleString()} bytes</p>
                        </Show>
                        <Show when={selected()}>
                          {(selection) => (
                            <div class="mcp-context-controls">
                              <label>
                                <span>Max bytes</span>
                                <input
                                  class="text-input"
                                  type="number"
                                  min="1"
                                  max={MAX_CONTEXT_BYTES}
                                  value={selection().maxBytes}
                                  onInput={(event) =>
                                    updateResource(resource.uri, {
                                      maxBytes: boundedBytes(event.currentTarget.value),
                                    })
                                  }
                                />
                              </label>
                              <Show when={resource.subscribable}>
                                <label class="checkbox-row">
                                  <input
                                    type="checkbox"
                                    checked={selection().subscribe}
                                    onChange={(event) =>
                                      updateResource(resource.uri, { subscribe: event.currentTarget.checked })
                                    }
                                  />
                                  <span>Subscribe during run</span>
                                </label>
                              </Show>
                              <Button
                                variant="secondary"
                                onClick={() => void showResourcePreview(resource.uri, selection().maxBytes)}
                              >
                                Preview
                              </Button>
                            </div>
                          )}
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </Show>
            </div>

            <div class="mcp-context-group">
              <h4>Prompts</h4>
              <Show when={loaded().prompts.length > 0} fallback={<p class="field-help">None advertised.</p>}>
                <For each={loaded().prompts}>
                  {(prompt) => {
                    const selected = () => promptSelection(prompt.name);
                    return (
                      <div class="mcp-context-item">
                        <label class="checkbox-row">
                          <input
                            type="checkbox"
                            checked={Boolean(selected())}
                            onChange={() =>
                              updatePrompt(
                                prompt.name,
                                undefined,
                                Object.fromEntries(prompt.arguments.map((argument) => [argument.name, ""])),
                              )
                            }
                          />
                          <span>{prompt.title ?? prompt.name}</span>
                        </label>
                        <code>{prompt.name}</code>
                        <Show when={prompt.description}><p>{prompt.description}</p></Show>
                        <Show when={selected()}>
                          {(selection) => (
                            <div class="mcp-context-controls">
                              <For each={prompt.arguments}>
                                {(argument) => (
                                  <label>
                                    <span>{argument.title ?? argument.name}{argument.required ? " *" : ""}</span>
                                    <input
                                      class="text-input"
                                      value={selection().arguments[argument.name] ?? ""}
                                      onInput={(event) =>
                                        updatePrompt(prompt.name, {
                                          arguments: {
                                            ...selection().arguments,
                                            [argument.name]: event.currentTarget.value,
                                          },
                                        })
                                      }
                                    />
                                  </label>
                                )}
                              </For>
                              <label>
                                <span>Max bytes</span>
                                <input
                                  class="text-input"
                                  type="number"
                                  min="1"
                                  max={MAX_CONTEXT_BYTES}
                                  value={selection().maxBytes}
                                  onInput={(event) =>
                                    updatePrompt(prompt.name, {
                                      maxBytes: boundedBytes(event.currentTarget.value),
                                    })
                                  }
                                />
                              </label>
                              <Button variant="secondary" onClick={() => void showPromptPreview(selection())}>
                                Preview
                              </Button>
                            </div>
                          )}
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </Show>
            </div>
          </>
        )}
      </Show>

      <Show when={preview()}>
        {(snapshot) => (
          <div class="mcp-context-preview">
            <strong>Preview · {snapshot().serverId} · {snapshot().source}</strong>
            <p class="field-help">
              {snapshot().includedSizeBytes.toLocaleString()} / {snapshot().originalSizeBytes.toLocaleString()} bytes
              {snapshot().truncated ? " · truncated" : ""}
            </p>
            <pre>{snapshot().content || snapshot().error || "No content"}</pre>
          </div>
        )}
      </Show>
    </div>
  );
}
