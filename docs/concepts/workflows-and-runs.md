# Workflows and Runs

This page defines the product concepts used in the editor, runtime, tests, and architecture docs.

## Workflow

A workflow is a directed graph of nodes and edges plus workflow-level settings. The engine owns validation and execution semantics for the graph.

Common workflow settings include:

- Shared context appended to node and subagent system prompts.
- Callable-agent visibility.
- Execution cwd used by runtime tools.
- Provider reasoning configuration where supported.

## Node

A node is one executable step in the graph. Agent nodes call a model provider. Manual or interaction nodes can pause until the user provides input.

Root nodes receive `entrypoint.text`. Downstream nodes receive upstream outputs in dependency order.

## Edge

An edge connects one node output to another node input. Branch and join behavior is validated by the engine and covered by orchestration workflow acceptance tests.

## Parallel layers

The engine schedules nodes in dependency layers. Nodes in the same layer have no unresolved upstream dependencies among themselves, so they can run at the same time. Downstream nodes start only after their dependencies finish and their outputs are available. You do not pass data between same-layer siblings unless you add explicit edges through a shared upstream node or a later join.

## Run

A run is one execution of a workflow. Orchestration owns active run sessions, run trace projection, chat logs, approval queues, durable run records, and resume coordination.

Run states include queued, running, paused, completed, and failed. Tests assert that trace entries expose these state transitions.

The editor **History** dock lists durable runs for the open workflow. Replay opens a read-only view; resume is offered for paused, stopped, or failed runs when checkpoints allow it.

After a successful completion, OpenFlow may attach **post-run review** suggestions to the run report (prompt, tool, workflow, model, or coordination categories). Review is advisory and does not change run success.

## Plan → Execute

Optional workflow setting: before normal execution, a designated **evidence source** node runs in a **Planning** phase with restricted tools (read-tier tools and controlled `docs/**/*.md` writes). That node produces a structured change-evidence packet and seals the plan artifact through an explicit approval step. After the packet is frozen, the run enters **Execution** and downstream nodes receive `input.change_evidence_packet`. Workflows without this setting use standard tool rules. UI: workflow settings → **Plan → Execute**. Behavioral detail: [`how-openflow-works.md`](how-openflow-works.md#plan--execute-mode).

## Tool Call

A tool call is a model-requested action handled by orchestration through `ToolPortImpl`. Tool calls can be approved, denied, executed, and routed back into the model loop as tool results.

Add tool access narrowly. A node should only receive tools that are relevant to its job. Per-tool reference: [`../reference/tools.md`](../reference/tools.md).

## Callable Agent

A callable agent is a saved agent definition exposed to another agent as a subagent. OpenFlow resolves callable-agent snapshots for a run so execution has a stable definition even if saved agents change later.

See [`../architecture/callable-agents.md`](../architecture/callable-agents.md).

## Provider Profile

A provider profile describes which model backend the app should use and how to authenticate it. Provider-specific transport belongs in `crates/providers`; provider readiness and key resolution are orchestrated by settings code.

See [`../reference/README.md#provider-key-resolution`](../reference/README.md#provider-key-resolution).
