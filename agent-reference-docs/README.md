# Agent Reference Docs

Purpose: fast, stable standards for contributors and coding agents.

## Read Order

1. `/Users/philipbotar/Developer/Step-through-agentic-workflow/AGENTS.md` for repo map and ownership.
2. `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/coding-patterns.md` for architecture and implementation rules.
3. `/Users/philipbotar/Developer/Step-through-agentic-workflow/agent-reference-docs/testing-workflows.md` for workflow acceptance and live-AI verification.

## Active Sections

- `crates/ui`: frontend shell, canvas, inspector, typed invoke wrappers.
- `crates/desktop`: Tauri adapter and desktop startup.
- `crates/orchestration`: runtime state, persistence, provider config, execution coordination.
- `crates/domain`: workflow model, validation, execution semantics, ports.
- `crates/providers`: concrete provider adapters and transport mapping.

## Dev Entry Points

- Desktop app: `npm --prefix crates/desktop run start -- dev`
- Frontend only: `npm --prefix crates/ui run dev`
- Frontend typecheck: `npm --prefix crates/ui run typecheck`

## Scope

- These docs define how we change code in this repository.
- If code and docs diverge, update docs in the same change set.
- Keep docs explicit and scan-friendly; prefer concrete file paths and exact token values.
