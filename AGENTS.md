---
description: 
alwaysApply: true
---

# AGENTS.md

Single-file orientation for contributors and coding agents.

## Crate orientation (AGENTS.md)

| Crate | File |
| --- | --- |
| engine | [`crates/engine/AGENTS.md`](crates/engine/AGENTS.md) |
| providers | [`crates/providers/AGENTS.md`](crates/providers/AGENTS.md) |
| orchestration | [`crates/orchestration/AGENTS.md`](crates/orchestration/AGENTS.md) |
| desktop | [`crates/desktop/AGENTS.md`](crates/desktop/AGENTS.md) |
| ui | [`crates/ui/AGENTS.md`](crates/ui/AGENTS.md) |

Each file covers architecture, dependency rules, code standards, patterns, and change checklists for that crate.

## 30-Second Intake

1. This is a Rust workspace with five crates: `engine`, `providers`, `orchestration`, `desktop`, `ui`.
2. Core rule: keep engine logic in `engine`; keep API transport/auth quirks in `providers`; keep runtime/state/storage in `orchestration`; keep Tauri adapter code in `desktop`; keep frontend code in `ui`.
   - **engine** — valid workflow + run behavior
   - **orchestration** — store, load, wire, host runs
   - **providers** — LLM transport
   - **ui** / **desktop** — user interaction
3. Start docs at `docs/README.md` — see [Documentation](#documentation) for the full tree.
4. Development lanes: `docs/contributing/development-lanes.md` (skills under `.cursor/skills/`).
5. Coding patterns: `docs/contributing/coding-patterns.md`.
6. Workflow verification: `docs/contributing/testing-workflows.md`.
7. Engine vocabulary: `docs/glossary.md`.
8. Run / pause / IPC path: [`docs/architecture/end-to-end-runtime.md`](docs/architecture/end-to-end-runtime.md).

## Known doc drift (trust code)

When these disagree with the tree, **code wins**. Fix the doc in the same change when you notice it.

| Claim that was wrong | Reality |
| --- | --- |
| Providers live in `openai_compat.rs` / `anthropic.rs` / `sse.rs` | Rig transport under `crates/providers/src/rig_adapter/`; shared mapping in `mapping/` |
| Engine runs one node at a time | Ready work in a layer runs concurrently via `FuturesUnordered` (`InteractiveEngine::run`) |
| Historical plans under `docs/superpowers/plans/` naming deleted provider files | Archive only — do not follow those paths for new edits |

## Boundary Seams

Add a port/trait only when a consumer is typed on that interface. Otherwise call the concrete type directly.

| Seam | Location |
| --- | --- |
| LLM invocation (`AiPort`, `AgentRequest`) | `crates/engine/src/ports/outbound.rs` |
| Tool and subagent execution (`ToolPort`) | `crates/engine/src/ports/outbound.rs` → `crates/orchestration/src/run/execution/tool_port.rs` |
| Provider client (`AiClient: AiPort`) | `crates/providers/src/client.rs` |
| UI → desktop IPC | `crates/ui/src/api.ts` |

## Documentation

```text
docs/
├── README.md
├── glossary.md
├── getting-started/
│   └── README.md
├── guides/
│   └── first-workflow.md
├── concepts/
│   ├── README.md
│   ├── how-openflow-works.md
│   └── workflows-and-runs.md
├── reference/
│   └── README.md
├── troubleshooting/
│   └── README.md
├── contributing/
│   ├── README.md
│   ├── development-lanes.md
│   ├── coding-patterns.md
│   └── testing-workflows.md
└── architecture/
    ├── README.md
    ├── end-to-end-runtime.md
    ├── contract.md
    ├── threading-concurrency.md
    └── diagrams/
```

| Doc | Use when |
| --- | --- |
| `docs/README.md` | First read; filesystem index |
| `docs/getting-started/README.md` | Running the app and configuring a provider |
| `docs/guides/first-workflow.md` | Building the first workflow |
| `docs/concepts/how-openflow-works.md` | Understanding the runtime path |
| `docs/reference/README.md` | Commands, storage paths, provider key resolution |
| `docs/troubleshooting/README.md` | Setup, provider, run, and verification failures |
| `docs/contributing/development-lanes.md` | Classifying a change, selecting playbook/skill, choosing verification |
| `docs/contributing/coding-patterns.md` | Ownership, runtime semantics, conventions |
| `docs/contributing/testing-workflows.md` | Acceptance tests, live-AI smoke |
| `docs/architecture/end-to-end-runtime.md` | UI → desktop → orchestration → engine → providers run path |
| `docs/architecture/contract.md` | Layer boundaries and dependency rules |
| `docs/architecture/threading-concurrency.md` | Runtimes, async I/O, parallelism |
| `docs/glossary.md` | Engine terms and naming |

## Development Lanes

Before editing, classify the change with [`docs/contributing/development-lanes.md`](docs/contributing/development-lanes.md). Agent skills and editor rules should route to that doc instead of carrying their own copy of architecture facts.

| Touched area | Lane | Local guide |
| --- | --- | --- |
| `crates/engine/**` | Engine semantics | `crates/engine/AGENTS.md` |
| `crates/orchestration/src/run/**` | Run orchestration | `crates/orchestration/AGENTS.md` |
| `crates/orchestration/src/{agent,workflow,project,settings,tool}/**` | Application/domain service | `crates/orchestration/AGENTS.md` |
| `crates/orchestration/src/adapters/**` | Concrete adapter/I/O | `crates/orchestration/AGENTS.md` |
| `crates/providers/**` | Provider adapter | `crates/providers/AGENTS.md` |
| `crates/desktop/**` | Desktop IPC adapter | `crates/desktop/AGENTS.md` |
| `crates/ui/**` | UI/Desktop seam and presentation | `crates/ui/AGENTS.md` |

## Repo Map

### Workspace and engine

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `Cargo.toml` | Workspace members and shared dependencies | Adding crates or shared dep versions |
| `crates/engine/src/graph/` | Workflow model, `WorkflowSettings`, node config, `CallableAgent`, DAG validation | Changing schema, graph rules, or scheduling |
| `crates/engine/src/execution/interactive_engine/` | Interactive engine `run()` loop | Changing pause/resume or self-driving run behavior |
| `crates/engine/src/execution/subagent_runtime.rs` | Subagent declare/call builtins + turn machine | Changing subagent invocation semantics |
| `crates/engine/src/execution/telemetry.rs` | `RunTelemetry` interactive event enum | Changing run event vocabulary |
| `crates/engine/src/execution/node_invocation.rs` | Shared `AgentRequest` assembly | Changing upstream input or prompt wiring |
| `crates/engine/src/template/` | `Template`, locked fields, builtins | Changing template definitions |
| `crates/engine/src/ports/outbound.rs` | `AiPort`, `AgentRequest`, turn outcomes | Changing LLM invocation contract |
| `crates/engine/src/execution/interactive_engine/` | Human input and tool approval resume methods | Changing pause/resume behavior |

### Providers

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `crates/providers/src/client.rs` | `AiClient` implementing `AiPort`, config types | Changing provider client wiring |
| `crates/providers/src/mapping/` | Transcript/tool-arg mapping, `jsonrepair-rs` | Changing wire payload shape |
| `crates/providers/src/rig_adapter/` | Rig transport (OpenAI-compat, Anthropic, Bedrock) | Adding/changing provider HTTP/stream behavior |
| `crates/providers/src/lib.rs` | `create_provider` factory | Adding a new provider adapter |

### Orchestration

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `crates/orchestration/src/backend/mod.rs` | Thin `AppBackend` orchestrator; delegates to catalog modules | Adding/remapping desktop IPC commands |
| `crates/orchestration/src/workflow/catalog.rs` | Workflow CRUD, app/project merge, assign/unassign | Changing workflow persistence rules |
| `crates/orchestration/src/agent.rs` | Saved agent CRUD, `create_agent_node`, `AgentStore` port | Changing callable agent library behavior |
| `crates/orchestration/src/project/registry.rs` | Project load/save/create | Changing project registration |
| `crates/orchestration/src/settings/facade.rs` | Settings, keys, skills, validation summaries | Changing settings or provider readiness UX |
| `crates/orchestration/src/run/coordinator.rs` | Active run session, start/submit/apply events | Changing run lifecycle coordination |
| `crates/orchestration/src/run/execution/` | `drive/`, `headless.rs`, `tool_port.rs`, event projection, cwd | Changing execution host semantics |
| `crates/orchestration/src/run/state.rs` | Run/edit state, trace, chat logs | Changing run state or editor mutations |
| `crates/orchestration/src/adapters/storage/app_workflow_store.rs` | App workflows (`workflows.json`) | Changing app workflow persistence |
| `crates/orchestration/src/adapters/storage/project_workflow_store.rs` | Project workflows (`.flow/workflows/`) | Changing repo workflow file layout |
| `crates/orchestration/src/adapters/storage/project_store.rs` | Projects (`openflow/projects.json`) | Changing project bindings |
| `crates/orchestration/src/adapters/storage/agent_store.rs` | Saved agents (`openflow/agents.json`) | Changing agent definitions storage |
| `crates/orchestration/src/adapters/storage/skill_store.rs` | Skill discovery (read-only) | Changing skill search roots |
| `crates/orchestration/src/adapters/storage/settings_store.rs` | App settings (`settings.json`) | Changing settings schema |
| `crates/orchestration/src/settings/provider.rs` | Provider readiness and key resolution | Changing key precedence or env fallback |
| `crates/orchestration/src/tool/` | Tool registry, approval, runner | Changing tool catalog or execution |

### Desktop and UI

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `crates/desktop/src/lib.rs` | Tauri commands/events and app bootstrap | Changing frontend/backend IPC |
| `crates/ui/src/App.tsx` | Thin shell: provider, sidebar, header, router | Changing top-level layout |
| `crates/ui/src/context/` | `AppProvider`, `AppContext` | Changing app state or run listeners |
| `crates/ui/src/screens/` | `EditorScreen`, `AgentsScreen`, `SettingsScreen` | Changing full-page routes |
| `crates/ui/src/components/` | Header, sidebar, conversation UI | Changing shared chrome |
| `crates/ui/src/panels/` | Inspector, workflow settings, dock | Changing editor side panels |
| `crates/ui/src/canvas/` | Workflow graph rendering | Changing canvas look/behavior |
| `crates/ui/src/forms/` | Node/agent configuration editors | Changing inspector forms |
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers and UI desktop boundary | Changing RPC names, event listeners, or payloads |
| `crates/ui/src/lib/types.ts` | Frontend DTO mirror types | Changing command payload shapes |
| `crates/ui/src/styles/index.css` | Global styles and layout tokens | Changing spacing, inspector, dock CSS |

### Agent tooling

| Path | Purpose |
| --- | --- |
| `scripts/verify.sh` | Verification gate — see [Verification Commands](#verification-commands) |
| `tools/plan-review.html` | Standalone plan review UI — load markdown plans, comment, verdict chips, export/import reviews (`open tools/plan-review.html`) |

### Examples

| Path | Purpose |
| --- | --- |
| `examples/*.workflow.json` | Demo and smoke workflows |

## Common Change Paths

| Goal | Primary Files |
| --- | --- |
| Add a workflow rule or validation | `engine/src/graph/validation.rs`, tests in same file |
| Add a new provider adapter | Implement `AiPort` in `providers/`, wire via `create_provider` |
| Add or change `AiPort` or `ToolPort` contracts | `engine/src/ports/` |
| Add or change UI desktop seam | `ui/src/api.ts` |
| Change run execution semantics | `orchestration/src/run/execution/drive.rs`, `engine/src/execution/interactive_engine.rs` |
| Add a new builtin tool | `orchestration/src/adapters/tool_impl/`, `orchestration/src/tool/registry.rs`, `engine/src/tools/config.rs` (tier); **also update `NODE_RUNTIME_PREAMBLE`** in `engine/src/execution/node_invocation.rs` so agents get when-to-use guidance in every node's system prompt |
| Change tool/subagent execution wiring | `orchestration/src/run/execution/tool_port.rs` |
| Change shared context or workflow settings | `engine/src/graph/workflow.rs`, `orchestration/src/run/execution/`, `ui/src/panels/WorkflowSettingsPanel.tsx` |
| Change project/workflow linking | `orchestration/src/adapters/storage/project_store.rs`, `project_workflow_store.rs`, `backend/mod.rs`, `ui/src/components/sidebar/` |
| Change saved agents or callable subagents | `orchestration/src/adapters/storage/agent_store.rs`, `agent.rs`, `ui/src/forms/CallableAgentsEditor.tsx` |
| Change skill discovery or invocation UX | `orchestration/src/adapters/storage/skill_store.rs`, `ui/src/components/conversation/` |
| Change canvas look/behavior | `ui/src/canvas/`, `ui/src/styles/index.css` |
| Change inspector or editor panels | `ui/src/panels/`, `ui/src/screens/EditorScreen.tsx` |
| Change settings UX or toasts | `ui/src/screens/SettingsScreen.tsx`, `ui/src/api.ts`, `settings_store.rs` |
| Change provider config or key resolution | `orchestration/src/provider_config.rs`, `settings_store.rs` |

## Dev Entry Points

- Full desktop app: `./scripts/start.sh`
- Frontend only: `npm --prefix crates/ui run dev`
- Frontend typecheck: `npm --prefix crates/ui run typecheck`

## Runtime/Persistence Locations

| Data | Path |
| --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` |
| Settings | `{data_local}/openflow/settings.json` |
| Projects | `{data_local}/openflow/projects.json` (migrates from legacy slug) |
| Saved agents | `{data_local}/openflow/agents.json` (migrates from legacy slug) |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` |
| Provider API keys | Plaintext in `settings.json` (`ProviderProfile.api_key`) |

`AppBackend::load_all_workflows` merges app-store and project-discovered workflows (project files win on ID collision).

API key resolution order (highest to lowest): transient input panel → stored settings key (`ProviderProfile.api_key`) → provider env var fallback (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.).

## Verification Commands

Primary gate — run after changes:

```bash
./scripts/verify.sh
```

Runs all steps (`fmt`, clippy-max `clippy`, `doc`, `test`, `public-api`, `machete`, `typos`, `ui-typecheck`, `ui-test`, `deny`, `arch`); prints one-line PASS/FAIL per step and a summary with repro commands. Optional `./scripts/verify.sh --deep` adds `mutants` and `miri` (engine + orchestration UB via `./scripts/miri.sh`). Filter steps: `./scripts/verify.sh fmt clippy`. `VERIFY_FAIL_FAST=1` stops on first failure.

For single-step debug with full untruncated output, run a granular script directly: `./scripts/verify/<step>.sh` (see `scripts/verify/`).

For execution changes, also run:

```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```

See `docs/contributing/testing-workflows.md` for layered test commands.

## Standards Docs

- `docs/README.md`
- `docs/contributing/README.md`
- `docs/contributing/coding-patterns.md`
- `docs/contributing/testing-workflows.md`
- `docs/architecture/README.md`
- `docs/architecture/contract.md`
- `docs/architecture/threading-concurrency.md`
- `docs/glossary.md`

## Cursor Cloud specific instructions

OpenFlow is a **Tauri desktop GUI app**. Standard commands are in the README and `scripts/start.sh`; lint/test/build use `./scripts/verify.sh` (see [Verification Commands](#verification-commands)). Notes below are only the non-obvious caveats for this VM.

- **Run the GUI:** export `DISPLAY=:1` (a headless X server runs there) before `./scripts/start.sh`. The Tauri dev command also auto-starts the Vite dev server on `http://localhost:1420`.
- **Benign noise:** `libEGL warning: DRI3 error ...` lines at launch are expected — the VM has no GPU, so it falls back to software rendering. Not an error.
- **No API key = editor only:** without an LLM key the header shows "API key missing" and the **Run** button cannot execute a workflow, but the visual editor (create workflow, add/rename/configure nodes, save) works fully. For real runs, set `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` (env fallback) or a key in Settings; resolution order is in [Runtime/Persistence Locations](#runtimepersistence-locations).
- **First Rust build is slow** (large AWS SDK + reqwest tree, ~25 min cold). Subsequent builds are incremental. `cargo`-based `verify.sh` steps reuse `./target` only with `VERIFY_SHARE_TARGET=1` (faster solo runs); otherwise each run gets its own `target/verify-<pid>`.
- **Verify-gate tooling** (`cargo-deny`, `cargo-machete`, `typos`) and Tauri Linux system libs (`libwebkit2gtk-4.1-dev`, `libxdo-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, ...) are preinstalled in the VM snapshot. If a `verify.sh` step reports one missing, reinstall it (cargo tool via `cargo install`, system lib via `apt-get`).
