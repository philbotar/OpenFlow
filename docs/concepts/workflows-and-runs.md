# Workflows and runs

## Workflow

A directed graph of nodes and edges plus workflow-level settings. OpenFlow validates the graph before executing it. Settings supply shared context for node and subagent system prompts, a shared provider, reasoning configuration, retry policy, and optional Plan → Execute mode.

Callable-agent visibility is set per node. The execution directory comes from the selected project or an isolated app-managed workspace; it isn't a workflow-level cwd setting.

## Node

One executable step in the graph. Agent is the only implemented node kind. Pausing for a question or tool approval is an interaction state, not a separate node kind. Root nodes receive `entrypoint.text`; downstream nodes receive upstream outputs in dependency order.

## Edge

Connects one node's output to another node's input. One output can feed several downstream nodes, and a node can combine several upstream results.

## Parallel layers

OpenFlow schedules nodes in dependency layers. Nodes in one layer have no unresolved dependencies on each other, so they can run at the same time. Downstream nodes start only after their dependencies finish and outputs are available. Same-layer siblings don't share data unless you add explicit edges through a shared upstream node or a later join.

## Run

One execution of a workflow, with its own session, trace, conversations, pending approvals, and saved history.

Durable run statuses are `running`, `paused`, `stopped`, `completed`, and `failed`. Per-node execution statuses are separate; a failed node can await retry while its run stays active.

The editor **History** dock lists the open workflow's durable runs. Replay is read-only; resume is offered for paused, stopped, or failed runs when checkpoints allow.

After a successful completion, OpenFlow may attach **post-run review** suggestions (prompt, tool, workflow, model, or coordination) to the run report. Review is advisory and doesn't change run success.

## Plan → Execute

An optional workflow setting (workflow settings → **Plan → Execute**) that adds a planning phase before normal execution:

1. **Planning**: a designated **evidence source** node runs with restricted tools (read-tier tools and controlled `docs/**/*.md` writes). It produces a structured change-evidence packet and seals the plan artifact through an explicit approval step.
2. **Execution**: once the packet is frozen, downstream nodes receive `input.change_evidence_packet`.

Without this setting, standard tool rules apply. [How OpenFlow works](how-openflow-works.md#plan--execute-mode) explains the approval flow; [Plan → Execute internals](../architecture/plan-execute.md) defines the evidence packet and artifact rules.

## Tool call

An action an agent requests, such as reading a file or running a command. Tool calls can be approved, denied, executed, and routed back into the model loop as tool results. Give a node the tools its task requires; the [tools reference](../reference/tools.md) lists arguments, limits, and approval tiers.

## Callable agent

A saved agent definition exposed to another agent as a subagent. OpenFlow resolves callable-agent snapshots for each run, so execution keeps a stable definition even if saved agents change later. [Callable agents](../architecture/callable-agents.md) describes how snapshots are created and used.

## Provider profile

A provider profile says which model backend to use and how to authenticate it. The active profile supplies the default provider; workflow and node settings can override it. [Provider key resolution](../reference/providers.md#provider-key-resolution) defines credential precedence.
