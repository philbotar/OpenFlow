import { createMemo, createSignal, For, Show } from "solid-js";
import { openExternalUrl } from "../../api";
import { useAppContext } from "../../context/AppContext";
import type { PendingMcpClientRequest } from "../../lib/types";
import { Button } from "../Button";

type SchemaProperty = {
  type?: string;
  title?: string;
  description?: string;
  enum?: unknown[];
};

export function McpClientRequestCard(props: { request: PendingMcpClientRequest }) {
  const ctx = useAppContext();
  const [values, setValues] = createSignal<Record<string, unknown>>({});
  const [submitting, setSubmitting] = createSignal(false);
  const [error, setError] = createSignal("");
  const schema = () => props.request.requestedSchema ?? {};
  const properties = createMemo(() =>
    Object.entries((schema().properties ?? {}) as Record<string, SchemaProperty>),
  );
  const required = createMemo(() => new Set((schema().required ?? []) as string[]));
  const formComplete = createMemo(() =>
    [...required()].every((name) => {
      const value = values()[name];
      return value !== undefined && value !== "";
    }),
  );

  const resolve = async (allow: boolean, content?: unknown) => {
    if (submitting()) return;
    setSubmitting(true);
    setError("");
    try {
      await ctx.handleMcpClientRequest(props.request.requestId, {
        allow,
        ...(content === undefined ? {} : { content }),
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSubmitting(false);
    }
  };

  const openAndResolve = async () => {
    const url = props.request.url;
    if (!url || submitting()) return;
    setSubmitting(true);
    setError("");
    try {
      await openExternalUrl(url);
      await ctx.handleMcpClientRequest(props.request.requestId, { allow: true });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSubmitting(false);
    }
  };

  const setField = (name: string, property: SchemaProperty, raw: string | boolean) => {
    let value: unknown = raw;
    if (property.type === "number" || property.type === "integer") {
      value = raw === "" ? undefined : Number(raw);
    }
    setValues((current) => ({ ...current, [name]: value }));
  };

  return (
    <section class="tool-approval-card mcp-client-request-card" aria-live="polite">
      <div class="tool-approval-card-header"><span>MCP client request</span></div>
      <h3>{props.request.serverId}</h3>
      <p class="tool-approval-node">
        {props.request.kind === "sampling"
          ? `Model sampling · max ${props.request.maxTokens ?? 0} requested tokens`
          : props.request.kind === "elicitationUrl"
            ? "Open external URL"
            : "Provide structured input"}
      </p>
      <p>{props.request.message}</p>
      <p class="mcp-server-meta">
        Origin: {props.request.toolName} · Server content is untrusted. Approval applies once.
      </p>

      <Show when={props.request.kind === "elicitationForm"}>
        <div class="mcp-client-request-fields">
          <For each={properties()}>
            {([name, property]) => (
              <label>
                <span>{property.title ?? name}{required().has(name) ? " *" : ""}</span>
                <Show
                  when={(property.enum?.length ?? 0) > 0}
                  fallback={
                    <Show
                      when={property.type === "boolean"}
                      fallback={
                        <input
                          class="text-input"
                          type={property.type === "number" || property.type === "integer" ? "number" : "text"}
                          aria-label={property.title ?? name}
                          disabled={submitting()}
                          onInput={(event) => setField(name, property, event.currentTarget.value)}
                        />
                      }
                    >
                      <input
                        type="checkbox"
                        aria-label={property.title ?? name}
                        disabled={submitting()}
                        onChange={(event) => setField(name, property, event.currentTarget.checked)}
                      />
                    </Show>
                  }
                >
                  <select
                    class="text-input"
                    aria-label={property.title ?? name}
                    disabled={submitting()}
                    onChange={(event) => setField(name, property, event.currentTarget.value)}
                  >
                    <option value="">Select…</option>
                    <For each={property.enum ?? []}>
                      {(choice) => <option value={String(choice)}>{String(choice)}</option>}
                    </For>
                  </select>
                </Show>
                <Show when={property.description}><small>{property.description}</small></Show>
              </label>
            )}
          </For>
        </div>
      </Show>

      <Show when={error()}><p class="mcp-probe-status" role="alert">{error()}</p></Show>
      <div class="tool-approval-actions">
        <Button
          variant="primary"
          size="small"
          disabled={submitting() || (props.request.kind === "elicitationForm" && !formComplete())}
          onClick={() =>
            props.request.kind === "elicitationUrl"
              ? void openAndResolve()
              : void resolve(true, props.request.kind === "elicitationForm" ? values() : undefined)
          }
        >
          {submitting()
            ? "Working…"
            : props.request.kind === "sampling"
              ? "Allow once"
              : props.request.kind === "elicitationUrl"
                ? "Open & continue"
                : "Submit"}
        </Button>
        <Button variant="secondary" size="small" disabled={submitting()} onClick={() => void resolve(false)}>
          Deny
        </Button>
      </div>
    </section>
  );
}
