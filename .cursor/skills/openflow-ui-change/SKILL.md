---
name: openflow-ui-change
description: >-
  Procedural playbook for OpenFlow UI edits. Use when changing crates/ui/**
  — api.ts, AppProvider, screens, panels, canvas, forms, or DTOs.
---

# openflow-ui-change

Procedural only. Architecture facts live in the docs below — do not invent a second model.

## Intake

1. Read `crates/ui/AGENTS.md`.
2. For invoke/event path: `docs/architecture/end-to-end-runtime.md`.
3. Confirm `docs/architecture/contract.md` — UI never imports Rust crates.

## Placement rules

- `@tauri-apps/*` only in `api.ts` and test mocks — there is no separate `port.ts`; `api.ts` is the whole desktop seam.
- New backend capability: wrapper in `api.ts` → types in `lib/types/` → consumer.
- No engine/orchestration business rules in the UI — display and submit only.

## Where to edit

| Change | Path |
| --- | --- |
| Desktop seam | `api.ts`, `lib/types/` |
| Run chat | `components/conversation/` |
| Canvas | `canvas/` |
| Inspector / settings panels | `panels/`, `forms/` |
| Global run state | `context/` |

## Verify

```bash
npm --prefix crates/ui run typecheck
npm --prefix crates/ui run test
```

Or: `./scripts/verify.sh ui-typecheck ui-test`.
