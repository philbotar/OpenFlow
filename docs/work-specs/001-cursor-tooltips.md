# WS-001: Cursor-style chrome tooltips

- **Date:** 2026-07-25
- **Scope:** Design only (approved). Custom delayed tooltips for UI chrome + labeled primary actions that have shortcuts; surface existing shortcuts and add a small set of missing chrome shortcuts. Implementation follows in a later work spec / plan.
- **Outcome:** Done (design approved)

## What we did

Locked product decisions after audit + brainstorm:

1. **Fidelity:** Custom delayed tooltip (Cursor-like), not native `title` alone.
2. **Scope:** Icon / unlabeled chrome first, then labeled primary actions that have shortcuts (Run, Save, Stop). Skip redundant tooltips on text buttons that already say their name and have no shortcut.
3. **Shortcuts:** Surface all existing wired keys; also add a few missing chrome shortcuts that map to real icon buttons (no orphan shortcuts).
4. **Implementation approach:** Shared Solid `<Tooltip>` + central shortcut registry (no new positioning library).

### Tooltip UX

- Show after **~400ms** hover or keyboard focus; hide on pointer leave, blur, or click.
- Content: **intention label** on the left; **shortcut** on the right as discrete key chips (e.g. `⌘` `S`).
- Position: prefer below trigger; flip above if clipped. Render in a **fixed portal** so `overflow: hidden` parents do not clip.
- Keep **`aria-label`** for accessibility.
- Where Tooltip is wired, **remove native `title=`** to avoid double tooltips.
- Disabled controls: no tooltip unless a distinct `disabledReason` is passed (optional; not required on every control).

### Shortcut registry

Central module (e.g. `crates/ui/src/lib/shortcuts/`) owns:

- Stable action ids
- Key chords
- Display formatting via existing `isMacOS()` (`⌘` vs `Ctrl`, `↵`, etc.)
- Labels used by tooltips and by the keydown handler (single source of truth)

#### Existing (surface only)

| Action id | Chord | Chrome |
| --- | --- | --- |
| `save` | Mod+S | Save (header) |
| `run` / `continue` | Mod+Enter | Run / Continue (header) |
| `stop` | Mod+. | Stop (header) |
| `toggleLeftSidebar` | Mod+B | Sidebar toggle (header) |
| `toggleRightPanel` | Mod+J | Right panel (inspector stack) |
| `zoomIn` / `zoomOut` / `zoomReset` | Mod+= / Mod+- / Mod+0 | No dedicated chrome button — registry only; do not invent toolbar buttons |

#### New (wire keydown + tooltip)

| Action id | Chord | Chrome | Notes |
| --- | --- | --- | --- |
| `toggleInspector` | Mod+I | Header Inspector icon | Editor screen only; same guards as other editor chords (`!isTextInputTarget` where applicable) |
| `toggleChatFocus` | Mod+Shift+F | Dock focus / show-canvas icon | Editor + dock context |

**Explicitly not added:** Mod+\` dock collapse. Dock open/collapse is resize-driven today; there is no dedicated toggle chrome, so a shortcut would be an orphan.

### Architecture

```text
lib/shortcuts/          # registry + formatShortcutDisplay + chord match helpers
components/Tooltip/     # Solid portal tooltip (delay, position, kbd chips)
styles (tokens/index)   # .app-tooltip* using --z-tooltip
keydown (useAppProviderState)  # consume registry for new + existing chords
```

- **Solid UI:** wrap triggers with `<Tooltip label shortcut?>…</Tooltip>` or pass `tooltip` / `shortcut` through shared primitives (`SidebarIconButton`, optionally `Button` when used as icon chrome).
- **React canvas island** (`WorkflowCanvas.react.tsx`, `WorkflowNode.react.tsx`): thin React twin sharing the **same CSS classes** (no second visual language). No Floating UI dependency. Canvas actions get labels; shortcuts only if already wired (most canvas actions have none today).
- Replace covered native `title=` strings during wiring; leave `title=` on non-chrome surfaces (truncation overflow on names, empty states, readiness chip message, etc.).

### Wiring groups (implementation batches)

1. **Primitives** — `Tooltip` (Solid), React twin CSS-sharing helper, shortcut registry, CSS, unit tests for format + delay show/hide.
2. **AppHeader** — sidebar toggle, Run/Continue/Stop, Inspector, Workflow settings (label only; save shortcut stays on Save), Save, Git (label only, no new shortcut).
3. **Sidebar icon chrome** — `SidebarIconButton` and other icon-only sidebar actions.
4. **Canvas toolbar / node chrome** — arrange, delete, interrupt, retry (React twin).
5. **Dock icon actions** — focus panel / show canvas + Mod+Shift+F.
6. **Sweep** — strip leftover native `title=` on covered chrome; confirm labeled text tabs (Chat, Terminal, …) stay without redundant tooltips.

### Out of scope

- Form fields, dense settings lists, modal primary text buttons without shortcuts
- Inventing shortcuts for every nav / settings item
- Command palette / shortcuts help sheet (may consume the registry later)
- Changing zoom UX chrome

### Testing

- Unit: shortcut display formatting (Mac vs non-Mac); Tooltip show after delay / hide on leave (jsdom timers).
- Existing AppHeader / EditorScreen tests: update queries if they relied on `title`; prefer `aria-label` (already used widely).
- Verify: `npm --prefix crates/ui run typecheck` and `npm --prefix crates/ui run test`.

### Error handling

- Missing registry entry: tooltip shows label only (no empty kbd row).
- Unknown OS: treat as non-Mac (`Ctrl`) via existing `isMacOS()` helper.

## Files touched

- `docs/work-specs/001-cursor-tooltips.md` (this design)
- `docs/work-specs/AGENTS.md` (numbering rules)
- `docs/work-specs/template.md`
- `docs/AGENTS.md` (File Map)
- `docs/README.md` (index entry for work-specs)

## Rationale

Native `title` is already used inconsistently (AppHeader has some shortcuts; much chrome does not). A shared delayed tooltip + registry gives Cursor-like discoverability without a new dependency, keeps chords in one place for keydown and UI, and limits noise by skipping redundant labeled buttons.

## Issues encountered

None.
