# Phase 2 UI Refinement And CSS Normalization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the Phase 2 refinement slice from the design audit: introduce a coherent typography system, unify route-level shells and surfaces, standardize radius/density language, improve empty states, and start the first controlled CSS normalization pass on top of the Phase 1 palette contract.

**Architecture:** Keep this slice in the `UI/Desktop seam` lane. The work lives primarily in `crates/ui/src/styles/index.css` plus screen/panel/component markup in `crates/ui/src/screens/`, `components/`, and `panels/`. Do not move behavior into `desktop` or `orchestration`; this is a presentation-system refinement pass. If repeated route structure is too expensive to maintain in pure CSS, introduce small shared UI-shell components under `crates/ui/src/components/` rather than duplicating screen-specific markup.

**Tech Stack:** SolidJS, TypeScript, CSS custom properties, existing Vitest suite, browser/manual breakpoint verification.

---

## Phase Boundary

### In Scope

- Add a semantic type scale and apply it to the main shells and route-level screens.
- Standardize surface, border, radius, and spacing usage on major screens and shared controls.
- Unify the full-page screen shell pattern across `Agents`, `Settings`, `Schedule`, and `Workflow Authoring`.
- Build a reusable empty-state pattern and migrate the major empty screens/panels touched in this slice.
- Perform the first intentional CSS normalization tranche against the new palette/tokens rather than leaving route-level CSS half-legacy, half-semantic.

### Out Of Scope

- Phase 1 fixes that are still red in tests or still structurally incomplete.
- Phase 3 polish work: motion grammar, transcript-surface unification, dark-mode finishing pass.
- Full repo-wide token deletion or one-shot renaming of every legacy CSS variable.
- Any functional reflows that change user flows or backend contracts.

### Assumptions

Phase 2 assumes one of these is true before implementation starts:

1. Phase 1 lands first and provides the palette contract + compact editor shell.
2. Or the implementer reconciles the current branch state with the Phase 1 plan before starting Phase 2 selectors.

If the Phase 1 token layer is still unstable, complete that stabilization before beginning Task 1 below.

---

## Design Intent

Phase 1 made the app legible and usable. Phase 2 should make it feel designed.

The target qualities for this slice are:

- calmer type hierarchy
- fewer visual dialects across screens
- consistent card/surface language
- screens that feel like one product family
- CSS that references a system instead of a pile of local decisions

This phase is where the palette becomes operational rather than merely defined.

---

## Proposed System Additions

### Typography Tokens

Add a semantic text scale in `crates/ui/src/styles/index.css`:

| Token | Intended use |
| --- | --- |
| `--text-display-sm` | route/page titles |
| `--text-title-md` | section titles / card headers |
| `--text-title-sm` | secondary panel titles |
| `--text-body-md` | default body copy |
| `--text-body-sm` | compact supporting copy |
| `--text-label-sm` | form labels / chips |
| `--text-meta-xs` | eyebrow/meta text |

### Radius Tokens

Add a small radius scale:

| Token | Intended use |
| --- | --- |
| `--radius-sm` | inputs, chips, small pills |
| `--radius-md` | cards, empty states, grouped controls |
| `--radius-lg` | large panels, preview shells |
| `--radius-pill` | fully rounded buttons/chips |

### Layout Tokens

Add screen-shell spacing tokens:

| Token | Intended use |
| --- | --- |
| `--page-gutter` | outer horizontal page padding |
| `--page-gutter-compact` | compact breakpoint padding |
| `--page-stack-gap` | vertical rhythm between major page sections |
| `--card-padding-md` | default content card padding |
| `--card-padding-lg` | larger route-level card padding |

### CSS Normalization Rule For Phase 2

Only normalize selectors touched in this slice. Do not mass-delete legacy tokens yet.

Allowed in Phase 2:

- replace raw route-level color literals
- replace duplicate radius values
- replace duplicated page-shell spacing values

Not allowed in Phase 2:

- global search/replace of every old token
- deleting compatibility aliases still used by untouched components

---

## File Structure

| File | Responsibility |
| --- | --- |
| `docs/superpowers/plans/2026-06-20-phase-2-ui-refinement-normalization.md` | Durable implementation plan for this slice |
| `crates/ui/src/styles/index.css` | Typography, radius, spacing tokens; shared shell classes; Phase 2 normalization tranche |
| `crates/ui/src/screens/SettingsScreen.tsx` | Route-shell convergence with shared page structure |
| `crates/ui/src/screens/AgentsScreen.tsx` | Route-shell convergence, sidebar/detail rhythm cleanup, empty-state adoption |
| `crates/ui/src/screens/ScheduleScreen.tsx` | Route-shell convergence, table/card polish, token cleanup |
| `crates/ui/src/screens/WorkflowAuthoringScreen.tsx` | Route-shell convergence, copy hierarchy, preview framing |
| `crates/ui/src/components/WorkflowPickerModal.tsx` | Empty-state and shell consistency if touched by new reusable pattern |
| `crates/ui/src/components/PanelEmptyState.tsx` | Reusable empty-state primitive if it survives Phase 1; otherwise create/replace here |
| `crates/ui/src/panels/RunHistoryPanel.tsx` | Empty-state and title hierarchy consistency if touched |
| `crates/ui/src/settings/SettingsNav.tsx` | Typography/rhythm consistency within full-page shell |
| `crates/ui/src/app/App.test.tsx` | Full-page shell and empty-state regressions |
| `crates/ui/src/components/AppHeader.test.tsx` | Typography and hierarchy assertions if the header text scale changes |

Optional shared primitives if needed:

| File | Responsibility |
| --- | --- |
| `crates/ui/src/components/PageShell.tsx` | Shared full-page shell wrapper |
| `crates/ui/src/components/PageHeader.tsx` | Shared route-level header layout |

---

## Acceptance Criteria

- `Agents`, `Settings`, `Schedule`, and `Workflow Authoring` feel like one visual family.
- Route headers, section titles, labels, and meta text all map to a defined semantic type scale.
- Major surfaces use a consistent radius/surface/border system instead of one-off card treatments.
- Empty states guide the next action and use one shared component pattern.
- The touched screen CSS uses semantic palette/type/layout tokens rather than route-specific raw values.

---

## Task 1: Introduce The Semantic Typography And Radius System

**Files:**
- Modify: `crates/ui/src/styles/index.css`

- [ ] **Step 1: Add type-scale tokens**

Create a clearly labeled typography block with the Phase 2 semantic text tokens.

- [ ] **Step 2: Add radius and page-spacing tokens**

Create a compact, explicit radius/layout token block for route shells and content cards.

- [ ] **Step 3: Map existing common selectors onto the new type scale**

Migrate at minimum:

- route/page titles
- section titles
- card titles
- label text
- eyebrow/meta text
- body/supporting copy

- [ ] **Step 4: Migrate touched cards and controls to radius tokens**

Replace repeated hardcoded radii on:

- page cards
- empty states
- route-level segmented controls
- schedule controls
- preview shells

- [ ] **Step 5: Leave a Phase 2 normalization inventory comment**

Document which typography and radius literals still remain outside the touched surfaces.

- [ ] **Step 6: Verify**

Run:

```bash
rg -n -- '--text-display-sm:|--text-title-md:|--text-body-md:|--radius-sm:|--radius-md:|--page-gutter:' crates/ui/src/styles/index.css
```

- [ ] **Step 7: Commit**

```bash
git add crates/ui/src/styles/index.css
git commit -m "plan(ui): add phase 2 typography and radius tokens"
```

---

## Task 2: Unify The Full-Page Screen Shell Pattern

**Files:**
- Modify: `crates/ui/src/screens/SettingsScreen.tsx`
- Modify: `crates/ui/src/screens/AgentsScreen.tsx`
- Modify: `crates/ui/src/screens/ScheduleScreen.tsx`
- Modify: `crates/ui/src/screens/WorkflowAuthoringScreen.tsx`
- Modify: `crates/ui/src/settings/SettingsNav.tsx`
- Modify: `crates/ui/src/styles/index.css`
- Optional: create `crates/ui/src/components/PageShell.tsx`
- Optional: create `crates/ui/src/components/PageHeader.tsx`

- [ ] **Step 1: Choose one shell pattern for full-page routes**

Recommended shared pattern:

- route wrapper
- route header
- route body
- standardized gutter and max width

- [ ] **Step 2: Normalize route header hierarchy**

Unify how these screens express:

- eyebrow
- title
- supporting description
- trailing actions

- [ ] **Step 3: Normalize route body rhythm**

Make page sections use the same vertical gap, padding, and inner-card density.

- [ ] **Step 4: Bring `Agents` into the same screen family**

Current `Agents` is the most likely outlier because it mixes app-shell primitives with a bespoke two-column detail layout. Keep the behavior, but bring the frame language in line with the other full-page screens.

- [ ] **Step 5: Keep compact behavior intentional**

At smaller widths, these route shells should collapse cleanly without inventing a second design language.

- [ ] **Step 6: Verify**

Run:

```bash
npm --prefix crates/ui run typecheck
```

Manual checks:

- `Agents`
- `Settings`
- `Schedule`
- `Workflow Authoring`

- [ ] **Step 7: Commit**

```bash
git add crates/ui/src/screens/SettingsScreen.tsx crates/ui/src/screens/AgentsScreen.tsx crates/ui/src/screens/ScheduleScreen.tsx crates/ui/src/screens/WorkflowAuthoringScreen.tsx crates/ui/src/settings/SettingsNav.tsx crates/ui/src/styles/index.css
git commit -m "plan(ui): unify full-page route shells for phase 2"
```

---

## Task 3: Build The Shared Empty-State System

**Files:**
- Modify or Create: `crates/ui/src/components/PanelEmptyState.tsx`
- Modify: `crates/ui/src/screens/AgentsScreen.tsx`
- Modify: `crates/ui/src/panels/RunHistoryPanel.tsx`
- Modify: `crates/ui/src/panels/DockPanel.tsx`
- Modify: `crates/ui/src/screens/WorkflowAuthoringScreen.tsx`
- Modify: `crates/ui/src/components/WorkflowPickerModal.tsx`
- Modify: `crates/ui/src/styles/index.css`

- [ ] **Step 1: Standardize the empty-state primitive**

Target pattern:

- icon
- title
- supporting description
- optional primary action slot

- [ ] **Step 2: Migrate the major empty states touched in this slice**

At minimum:

- no saved agents
- no selected agent detail
- no workflow runs yet / no history
- workflow authoring initial prompt
- workflow picker empty state if still inconsistent

- [ ] **Step 3: Keep each empty state action-oriented**

Every migrated empty state should point toward the first useful next action, not just describe absence.

- [ ] **Step 4: Unify spacing and tone**

Use the same padding, icon scale, title/body spacing, and surface treatment across empty states.

- [ ] **Step 5: Verify**

Run:

```bash
npm --prefix crates/ui exec vitest run src/app/App.test.tsx
npm --prefix crates/ui run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/components/PanelEmptyState.tsx crates/ui/src/screens/AgentsScreen.tsx crates/ui/src/panels/RunHistoryPanel.tsx crates/ui/src/panels/DockPanel.tsx crates/ui/src/screens/WorkflowAuthoringScreen.tsx crates/ui/src/components/WorkflowPickerModal.tsx crates/ui/src/styles/index.css
git commit -m "plan(ui): standardize empty states for phase 2"
```

---

## Task 4: Unify Surface, Border, And Density Language

**Files:**
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/screens/AgentsScreen.tsx`
- Modify: `crates/ui/src/screens/ScheduleScreen.tsx`
- Modify: `crates/ui/src/screens/WorkflowAuthoringScreen.tsx`
- Modify: `crates/ui/src/panels/RunHistoryPanel.tsx`
- Modify: `crates/ui/src/components/WorkflowPickerModal.tsx`

- [ ] **Step 1: Define the Phase 2 card/surface ladder**

Use the semantic palette to distinguish:

- route background
- main cards/panels
- raised subcards/chips
- subtle muted blocks

- [ ] **Step 2: Normalize major screen cards to that ladder**

Remove ad hoc blends where equivalent screen elements should share one surface level.

- [ ] **Step 3: Normalize border weight and card padding**

Use the new radius/layout tokens so major panels stop feeling independently tuned.

- [ ] **Step 4: Reduce accidental density differences**

Pay special attention to:

- `Agents` detail panel
- `Schedule` rows/controls
- `Workflow Authoring` preview and validation panels
- picker/modal shells

- [ ] **Step 5: Verify**

Run:

```bash
npm --prefix crates/ui run typecheck
git diff --check
```

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/styles/index.css crates/ui/src/screens/AgentsScreen.tsx crates/ui/src/screens/ScheduleScreen.tsx crates/ui/src/screens/WorkflowAuthoringScreen.tsx crates/ui/src/panels/RunHistoryPanel.tsx crates/ui/src/components/WorkflowPickerModal.tsx
git commit -m "plan(ui): standardize phase 2 surface and density language"
```

---

## Task 5: CSS Normalization Tranche 1

**Files:**
- Modify: `crates/ui/src/styles/index.css`

- [ ] **Step 1: Inventory the touched route-level literals and legacy aliases**

Capture only the selectors involved in Phase 2 screens/components.

Useful commands:

```bash
rg -n '#[0-9a-fA-F]{3,8}|rgba?\\(|color-mix\\(' crates/ui/src/styles/index.css
rg -n -- '--app-bg|--surface|--raised-surface|--bar-bg|--chrome-bg|--dock-bg|--panel-surface' crates/ui/src/styles/index.css
```

- [ ] **Step 2: Replace route-level raw color usage with semantic tokens where safe**

Focus only on:

- full-page screens
- empty states
- route cards
- route headers

- [ ] **Step 3: Consolidate duplicate shell classes**

If the same layout or card treatment appears in multiple screens, pull it into one shared class or small component abstraction rather than preserving copy/paste CSS.

- [ ] **Step 4: Preserve compatibility aliases for untouched areas**

Do not remove the legacy bridge if other components still depend on it.

- [ ] **Step 5: Leave a post-Phase-2 normalization note**

Document what remains for the later wider cleanup:

- conversation surfaces
- dock/chat micro-components
- older inspector/panel blocks

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/styles/index.css
git commit -m "plan(ui): normalize phase 2 route css onto semantic tokens"
```

---

## Task 6: Final Verification And Review Capture

**Files:**
- No new production files required

- [ ] **Step 1: Run the focused Phase 2 lane**

```bash
npm --prefix crates/ui exec vitest run src/app/App.test.tsx src/components/AppHeader.test.tsx
npm --prefix crates/ui run typecheck
git diff --check
```

- [ ] **Step 2: Capture breakpoint evidence**

Verify at:

- desktop
- tablet
- compact/mobile

For:

- `Agents`
- `Settings`
- `Schedule`
- `Workflow Authoring`

- [ ] **Step 3: Confirm the Phase 2 audit targets are met**

Checklist:

- type hierarchy calmer
- route shells unified
- empty states standardized
- screen-level surface language aligned
- touched CSS normalized to semantic tokens

- [ ] **Step 4: Record remaining Phase 3 prerequisites**

Leave a short implementation summary note covering:

- dark-mode gaps still visible
- conversation/tool-line surfaces still inconsistent
- motion grammar still deferred

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "plan(ui): verify phase 2 refinement and normalization"
```

---

## Recommended Execution Order

1. Task 1: typography and radius tokens
2. Task 2: full-page shell unification
3. Task 3: empty-state system
4. Task 4: surface/density unification
5. Task 5: CSS normalization tranche 1
6. Task 6: verification

This order lets the design system primitives land first, then uses them to refactor screens without churning the same selectors repeatedly.

---

## Phase 3 Seed

Phase 3 should start after this plan lands and should focus on:

- motion grammar
- conversation/transcript surface unification
- loading/error-state polish
- dark-mode completion
- subtle interaction details

Do not backfill those into Phase 2 unless a touched selector cannot be left in a visibly broken state.
