import { createSignal, For, Show, type JSX } from "solid-js";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { formatIndentedValue } from "../../lib/utils";

export interface NodeCompletedBubbleProps {
  summary: string;
}

function tryParseJson(text: string): unknown | undefined {
  const trimmed = text.trim();
  if (!trimmed || (trimmed[0] !== "{" && trimmed[0] !== "[")) {
    return undefined;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    return undefined;
  }
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cleanXmlLeak(text: string): string {
  const tagRe = /<\/?[A-Za-z_][\w:.-]*\s*\/?>/g;
  let prev = "";
  let current = text;
  while (current !== prev) {
    prev = current;
    current = current.replace(tagRe, "");
  }
  return current.trim();
}

/** True when object is only `$text` / nested `item` (XML list mangled into a chain). */
function isItemChain(value: Record<string, unknown>): boolean {
  return Object.keys(value).every((key) => key === "$text" || key === "item");
}

function flattenItemChain(value: Record<string, unknown>): unknown[] | undefined {
  // Need nested `item` — lone `{ $text }` unwraps elsewhere.
  if (!isItemChain(value) || !("item" in value)) return undefined;
  const items: unknown[] = [];
  let current: unknown = value;
  while (isPlainObject(current) && isItemChain(current)) {
    if (current.$text !== undefined) {
      items.push(current.$text);
    }
    if (!("item" in current)) break;
    current = current.item;
    if (Array.isArray(current)) {
      items.push(...current);
      break;
    }
  }
  return items.length > 0 ? items : undefined;
}

/** Unwrap XML-ish `$text` / nested `item` chains into flat lists and plain strings. */
export function normalizeDisplayValue(value: unknown): unknown {
  if (typeof value === "string") {
    return cleanXmlLeak(value);
  }
  if (Array.isArray(value)) {
    const out: unknown[] = [];
    for (const item of value) {
      if (isPlainObject(item)) {
        const chain = flattenItemChain(item);
        if (chain) {
          out.push(...chain.map(normalizeDisplayValue));
          continue;
        }
      }
      out.push(normalizeDisplayValue(item));
    }
    return out;
  }
  if (!isPlainObject(value)) {
    return value;
  }
  const chain = flattenItemChain(value);
  if (chain) {
    return chain.map(normalizeDisplayValue);
  }
  const entries = Object.entries(value);
  if (entries.length === 1 && entries[0][0] === "$text") {
    return normalizeDisplayValue(entries[0][1]);
  }
  const out: Record<string, unknown> = {};
  for (const [key, child] of entries) {
    out[key] = normalizeDisplayValue(child);
  }
  return out;
}

function scalarText(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "string") return value;
  if (typeof value === "boolean" || typeof value === "number") return String(value);
  return formatIndentedValue(JSON.stringify(value));
}

function isNested(value: unknown): boolean {
  if (Array.isArray(value)) return value.length > 0;
  if (isPlainObject(value)) return Object.keys(value).length > 0;
  return typeof value === "string" && (value.includes("\n") || value.length > 80);
}

function AttributeRow(props: { label: string; value: unknown }) {
  const nested = () => isNested(props.value);
  const [open, setOpen] = createSignal(true);

  return (
    <div class="node-completed-attr">
      <div
        class="node-completed-attr-row"
        classList={{ "node-completed-attr-row--nested": nested() }}
        onClick={() => setOpen((value) => !value)}
      >
        <button
          type="button"
          class="node-completed-chevron"
          classList={{ "node-completed-chevron--expanded": open() }}
          aria-expanded={open()}
          aria-label={open() ? `Hide ${props.label}` : `Show ${props.label}`}
          onClick={(event) => {
            event.stopPropagation();
            setOpen((value) => !value);
          }}
        >
          <ChevronRight width={14} height={14} />
        </button>
        <span class="node-completed-attr-key">{props.label}</span>
        <span class="node-completed-attr-sep">:</span>
        <Show when={open() && !nested()}>
          <span class="node-completed-attr-value">{scalarText(props.value)}</span>
        </Show>
        <Show when={!open()}>
          <span class="node-completed-attr-value node-completed-attr-value--hidden">…</span>
        </Show>
      </div>
      <Show when={open() && nested()}>
        <div class="node-completed-attr-children">
          <Show
            when={isPlainObject(props.value) || Array.isArray(props.value)}
            fallback={
              <pre class="node-completed-attr-block">{scalarText(props.value)}</pre>
            }
          >
            <ValueTree value={props.value} />
          </Show>
        </div>
      </Show>
    </div>
  );
}

function ObjectTree(props: { value: Record<string, unknown> }): JSX.Element {
  const text = () => props.value.$text;
  const rest = () => Object.entries(props.value).filter(([key]) => key !== "$text");

  return (
    <>
      <Show when={text() !== undefined}>
        <pre class="node-completed-attr-block">{scalarText(text())}</pre>
      </Show>
      <For each={rest()}>{([key, child]) => <AttributeRow label={key} value={child} />}</For>
    </>
  );
}

function ArrayItems(props: { items: unknown[] }) {
  return (
    <div class="node-completed-list">
      <For each={props.items}>
        {(item) => (
          <div class="node-completed-list-item">
            <span class="node-completed-list-bullet" aria-hidden="true">
              •
            </span>
            <Show
              when={isPlainObject(item) || Array.isArray(item)}
              fallback={<span class="node-completed-attr-value">{scalarText(item)}</span>}
            >
              <div class="node-completed-list-item-body">
                <ValueTree value={item} />
              </div>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
}

function ValueTree(props: { value: unknown }) {
  if (Array.isArray(props.value)) {
    return <ArrayItems items={props.value} />;
  }
  if (isPlainObject(props.value)) {
    return <ObjectTree value={props.value} />;
  }
  return <pre class="node-completed-attr-block">{scalarText(props.value)}</pre>;
}

export function NodeCompletedBubble(props: NodeCompletedBubbleProps) {
  const parsed = () => {
    const raw = tryParseJson(props.summary);
    return raw === undefined ? undefined : normalizeDisplayValue(raw);
  };

  return (
    <div class="node-completed-row" role="status" aria-live="polite">
      <div class="node-completed-bubble">
        <div class="node-completed-header">
          <span class="node-completed-icon" aria-hidden="true">
            ✓
          </span>
          <span>Node completed</span>
        </div>
        <Show
          when={parsed()}
          fallback={<pre class="node-completed-summary">{formatIndentedValue(props.summary)}</pre>}
        >
          {(value) => (
            <div class="node-completed-tree">
              <ValueTree value={value()} />
            </div>
          )}
        </Show>
      </div>
    </div>
  );
}
