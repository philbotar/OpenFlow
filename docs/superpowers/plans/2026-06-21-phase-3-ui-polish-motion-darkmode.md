# Phase 3 UI Polish, Motion, Transcript Surfaces, and Dark Mode Completion Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the Phase 3 polish slice from the design audit: unify the conversation/transcript visual language, standardize motion and feedback states, and complete the touched dark-theme surfaces so the UI feels deliberate in both light and dark mode.

**Architecture:** Keep this slice in the `UI/Desktop seam` lane. The work stays in `crates/ui/src/styles/index.css` plus the conversation, dock, terminal, and run-history presentation components under `crates/ui/src/components/` and `crates/ui/src/panels/`. Do not change backend contracts, run semantics, or theme persistence behavior; this is a visual-system completion pass on top of the Phase 1 palette layer and the Phase 2 typography/shell work.

**Tech Stack:** SolidJS, TypeScript, CSS custom properties, existing theme helpers in `crates/ui/src/lib/theme/`, xterm theme bridging, focused Vitest, browser/manual breakpoint and theme verification.

---

## Phase Boundary

### In Scope

- Define a small Phase 3 motion grammar and use it consistently on touched UI states.
- Unify chat transcript surfaces across message rows, thinking rows, tool rows, replay banners, and segment framing.
- Standardize loading, streaming, and error presentation for the dock-adjacent panels touched in this slice.
- Complete a semantic dark-mode pass for the touched conversation/dock/terminal/history surfaces.
- Reduce one-off dark overrides for touched selectors where semantic tokens can carry the theme instead.

### Out Of Scope

- Any change to run lifecycle, replay behavior, terminal behavior, or tool execution semantics.
- A full repo-wide dark-mode rewrite across untouched inspector/canvas/sidebar selectors.
- Functional changes to chat segmentation, filtering, replay, or approval flows.
- A global CSS rename/delete pass across every remaining legacy variable.

### Assumptions

Phase 3 should begin only after the branch state is reconciled with the approved Phase 1 and Phase 2 decisions:

1. the stronger dock tabs stay out, per current user preference
2. the palette contract from Phase 1 remains the source of truth
3. the Phase 2 typography and empty-state primitives are either landed or treated as the baseline for touched selectors

If those assumptions are not true in the execution branch, reconcile them before Task 1 rather than layering polish on unstable selectors.

---

## Design Intent

Phase 3 is where the UI stops feeling like a set of improved screens and starts feeling like one finished product.

The target qualities for this slice are:

- transcript surfaces that read as one cohesive system
- live activity that is obvious without becoming noisy
- loading and error states that feel intentional instead of incidental
- dark mode that looks designed, not patched
- motion that supports orientation and feedback rather than drawing attention to itself

This is the polish pass, but it still needs discipline: fewer visual dialects, fewer one-off overrides, and fewer states that rely on raw color intuition.

---

## Proposed System Additions

### Motion Grammar

Use one small motion vocabulary in `crates/ui/src/styles/index.css`:

| Token / pattern | Intended use |
| --- | --- |
| `--dur-fast`, `--dur-med`, `--dur-slow` | keep existing durations; do not invent more timing buckets unless needed |
| `message-enter` | transcript items, approvals, and inline feedback entering the flow |
| `screen-fade-in` | route-level transitions only |
| `spinner-rotate` | active loading indicators |
| `thinking-pulse` / `chat-live-pulse` | live/streaming emphasis only, never for static UI |

Phase 3 rule: touched components should use these shared patterns instead of introducing fresh bespoke animations.

### Transcript Surface Ladder

Conversation-related surfaces should map onto one visual ladder:

| Surface | Intended role |
| --- | --- |
| transcript background | neutral conversation ground |
| segment shell | group frame for one node transcript |
| assistant/user message body | readable content surfaces |
| tool/thinking row | lower-emphasis operational status surface |
| replay/live banners | system-state callouts |
| scroll button / inline chips | small raised utility surfaces |

### Feedback-State Contract

Touched loading and error states should use one system:

| State | Intended treatment |
| --- | --- |
| loading | spinner or skeleton, never plain text alone when a surface exists |
| streaming/live | subtle motion + status tint, never large saturated blocks |
| empty | continue using the shared empty-state primitive from Phase 2 |
| error | shared danger tone, readable border/background pair, calm copy |
| replay/read-only | subdued banner distinct from destructive or warning states |

### Dark-Mode Rule For Phase 3

For touched selectors:

- prefer semantic tokens first
- keep selector-specific dark overrides only where gradients, xterm surfaces, or canvas-specific visuals genuinely need them
- avoid adding new hardcoded dark literals when an existing semantic token can express the role

---

## File Structure

| File | Responsibility |
| --- | --- |
| `docs/superpowers/plans/2026-06-21-phase-3-ui-polish-motion-darkmode.md` | Durable implementation plan for this slice |
| `crates/ui/src/styles/index.css` | Phase 3 motion/state tokens, transcript surface ladder, loading/error styling, touched dark-mode cleanup |
| `crates/ui/src/components/conversation/Conversation.tsx` | Shared conversation shell, scroll button, empty-state surface rules if markup changes are needed |
| `crates/ui/src/components/conversation/ConversationMessages.tsx` | Segment framing, filter-chip context, live/focus indicators |
| `crates/ui/src/components/conversation/ChatPanel.tsx` | Replay banner, live-status strip, inline chat-state presentation |
| `crates/ui/src/components/conversation/ToolBubble.tsx` | Tool-row hierarchy, expandable output framing, streaming preview treatment |
| `crates/ui/src/components/conversation/ThinkingBubble.tsx` | Thinking-row hierarchy, expansion framing, streaming treatment |
| `crates/ui/src/components/conversation/ToolApprovalCard.tsx` | Loading/error preview consistency if touched by the shared feedback-state system |
| `crates/ui/src/components/conversation/FileChangesPanel.tsx` | Error/loading tone consistency if touched |
| `crates/ui/src/panels/DockPanel.tsx` | Dock-state framing and overview/trace fallback consistency |
| `crates/ui/src/panels/RunHistoryPanel.tsx` | Loading state, row hierarchy, action affordance, replay selection treatment |
| `crates/ui/src/panels/TerminalPanel.tsx` | Empty/loading overlay polish, tab-bar and host visual consistency under both themes |
| `crates/ui/src/settings/AppearanceSection.tsx` | Theme selector polish if needed for dark-mode completion |
| `crates/ui/src/lib/theme/index.ts` | Theme-token consumers only if terminal colors or theme hooks need a visual-support change, not a behavior change |
| `crates/ui/src/app/App.test.tsx` | Dock/chat/route-level presentation regressions where app-shell behavior is visible |
| `crates/ui/src/components/conversation/toolBubbleState.test.ts` | Tool transcript wording regressions if label copy changes |

Optional focused tests if created during execution:

| File | Responsibility |
| --- | --- |
| `crates/ui/src/components/conversation/ThinkingBubble.test.tsx` | Expand/collapse and streaming presentation |
| `crates/ui/src/components/conversation/ToolBubble.test.tsx` | Expand/collapse, preview, and error-state presentation |

---

## Acceptance Criteria

- Chat, tool output, thinking output, replay banners, and run-history surfaces feel like one coherent transcript system.
- Streaming, loading, and error states are visually distinct but calmer than the current mix of text-only, pulse-only, and one-off danger styling.
- Touched components use the shared motion grammar and still respect reduced-motion behavior.
- Dark mode for touched surfaces preserves the same hierarchy and affordance clarity as light mode.
- The touched dark-theme rules rely more on semantic tokens and less on selector-specific raw dark literals.

---

## Task 1: Establish The Phase 3 Motion And Feedback-State Contract

**Files:**
- Modify: `crates/ui/src/styles/index.css`

- [ ] **Step 1: Inventory touched motion and state patterns**

Capture the current live patterns before changing them:

- transcript/message enter
- spinner usage
- thinking/tool streaming pulse
- replay/read-only banners
- loading/error utilities

Useful commands:

```bash
rg -n 'screen-fade-in|message-enter|spinner-rotate|thinking-pulse|chat-live-pulse|skeleton-shimmer|loading-inline|error' crates/ui/src/styles/index.css
```

- [ ] **Step 2: Define a compact Phase 3 contract in the CSS**

Add a clearly labeled block documenting:

- approved motion patterns
- approved state treatments
- which selectors should use them

- [ ] **Step 3: Normalize touched motion utilities onto the shared timing tokens**

Replace ad hoc `150ms`, `200ms`, and similar transition declarations in the touched conversation/dock selectors with the existing duration/easing tokens where safe.

- [ ] **Step 4: Preserve reduced-motion behavior**

Keep the current reduced-motion policy intact and extend it to any new animation or transition introduced in this slice.

- [ ] **Step 5: Verify**

Run:

```bash
rg -n 'transition:|animation:' crates/ui/src/styles/index.css
git diff --check
```

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/styles/index.css
git commit -m "plan(ui): define phase 3 motion and feedback-state contract"
```

---

## Task 2: Unify The Conversation And Transcript Surface System

**Files:**
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/components/conversation/Conversation.tsx`
- Modify: `crates/ui/src/components/conversation/ConversationMessages.tsx`
- Modify: `crates/ui/src/components/conversation/ChatPanel.tsx`
- Modify: `crates/ui/src/components/conversation/ToolBubble.tsx`
- Modify: `crates/ui/src/components/conversation/ThinkingBubble.tsx`
- Optional: Modify `crates/ui/src/components/conversation/Message.tsx`

- [ ] **Step 1: Define one transcript surface ladder**

Bring the following into one visual family:

- message shells
- node segment containers
- tool rows
- thinking rows
- replay banners
- live status strips
- scroll-to-latest button

- [ ] **Step 2: Reduce the separate “tool-line” and “thinking” dialects**

The current code shares some structure but still reads as different ad hoc treatments. Keep the semantic distinction, but align spacing, padding, radius, border language, and header affordances.

- [ ] **Step 3: Tighten transcript hierarchy**

Make the user’s eye reliably distinguish:

- node/group label
- live or replay state
- assistant/user content
- operational rows such as tools/thinking

- [ ] **Step 4: Keep long content safe**

Preserve the current behavior for:

- wrapped tool output
- markdown blocks and tables
- scrollable expanded content
- scroll-to-bottom affordance

This is visual unification, not a behavior rewrite.

- [ ] **Step 5: Verify**

Run:

```bash
npm --prefix crates/ui run typecheck
```

Manual checks:

- settled chat with multiple node segments
- replay mode banner
- streaming thinking/tool rows
- long tool output expansion
- empty conversation state

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/styles/index.css crates/ui/src/components/conversation/Conversation.tsx crates/ui/src/components/conversation/ConversationMessages.tsx crates/ui/src/components/conversation/ChatPanel.tsx crates/ui/src/components/conversation/ToolBubble.tsx crates/ui/src/components/conversation/ThinkingBubble.tsx
git commit -m "plan(ui): unify phase 3 transcript surfaces"
```

---

## Task 3: Standardize Loading, Replay, And Error States Across Dock Surfaces

**Files:**
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/panels/DockPanel.tsx`
- Modify: `crates/ui/src/panels/RunHistoryPanel.tsx`
- Modify: `crates/ui/src/panels/TerminalPanel.tsx`
- Modify: `crates/ui/src/components/conversation/ToolApprovalCard.tsx`
- Modify: `crates/ui/src/components/conversation/FileChangesPanel.tsx`
- Optional: Modify `crates/ui/src/components/AppHeader/AppHeader.tsx`

- [ ] **Step 1: Replace text-only loading placeholders where a surfaced state exists**

Examples already in scope:

- run history loading
- terminal starting overlay
- tool approval preview loading
- file changes loading

- [ ] **Step 2: Normalize error styling across touched panels**

Bring `file-change-error`, `file-edit-preview-error`, terminal-related messaging, and any touched inline danger rows onto the same border/background/copy pattern.

- [ ] **Step 3: Distinguish replay/read-only from failure**

Replay mode should read as a calm system state, not as warning or success. Keep resume actions visible but visually secondary to the banner copy.

- [ ] **Step 4: Keep dock overview and trace fallbacks consistent**

Overview empty states, trace-detail empty states, and run-history empty/loading states should all feel like the same family of system feedback.

- [ ] **Step 5: Verify**

Run:

```bash
npm --prefix crates/ui run typecheck
```

Manual checks:

- no runs yet
- loading runs
- replay open
- terminal starting
- tool approval preview error

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/styles/index.css crates/ui/src/panels/DockPanel.tsx crates/ui/src/panels/RunHistoryPanel.tsx crates/ui/src/panels/TerminalPanel.tsx crates/ui/src/components/conversation/ToolApprovalCard.tsx crates/ui/src/components/conversation/FileChangesPanel.tsx
git commit -m "plan(ui): standardize loading and error states for phase 3"
```

---

## Task 4: Complete The Touched Dark-Mode Pass

**Files:**
- Modify: `crates/ui/src/styles/index.css`
- Modify: `crates/ui/src/panels/TerminalPanel.tsx`
- Modify: `crates/ui/src/settings/AppearanceSection.tsx`
- Optional: Modify `crates/ui/src/lib/theme/index.ts`

- [ ] **Step 1: Inventory touched dark overrides**

Focus on:

- conversation surfaces
- dock and terminal surfaces
- run history
- theme selector
- touched banners and feedback states

Useful command:

```bash
rg -n '\[data-theme="dark"\]' crates/ui/src/styles/index.css
```

- [ ] **Step 2: Move touched selectors back onto semantic tokens where possible**

If a dark override only exists to supply a surface, border, or text role that now has a semantic token, collapse it back to the token-driven rule.

- [ ] **Step 3: Keep selector-specific dark styling only where it earns its place**

Likely valid exceptions:

- background gradients for app/chrome surfaces
- xterm-specific colors
- canvas-specific rendering

- [ ] **Step 4: Verify theme parity manually**

Check the touched transcript/dock/terminal surfaces in:

- system
- light
- dark

Look specifically for:

- contrast drift
- inactive controls disappearing
- replay/loading/error banners reading as the wrong severity

- [ ] **Step 5: Verify**

Run:

```bash
npm --prefix crates/ui run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/styles/index.css crates/ui/src/panels/TerminalPanel.tsx crates/ui/src/settings/AppearanceSection.tsx crates/ui/src/lib/theme/index.ts
git commit -m "plan(ui): complete touched dark-mode polish for phase 3"
```

---

## Task 5: Add Focused Regression Coverage And Final Verification

**Files:**
- Modify: `crates/ui/src/app/App.test.tsx`
- Modify: `crates/ui/src/components/conversation/toolBubbleState.test.ts`
- Optional: Create `crates/ui/src/components/conversation/ThinkingBubble.test.tsx`
- Optional: Create `crates/ui/src/components/conversation/ToolBubble.test.tsx`

- [ ] **Step 1: Cover transcript interaction regressions**

At minimum verify the touched execution branch still supports:

- expandable tool output
- thinking row expansion
- replay banner rendering
- empty-state fallbacks

- [ ] **Step 2: Cover copy-sensitive state labels only where they matter**

If label wording changes during the polish pass, update the small focused tests rather than relying only on broad snapshots.

- [ ] **Step 3: Run the focused UI lane**

Run:

```bash
npm --prefix crates/ui exec vitest run src/app/App.test.tsx src/components/conversation/toolBubbleState.test.ts
npm --prefix crates/ui run typecheck
git diff --check
```

- [ ] **Step 4: Run the repo verification gate before handoff**

Run:

```bash
./scripts/verify.sh ui-typecheck ui-test
```

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/app/App.test.tsx crates/ui/src/components/conversation/toolBubbleState.test.ts crates/ui/src/components/conversation/ThinkingBubble.test.tsx crates/ui/src/components/conversation/ToolBubble.test.tsx
git commit -m "plan(ui): add phase 3 polish regression coverage"
```

---

## Phase 3 Notes For The Implementer

- Do not reintroduce the stronger dock-tab treatment the user explicitly declined.
- Keep this slice presentation-only even when it touches replay, terminal, or approval surfaces.
- Prefer shared CSS/system fixes over stacking more local overrides at the bottom of `index.css`.
- If execution reveals that a touched component still depends on Phase 2 shell work that never landed, finish the narrow prerequisite first and then continue with this plan.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-phase-3-ui-polish-motion-darkmode.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints

Which approach?
