# MCP Settings Card Restructure Design

**Date:** 2026-06-22

## Goal

Redesign the `MCP Servers` settings section in `crates/ui` so it feels intentional and readable instead of like one long raw form, while preserving all current behavior and backend contracts.

## Scope

### In Scope

- The `MCP Servers` section rendered by `crates/ui/src/settings/McpSection.tsx`
- Supporting styles in `crates/ui/src/styles/index.css`
- Focused UI tests for the reshaped MCP section

### Out Of Scope

- `Appearance` and `Providers` settings sections
- Any `desktop`, `orchestration`, or settings-schema changes
- Adding new MCP capabilities, validation rules, or persistence behavior

## Approved Direction

The approved direction is a **card-based restructure**.

The section should feel like a small control center rather than a single tall form. The user selected the calmer `Option B` direction, with the `Add custom server` composer visually secondary to the live/discovered management surfaces.

## Architecture And Ownership

This stays in the `UI/Desktop seam` lane.

- `crates/ui/src/settings/McpSection.tsx` owns the MCP settings markup, local draft state, and probe-status presentation.
- `crates/ui/src/styles/index.css` owns the visual system for the MCP cards, rows, spacing, and responsive behavior.
- Existing `AppContext` methods and settings DTOs remain the source of truth for behavior.

No new port, API, or orchestration work is needed.

## Interaction Model

Behavior stays exactly as it is today:

- `Discover external MCP configs` still toggles `settings.mcp.discoverExternal` and then refreshes discovered MCP rows.
- Discovered server toggles still update `disabledDiscoveredIds`.
- Configured server rows still edit in place.
- `Test` still probes one server through `probeMcpServer`.
- `Add server` still creates a config from the draft state when `id` and `command` are present.

The redesign changes presentation only:

- better grouping
- better scanability
- clearer empty states
- clearer visual priority

## Layout

Keep one outer `settings-section` container, but split its contents into four internal cards:

1. `Discovery`
   - Compact status card near the top
   - Contains the external discovery toggle
   - Includes a short summary line such as discovered/configured counts

2. `Discovered servers`
   - Primary management card
   - Each discovered server becomes a contained row with stronger name/path hierarchy
   - Toggle action stays aligned to the right

3. `Configured servers`
   - Peer management card
   - Each configured server becomes a contained editor row
   - Keep inline edit fields and `Test` action
   - Probe feedback should appear as a deliberate status surface inside this card, not as loose helper text

4. `Add custom server`
   - Secondary composer card at the bottom
   - Lower visual weight than the management cards
   - Softer surface, tighter spacing, more subdued heading/copy

## Visual Treatment

### Hierarchy

- The section intro remains at the top, but the real visual structure comes from the internal cards.
- Discovery and live server management should dominate.
- The custom-server composer should be present but not loud.

### Card Language

- Use rounded internal cards with subtle borders and differentiated surfaces.
- Give each card a short title and optional supporting copy.
- Replace plain divider lines with contained sections.

### Row Design

- Each discovered/configured item should read as one row-level unit.
- Server name should be the strongest text.
- Source/path/command metadata should be subdued secondary text.
- Actions should align consistently on the right edge.

### Empty States

- Replace naked `No ... configured.` text with muted empty-state blocks inside the relevant card.
- Empty states should still be compact; this is settings UI, not a marketing panel.

### Form Weight

- The add form should use a quieter surface treatment than the management cards.
- The primary action remains clear, but the whole composer should not visually compete with discovered/configured cards.

## Responsive Behavior

- On desktop, the section remains a single-column stack of internal cards.
- Internal grids for configured rows or form fields may use two columns when space allows.
- On narrower widths, all card content collapses cleanly to one column.
- Right-aligned actions should wrap beneath content rather than overflowing.

## Accessibility

- Preserve existing labels and form semantics.
- Card titles should continue using heading structure that makes sense inside settings.
- Focus rings should keep the app’s existing behavior; no custom focus treatment should reduce visibility.
- Text contrast for metadata and empty states must remain legible in dark mode.

## Acceptance Criteria

- The MCP section reads as four clearly grouped cards rather than one continuous form.
- `Discovered servers` and `Configured servers` feel like the primary operational surfaces.
- `Add custom server` is visually secondary.
- Empty and probe-result states feel intentionally placed.
- No behavior changes are introduced.

## Verification

- `npm --prefix crates/ui exec vitest run src/settings/McpSection.test.tsx`
- `npm --prefix crates/ui run typecheck`
- Manual visual check in the settings screen at desktop and narrow widths
