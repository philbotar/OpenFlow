# Contributing

How to change code in this repository.

## Filesystem

```text
contributing/
├── README.md              # This index
├── coding-patterns.md     # Architecture rules, ownership, persistence, conventions
└── testing-workflows.md   # Test layers, acceptance rules, when to run each suite
```

## Read Order

1. [`coding-patterns.md`](coding-patterns.md) — where logic lives, runtime semantics, change checklist.
2. [`testing-workflows.md`](testing-workflows.md) — how to verify behavior without the desktop UI.

Also read before larger changes:

- [`../architecture/contract.md`](../architecture/contract.md) — layer dependency rules.
- [`../../AGENTS.md`](../../AGENTS.md) — repo map and file-level ownership.
- [`../glossary.md`](../glossary.md) — domain vocabulary.

## Boundary Seams

Add a port/trait only when a consumer is typed on that interface. Current seams:

- `crates/engine/src/ports/` — `AiPort`, human input, tool approval
- `crates/providers/src/client.rs` — `AiClient` implements `AiPort`
- `crates/ui/src/port.ts` — `UiDesktopOutboundPort` for swappable desktop backend

## UI Layout

| Path | Purpose |
| --- | --- |
| `crates/ui/src/context/` | `AppProvider` / `AppContext` — app state, run listeners, navigation |
| `crates/ui/src/screens/` | Full-page routes: `EditorScreen`, `AgentsScreen`, `SettingsScreen` |
| `crates/ui/src/components/` | Shared chrome: `AppHeader`, sidebar primitives, conversation UI |
| `crates/ui/src/panels/` | Editor overlays: `InspectorPanel`, `WorkflowSettingsPanel`, `DockPanel` |
| `crates/ui/src/canvas/` | Workflow graph rendering |
| `crates/ui/src/forms/` | Node/agent configuration editors |
| `crates/ui/src/lib/types.ts` | Frontend DTO mirror types |
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers |

`App.tsx` is a thin shell: provider, toaster, sidebar, header, and `ScreenRouter`.

## Persistence Overview

| Data | Location | Owning module |
| --- | --- | --- |
| App workflows | `{data_local}/step-through-agentic-workflow/workflows.json` | `orchestration/src/storage.rs` |
| Projects | `{data_local}/openflow/projects.json` (migrates from legacy slug) | `orchestration/src/project_store.rs` |
| Saved agents | `{data_local}/openflow/agents.json` (migrates from legacy slug) | `orchestration/src/agent_store.rs` |
| Settings | `{data_local}/step-through-agentic-workflow/settings.json` | `orchestration/src/settings_store.rs` |
| Node templates | `{data_local}/openflow/templates.json` (migrates from legacy slug) | `orchestration/src/template_store.rs` |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` | `orchestration/src/flow_store.rs` |
| Provider API keys | Plaintext in `settings.json` (`ProviderProfile.api_key`) | `orchestration/src/settings_store.rs` |
| Skills | Discovered at runtime from Cursor/Claude skill dirs (not persisted) | `orchestration/src/skill_store.rs` |

`AppBackend::load_all_workflows` merges app-store workflows with project-discovered workflows (project files win on ID collision).

## Verification (Quick)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

See [`testing-workflows.md`](testing-workflows.md) for layered test commands.
