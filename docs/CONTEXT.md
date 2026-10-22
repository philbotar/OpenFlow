# Maintainer context

`AppBackend` connects application services to stores, providers, and run hosts. The owners below divide that work. The [glossary](glossary.md) defines their vocabulary, the [folder map](FOLDER_STRUCTURE.md) locates their code, and the [architecture contract](architecture/contract.md) defines dependency rules.

## Runtime owners

| Term | Responsibility |
| --- | --- |
| `AppBackend` | Desktop-facing façade and composition root. Wires services, stores, provider configuration, and run hosting. |
| `WorkflowCatalog` | Workflow CRUD, project assignment, and app/project catalog merge. Project files win on ID collision. |
| `RunRegistry` | Independent top-level sessions keyed by run ID. Routes controls to a selected run and shares execution budgets. |
| `RunCoordinator` | One run's lifecycle, action channel, checkpoint, and state projection. |
| `InteractiveEngine` | Workflow execution semantics. Calls `AiPort` and `ToolPort`, then yields interaction or terminal outcomes to its host. |
| `CallableAgent` | Saved agent definition snapshotted for a run; orchestration re-exports it as `AgentDefinition`. |
| `RunTelemetry` | Engine events projected into `WorkflowRunState` for the UI. |
| Provider factory | `providers::create_provider()` returns `Box<dyn AiPort>`; orchestration does not depend on concrete provider clients. |

[End-to-end runtime](architecture/end-to-end-runtime.md) traces these owners through a request. [Run persistence](architecture/run-persistence.md) distinguishes active state, saved replay, and durable resume.

## Enforced boundaries

`./scripts/check-architecture.sh` checks the inward workspace dependency graph, forbidden imports, provider symbol allowlists, engine construction locality, domain/store boundaries, and the UI Tauri seam.

The [machine-readable rules](../crates/workspace-checks/arch-check-rules.toml) define exact scope and exceptions. In particular, engine construction is restricted to `crates/orchestration/src/run/execution/`. Tauri imports are allowed in `api.ts`, `port.ts`, and designated test files; UI callers use `api.ts` wrappers.

## Tool policy

Approval modes are `read_only`, `write` (default), `always_ask`, and `yolo`. The UI labels them **Read only**, **Read auto-approve, write prompt**, **Always ask**, and **Auto-approve all**.

`yolo` auto-approves tools allowed by the run policy, including destructive shell commands. It does not bypass Plan → Execute restrictions or the explicit plan-seal approval. The [tools reference](reference/tools.md) lists tiers, availability, and exceptions.

`NodeToolConfig` includes approval mode and structured/free-text input flags. Saved agents carry their own approval mode; ad-hoc subagents inherit their parent's policy. Use the current schema and compatibility tests when changing persisted fields.

## Storage and diagnostics

Concrete storage adapters live under `crates/orchestration/src/adapters/storage/`. Skill discovery reads external `SKILL.md` files; it does not persist a copied skill library. The [operational reference](reference/README.md#runtime-and-persistence-paths) lists data locations.

Optional debug output lives in a process-specific `openflow-debug-{pid}.jsonl` file in the system temporary directory, written by `diagnostics.rs`.
