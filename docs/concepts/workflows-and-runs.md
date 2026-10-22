# Workflows and runs

## Workflow

A workflow is a directed graph of nodes and edges plus workflow-level settings. OpenFlow validates the graph before executing it.

Workflow settings supply shared context for node and subagent system prompts, a shared provider, reasoning configuration, and retry policy. They also control optional Plan → Execute mode.

Callable-agent visibility belongs to each node. OpenFlow resolves the execution directory from the selected project or an isolated app-managed workspace; it is not a workflow-level cwd setting.

## Node

A node is one executable step in the graph. Agent is the only implemented node kind. An agent can pause for a question or tool approval; these are interaction states, not separate node kinds.

Root nodes receive `entrypoint.text`. Downstream nodes receive upstream outputs in dependency order.

## Edge

An edge connects one node output to another node input. One output can feed several downstream nodes, and a node can combine results from several upstream nodes.

## Parallel layers

OpenFlow schedules nodes in dependency layers. Nodes in the same layer have no unresolved upstream dependencies among themselves, so they can run at the same time. Downstream nodes start only after their dependencies finish and their outputs are available. You do not pass data between same-layer siblings unless you add explicit edges through a shared upstream node or a later join.

## Run

A run is one execution of a workflow. Each run has its own session, trace, conversations, pending approvals, and saved history.

Durable run statuses are `running`, `paused`, `stopped`, `completed`, and `failed`. Per-node execution statuses are separate; a failed node can await retry while its run remains active.

The editor **History** dock lists durable runs for the open workflow. Replay opens a read-only view; resume is offered for paused, stopped, or failed runs when checkpoints allow it.

After a successful completion, OpenFlow may attach **post-run review** suggestions to the run report (prompt, tool, workflow, model, or coordination categories). Review is advisory and does not change run success.

## Plan → Execute

Optional workflow setting: before normal execution, a designated **evidence source** node runs in a **Planning** phase with restricted tools (read-tier tools and controlled `docs/**/*.md` writes). That node produces a structured change-evidence packet and seals the plan artifact through an explicit approval step. After the packet is frozen, the run enters **Execution** and downstream nodes receive `input.change_evidence_packet`. Workflows without this setting use standard tool rules. UI: workflow settings → **Plan → Execute**. [How OpenFlow works](how-openflow-works.md#plan--execute-mode) explains the approval flow; [Plan → Execute internals](../architecture/plan-execute.md) defines the evidence packet and artifact rules.

## Tool call

A tool call is an action requested by an agent, such as reading a file or running a command. Tool calls can be approved, denied, executed, and routed back into the model loop as tool results.

Give a node the tools its task requires. The [tools reference](../reference/tools.md) lists arguments, limits, and approval tiers.

## Callable agent

A callable agent is a saved agent definition exposed to another agent as a subagent. OpenFlow resolves callable-agent snapshots for a run so execution has a stable definition even if saved agents change later.

[Callable agents](../architecture/callable-agents.md) describes how those snapshots are created and used.

## Provider profile

A provider profile describes which model backend the app should use and how to authenticate it. The active profile supplies the default provider; workflow and node settings can override it.

[Provider key resolution](../reference/providers.md#provider-key-resolution) defines credential precedence.
