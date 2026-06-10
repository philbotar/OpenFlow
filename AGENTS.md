---
description: 
alwaysApply: true
---

# AGENTS.md

Single-file orientation for contributors and coding agents.

## 30-Second Intake

1. This is a Rust workspace with five sections: `engine`, `providers`, `orchestration`, `desktop`, `ui`.
2. Core rule: keep engine logic in `engine`; keep API transport/auth quirks in `providers`; keep runtime/state/storage in `orchestration`; keep Tauri adapter code in `desktop`; keep frontend code in `ui`.
   - **engine** — valid workflow + run behavior
   - **orchestration** — store, load, wire, host runs
   - **providers** — LLM transport
   - **ui** / **desktop** — user interaction
3. Start docs at `docs/README.md` — see [Documentation](#documentation) for the full tree.
4. Coding patterns: `docs/contributing/coding-patterns.md`.
5. Workflow verification: `docs/contributing/testing-workflows.md`.
6. Engine vocabulary: `docs/glossary.md`.

## Boundary Seams

Add a port/trait only when a consumer is typed on that interface. Otherwise call the concrete type directly.

| Seam | Location |
| --- | --- |
| LLM invocation (`AiPort`, `AgentRequest`) | `crates/engine/src/ports/outbound.rs` |
| Tool and subagent execution (`ToolPort`) | `crates/engine/src/ports/outbound.rs` → `crates/orchestration/src/run/application/execution/tool_port.rs` |
| Human input / tool approval | `crates/engine/src/ports/inbound.rs` |
| Provider client (`AiClient: AiPort`) | `crates/providers/src/client.rs` |
| UI → desktop IPC | `crates/ui/src/port.ts` (`UiDesktopOutboundPort`) |

## Documentation

```text
docs/
├── README.md
├── glossary.md
├── contributing/
│   ├── README.md
│   ├── coding-patterns.md
│   └── testing-workflows.md
└── architecture/
    ├── README.md
    ├── contract.md
    ├── threading-concurrency.md
    └── diagrams/
```

| Doc | Use when |
| --- | --- |
| `docs/README.md` | First read; filesystem index |
| `docs/contributing/coding-patterns.md` | Ownership, runtime semantics, conventions |
| `docs/contributing/testing-workflows.md` | Acceptance tests, live-AI smoke |
| `docs/architecture/contract.md` | Layer boundaries and dependency rules |
| `docs/architecture/threading-concurrency.md` | Runtimes, async I/O, parallelism |
| `docs/glossary.md` | Engine terms and naming |

## Repo Map

### Workspace and engine

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `Cargo.toml` | Workspace members and shared dependencies | Adding crates or shared dep versions |
| `crates/engine/src/graph/` | Workflow model, `WorkflowSettings`, node config, `CallableAgent`, DAG validation | Changing schema, graph rules, or scheduling |
| `crates/engine/src/execution/workflow_runner.rs` | Non-interactive `WorkflowRunner` | Changing batch run semantics |
| `crates/engine/src/execution/interactive_engine.rs` | Interactive engine `poll()` + `run()` loop | Changing pause/resume or self-driving run behavior |
| `crates/engine/src/execution/subagent_runtime.rs` | Subagent declare/call builtins + turn machine | Changing subagent invocation semantics |
| `crates/engine/src/execution/telemetry.rs` | `RunTelemetry` interactive event enum | Changing run event vocabulary |
| `crates/engine/src/execution/node_invocation.rs` | Shared `AgentRequest` assembly | Changing upstream input or prompt wiring |
| `crates/engine/src/template/` | `Template`, locked fields, builtins | Changing template definitions |
| `crates/orchestration/src/template_store.rs` | Template persistence (`openflow/templates.json`) | Changing template file format |
| `crates/engine/src/ports/outbound.rs` | `AiPort`, `AgentRequest`, turn outcomes | Changing LLM invocation contract |
| `crates/engine/src/ports/inbound.rs` | Human input and tool approval ports | Adding engine input contracts |

### Providers

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `crates/providers/src/client.rs` | `AiClient` implementing `AiPort` | Changing provider client wiring |
| `crates/providers/src/mapping.rs` | Transcript/tool-arg mapping, `jsonrepair-rs` | Changing wire payload shape |
| `crates/providers/src/openai_compat.rs` | OpenAI-compatible transport | Adding/changing OpenAI-compat APIs |
| `crates/providers/src/anthropic.rs` | Anthropic transport | Adding/changing Anthropic APIs |
| `crates/providers/src/lib.rs` | `create_provider` factory | Adding a new provider adapter |

### Orchestration

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `crates/orchestration/src/backend/mod.rs` | Thin `AppBackend` orchestrator; delegates to catalog modules | Adding/remapping desktop IPC commands |
| `crates/orchestration/src/workflow/catalog.rs` | Workflow CRUD, app/project merge, assign/unassign | Changing workflow persistence rules |
| `crates/orchestration/src/agent/library.rs` | Saved agent CRUD, `create_agent_node` | Changing callable agent library behavior |
| `crates/orchestration/src/project/registry.rs` | Project load/save/create | Changing project registration |
| `crates/orchestration/src/settings/facade.rs` | Settings, keys, skills, validation summaries | Changing settings or provider readiness UX |
| `crates/orchestration/src/run/coordinator.rs` | Active run session, start/submit/apply events | Changing run lifecycle coordination |
| `crates/orchestration/src/run/execution/` | `drive.rs`, `tool_port.rs`, event projection, cwd | Changing execution host semantics |
| `crates/orchestration/src/run/state/` | Run/edit state, trace, chat logs | Changing run state or editor mutations |
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
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers | Changing RPC names or payloads |
| `crates/ui/src/port.ts` | `UiDesktopOutboundPort` + factory for swappable desktop backend | Changing how UI talks to Tauri |
| `crates/ui/src/lib/types.ts` | Frontend DTO mirror types | Changing command payload shapes |
| `crates/ui/src/styles/index.css` | Global styles and layout tokens | Changing spacing, inspector, dock CSS |

### Examples

| Path | Purpose |
| --- | --- |
| `examples/*.workflow.json` | Demo and smoke workflows |

## Common Change Paths

| Goal | Primary Files |
| --- | --- |
| Add a workflow rule or validation | `engine/src/graph/validation.rs`, tests in same file |
| Add a new provider adapter | Implement `AiPort` in `providers/`, wire via `create_provider` |
| Add or change `AiPort`, `ToolPort`, or engine input contracts | `engine/src/ports/` |
| Add or change UI desktop seam | `ui/src/port.ts` |
| Change run execution semantics | `orchestration/src/run/execution/drive.rs`, `engine/src/execution/interactive_engine.rs` |
| Change tool/subagent execution wiring | `orchestration/src/run/execution/tool_port.rs` |
| Change shared context or workflow settings | `engine/src/graph/workflow.rs`, `orchestration/src/run/execution/`, `ui/src/panels/WorkflowSettingsPanel.tsx` |
| Change project/workflow linking | `orchestration/src/adapters/storage/project_store.rs`, `project_workflow_store.rs`, `backend/mod.rs`, `ui/src/components/sidebar/` |
| Change saved agents or callable subagents | `orchestration/src/adapters/storage/agent_store.rs`, `agent/library.rs`, `ui/src/forms/CallableAgentsEditor.tsx` |
| Change skill discovery or invocation UX | `orchestration/src/adapters/storage/skill_store.rs`, `ui/src/components/conversation/` |
| Change canvas look/behavior | `ui/src/canvas/`, `ui/src/styles/index.css` |
| Change inspector or editor panels | `ui/src/panels/`, `ui/src/screens/EditorScreen.tsx` |
| Change settings UX or toasts | `ui/src/screens/SettingsScreen.tsx`, `ui/src/api.ts`, `settings_store.rs` |
| Change provider config or key resolution | `orchestration/src/provider_config.rs`, `settings_store.rs` |

## Dev Entry Points

- Full desktop app: `npm --prefix crates/desktop run start -- dev`
- Frontend only: `npm --prefix crates/ui run dev`
- Frontend typecheck: `npm --prefix crates/ui run typecheck`

## Runtime/Persistence Locations

| Data | Path |
| --- | --- |
| App workflows | `{data_local}/step-through-agentic-workflow/workflows.json` |
| Settings | `{data_local}/step-through-agentic-workflow/settings.json` |
| Projects | `{data_local}/openflow/projects.json` (migrates from legacy slug) |
| Saved agents | `{data_local}/openflow/agents.json` (migrates from legacy slug) |
| Node templates | `{data_local}/openflow/templates.json` (migrates from legacy slug) |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` |
| Provider API keys | Plaintext in `settings.json` (`ProviderProfile.api_key`) |

`AppBackend::load_all_workflows` merges app-store and project-discovered workflows (project files win on ID collision).

API key resolution order (highest to lowest): transient input panel → stored settings key (`ProviderProfile.api_key`) → provider env var fallback (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.).

## Verification Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

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
