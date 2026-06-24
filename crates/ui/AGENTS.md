---
description: Coding agent orientation for the ui crate
globs: crates/ui/**
alwaysApply: false
---

# AGENTS.md — UI

**Question this crate answers:** How does the user edit workflows, chat with agents, and control runs?

React/TypeScript presentation layer. Talks to desktop only through `UiDesktopOutboundPort`.

## Architecture

```
App.tsx (shell)
├── context/          AppProvider, run listeners, global state
├── screens/          EditorScreen, AgentsScreen, SettingsScreen
├── components/       Header, sidebar, conversation UI
├── panels/           Inspector, workflow settings, dock, terminal
├── canvas/           Workflow graph rendering
├── forms/            Node/agent configuration editors
├── api.ts            Tauri invoke wrappers (ONLY @tauri-apps usage with port.ts)
└── port.ts           UiDesktopOutboundPort seam
```

### Layer rules

| Layer | Role |
| --- | --- |
| `port.ts` + `api.ts` | Desktop seam — swap backend in tests |
| `lib/types/` | DTO mirror of orchestration IPC payloads |
| `context/` | App-wide state, run event subscription |
| `screens/` | Full-page routes |
| `components/` | Reusable UI; conversation, sidebar primitives |
| `panels/` | Editor chrome (inspector, settings, dock) |
| `canvas/` | Graph layout and interaction |
| `lib/` | Pure helpers (workflow utils, file refs, theme) |

### Folder layout (`components/` and `lib/`)

**Root rule:** `components/` and `lib/` contain **only subdirectories + one root `index.ts` barrel**. No loose source files at those roots.

**Single-component folder:**

```text
AppHeader/
  AppHeader.tsx
  AppHeader.test.tsx   # when present
  index.ts             # export * from "./AppHeader";
```

**Domain folder** (e.g. `conversation/`): multiple related files are fine; expose public API via domain `index.ts`.

**Imports:** prefer `@/components` or `@/lib` for new code; relative paths like `../components/AppHeader` keep working via directory resolution.

**Mechanical moves:** `./scripts/ui-move-module.sh crates/ui/src/components AppHeader` and `./scripts/ui-move-lib-module.sh workflow`.

## Dependency rules

**Allowed:** React, Vite, test utils; `@tauri-apps/*` **only** in `api.ts`, `port.ts`, and test mocks

**Forbidden (CI-enforced):**
- Direct `@tauri-apps/*` in components, screens, or hooks
- Importing Rust crates or duplicating orchestration business rules
- Bypassing `UiDesktopOutboundPort` for backend calls

UI never calls `engine` or `orchestration` — always `invoke` through the port.

## Code standards

1. **Seam-first** — new backend capability → extend `port.ts`, implement in `api.ts`, add types in `lib/types/`.
2. **No domain logic** — validation summaries and run semantics come from backend; UI displays and submits.
3. **Sidebar primitives** — use `SidebarNavButton`, `SidebarList`, `SidebarListRow` for consistent lists.
4. **Inspector visibility** — hide when no node selected; toggle `WorkflowSettingsPanel` vs `InspectorPanel` by editor mode.
5. **Naming** — match backend DTO field names in `types.ts`; camelCase in IPC via serde on desktop side.
6. **Styles** — global tokens and layout in `styles/index.css`; component-scoped classes over inline styles.

## Patterns

### Where to add code

| Change | Location |
| --- | --- |
| New backend call | `port.ts` → `api.ts` → `lib/types/` → consumer |
| Editor layout / dock | `screens/EditorScreen.tsx`, `panels/DockPanel.tsx` |
| Run conversation UI | `components/conversation/` |
| Workflow canvas | `canvas/` |
| Node/agent forms | `forms/` |
| Settings UX | `screens/SettingsScreen.tsx`, `settings/` |
| Sidebar / projects | `components/sidebar/` |
| Global run state | `context/AppProvider.tsx` |

### Desktop seam

```typescript
// port.ts — interface tests mock
export interface UiDesktopOutboundPort {
  startRun(...): Promise<WorkflowRunState>;
  // ...
}

// api.ts — sole Tauri invoke site
export const desktopPort: UiDesktopOutboundPort = { ... };
```

Components use `useApp()` / injected port — never raw `invoke`.

### Testing

| Pattern | When |
| --- | --- |
| `foo.test.ts` / `Foo.test.tsx` | Colocated with source |
| Mock port | `AppProvider` tests, screen tests |

No `__tests__/` directories.

```bash
npm --prefix crates/ui run typecheck
npm --prefix crates/ui run test
```

Or full gate: `./scripts/verify.sh ui-typecheck ui-test`

## Change checklist

1. Tauri imports only in `api.ts` / `port.ts`?
2. Types updated in `lib/types/` for new IPC fields?
3. Port interface extended before component work?
4. Tests mock `UiDesktopOutboundPort`, not Tauri directly?
5. Run `./scripts/verify.sh ui-typecheck ui-test`.

## Dev commands

```bash
npm --prefix crates/ui run dev          # frontend only
npm --prefix crates/desktop run start -- dev   # full app
```

## Related docs

- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`docs/contributing/coding-patterns.md`](../../docs/contributing/coding-patterns.md) — UI ownership table
- [`../../AGENTS.md`](../../AGENTS.md) — workspace map
