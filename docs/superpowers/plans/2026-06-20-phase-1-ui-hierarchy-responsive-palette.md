# Phase 1 UI Hierarchy, Responsive Shell, and Palette Contract Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Phase 1 design fixes from the UI audit: restore editor usability on small screens, make the primary run action visually obvious, reduce dock competition with the canvas, and establish a unified color palette contract that stops screen-level CSS from drifting.

**Architecture:** Keep this slice in the `UI/Desktop seam` lane. Visual tokens and layout live in `crates/ui/src/styles/index.css`; header, sidebar, dock, and screen shells live in `crates/ui/src/components/`, `panels/`, and `App.tsx`. If the compact-screen fix requires drawer state, keep that state in `AppProvider` / `AppContext`; do not move any behavior into `desktop` or `orchestration`.

**Tech Stack:** SolidJS, TypeScript, CSS custom properties, existing Vitest suite, browser/manual breakpoint verification.

---

## Phase Boundary

### In Scope

- Define a unified palette contract and semantic alias layer inside `crates/ui/src/styles/index.css`.
- Fix the undefined semantic color/surface tokens already used by `Schedule` and `Workflow Authoring`.
- Rework editor-shell hierarchy for the header and dock.
- Make the editor usable at compact widths without stacking the full sidebar above the canvas.
- Add focused tests for the new UI state and critical shell behavior.

### Out Of Scope

- Full repo-wide CSS normalization and naming cleanup across every selector.
- Phase 2 work: typography overhaul, full-page route framing unification, empty-state redesign across all screens.
- Phase 3 work: motion system cleanup, conversation-surface unification, dark-mode polish beyond touched components.

### Phase 1 Deliverable

Phase 1 should leave the repo with:

1. A canonical palette contract that new UI work can reference.
2. Backward-compatible aliases so existing legacy tokens keep working during later normalization.
3. A compact-screen editor shell that preserves canvas access first.
4. A clearer primary action hierarchy in the topbar and dock.

---

## Proposed Palette Contract

Use a two-layer token model in `crates/ui/src/styles/index.css`:

### Layer 1: Base Palette

Warm neutrals + one cool accent family. Keep the existing visual direction instead of switching themes midstream.

| Token family | Intended role | Light direction | Dark direction |
| --- | --- | --- | --- |
| `--base-sand-*` | warm backgrounds / chrome | current `--app-bg`, `--app-bg-strong`, `--accent-soft` family | warm-charcoal equivalents |
| `--base-ink-*` | text / strong borders | current `#18181b`, muted warm grays | current dark text family |
| `--base-indigo-*` | primary action / focus / active UI | current `#6f7cf7`, `#5360d9`, soft focus tint | current dark-mode focus family |
| `--base-green-*` | success | current `--success`, `--success-soft` | existing dark success values |
| `--base-amber-*` | warning | current `--warning`, `--warning-soft` | existing dark warning values |
| `--base-red-*` | danger / destructive | current `--danger`, `--danger-soft` | existing dark danger values |

### Layer 2: Semantic UI Tokens

These are the tokens components should use after Phase 1:

| Semantic token | Role |
| --- | --- |
| `--surface-ground` | route/page background |
| `--surface-panel` | major panels and cards |
| `--surface-raised` | elevated chips, pills, smaller cards |
| `--surface-muted` | subtle contrast blocks |
| `--text-primary` | primary content |
| `--text-secondary` | supporting text |
| `--text-tertiary` | faint metadata |
| `--border-subtle` | low-emphasis dividers |
| `--border-strong` | selected/active containers |
| `--accent-primary` | primary actions and active emphasis |
| `--accent-primary-hover` | primary hover state |
| `--accent-soft` | tinted active backgrounds |
| `--status-success` / `--status-success-soft` | status states |
| `--status-warning` / `--status-warning-soft` | status states |
| `--status-danger` / `--status-danger-soft` | status states |

### Backward Compatibility Rule

Do **not** rename every existing token in Phase 1. Instead:

- Add the new base + semantic tokens.
- Map existing legacy tokens to them where practical.
- Migrate only the selectors touched in this slice.
- Leave a follow-up normalization pass for later once the semantic layer is stable.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `docs/superpowers/plans/2026-06-20-phase-1-ui-hierarchy-responsive-palette.md` | Durable implementation plan for this slice |
| `crates/ui/src/styles/index.css` | Base palette, semantic aliases, compact-shell CSS, header/dock hierarchy styling |
| `crates/ui/src/App.tsx` | Editor-shell composition if compact-screen sidebar mounting changes |
| `crates/ui/src/context/AppContext.tsx` | Compact-screen sidebar state contract if needed |
| `crates/ui/src/context/AppProvider.tsx` | Compact-screen viewport/drawer state, dock defaults, close-on-navigation behavior |
| `crates/ui/src/components/AppHeader.tsx` | Primary action hierarchy, compact nav trigger, grouped utility actions |
| `crates/ui/src/components/sidebar/AppSidebar.tsx` | Drawer-compatible sidebar behavior and close-on-select wiring |
| `crates/ui/src/panels/DockPanel.tsx` | Stronger tab hierarchy and active-panel context |
| `crates/ui/src/lib/utils.ts` | Compact dock sizing thresholds if Phase 1 changes dock min/collapsed behavior |
| `crates/ui/src/components/AppHeader.test.tsx` | Header hierarchy regression tests |
| `crates/ui/src/app/App.test.tsx` | Shell-level navigation / compact-sidebar tests |
| `crates/ui/src/screens/EditorScreen.test.tsx` | Editor shell regression tests |

---

## Acceptance Criteria

- At `390x844`, the canvas and topbar are visible without scrolling past the sidebar first.
- The primary run action is visually dominant and readable without icon memorization.
- The dock no longer competes with the canvas when empty or on compact screens.
- `Schedule` and `Workflow Authoring` no longer depend on undefined CSS tokens.
- All new/touched color usage references semantic palette tokens rather than ad hoc raw values.

---

## Task 1: Establish The Palette Contract And Stop Token Drift

**Files:**
- Modify: `crates/ui/src/styles/index.css`

- [ ] **Step 1: Add a base palette section near `:root`**

Create a clearly labeled token block for warm neutrals, indigo accent, and status hues in both light and dark themes.

- [ ] **Step 2: Add semantic aliases for surfaces, text, borders, accent, and status**

Define at minimum:

```css
--surface-ground
--surface-panel
--surface-raised
--text-primary
--text-secondary
--text-tertiary
--border-subtle
--accent-primary
--accent-primary-hover
```

- [ ] **Step 3: Bridge the existing token set instead of breaking it**

Keep legacy tokens such as `--app-bg`, `--surface`, `--text`, `--focus-strong`, `--success`, and `--danger` mapped onto the new layer where practical so Phase 1 stays safe.

- [ ] **Step 4: Fix the currently undefined semantic tokens already used later in the file**

Replace the current accidental fallthrough for:

- `--surface-ground`
- `--surface-panel`
- `--surface-raised`
- `--border-subtle`
- `--text-primary`
- `--text-secondary`

- [ ] **Step 5: Migrate touched selectors in this slice to semantic tokens**

Use the new semantic layer for:

- topbar
- sidebar
- dock
- primary/secondary buttons
- schedule screen
- workflow authoring screen

- [ ] **Step 6: Add a normalization inventory comment block**

At the end of the token section, leave a short comment listing major legacy token families still to normalize later. This is not the full cleanup pass; it is a breadcrumb for the follow-up slice.

- [ ] **Step 7: Verify no undefined semantic tokens remain in `index.css`**

Run:

```bash
rg -n -- '--surface-ground:|--surface-panel:|--surface-raised:|--border-subtle:|--text-primary:|--text-secondary:' crates/ui/src/styles/index.css
rg -n -- 'var\\(--surface-ground|var\\(--surface-panel|var\\(--surface-raised|var\\(--border-subtle|var\\(--text-primary|var\\(--text-secondary' crates/ui/src/styles/index.css
```

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/styles/index.css
git commit -m "plan(ui): establish phase 1 palette contract and semantic aliases"
```

---

## Task 2: Rebuild Header Hierarchy Around A Real Primary Action

**Files:**
- Modify: `crates/ui/src/components/AppHeader.tsx`
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/components/AppHeader.test.tsx`

- [ ] **Step 1: Split header actions into primary and utility groups**

Current problem: every action reads as a same-weight icon.

Target:

- one obvious primary run/continue button with text label
- destructive stop remains separate while active
- panel/save/validate stay in a utility group
- compact-screen nav trigger can live at the leading edge if Task 4 needs it

- [ ] **Step 2: Raise control size to a compact-touch-safe baseline**

Move icon-only controls away from `28x28` toward a semantic control size token in the `40-44px` range for compact viewports and no smaller than `32-36px` on desktop.

- [ ] **Step 3: Strengthen provider-readiness hierarchy**

Keep the readiness chip visually secondary to the run action. It should not be the loudest element in the topbar when the user is trying to execute a workflow.

- [ ] **Step 4: Update tests**

Add or update tests to assert:

- labeled primary run / continue action renders
- stop state still renders separately
- utility actions still exist
- compact nav trigger appears only when the compact-shell state requires it

- [ ] **Step 5: Verify**

Run:

```bash
npm --prefix crates/ui exec vitest run src/components/AppHeader.test.tsx
npm --prefix crates/ui run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/components/AppHeader.tsx crates/ui/src/styles/index.css crates/ui/src/components/AppHeader.test.tsx
git commit -m "plan(ui): restore topbar action hierarchy for phase 1"
```

---

## Task 3: Reduce Dock Competition And Clarify Active Context

**Files:**
- Modify: `crates/ui/src/panels/DockPanel.tsx`
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/lib/utils.ts`
- Modify: `crates/ui/src/screens/EditorScreen.test.tsx`

- [ ] **Step 1: Strengthen active tab styling**

Current problem: `Overview`, `Chat`, `Terminal`, `Run trace`, and `Runs` all read as similar low-weight pills.

Target:

- stronger active contrast
- clearer inactive state
- more deliberate spacing
- reduced visual noise when dock content is empty

- [ ] **Step 2: Add active-panel context**

Without changing backend behavior, add lightweight context in the dock chrome so the user can read what panel is open, not just which tab is tinted.

Acceptable examples:

- panel title + short descriptor
- empty-state message scoped to the active tab

- [ ] **Step 3: Tune dock defaults for compact space**

Revisit:

- `DEFAULT_DOCK_HEIGHT`
- `COLLAPSED_DOCK_HEIGHT`
- `minimumDockHeight(tab)`
- `shouldCollapseDock(...)`

The compact-screen default should preserve editor visibility first.

- [ ] **Step 4: Keep empty states quiet**

`No workflow runs yet.` should not dominate the dock. Use quieter spacing and hierarchy so an empty dock does not visually outweigh the canvas.

- [ ] **Step 5: Update editor-shell tests**

Cover:

- dock collapse behavior still works
- compact default height logic does not break editor rendering
- no inspector regression from the dock changes

- [ ] **Step 6: Verify**

Run:

```bash
npm --prefix crates/ui exec vitest run src/screens/EditorScreen.test.tsx
npm --prefix crates/ui run typecheck
```

- [ ] **Step 7: Commit**

```bash
git add crates/ui/src/panels/DockPanel.tsx crates/ui/src/styles/index.css crates/ui/src/lib/utils.ts crates/ui/src/screens/EditorScreen.test.tsx
git commit -m "plan(ui): rebalance dock hierarchy for phase 1"
```

---

## Task 4: Make The Editor Shell Viable On Compact Widths

**Files:**
- Modify: `crates/ui/src/App.tsx`
- Modify: `crates/ui/src/context/AppContext.tsx`
- Modify: `crates/ui/src/context/AppProvider.tsx`
- Modify: `crates/ui/src/components/AppHeader.tsx`
- Modify: `crates/ui/src/components/sidebar/AppSidebar.tsx`
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/app/App.test.tsx`

- [ ] **Step 1: Treat compact-screen navigation as a UI-state problem, not just a media query**

Current problem: the sidebar becomes normal document flow below `980px`, so it occupies the first half of the screen and pushes the editor below it.

Target:

- compact viewport keeps the editor as the primary surface
- sidebar becomes an overlay drawer or sheet
- navigation can be opened intentionally and dismissed cleanly

- [ ] **Step 2: Add compact-shell state in `AppProvider` / `AppContext` only if needed**

Recommended state:

- `isCompactViewport`
- `sidebarDrawerOpen`
- `openSidebarDrawer()`
- `closeSidebarDrawer()`
- `toggleSidebarDrawer()`

Do not involve `desktop`, `orchestration`, or persistence.

- [ ] **Step 3: Update `AppHeader` and `AppSidebar` to use the compact-shell state**

Recommended behavior:

- leading nav button appears only in compact mode
- selecting a sidebar destination closes the drawer
- scrim click / escape closes the drawer

- [ ] **Step 4: Replace the stacked mobile shell CSS**

Remove the current small-screen behavior that turns the full shell into:

- sidebar block
- then topbar
- then editor

Replace it with:

- full-width editor shell
- overlay sidebar drawer
- compact topbar that still exposes the primary run action

- [ ] **Step 5: Ensure dock behavior respects compact mode**

In compact mode:

- dock should start collapsed or at a reduced safe height
- chat focus mode should still work
- canvas should remain visible without vertical dead space

- [ ] **Step 6: Add shell-level tests**

Cover:

- compact nav trigger opens/closes drawer
- selecting a nav item closes drawer
- editor shell still renders without sidebar inline in compact mode

- [ ] **Step 7: Verify**

Run:

```bash
npm --prefix crates/ui exec vitest run src/app/App.test.tsx src/components/AppHeader.test.tsx src/screens/EditorScreen.test.tsx
npm --prefix crates/ui run typecheck
```

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/App.tsx crates/ui/src/context/AppContext.tsx crates/ui/src/context/AppProvider.tsx crates/ui/src/components/AppHeader.tsx crates/ui/src/components/sidebar/AppSidebar.tsx crates/ui/src/styles/index.css crates/ui/src/app/App.test.tsx
git commit -m "plan(ui): restore compact-screen editor usability"
```

---

## Task 5: Final Verification And Review Capture

**Files:**
- No new production files required

- [ ] **Step 1: Run the focused Phase 1 lane**

```bash
npm --prefix crates/ui exec vitest run src/components/AppHeader.test.tsx src/app/App.test.tsx src/screens/EditorScreen.test.tsx
npm --prefix crates/ui run typecheck
git diff --check
```

- [ ] **Step 2: Capture breakpoint evidence**

Verify the editor shell at:

- desktop (`1440x900` or similar)
- tablet (`1024x900`)
- compact/mobile (`390x844`)

Checks:

- canvas visible on first view
- primary action obvious
- dock not visually dominant
- sidebar no longer stacked inline at compact widths

- [ ] **Step 3: Record follow-on debt for the later normalization pass**

If Phase 1 leaves legacy token families untouched, add a short follow-up note in the implementation summary listing:

- remaining legacy aliases
- screens not yet migrated to semantic tokens
- any hardcoded raw colors intentionally deferred

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "plan(ui): verify phase 1 hierarchy and compact-shell changes"
```

---

## Follow-On Plan Seed: CSS Normalization Pass

Do **not** execute this in Phase 1, but preserve the direction:

1. Inventory all remaining legacy tokens and raw color literals in `crates/ui/src/styles/index.css`.
2. Move from mixed naming (`--app-bg`, `--surface`, `--panel-surface`, `--bar-bg`) to one semantic system.
3. Migrate component blocks screen-by-screen rather than global search/replace.
4. Remove legacy aliases only after all consumers have moved.

Recommended inventory command for the later pass:

```bash
rg -n '#[0-9a-fA-F]{3,8}|rgba?\\(|color-mix\\(' crates/ui/src/styles/index.css
```

---

## Recommended Execution Order

1. Task 1: palette contract
2. Task 2: topbar hierarchy
3. Task 3: dock hierarchy
4. Task 4: compact editor shell
5. Task 5: verification

This order keeps the palette contract in place before the visible hierarchy work and avoids re-styling the same selectors twice.
