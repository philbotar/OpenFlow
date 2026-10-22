# Folder structure

OpenFlow separates workflow execution, provider transport, application services, and presentation into workspace packages. The [architecture contract](architecture/contract.md) defines their dependencies; the [orchestration layout](architecture/orchestration-layout.md) locates individual services and adapters.

## Workspace

OpenFlow has five Rust workspace members and one TypeScript frontend package:

| Path | Responsibility |
| --- | --- |
| [crates/engine](../crates/engine/) | Workflow models, validation, execution semantics, prompts, ports, and tool policy. |
| [crates/providers](../crates/providers/) | Model transport, auth, streaming, wire mapping, and provider specifications. |
| [crates/orchestration](../crates/orchestration/) | Application services, run hosting, persistence, tools, and provider construction. |
| [crates/desktop](../crates/desktop/) | Tauri commands, event bridge, and app bootstrap. |
| [crates/workspace-checks](../crates/workspace-checks/) | Architecture and source-boundary verification. |
| [crates/ui](../crates/ui/) | SolidJS/TypeScript frontend with an isolated React Flow canvas. |
| [scripts](../scripts/) | Development, build, verification, and release entry points. |
| [docs](README.md) | Product guides, reference, architecture, and contributor documentation. |
| [examples](../examples/) | Example workflows and their instructions. |

Read the nearest `AGENTS.md` before changing an area. UI and desktop have their own boundaries; both participate in the architecture contract.

## Application services and adapters

Orchestration groups services by app concern. Small services can remain single files, such as `agent.rs` and `chat.rs`. Larger concerns use folders, such as `workflow/`, `project/`, `settings/`, `run/`, and `tool/`. Group implementation details when they share a responsibility; do not add an `application/` layer just to hold one service.

Concrete I/O lives under `crates/orchestration/src/adapters/`:

| Folder | Owns |
| --- | --- |
| `storage/` | Workflow, chat, project, agent, settings, attachment, and checkpoint storage; skill discovery. |
| `tool_impl/` | File, search, shell, edit, and other built-in tool implementations. |
| `mcp/` | MCP connections, discovery, package installation, and secrets. |
| `infrastructure/` | Git, LSP, and shared subprocess support. |

`backend/` constructs and connects services and adapters. Run hosting and engine construction belong under `run/execution/`; the run coordinator manages lifecycle and interactions.

The providers crate is itself an adapter crate. Its `rig_adapter/` and `mapping/` folders sit directly under `crates/providers/src/`, not inside an `adapters/ai_provider/` tree.

## Ports and public boundaries

Add a trait only when a consumer depends on it. Engine exports `AiPort` and `ToolPort`. Orchestration store/catalog traits live beside their consumers, usually in `ports.rs`; `AgentStore` lives in `agent.rs`.

Adapters implement those interfaces. Entity services use their ports; `backend/` supplies concrete stores. Do not introduce a trait for every helper or expose an internal module merely to mirror its filesystem path.

Orchestration obtains provider clients through `create_provider()` and `Box<dyn AiPort>`. Desktop uses orchestration's public API and re-exported types. UI Tauri calls go through `crates/ui/src/api.ts`; DTO types live in `crates/ui/src/lib/types/index.ts`.

## UI layout

| Path | Purpose |
| --- | --- |
| `crates/ui/src/context/` | `AppProvider` / `AppContext` - app state, run listeners, navigation |
| `crates/ui/src/screens/` | Full-page routes: `EditorScreen`, `AgentsScreen`, `SettingsScreen` |
| `crates/ui/src/components/` | Shared chrome: `AppHeader`, sidebar primitives, conversation UI |
| `crates/ui/src/panels/` | Editor overlays: `InspectorPanel`, `WorkflowSettingsPanel`, `DockPanel` |
| `crates/ui/src/canvas/` | Workflow graph rendering |
| `crates/ui/src/forms/` | Node/agent configuration editors |
| `crates/ui/src/lib/types/index.ts` | Frontend DTO mirror types |
| `crates/ui/src/api.ts` | Typed Tauri invoke/event wrappers |

`App.tsx` is a thin shell: provider, toaster, sidebar, header, and `ScreenRouter`.

## Verify structural changes

Run `./scripts/check-architecture.sh` after changing dependencies or boundaries. The [rules file](../crates/workspace-checks/arch-check-rules.toml) records exact import allowlists and engine construction scope. Intentional engine public API changes also need `./scripts/check-engine-public-api.sh`.

[Development lanes](contributing/development-lanes.md) maps behavior changes to checks; [coding patterns](contributing/coding-patterns.md) identifies the owning files.
