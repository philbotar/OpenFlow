# WS-002: Cursor-style chrome tooltips — implementation plan

- **Date:** 2026-07-25
- **Scope:** Implementation plan for WS-001 (Cursor-style chrome tooltips + shortcut registry + Mod+I / Mod+Shift+F). Execution not started.
- **Outcome:** Partial

## What we did

Wrote the implementation plan below from approved design [`001-cursor-tooltips.md`](001-cursor-tooltips.md).

## Files touched

| Path | Change |
|------|--------|
| `docs/work-specs/002-cursor-tooltips-plan.md` | this plan |
| `docs/AGENTS.md` | File Map entry |

## Rationale

Task order builds registry → Tooltip primitives → keydown → chrome wiring → canvas React twin → dock → sweep. Each task ends with a runnable test or typecheck so a subagent can gate independently. New chords only for Inspector and chat focus (no orphan dock shortcut).

## Issues encountered

None.

---

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Cursor-like delayed tooltips (label + kbd chips) on UI chrome and shortcut-bearing primary actions, with a single shortcut registry driving display and keydown (including new Mod+I / Mod+Shift+F).

**Architecture:** Pure `lib/shortcuts` registry formats and matches chords. Solid `Tooltip` portals a delayed popover; React canvas island uses a thin twin sharing `.app-tooltip*` CSS. Call sites pass intention `label` + optional `shortcutId`; remove native `title=` where Tooltip is wired.

**Tech Stack:** SolidJS, React (canvas island only), Vitest/jsdom, existing `isMacOS()` / `isTextInputTarget()`, CSS tokens (`--z-tooltip`). No new npm deps.

## Global Constraints

- No new positioning / tooltip library.
- Delay ≈ 400ms; portal + flip; keep `aria-label`; strip `title=` on covered chrome.
- Scope: icon/unlabeled chrome + Run/Save/Stop (and Continue). Skip labeled text buttons without shortcuts (dock tabs, “Add node”, etc.).
- New shortcuts only: Mod+I inspector, Mod+Shift+F chat focus. No Mod+\` dock collapse.
- Follow `crates/ui/AGENTS.md` folder layout (`components/Tooltip/`, `lib/shortcuts/`).
- Verify with `npm --prefix crates/ui run typecheck` and `npm --prefix crates/ui run test`.

## File structure

| Path | Responsibility |
|------|----------------|
| `crates/ui/src/lib/shortcuts/index.ts` | Chord table, `formatShortcutParts`, `eventMatchesShortcut` |
| `crates/ui/src/lib/shortcuts/shortcuts.test.ts` | Mac/non-Mac formatting + match tests |
| `crates/ui/src/components/Tooltip/Tooltip.tsx` | Solid portal tooltip |
| `crates/ui/src/components/Tooltip/Tooltip.test.tsx` | Delay show/hide |
| `crates/ui/src/components/Tooltip/index.ts` | Barrel |
| `crates/ui/src/canvas/AppTooltip.react.tsx` | React twin sharing CSS |
| `crates/ui/src/styles/index.css` | `.app-tooltip*` rules |
| `crates/ui/src/context/appProvider/useAppProviderState.ts` | Keydown via registry + new chords |
| `crates/ui/src/components/AppHeader.tsx` | Header chrome tooltips |
| `crates/ui/src/components/sidebar/SidebarIconButton.tsx` | Icon button tooltip |
| `crates/ui/src/canvas/WorkflowNode.react.tsx` | Interrupt/Retry tooltips |
| `crates/ui/src/panels/DockPanel.tsx` | Focus-mode tooltip + shortcut |
| `crates/ui/src/components/index.ts` | Export Tooltip |

---

### Task 1: Shortcut registry

**Files:**
- Create: `crates/ui/src/lib/shortcuts/index.ts`
- Create: `crates/ui/src/lib/shortcuts/shortcuts.test.ts`

**Interfaces:**
- Consumes: `isMacOS` from `../utils`
- Produces:
  - `export type ShortcutId = "save" | "run" | "stop" | "toggleLeftSidebar" | "toggleRightPanel" | "zoomIn" | "zoomOut" | "zoomReset" | "toggleInspector" | "toggleChatFocus"`
  - `export function formatShortcutParts(id: ShortcutId, mac?: boolean): string[]`
  - `export function eventMatchesShortcut(event: KeyboardEvent, id: ShortcutId): boolean`

- [ ] **Step 1: Write the failing test**

```ts
// @vitest-environment jsdom
import { describe, expect, test, vi } from "vitest";
import { formatShortcutParts, eventMatchesShortcut } from "./index";

describe("formatShortcutParts", () => {
  test("formats save on Mac", () => {
    expect(formatShortcutParts("save", true)).toEqual(["⌘", "S"]);
  });

  test("formats save on non-Mac", () => {
    expect(formatShortcutParts("save", false)).toEqual(["Ctrl", "S"]);
  });

  test("formats run Enter", () => {
    expect(formatShortcutParts("run", true)).toEqual(["⌘", "↵"]);
  });

  test("formats toggleChatFocus with Shift", () => {
    expect(formatShortcutParts("toggleChatFocus", true)).toEqual(["⌘", "⇧", "F"]);
  });
});

describe("eventMatchesShortcut", () => {
  test("matches Mod+I for toggleInspector", () => {
    const event = new KeyboardEvent("keydown", { key: "i", metaKey: true });
    expect(eventMatchesShortcut(event, "toggleInspector")).toBe(true);
  });

  test("requires Shift for toggleChatFocus", () => {
    const withShift = new KeyboardEvent("keydown", {
      key: "f",
      metaKey: true,
      shiftKey: true,
    });
    const without = new KeyboardEvent("keydown", { key: "f", metaKey: true });
    expect(eventMatchesShortcut(withShift, "toggleChatFocus")).toBe(true);
    expect(eventMatchesShortcut(without, "toggleChatFocus")).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix crates/ui run test -- src/lib/shortcuts/shortcuts.test.ts`  
Expected: FAIL (module missing)

- [ ] **Step 3: Write minimal implementation**

```ts
import { isMacOS } from "../utils";

export type ShortcutId =
  | "save"
  | "run"
  | "stop"
  | "toggleLeftSidebar"
  | "toggleRightPanel"
  | "zoomIn"
  | "zoomOut"
  | "zoomReset"
  | "toggleInspector"
  | "toggleChatFocus";

type Chord = { key: string; shift?: boolean };

const CHORDS: Record<ShortcutId, Chord> = {
  save: { key: "s" },
  run: { key: "Enter" },
  stop: { key: "." },
  toggleLeftSidebar: { key: "b" },
  toggleRightPanel: { key: "j" },
  zoomIn: { key: "=" },
  zoomOut: { key: "-" },
  zoomReset: { key: "0" },
  toggleInspector: { key: "i" },
  toggleChatFocus: { key: "f", shift: true },
};

const DISPLAY_KEY: Record<string, string> = {
  Enter: "↵",
  "=": "+",
  "-": "−",
};

export function formatShortcutParts(id: ShortcutId, mac = isMacOS()): string[] {
  const chord = CHORDS[id];
  const parts = [mac ? "⌘" : "Ctrl"];
  if (chord.shift) parts.push(mac ? "⇧" : "Shift");
  const key = chord.key.length === 1 ? chord.key.toUpperCase() : (DISPLAY_KEY[chord.key] ?? chord.key);
  parts.push(key);
  return parts;
}

export function eventMatchesShortcut(event: KeyboardEvent, id: ShortcutId): boolean {
  const chord = CHORDS[id];
  const mod = event.metaKey || event.ctrlKey;
  if (!mod) return false;
  if (Boolean(chord.shift) !== event.shiftKey) return false;
  const key = event.key;
  if (chord.key === "Enter") return key === "Enter";
  if (chord.key === "=") return key === "=" || key === "+";
  if (chord.key === "-") return key === "-" || key === "_";
  if (chord.key === ".") return key === ".";
  return key.toLowerCase() === chord.key.toLowerCase();
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix crates/ui run test -- src/lib/shortcuts/shortcuts.test.ts`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/lib/shortcuts/
git commit -m "$(cat <<'EOF'
Add UI shortcut registry for tooltip and keydown.

Centralize chord definitions and Mac/Ctrl display parts so chrome tooltips and handlers share one source of truth.
EOF
)"
```

---

### Task 2: Solid Tooltip + CSS

**Files:**
- Create: `crates/ui/src/components/Tooltip/Tooltip.tsx`
- Create: `crates/ui/src/components/Tooltip/Tooltip.test.tsx`
- Create: `crates/ui/src/components/Tooltip/index.ts`
- Modify: `crates/ui/src/components/index.ts` (add `export * from "./Tooltip";`)
- Modify: `crates/ui/src/styles/index.css` (append `.app-tooltip*` block near other chrome styles)

**Interfaces:**
- Consumes: `formatShortcutParts`, `ShortcutId` from `@/lib/shortcuts` (or relative `../../lib/shortcuts`)
- Produces:
  - `export type TooltipProps = { label: string; shortcutId?: ShortcutId; disabledReason?: string; children: JSX.Element }`
  - `export function Tooltip(props: TooltipProps): JSX.Element`

- [ ] **Step 1: Write the failing test**

```tsx
// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { Tooltip } from "./Tooltip";

describe("Tooltip", () => {
  let container: HTMLDivElement;
  let dispose: () => void;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement("div");
    document.body.append(container);
  });

  afterEach(() => {
    dispose?.();
    container?.remove();
    vi.useRealTimers();
    document.querySelectorAll(".app-tooltip").forEach((el) => el.remove());
  });

  test("shows label and shortcut chips after delay on hover", () => {
    dispose = render(
      () => (
        <Tooltip label="Save workflow" shortcutId="save">
          <button type="button" aria-label="Save workflow">
            Save
          </button>
        </Tooltip>
      ),
      container,
    );

    const trigger = container.querySelector("button")!;
    trigger.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
    expect(document.querySelector(".app-tooltip")).toBeNull();

    vi.advanceTimersByTime(400);
    const tip = document.querySelector(".app-tooltip");
    expect(tip).not.toBeNull();
    expect(tip?.textContent).toContain("Save workflow");
    expect(tip?.querySelectorAll(".app-tooltip-key").length).toBeGreaterThan(0);
  });

  test("hides on pointer leave", () => {
    dispose = render(
      () => (
        <Tooltip label="Inspector">
          <button type="button" aria-label="Inspector">
            I
          </button>
        </Tooltip>
      ),
      container,
    );
    const trigger = container.querySelector("button")!;
    trigger.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
    vi.advanceTimersByTime(400);
    trigger.dispatchEvent(new PointerEvent("pointerleave", { bubbles: true }));
    expect(document.querySelector(".app-tooltip")).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix crates/ui run test -- src/components/Tooltip/Tooltip.test.tsx`  
Expected: FAIL (module missing)

- [ ] **Step 3: Write minimal Tooltip + CSS**

`Tooltip.tsx` sketch (implement fully in task):

```tsx
import { Show, createSignal, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { formatShortcutParts, type ShortcutId } from "../../lib/shortcuts";

const SHOW_DELAY_MS = 400;

export type TooltipProps = {
  label: string;
  shortcutId?: ShortcutId;
  disabledReason?: string;
  children: JSX.Element;
};

export function Tooltip(props: TooltipProps) {
  const [open, setOpen] = createSignal(false);
  const [coords, setCoords] = createSignal({ top: 0, left: 0 });
  let timer: number | undefined;
  let triggerEl: HTMLElement | undefined;

  const clearTimer = () => {
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timer = undefined;
    }
  };

  const hide = () => {
    clearTimer();
    setOpen(false);
  };

  const scheduleShow = (el: HTMLElement) => {
    clearTimer();
    triggerEl = el;
    timer = window.setTimeout(() => {
      const rect = el.getBoundingClientRect();
      const tipHeight = 32;
      const spaceBelow = window.innerHeight - rect.bottom;
      const top =
        spaceBelow < tipHeight + 8 ? rect.top - tipHeight - 6 : rect.bottom + 6;
      setCoords({ top, left: rect.left + rect.width / 2 });
      setOpen(true);
    }, SHOW_DELAY_MS);
  };

  onCleanup(hide);

  const text = () => props.disabledReason ?? props.label;
  const parts = () =>
    props.disabledReason || !props.shortcutId
      ? []
      : formatShortcutParts(props.shortcutId);

  return (
    <>
      <span
        class="app-tooltip-trigger"
        onPointerEnter={(e) => {
          const el = e.currentTarget;
          if (el instanceof HTMLElement) scheduleShow(el);
        }}
        onPointerLeave={hide}
        onFocusIn={(e) => {
          const el = e.currentTarget;
          if (el instanceof HTMLElement) scheduleShow(el);
        }}
        onFocusOut={hide}
        onPointerDown={hide}
      >
        {props.children}
      </span>
      <Show when={open()}>
        <Portal>
          <div
            class="app-tooltip"
            style={{
              top: `${coords().top}px`,
              left: `${coords().left}px`,
            }}
            role="tooltip"
          >
            <span class="app-tooltip-label">{text()}</span>
            <Show when={parts().length > 0}>
              <span class="app-tooltip-keys" aria-hidden="true">
                {parts().map((part) => (
                  <kbd class="app-tooltip-key">{part}</kbd>
                ))}
              </span>
            </Show>
          </div>
        </Portal>
      </Show>
    </>
  );
}
```

CSS (append to `styles/index.css`):

```css
.app-tooltip-trigger {
  display: contents;
}

.app-tooltip {
  position: fixed;
  z-index: var(--z-tooltip);
  transform: translateX(-50%);
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: 6px;
  background: var(--bg-elevated, var(--panel));
  color: var(--text);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-sm, 0 4px 16px rgba(0, 0, 0, 0.18));
  font-size: 12px;
  line-height: 1.2;
  pointer-events: none;
  white-space: nowrap;
}

.app-tooltip-keys {
  display: inline-flex;
  gap: 3px;
  margin-left: 2px;
}

.app-tooltip-key {
  display: inline-flex;
  min-width: 1.25em;
  justify-content: center;
  padding: 1px 4px;
  border-radius: 4px;
  border: 1px solid var(--border);
  background: var(--bg-muted, var(--surface));
  font: inherit;
  font-size: 11px;
}
```

Use existing token names from `tokens.css` if `--bg-elevated` / `--panel` differ — match neighboring chrome styles; do not invent a purple theme.

`index.ts`: `export * from "./Tooltip";`  
`components/index.ts`: add `export * from "./Tooltip";`

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix crates/ui run test -- src/components/Tooltip/Tooltip.test.tsx`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/components/Tooltip/ crates/ui/src/components/index.ts crates/ui/src/styles/index.css
git commit -m "$(cat <<'EOF'
Add Solid Tooltip with delayed portal and kbd chips.

Shared chrome tooltip uses existing z-index token and shortcut registry parts.
EOF
)"
```

---

### Task 3: React AppTooltip twin

**Files:**
- Create: `crates/ui/src/canvas/AppTooltip.react.tsx`
- Create: `crates/ui/src/canvas/AppTooltip.react.test.ts` (or `.tsx` if JSX needed — prefer `.tsx`)

**Interfaces:**
- Consumes: `formatShortcutParts`, `ShortcutId` from `../lib/shortcuts`
- Produces: `export function AppTooltip(props: { label: string; shortcutId?: ShortcutId; children: React.ReactElement }): React.ReactElement`

- [ ] **Step 1: Write failing test** (fake timers, assert `.app-tooltip` after 400ms hover)

Mirror Solid test behavior with `@testing-library/react` already in package.json.

- [ ] **Step 2: Run — expect FAIL**

Run: `npm --prefix crates/ui run test -- src/canvas/AppTooltip.react.test.tsx`

- [ ] **Step 3: Implement React twin**

Reuse same delay (400), portal via `createPortal` to `document.body`, same class names (`.app-tooltip`, `.app-tooltip-label`, `.app-tooltip-key`). Position with `getBoundingClientRect` + flip. Do **not** duplicate CSS.

- [ ] **Step 4: Run — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/canvas/AppTooltip.react.tsx crates/ui/src/canvas/AppTooltip.react.test.tsx
git commit -m "$(cat <<'EOF'
Add React AppTooltip twin for canvas chrome.

Shares app-tooltip CSS with the Solid Tooltip so the React Flow island matches header chrome.
EOF
)"
```

---

### Task 4: Keydown — use registry + new chords

**Files:**
- Modify: `crates/ui/src/context/appProvider/useAppProviderState.ts` (keydown block ~301–360)

**Interfaces:**
- Consumes: `eventMatchesShortcut` from `../../lib/shortcuts`
- Produces: same public context API; behavior adds Mod+I → `handleToggleInspector`, Mod+Shift+F → `handleToggleChatFocusMode` when `screen() === "editor"` and not text-input (for inspector / focus, use `!isTextInputTarget` like Mod+B/J)

- [ ] **Step 1: Write / extend a focused unit or AppHeader/Editor test that dispatches keydown**

Prefer a small test near existing keyboard coverage in `App.test.tsx` or add assertions in an existing editor keyboard test if one exists. Minimal new test:

```ts
// In an existing App.test.tsx describe, or new file if cheaper:
// after mounting editor-ready app, dispatch meta+i and assert inspector opens
// (use aria-pressed / aria-label="Inspector" patterns already in App.test.tsx)
```

If full App harness is too heavy for Mod+Shift+F, add a tiny extracted helper test for match-only and wire keydown without a new E2E — still manually verify both chords in Step 4 typecheck + existing suite.

- [ ] **Step 2: Refactor keydown to prefer `eventMatchesShortcut` for existing chords where drop-in safe**

Replace Mod+S / Mod+Enter / Mod+. / Mod+B / Mod+J / zoom branches with `eventMatchesShortcut(event, id)` checks. Keep screen / text-input / continuable-run guards identical.

Add:

```ts
if (
  eventMatchesShortcut(event, "toggleInspector") &&
  !isTextInputTarget(event.target) &&
  appShell.screen() === "editor"
) {
  event.preventDefault();
  workflowEditor.handleToggleInspector();
  return;
}
if (
  eventMatchesShortcut(event, "toggleChatFocus") &&
  !isTextInputTarget(event.target) &&
  appShell.screen() === "editor"
) {
  event.preventDefault();
  dock.handleToggleChatFocusMode();
  return;
}
```

- [ ] **Step 3: Run UI tests**

Run: `npm --prefix crates/ui run test -- src/context src/app/App.test.tsx`  
Expected: PASS (fix any title-based assertions only if this task touched them — prefer leave title changes to Task 5)

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/context/appProvider/useAppProviderState.ts crates/ui/src/app/App.test.tsx
git commit -m "$(cat <<'EOF'
Wire inspector and chat-focus shortcuts via registry.

Reuse eventMatchesShortcut for existing chords and add Mod+I / Mod+Shift+F on the editor screen.
EOF
)"
```

---

### Task 5: AppHeader tooltips

**Files:**
- Modify: `crates/ui/src/components/AppHeader.tsx`
- Modify: `crates/ui/src/components/AppHeader.test.tsx` (assert via `aria-label`, not `title`)

**Interfaces:**
- Consumes: `Tooltip` from `./Tooltip` (or `@/components`), `ShortcutId` values
- Produces: unchanged context usage; buttons wrapped in Tooltip; **no** `title=` on covered controls

- [ ] **Step 1: Update AppHeader.test if it queries `title` for sidebar/save/run**

Grep `AppHeader.test.tsx` for `title`; switch to `aria-label` / role queries.

- [ ] **Step 2: Wrap header controls**

| Control | label (dynamic OK) | shortcutId |
|---------|--------------------|------------|
| Compact nav / sidebar toggle | Show/Hide sidebar or Open navigation | `toggleLeftSidebar` when desktop toggle |
| Run | Run workflow without a starter message (or readiness message as `disabledReason`) | `run` when ready |
| Continue | Continue the paused workflow run | `run` (same chord) |
| Stop | Stop | `stop` |
| Git | Git | none |
| Inspector | Inspector | `toggleInspector` |
| Workflow settings | Workflow settings | none (save chord stays on Save) |
| Save | Save | `save` |

Remove local `mod()` title string building for those controls. Keep readiness chip `title=` (non-chrome message).

Example:

```tsx
<Tooltip
  label={ctx.leftPanelHidden() ? "Show sidebar" : "Hide sidebar"}
  shortcutId="toggleLeftSidebar"
>
  <button … aria-label={…} /* no title */>
    …
  </button>
</Tooltip>
```

- [ ] **Step 3: Run**

Run: `npm --prefix crates/ui run test -- src/components/AppHeader.test.tsx`  
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/components/AppHeader.tsx crates/ui/src/components/AppHeader.test.tsx
git commit -m "$(cat <<'EOF'
Wire AppHeader chrome to Tooltip and shortcut chips.

Replace native title strings on run/save/stop/sidebar/inspector with delayed tooltips.
EOF
)"
```

---

### Task 6: SidebarIconButton

**Files:**
- Modify: `crates/ui/src/components/sidebar/SidebarIconButton.tsx`

**Interfaces:**
- Consumes: `Tooltip`
- Produces: same props; drop `title={props.label}`; keep `aria-label={props.label}`

- [ ] **Step 1: Wrap button**

```tsx
import { Tooltip } from "../Tooltip";

export function SidebarIconButton(props: SidebarIconButtonProps) {
  return (
    <Tooltip label={props.label}>
      <button
        type="button"
        class={props.class ? `sidebar-icon-button ${props.class}` : "sidebar-icon-button"}
        classList={{ active: props.active }}
        aria-label={props.label}
        onClick={() => props.onClick()}
      >
        <SidebarIcon name={props.icon} />
      </button>
    </Tooltip>
  );
}
```

Leave `ProjectFolderRow` “Add workflow” for this task only if it is icon-only chrome using a raw button with `title` — if so, wrap that icon button the same way (label only, no shortcut). Skip truncation `title=` on workflow/project names.

- [ ] **Step 2: Run sidebar-related tests if present; else typecheck**

Run: `npm --prefix crates/ui run typecheck`  
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/components/sidebar/SidebarIconButton.tsx
git commit -m "$(cat <<'EOF'
Use Tooltip on SidebarIconButton instead of native title.

Icon-only sidebar actions get the shared delayed tooltip chrome.
EOF
)"
```

---

### Task 7: Canvas node icon actions (React)

**Files:**
- Modify: `crates/ui/src/canvas/WorkflowNode.react.tsx`
- Modify: `crates/ui/src/canvas/WorkflowCanvas.react.tsx` only if an **icon-only** control lacks a tooltip; do **not** wrap labeled “Add node” / “Auto layout” / “Delete” text buttons (scope: skip redundant labeled text). For Auto layout, **remove** native `title=` (label already visible) rather than adding Tooltip.

**Interfaces:**
- Consumes: `AppTooltip` from `./AppTooltip.react`

- [ ] **Step 1: Wrap Interrupt / Retry**

```tsx
<AppTooltip label="Interrupt node">
  <button type="button" className="…" aria-label="Interrupt node" onClick={…}>
    ■
  </button>
</AppTooltip>
```

Same for Retry. Remove `title=`.

- [ ] **Step 2: Strip Auto layout `title=`** (labeled text, no shortcut)

- [ ] **Step 3: Run**

Run: `npm --prefix crates/ui run test -- src/canvas/`  
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/canvas/WorkflowNode.react.tsx crates/ui/src/canvas/WorkflowCanvas.react.tsx
git commit -m "$(cat <<'EOF'
Add canvas node action tooltips via React AppTooltip.

Interrupt/retry icon buttons use shared tooltip CSS; drop redundant Auto layout title.
EOF
)"
```

---

### Task 8: Dock focus tooltip + shortcut chip

**Files:**
- Modify: `crates/ui/src/panels/DockPanel.tsx`

**Interfaces:**
- Consumes: Solid `Tooltip`, `shortcutId="toggleChatFocus"`

- [ ] **Step 1: Wrap focus icon button**

```tsx
<Tooltip
  label={ctx.chatFocusMode() ? "Show canvas" : "Focus panel"}
  shortcutId="toggleChatFocus"
>
  <button
    type="button"
    class="dock-icon-action dock-focus-action"
    aria-label={ctx.chatFocusMode() ? "Show canvas" : "Focus panel"}
    aria-pressed={ctx.chatFocusMode()}
    onClick={() => ctx.handleToggleChatFocusMode()}
  >
    …
  </button>
</Tooltip>
```

Remove `title=`. Do **not** add tooltips to Chat / Terminal / Run trace / History text tabs.

- [ ] **Step 2: Run EditorScreen / App tests that query Focus panel**

Run: `npm --prefix crates/ui run test -- src/screens/EditorScreen.test.tsx src/app/App.test.tsx`  
Expected: PASS (`aria-label` queries already preferred)

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/panels/DockPanel.tsx
git commit -m "$(cat <<'EOF'
Tooltip dock focus action with Mod+Shift+F chip.

Icon-only focus toggle shows intention plus the new chat-focus shortcut.
EOF
)"
```

---

### Task 9: Sweep + verify gate

**Files:**
- Grep/modify any remaining covered chrome `title=` that should be Tooltip (header/sidebar icon/dock focus/canvas interrupt-retry). Leave truncation titles, empty states, readiness chip, settings section headers.

**Files likely untouched on purpose:** `ScheduleScreen` save/remove icon actions — if icon-only and in chrome scope, wrap with Tooltip (label only) in this sweep; if timeboxed, at least list residual icon `title=` in the commit body.

- [ ] **Step 1: Grep audit**

Run: `rg -n 'title=' crates/ui/src/components crates/ui/src/panels crates/ui/src/canvas crates/ui/src/screens --glob '*.tsx'`  
For each hit: keep or convert per WS-001 scope.

- [ ] **Step 2: Full UI verify**

Run:

```bash
npm --prefix crates/ui run typecheck
npm --prefix crates/ui run test
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src
git commit -m "$(cat <<'EOF'
Finish tooltip chrome sweep and clear UI verify.

Convert remaining in-scope icon titles and confirm typecheck plus full UI tests.
EOF
)"
```

- [ ] **Step 4: Mark WS-002 Outcome → Done when implementation complete; update WS-001 only if design drift occurred (prefer not).**

---

## Spec coverage checklist (self-review)

| WS-001 requirement | Task |
| --- | --- |
| Custom delayed tooltip ~400ms | Task 2 |
| Label + kbd chips | Task 2 |
| Portal + flip | Task 2 |
| Keep aria-label; strip title on wire | Tasks 5–8 |
| Shortcut registry + isMacOS | Task 1 |
| Surface existing chords | Tasks 1, 4, 5 |
| New Mod+I / Mod+Shift+F | Tasks 4, 5, 8 |
| No Mod+\` dock | Global Constraints |
| Solid chrome wiring groups | Tasks 5, 6, 8 |
| React canvas twin | Tasks 3, 7 |
| Skip labeled text without shortcuts | Tasks 7, 8 |
| Tests + typecheck | Tasks 1–9 |
| No new deps | Global Constraints |
