---
name: OpenFlow
description: Visual IDE for multi-agent workflows — warm sand surfaces, indigo actions, tonal depth.
colors:
  sand-ground: "#f6f3ed"
  sand-strong: "#fcfbf8"
  sand-muted: "#f7f4ee"
  ink-primary: "#18181b"
  ink-secondary: "#66645d"
  ink-tertiary: "#8a877f"
  indigo-primary: "#6f7cf7"
  indigo-hover: "#5360d9"
  indigo-soft: "#6f7cf72e"
  surface-panel: "#ffffffcc"
  border-subtle: "#18181b14"
  status-success: "#2f8b63"
  status-warning: "#b7821b"
  status-danger: "#b04d57"
  input-bg: "#fffffff0"
typography:
  display:
    fontFamily: "-apple-system-body, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.75rem"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "normal"
  title:
    fontFamily: "-apple-system-body, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.05rem"
    fontWeight: 600
    lineHeight: 1.35
    letterSpacing: "normal"
  body:
    fontFamily: "-apple-system-body, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  label:
    fontFamily: "-apple-system-body, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: "normal"
  meta:
    fontFamily: "-apple-system-body, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: "normal"
rounded:
  sm: "8px"
  md: "12px"
  lg: "14px"
  pill: "999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  page-gutter: "24px"
components:
  button-primary:
    backgroundColor: "{colors.indigo-primary}"
    textColor: "#ffffff"
    rounded: "{rounded.pill}"
    padding: "10px 16px"
  button-primary-hover:
    backgroundColor: "{colors.indigo-hover}"
    textColor: "#ffffff"
    rounded: "{rounded.pill}"
    padding: "10px 16px"
  button-secondary:
    backgroundColor: "#ffffffb8"
    textColor: "{colors.ink-primary}"
    rounded: "{rounded.pill}"
    padding: "9px 13px"
  button-secondary-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink-primary}"
    rounded: "{rounded.pill}"
    padding: "9px 13px"
  button-danger:
    backgroundColor: "#fff0f2"
    textColor: "#9c3946"
    rounded: "{rounded.pill}"
    padding: "10px 14px"
  input-text:
    backgroundColor: "{colors.input-bg}"
    textColor: "{colors.ink-primary}"
    rounded: "{rounded.sm}"
    padding: "8px 10px"
  sidebar-nav-active:
    backgroundColor: "#ffffffb8"
    textColor: "{colors.ink-primary}"
    rounded: "{rounded.md}"
    padding: "3px 6px"
---

# Design System: OpenFlow

## Overview

OpenFlow is a desktop workflow IDE: canvas graph, conversation dock, inspector panels, settings. Design serves the task — users spend hours wiring agents, approving tools, reading run output. Surfaces stay warm and quiet; indigo marks primary actions and selection; the canvas and chat carry visual energy.

Light theme is default (`color-scheme: light` on `:root`). Dark theme via `data-theme="dark"` on `<html>`. Tokens live in `crates/ui/src/styles/tokens.css`; patterns in `index.css` and `chat.css`. SolidJS components wrap shared class vocabulary (`Button`, `SettingsSection`, sidebar primitives).

**Key Characteristics:**

- **Restrained accent** — indigo on primary buttons, focus rings, active nav; not decorative wash
- **Tonal depth** — frosted panels (`rgba` whites), border shifts, hover washes; shadows reserved for modals and primary CTAs
- **System sans throughout** — one family, fixed rem scale; no display/body pairing
- **Pill actions, rounded fields** — buttons use `--radius-pill`; inputs use `--radius-sm` (8px)
- **Product density** — 13px sidebar labels, compact topbar (40px), inspector blocks collapse
- **Motion at state speed** — 150ms fast / 250ms medium; `prefers-reduced-motion` honored globally

Explicitly rejects: gradient text, glassmorphism-as-decoration, hero-metric templates, side-stripe card accents, bounce/elastic easing, SaaS cream monoculture as the only identity move.

## Colors

Warm sand neutrals carry the chrome; indigo is the sole interactive accent; semantic greens/ambers/reds for status only.

### Primary

- **Signal Indigo** (`#6f7cf7` / `--accent-primary`): Primary buttons, focus-strong, composer send, active workflow emphasis. Hover deepens to **Indigo Press** (`#5360d9` / `--accent-primary-hover`).
- **Indigo Mist** (`rgba(111, 124, 247, 0.18)` / `--base-indigo-soft`): Focus rings (`box-shadow: 0 0 0 4px var(--focus)`), selection washes — never full-surface fill.

### Neutral

- **Workbench Ground** (`#f6f3ed` / `--surface-ground`): App background layer under radial gradients.
- **Paper Lift** (`#fcfbf8` / `--base-sand-50`): Strongest background tier (`--app-bg-strong`).
- **Frost Panel** (`rgba(255, 255, 255, 0.8)` / `--surface-panel`): Sidebars, cards, dock shells — readability via opacity, not heavy shadow.
- **Ink Primary** (`#18181b` / `--text-primary`): Body copy, headings, icons.
- **Ink Secondary** (`#66645d` / `--text-secondary`): Descriptions, metadata — must stay ≥4.5:1 on sand grounds.
- **Ink Tertiary** (`#8a877f` / `--text-tertiary`): Placeholders, disabled-adjacent labels — verify contrast; bump toward secondary if borderline.
- **Hairline** (`rgba(24, 24, 27, 0.08)` / `--border-subtle`): Inputs, dividers, button outlines on secondary.

### Tertiary (status)

- **Run Green** (`#2f8b63`), **Caution Amber** (`#b7821b`), **Stop Rose** (`#b04d57`) — chips, toasts, danger buttons only. Each has a soft background token (`--status-*-soft`).

### Named Rules

**The Accent Rarity Rule.** Indigo appears on primary actions, focus, and current selection — not on static labels, section headers, or canvas decoration. If more than ~10% of a panel reads indigo, pull back.

**The Sand Not Cream Rule.** Warmth lives in sand ink tints and accent, not a generic near-white cream body. Body bg uses `--surface-ground` and layered gradients; avoid introducing new untokenized beige hex values.

## Typography

**Body Font:** System UI stack (`-apple-system-body`, `ui-sans-serif`, `system-ui`, `Segoe UI`, `Helvetica`, `Arial`, sans-serif)

**Character:** Native desktop tool — no custom webfonts. Hierarchy via weight and fixed rem steps, not fluid clamp on chrome.

### Hierarchy

- **Display** (650, 1.75rem / `--text-display-sm`, 1.2): Empty states, onboarding headlines — rare in product chrome.
- **Title** (600, 1.05rem / `--text-title-md`, 1.35): Section headers (`SectionHeader`), settings panel titles.
- **Title Small** (600, 0.9375rem / `--text-title-sm`): Inspector block titles, compact panel headers.
- **Body** (400, 1rem / `--text-body-md`, 1.5): Default UI copy; conversation prose may run to 65–75ch in chat bubbles.
- **Body Small** (400, 0.875rem / `--text-body-sm`): Secondary descriptions, form hints.
- **Label** (500, 0.8125rem / `--text-label-sm`): Form labels, button text on compact controls.
- **Meta** (500, 0.6875rem / `--text-meta-xs`): Timestamps, eyebrows — use sparingly; sidebar nav uses 13px (0.8125rem) at weight 400.

### Named Rules

**The One Family Rule.** Do not introduce a second sans or a display face for marketing flair inside the app shell. Canvas node labels follow the same stack.

## Elevation

Tonal layering first: frosted `rgba` panels, border shifts, and hover washes (`--surface-hover`, `--sidebar-hover`) establish depth. Shadows are secondary — they signal lift on modals and primary buttons, not every card.

### Shadow Vocabulary

- **Panel float** (`--shadow-panel`: `0 12px 30px rgba(15, 23, 42, 0.06)`): Settings cards, dropdown panels, dock elevation.
- **Modal lift** (`--shadow-soft`: `0 18px 50px rgba(15, 23, 42, 0.08)`): `AnimatedModal`, picker overlays.
- **Primary glow** (`0 12px 24px color-mix(in srgb, var(--accent-primary) 22%, transparent)`): Default `.primary-button` only — not secondary or ghost.
- **Focus ring** (`0 0 0 4px var(--focus)`): Keyboard focus on inputs and interactive controls.

Dark theme scales shadow alphas up (`0.28`–`0.35`); same roles, stronger separation.

### Named Rules

**The Flat-By-Default Rule.** Cards and list rows sit on tonal backgrounds at rest. Add `--shadow-panel` when a surface detaches from its parent (popover, modal, floating picker) — not for every `SettingsSection`.

## Components

### Buttons

- **Shape:** Full pill (`--radius-pill` / 999px).
- **Primary:** `--accent-primary` fill, white text, 10×16px padding; soft indigo drop shadow on rest.
- **Hover / Focus:** `--accent-primary-hover`; no scale transform (explicitly `transform: none` on hover).
- **Secondary:** White 72% fill, `--border` outline, inset highlight; ghost variant transparent with hover wash.
- **Danger:** `--danger-soft` background, `#9c3946` text, rose border tint.
- **Sizes:** `small` and `compact` reduce padding; `stretch` full width. Implemented via `Button` SolidJS component mapping to `.primary-button`, `.secondary-button`, `.danger-button`.

### Inputs / Fields

- **Style:** 1px `--border`, `--radius-sm` (8px), `--input-bg` fill, 8×10px padding.
- **Hover:** `--border-strong`.
- **Focus:** Ring via `--focus` / `--focus-strong` on parent patterns; no glow on every hover.
- **Select:** `TextSelect` — custom trigger matching text-input; chevron as inline SVG data URI.

### Cards / Containers

- **Corner Style:** `--radius-md` (12px) on dock cards; settings use `SettingsSection` panel surfaces.
- **Background:** `--surface-panel`, `--panel-surface`, or `--dock-bg` depending on shell.
- **Border:** `--border-subtle` when needed; prefer background delta over nested cards.
- **Internal Padding:** `--card-padding-md` (16px) / `--card-padding-lg` (24px); dock tabs use `--dock-tab-card-padding` (12×14px).

### Navigation

- **Sidebar:** `--sidebar-bg` frosted column; nav buttons 38px min-height, `--radius-md`, 13px label.
- **Default:** Transparent; **hover:** `--sidebar-hover`; **active:** `--sidebar-active` + `--sidebar-active-border`.
- **Topbar:** 40px height; icon buttons 24px; primary run action uses compact topbar primary variant with accent shadow.

### Conversation / Dock

- **Composer:** Pill container (`chat-composer-pill`); circular send button reuses primary token at `--composer-send-size` (36px).
- **Segments:** `--chat-segment-gap` (20px) between run segments; tool bubbles use status soft colors.

### Workflow Canvas

- **React island** — keeps CSS button classes directly; dot grid `--canvas-bg-dot`, edges `--canvas-edge-stroke`.
- Node chrome separate from app shell; do not import marketing styles into canvas.

## Do's and Don'ts

### Do:

- **Do** use semantic tokens (`--text-primary`, `--accent-primary`) in new CSS — not raw hex from the base palette unless defining tokens.
- **Do** verify secondary/tertiary text contrast on sand and frosted panels; bump toward `--text-secondary` ink when placeholders fail 4.5:1.
- **Do** honor `prefers-reduced-motion: reduce` — transitions collapse to 0.01ms in `index.css` global block.
- **Do** use `Button` from `@/components` for new SolidJS actions; keep class names stable for tests (`.primary-button`).
- **Do** use sidebar primitives (`SidebarNavButton`, `SidebarList`, `SidebarListRow`) for list consistency.
- **Do** put new backend calls in `api.ts` only — UI stays presentation.

### Don't:

- **Don't** use gradient text (`background-clip: text`) on any chrome or marketing inside the app.
- **Don't** add colored `border-left` / `border-right` stripes on cards, alerts, or list rows.
- **Don't** default to glassmorphism (backdrop-blur cards) without a functional reason (modals already use tonal + shadow).
- **Don't** use bounce or elastic easing — `--ease-out` (`cubic-bezier(0.22, 1, 0.36, 1)`) for UI; reserve `--ease-spring` only where already established.
- **Don't** nest cards inside cards — use spacing and a single surface tier.
- **Don't** invent a second button shape vocabulary in product chrome; topbar/canvas exceptions stay scoped to their modules.
- **Don't** ship modal-first flows when inline or panel expansion works (inspector, dock, settings patterns).
- **Don't** add arbitrary z-index (`999`, `9999`) — follow dropdown → sticky → modal-backdrop → modal → toast order.
