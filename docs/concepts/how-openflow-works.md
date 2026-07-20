# How OpenFlow Works

OpenFlow is a desktop workflow runner for agentic model workflows. A user builds a workflow in the UI, the desktop adapter sends commands to orchestration, orchestration hosts the active run, the engine decides legal execution behavior, and providers perform model calls.

## Runtime Path

```text
UI
  -> Desktop IPC
    -> Orchestration
      -> Engine
      -> Providers
      -> Tools and storage adapters
```

## What Each Layer Owns

| Layer | Owns | Does not own |
| --- | --- | --- |
| UI | Presentation, editor state, screens, panels, canvas | Engine rules, provider transport |
| Desktop | Tauri command and event bridge | Workflow semantics |
| Orchestration | App state, workflow catalog, run sessions, storage, tools, provider wiring | Legal engine transitions |
| Engine | Workflow model, validation, execution state machine, ports | Filesystem, Tauri, HTTP transport |
| Providers | Model API transport and wire mapping | App state or workflow catalog rules |

The architecture contract is maintained in [`../architecture/contract.md`](../architecture/contract.md).

## What Happens During a Run

1. The UI sends a start, continue, or resume command through the desktop IPC seam.
2. Orchestration resolves settings, provider readiness, workflow storage, run root, and execution cwd.
3. Orchestration creates the provider adapter through `providers::create_provider`.
4. The engine validates and advances the workflow.
5. Agent nodes assemble an `AgentRequest`, including system prompt, upstream input, shared context, tools, and callable-agent snapshots.
6. The provider returns model output, tool calls, or terminal output.
7. Tool calls go through `ToolPortImpl` in orchestration and can pause for approval.
8. Orchestration projects engine events into run state, trace entries, chat logs, and desktop events.
9. Durable run data is written so paused or completed runs can be listed and resumed when supported.

## Plan → Execute Mode

Plan → Execute is an optional `WorkflowSettings.planMode` gate. Its configured review node must
allow follow-up questions. Before that node produces a schema-valid change evidence packet, the
run is in **Planning**: agents can use read-tier tools and can seal one run-owned Markdown plan
with `openflow_write_plan_artifact`. Repository writes, shell execution, MCP tools, and subagent
calls are denied even if a model attempts an unadvertised call.

The host gives the Markdown artifact an opaque UUID path under the run artifact root. The tool
returns `artifact:<uuid>`, SHA-256, and size; it never accepts a repository path. Once the review
node completes, the engine freezes its exact structured output with a canonical SHA-256 and moves
to **Execution**. The frozen packet is checkpointed and added as
`input.change_evidence_packet` to every later agent request. It is data, not a system instruction.

The packet should stay compact: scope, exclusions, criteria, decisions, implementation slices,
plan artifact reference and hash, assumptions, open questions, approval summary, and optional
standards or ADR references. Downstream agents treat it as authoritative and read the artifact
only when they need plan detail. Workflows without Plan → Execute retain normal tool behavior.

## Why Ports Exist

Ports are added only when a consumer is typed against the interface. OpenFlow currently uses explicit seams for:

- `AiPort` and `AgentRequest` for model invocation.
- `ToolPort` for tool and subagent execution.
- Human input and tool approval ports for pauses.
- `api.ts` wrappers for UI-to-desktop calls.

If there is no typed consumer, prefer calling the concrete type directly instead of adding a trait.

## Where to Go Deeper

- [`workflows-and-runs.md`](workflows-and-runs.md) - product vocabulary.
- [`../architecture/technical-overview.md`](../architecture/technical-overview.md) - deeper runtime overview.
- [`../architecture/threading-concurrency.md`](../architecture/threading-concurrency.md) - runtimes, async I/O, and blocking work.
- [`../architecture/run-persistence.md`](../architecture/run-persistence.md) - durable run records, replay, and resume.
