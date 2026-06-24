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

## Run

A run is one execution of a workflow. Orchestration owns active run sessions, run trace projection, chat logs, approval queues, durable run records, and resume coordination.

Run states include queued, running, paused, completed, and failed. Tests assert that trace entries expose these state transitions.

## Tool Call

A tool call is a model-requested action handled by orchestration through `ToolPortImpl`. Tool calls can be approved, denied, executed, and routed back into the model loop as tool results.

Add tool access narrowly. A node should only receive tools that are relevant to its job.

## Callable Agent

A callable agent is a saved agent definition exposed to another agent as a subagent. OpenFlow resolves callable-agent snapshots for a run so execution has a stable definition even if saved agents change later.

See [`../architecture/callable-agents.md`](../architecture/callable-agents.md).

## Provider Profile

A provider profile describes which model backend the app should use and how to authenticate it. Provider-specific transport belongs in `crates/providers`; provider readiness and key resolution are orchestrated by settings code.

See [`../reference/README.md#provider-key-resolution`](../reference/README.md#provider-key-resolution).
