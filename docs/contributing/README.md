# Contributing

How to change code in this repository.

## Filesystem

```text
contributing/
├── README.md              # This index
├── development-lanes.md   # Change classification, skill routing, verification lanes
├── coding-patterns.md     # Architecture rules, ownership, persistence, conventions
├── releasing.md           # Desktop version bumps, GitHub tags, updater releases
└── testing-workflows.md   # Test layers, acceptance rules, when to run each suite
```

## Read order

1. [`development-lanes.md`](development-lanes.md) - classify the change, pick the matching playbook, and choose verification.
2. [`coding-patterns.md`](coding-patterns.md) - where logic lives, runtime semantics, change checklist.
3. [`testing-workflows.md`](testing-workflows.md) - how to verify behavior without the desktop UI.
4. [`releasing.md`](releasing.md) - when and how to bump desktop version, tag, and publish GitHub Releases.

Also read before larger changes:

- [`../architecture/end-to-end-runtime.md`](../architecture/end-to-end-runtime.md) - code-grounded run path.
- [`../architecture/contract.md`](../architecture/contract.md) - layer dependency rules.
- [`../../AGENTS.md`](../../AGENTS.md) - repo map and file-level ownership.
- [`../glossary.md`](../glossary.md) - engine and app vocabulary.
- Project skills under [`.cursor/skills/`](../../.cursor/skills/) — routed from [`development-lanes.md`](development-lanes.md).

## Boundary seams

Add a port/trait only when a consumer is typed on that interface. Current seams:

- `crates/engine/src/ports/` - `AiPort`, `ToolPort`
- `crates/providers/src/client.rs` - `AiClient` implements `AiPort`
- `crates/ui/src/api.ts` - typed Tauri invoke/event wrappers

## UI layout

| Path | Purpose |
| --- | --- |
| `crates/ui/src/context/` | `AppProvider` / `AppContext` - app state, run listeners, navigation |
| `crates/ui/src/screens/` | Full-page routes: `EditorScreen`, `AgentsScreen`, `SettingsScreen` |
| `crates/ui/src/components/` | Shared chrome: `AppHeader`, sidebar primitives, conversation UI |
| `crates/ui/src/panels/` | Editor overlays: `InspectorPanel`, `WorkflowSettingsPanel`, `DockPanel` |
| `crates/ui/src/canvas/` | Workflow graph rendering |
| `crates/ui/src/forms/` | Node/agent configuration editors |
| `crates/ui/src/lib/types.ts` | Frontend DTO mirror types |
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers |

`App.tsx` is a thin shell: provider, toaster, sidebar, header, and `ScreenRouter`.

## Persistence overview

| Data | Location | Owning module |
| --- | --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` | `orchestration/src/adapters/storage/app_workflow_store.rs` |
| Projects | `{data_local}/openflow/projects.json` (migrates from legacy slug) | `orchestration/src/adapters/storage/project_store.rs` |
| Saved agents | `{data_local}/openflow/agents.json` (migrates from legacy slug) | `orchestration/src/adapters/storage/agent_store.rs` |
| Settings | `{data_local}/openflow/settings.json` | `orchestration/src/adapters/storage/settings_store.rs` |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` | `orchestration/src/adapters/storage/project_workflow_store.rs` |
| Provider API keys | Plaintext in `settings.json` (`ProviderProfile.api_key`) | `orchestration/src/adapters/storage/settings_store.rs` |
| Skills | Discovered at runtime from Cursor/Claude skill dirs (not persisted) | `orchestration/src/adapters/storage/skill_store.rs` |

`AppBackend::load_all_workflows` merges app-store workflows with project-discovered workflows (project files win on ID collision).

## Verification quick path

| Goal | Command |
| --- | --- |
| Compile loop | `./scripts/check-fast.sh` |
| Iterate | `./scripts/test-fast.sh` (+ `--execution` if touching runs) |
| Handoff / PR | `./scripts/verify.sh` |
| Full workspace Rust (incl. desktop) | `./scripts/verify.sh test` |
| Parallel agents on one machine | `VERIFY_ISOLATE_TARGET=1 ./scripts/verify.sh` |

```bash
./scripts/check-fast.sh
./scripts/test-fast.sh
./scripts/verify.sh
```

Default verify reuses `./target` and runs the CI-aligned `test-fast` lane (no desktop; Bedrock/AWS off unless desktop/featured). Opt-in full workspace: `./scripts/verify.sh test`.

Intentional changes to engine's public surface require updating `crates/engine/tests/snapshots/public_api.txt` (`cargo +nightly public-api` from `crates/engine/`).

See [`testing-workflows.md`](testing-workflows.md) for layered test commands and [Miri](https://github.com/rust-lang/miri) undefined-behavior checks.

## Miri (undefined behavior)

We run [Miri](https://github.com/rust-lang/miri) on **`engine`** and **`orchestration`** to catch invalid memory use and aliasing in pure Rust code. Local: `./scripts/miri.sh` or `./scripts/verify.sh --deep miri`. CI: parallel per-crate Miri matrix in `.github/workflows/ci.yml` (Ubuntu; only changed crates). Requires nightly (`rustup toolchain install nightly --component miri`). Details: [`testing-workflows.md` § Miri](testing-workflows.md#miri).
