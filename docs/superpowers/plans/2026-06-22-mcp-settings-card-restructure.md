# MCP Settings Card Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the `MCP Servers` settings section into a calmer card-based control surface that improves hierarchy and scanability without changing MCP behavior, settings persistence, or backend seams.

**Architecture:** Keep the work entirely inside the `UI/Desktop seam` lane. `crates/ui/src/settings/McpSection.tsx` owns the presentation structure and local probe/draft state, while `crates/ui/src/styles/index.css` owns the card language, row layout, secondary composer treatment, and responsive collapse rules. Existing `AppContext` methods, DTOs, and `probeMcpServer` calls remain unchanged.

**Tech Stack:** SolidJS, TypeScript, CSS custom properties, existing settings screen patterns, focused Vitest, manual browser verification.

---

## Scope Boundary

### In Scope

- Reshape the MCP settings markup into four internal cards
- Improve row hierarchy and empty states
- Make probe feedback a deliberate status block
- Make the add-server composer visually secondary
- Add focused regression coverage for the new structure

### Out Of Scope

- Any MCP feature expansion
- Settings schema changes
- Provider or appearance settings redesign
- Desktop or orchestration changes

## File Structure

| File | Responsibility |
| --- | --- |
| `docs/superpowers/specs/2026-06-22-mcp-settings-card-restructure-design.md` | Approved design for this slice |
| `docs/superpowers/plans/2026-06-22-mcp-settings-card-restructure.md` | Durable execution plan |
| `crates/ui/src/settings/McpSection.tsx` | Card markup, row grouping, probe-status placement, secondary composer structure |
| `crates/ui/src/styles/index.css` | MCP card styles, row layout, empty-state blocks, responsive rules, secondary composer treatment |
| `crates/ui/src/settings/McpSection.test.tsx` | Focused markup/regression coverage for the reshaped MCP section |

## Acceptance Criteria

- The MCP settings section renders as four clearly separated cards.
- `Discovery`, `Discovered servers`, and `Configured servers` read as the primary surfaces.
- `Add custom server` is visibly quieter than the management cards.
- Probe results render as a contained status surface inside the configured card.
- Existing toggles, inline edits, and add/probe actions keep their current behavior.

## Task 1: Lock The New Structure With Focused UI Tests

**Files:**
- Modify: `crates/ui/src/settings/McpSection.test.tsx`

- [ ] **Step 1: Expand the fixture coverage so the test can assert all four cards**

Keep one discovered row and one configured row in the fixture, then assert the new card headings and status text.

Add expectations shaped like:

```ts
expect(mountPoint.textContent).toContain("Discovery");
expect(mountPoint.textContent).toContain("Discovered servers");
expect(mountPoint.textContent).toContain("Configured servers");
expect(mountPoint.textContent).toContain("Add custom server");
```

- [ ] **Step 2: Add an assertion for the visually secondary composer copy**

Use stable copy rather than CSS assertions so the test survives style refactors.

Example:

```ts
expect(mountPoint.textContent).toContain("Create a manual MCP entry");
```

- [ ] **Step 3: Add an assertion for the discovered/configured metadata**

Ensure the new structure still renders the important row metadata:

```ts
expect(mountPoint.textContent).toContain("cursor");
expect(mountPoint.textContent).toContain("GitHub");
expect(mountPoint.textContent).toContain("npx");
```

- [ ] **Step 4: Run the focused test before implementation**

Run:

```bash
npm --prefix crates/ui exec vitest run src/settings/McpSection.test.tsx
```

Expected: FAIL because the new headings/copy are not present yet.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/settings/McpSection.test.tsx
git commit -m "test(ui): lock mcp card restructure expectations"
```

---

## Task 2: Reshape `McpSection` Into Four Cards Without Changing Behavior

**Files:**
- Modify: `crates/ui/src/settings/McpSection.tsx`

- [ ] **Step 1: Add small presentational helpers for counts and probe state**

Keep logic local to the section rather than pushing it into context.

Add helpers shaped like:

```ts
const discoveredCount = () => ctx.discoveredMcp().length;
const configuredCount = () => servers().length;
const probeResultTone = () => {
  const result = probeResult();
  if (!result) return null;
  return result === "Probing…" ? "pending" : "ready";
};
```

- [ ] **Step 2: Replace the flat section body with four internal cards**

Rework the JSX from stacked subsections into:

```tsx
<div class="mcp-card-grid">
  <section class="mcp-card mcp-card--discovery">...</section>
  <section class="mcp-card">...</section>
  <section class="mcp-card">...</section>
  <section class="mcp-card mcp-card--secondary">...</section>
</div>
```

Each card should keep the current controls but reframe them under:

- `Discovery`
- `Discovered servers`
- `Configured servers`
- `Add custom server`

- [ ] **Step 3: Turn discovered rows into contained management items**

Wrap each discovered server in a row surface with metadata and a right-aligned toggle.

Target markup shape:

```tsx
<div class="mcp-server-row">
  <div class="mcp-server-copy">
    <strong>{row.displayName}</strong>
    <p class="settings-hint">
      {row.source} · {shortenPath(row.sourcePath)}
    </p>
  </div>
  <label class="checkbox-label mcp-server-toggle">...</label>
</div>
```

- [ ] **Step 4: Turn configured rows into contained editor rows**

Keep inline editing and `Test`, but group fields under a row shell:

```tsx
<div class="mcp-configured-row">
  <div class="mcp-configured-fields">
    <label>...</label>
    <label>...</label>
  </div>
  <div class="mcp-configured-actions">
    <label class="checkbox-label">...</label>
    <button type="button" class="btn-secondary">Test</button>
  </div>
</div>
```

- [ ] **Step 5: Move probe feedback into a contained status block**

Replace the loose paragraph after the configured list with a status surface:

```tsx
<Show when={probeResult()}>
  <div class="mcp-probe-status" data-tone={probeResultTone() ?? undefined}>
    {probeResult()}
  </div>
</Show>
```

- [ ] **Step 6: Make the add-server card explicitly secondary**

Add copy and structure that makes the composer quieter:

```tsx
<section class="mcp-card mcp-card--secondary" aria-labelledby="mcp-add-heading">
  <h3 id="mcp-add-heading" class="settings-subheading">Add custom server</h3>
  <p class="settings-hint">Create a manual MCP entry when discovery is not enough.</p>
  ...
</section>
```

- [ ] **Step 7: Preserve the existing mutation behavior exactly**

Do not rename or reroute:

- `toggleDiscoverExternal`
- `toggleDiscoveredEnabled`
- `updateServer`
- `addServer`
- `probeServer`

The implementation change is presentational only.

- [ ] **Step 8: Run typecheck after the component rewrite**

Run:

```bash
npm --prefix crates/ui run typecheck
```

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/ui/src/settings/McpSection.tsx
git commit -m "feat(ui): restructure mcp settings into cards"
```

---

## Task 3: Add The Card System And Secondary Composer Styling

**Files:**
- Modify: `crates/ui/src/styles/index.css`

- [ ] **Step 1: Add a dedicated MCP styles block near the settings styles**

Create selectors for:

```css
.mcp-card-grid
.mcp-card
.mcp-card--secondary
.mcp-card-header
.mcp-server-row
.mcp-configured-row
.mcp-probe-status
```

- [ ] **Step 2: Define the primary card language**

Use the existing semantic surfaces and borders instead of hardcoded one-off colors.

The core shape should resemble:

```css
.mcp-card {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
  border: 1px solid var(--border-subtle);
  border-radius: calc(var(--radius-md) + 4px);
  background: var(--surface-muted);
}
```

- [ ] **Step 3: Make the composer card visually quieter**

Reduce its contrast and emphasis relative to the management cards:

```css
.mcp-card--secondary {
  background: color-mix(in srgb, var(--surface-panel) 72%, transparent);
}
```

If `color-mix` is not already used or desired in this file, use a safer existing surface token instead of introducing new browser support risk.

- [ ] **Step 4: Improve row-level hierarchy and action alignment**

Add styles that keep copy and actions balanced:

```css
.mcp-server-row,
.mcp-configured-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
  padding: 14px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-panel);
}
```

- [ ] **Step 5: Add a contained empty-state treatment**

Use a compact block instead of naked paragraph copy:

```css
.mcp-empty-state {
  padding: 14px;
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  background: var(--surface-panel);
}
```

- [ ] **Step 6: Add responsive collapse rules**

At compact widths, stack configured row actions and form grids:

```css
@media (max-width: 980px) {
  .mcp-configured-row,
  .mcp-server-row {
    flex-direction: column;
  }

  .mcp-configured-actions,
  .mcp-form-grid {
    width: 100%;
  }
}
```

- [ ] **Step 7: Run CSS sanity checks**

Run:

```bash
rg -n "mcp-card|mcp-server-row|mcp-probe-status|mcp-empty-state" crates/ui/src/styles/index.css
git diff --check
```

Expected: selectors present, no whitespace errors.

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/styles/index.css
git commit -m "style(ui): add mcp settings card system"
```

---

## Task 4: Verify The Section End-To-End In The UI Lane

**Files:**
- Modify: `crates/ui/src/settings/McpSection.test.tsx` if the verification pass reveals missing stable assertions

- [ ] **Step 1: Re-run the focused MCP test**

Run:

```bash
npm --prefix crates/ui exec vitest run src/settings/McpSection.test.tsx
```

Expected: PASS

- [ ] **Step 2: Re-run UI typecheck**

Run:

```bash
npm --prefix crates/ui run typecheck
```

Expected: PASS

- [ ] **Step 3: Manually inspect the settings section in the app**

Run one of:

```bash
npm --prefix crates/ui run dev
```

or

```bash
npm --prefix crates/desktop run start -- dev
```

Check:

- card grouping is obvious at first glance
- `Discovery` / `Discovered` / `Configured` dominate visually
- `Add custom server` reads as secondary
- empty states and probe feedback feel contained
- narrow width does not break row actions

- [ ] **Step 4: Fold any small verification fixes back into the touched files**

If manual inspection reveals spacing or wrapping issues, limit follow-up edits to:

- `crates/ui/src/settings/McpSection.tsx`
- `crates/ui/src/styles/index.css`
- `crates/ui/src/settings/McpSection.test.tsx`

Do not widen scope into the rest of settings.

- [ ] **Step 5: Run the final UI verification lane**

Run:

```bash
npm --prefix crates/ui exec vitest run src/settings/McpSection.test.tsx
npm --prefix crates/ui run typecheck
```

Expected: PASS on both commands.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/settings/McpSection.tsx crates/ui/src/styles/index.css crates/ui/src/settings/McpSection.test.tsx
git commit -m "chore(ui): verify mcp settings card restructure"
```

---

## Self-Review

### Spec Coverage Check

- Four-card structure: covered by Task 2
- Visually secondary custom-server composer: covered by Tasks 2 and 3
- Contained probe-result and empty states: covered by Tasks 2 and 3
- Responsive behavior and UI-lane verification: covered by Tasks 3 and 4

### Placeholder Scan

- No `TODO` / `TBD` placeholders remain
- Each task includes exact files and concrete commands
- JSX/CSS changes are represented with code-shaped snippets

### Type Consistency

- Plan keeps current function names from `McpSection.tsx`
- CSS class names introduced in Task 2 are carried through Task 3 and Task 4 consistently
