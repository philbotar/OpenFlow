---
description: 
alwaysApply: true
---

# AGENTS.md

Single-file orientation for contributors and coding agents.

## 30-Second Intake

1. This is a Rust workspace with five sections: `domain`, `providers`, `orchestration`, `desktop`, `ui`.
2. Core rule: keep domain logic in `domain`; keep API transport/auth quirks in `providers`; keep runtime/state/storage in `orchestration`; keep Tauri adapter code in `desktop`; keep frontend code in `ui`.
3. Start docs at `docs/README.md` — see [Documentation](#documentation) for the full tree.
4. Coding patterns: `docs/contributing/coding-patterns.md`.
5. Workflow verification: `docs/contributing/testing-workflows.md`.
6. Domain vocabulary: `UBIQUITOUS_LANGUAGE.md`.

## Standard Module Layout

Use this structure consistently in each section crate so seam changes stay mechanical:

- `src/ports/inbound.*`
- `src/ports/outbound.*`
- `src/adapters/inbound.*`
- `src/adapters/outbound.*`

Ports are contracts owned by the section. Adapters are concrete implementations and transport wiring.

## Documentation

```text
docs/
├── README.md
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
| `UBIQUITOUS_LANGUAGE.md` | Domain terms and naming |

## Repo Map

### Workspace and domain

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `Cargo.toml` | Workspace members and shared dependencies | Adding crates or shared dep versions |
| `crates/domain/src/model.rs` | Workflow model, `WorkflowSettings`, node config | Changing schema or defaults |
| `crates/domain/src/validation.rs` | DAG validation + execution layers | Changing graph rules or scheduling |
| `crates/domain/src/runner.rs` | Non-interactive workflow execution | Changing batch run semantics |
| `crates/domain/src/interactive.rs` | Interactive engine poll loop + pauses | Changing pause/resume behavior |
| `crates/domain/src/template.rs` | Node template defaults and locked fields | Changing template definitions |
| `crates/domain/src/template_store.rs` | Template persistence (`openflow/templates.json`) | Changing template file format |
| `crates/domain/src/ports/outbound.rs` | `AiPort`, `AgentRequest`, turn outcomes | Changing LLM invocation contract |
| `crates/domain/src/ports/inbound.rs` | Human input and tool approval ports | Adding engine input contracts |
| `crates/domain/src/adapters/outbound.rs` | `ScriptedAiAdapter` for tests | Adding domain test doubles |

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
| `crates/orchestration/src/backend.rs` | `AppBackend`, bootstrap, IPC-facing ops | Changing app-level commands or load/save |
| `crates/orchestration/src/execution.rs` | Run lifecycle, shared context, callable agents, cwd | Changing execution semantics |
| `crates/orchestration/src/state.rs` | Run/edit state, trace, chat logs | Changing run state or editor mutations |
| `crates/orchestration/src/storage.rs` | App workflows (`workflows.json`) | Changing app workflow persistence |
| `crates/orchestration/src/flow_store.rs` | Project workflows (`.flow/workflows/`) | Changing repo workflow file layout |
| `crates/orchestration/src/project_store.rs` | Projects (`openflow/projects.json`) | Changing project bindings |
| `crates/orchestration/src/agent_store.rs` | Saved agents (`openflow/agents.json`) | Changing agent definitions storage |
| `crates/orchestration/src/skill_store.rs` | Skill discovery (read-only) | Changing skill search roots |
| `crates/orchestration/src/settings_store.rs` | App settings (`settings.json`) | Changing settings schema |
| `crates/orchestration/src/provider_config.rs` | Provider readiness and key resolution | Changing key precedence or env fallback |
| `crates/orchestration/src/credential_store.rs` | OS keychain integration | Changing credential storage |
| `crates/orchestration/src/tools/` | Tool registry, approval, runner | Changing tool catalog or execution |

### Desktop and UI

| Path | Purpose | Change Here When... |
| --- | --- | --- |
| `crates/desktop/src/lib.rs` | Tauri commands/events and app bootstrap | Changing frontend/backend IPC |
| `crates/desktop/src/ports/` | Desktop seam declarations | Adding desktop contracts |
| `crates/desktop/src/adapters/` | Tauri transport adapters | Wiring invoke/event to orchestration |
| `crates/ui/src/App.tsx` | Thin shell: provider, sidebar, header, router | Changing top-level layout |
| `crates/ui/src/context/` | `AppProvider`, `AppContext` | Changing app state or run listeners |
| `crates/ui/src/screens/` | `EditorScreen`, `AgentsScreen`, `SettingsScreen` | Changing full-page routes |
| `crates/ui/src/components/` | Header, sidebar, conversation UI | Changing shared chrome |
| `crates/ui/src/panels/` | Inspector, workflow settings, dock | Changing editor side panels |
| `crates/ui/src/canvas/` | Workflow graph rendering | Changing canvas look/behavior |
| `crates/ui/src/forms/` | Node/agent configuration editors | Changing inspector forms |
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers | Changing RPC names or payloads |
| `crates/ui/src/lib/types.ts` | Frontend DTO mirror types | Changing command payload shapes |
| `crates/ui/src/styles/index.css` | Global styles and layout tokens | Changing spacing, inspector, dock CSS |

### Examples

| Path | Purpose |
| --- | --- |
| `examples/*.workflow.json` | Demo and smoke workflows |

## Common Change Paths

| Goal | Primary Files |
| --- | --- |
| Add a workflow rule or validation | `domain/src/validation.rs`, tests in same file |
| Add a new provider adapter | Implement `AiPort` in `providers/`, wire via `create_provider` |
| Add or change a seam contract | `*/src/ports/inbound.*`, `*/src/ports/outbound.*` in owning section |
| Add or change a concrete integration | `*/src/adapters/inbound.*`, `*/src/adapters/outbound.*` in owning section |
| Change run execution semantics | `orchestration/src/execution.rs`, `domain/src/interactive.rs` |
| Change shared context or workflow settings | `domain/src/model.rs`, `orchestration/src/execution.rs`, `ui/src/panels/WorkflowSettingsPanel.tsx` |
| Change project/workflow linking | `orchestration/src/project_store.rs`, `flow_store.rs`, `backend.rs`, `ui/src/components/sidebar/` |
| Change saved agents or callable subagents | `orchestration/src/agent_store.rs`, `execution.rs`, `ui/src/forms/CallableAgentsEditor.tsx` |
| Change skill discovery or invocation UX | `orchestration/src/skill_store.rs`, `ui/src/components/conversation/` |
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
| Provider API keys | OS credential store via key refs in settings |

`AppBackend::load_all_workflows` merges app-store and project-discovered workflows (project files win on ID collision).

API key resolution order (highest to lowest): transient input panel → OS credential store → provider env var fallback (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.).

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
- `UBIQUITOUS_LANGUAGE.md`
