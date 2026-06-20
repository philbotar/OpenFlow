# Glossary

Domain vocabulary for Step-through-agentic-workflow. Use these terms in code, docs, and UI copy. Avoid the listed aliases.

For where terms live in code, see [Domain modules](#domain-modules) and [`docs/sections/domain/README.md`](sections/domain/README.md).

## Graph structure

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Workflow** | A directed acyclic graph (DAG) of nodes and edges defining an AI pipeline | Graph, pipeline, flowchart |
| **Node** | A vertex in the workflow graph that runs an AI agent | Step, task, stage |
| **Edge** | A directed connection between two nodes, carrying output as input | Connection, link, arrow |
| **NodeId** | A unique identifier for a node within a workflow | |
| **EdgeId** | A unique identifier for an edge within a workflow | |
| **WorkflowId** | A unique identifier for a workflow | |
| **WorkflowSettings** | Portable per-workflow configuration: shared context, retry, schedule, provider override | Workflow config |
| **SharedContext** | Text appended to every node's system prompt for the duration of a run (`WorkflowSettings.shared_context`) | Shared node context |
| **ExecutionCwd** | Folder used as the working directory for filesystem tools during a run; chosen at run time, not stored on the workflow | Working directory, run folder |

## Node lifecycle

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **NodeKind** | The variant of a node (e.g. Agent) determining its runtime behavior | Node type, node variant |
| **AgentNodeConfig** | Configuration for an agent node: model, prompts, tools, auto-start, callable agents | Node config, agent config |
| **NodePosition** | Coordinates for rendering a node on the canvas | Position, coordinates |
| **AutoStart** | Whether a node begins when its execution layer is reached (`auto_start: true`), or pauses for human input (`false`) | Auto-execute |
| **CallableAgent** | A saved agent definition a node may invoke as a subagent during a run (`domain::graph::CallableAgent`) | Saved subagent, AgentDefinition |
| **CallableAgentSelection** | Agent IDs on `AgentNodeConfig.callable_agents`; snapshotted at run start | Allowed agents, callable agents |
| **AllowAllCallableAgents** | When true, every saved agent is snapshotted at run start instead of `callable_agents` | Allow all agents |

## Conversation

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **ChatMessage** | A single message in a node's conversation with a given role | Message, turn |
| **ChatRole** | Sender of a message: System, Thinking, User, or Assistant | Sender, author |
| **AgentTranscriptItem** | Chronological transcript entry: assistant/user message, ToolCall, or ToolResult | Transcript entry, log item |

## Tool configuration

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **NodeToolConfig** | Approval mode for a node or saved agent (`read_only`, `write`, `always_ask`, `yolo`) | Tool settings, tool setup |
| **ApprovalMode** | Node-level tool approval strategy: `read_only` (read-class tools only, auto-approved), `write` (all tools; read-class auto, write-class prompt — default), `always_ask` (prompt every call), `yolo` (never prompt) | Approval policy |
| **Tool capability class** | Static read/write grouping for builtins. Read: retrieval/search tools. Write: mutation, shell, subagent tools. Drives approval and `read_only` availability. | Tool tier |
| **ToolTier** | Serialized capability class on tool definitions: `read` or `write` | Tool level, access tier |
| **ToolConcurrency** | Whether tool calls share or exclude concurrent access: `shared` or `exclusive` | Parallelism, execution mode |
| **ToolCallStatus** | Lifecycle of a tool call: proposed, awaiting_approval, running, completed, blocked, failed, aborted | Call status |
| **ToolTruncation** | Limits and strategy for truncating tool output | Output limit, size cap |
| **PendingToolApproval** | A tool call awaiting human approval before execution | Pending tool, queued call |

## Template system

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Template** | Reusable node definition with default `AgentNodeConfig` and locked fields | Node preset, blueprint |
| **LockedField** | Field name users cannot edit when a template is applied (e.g. `output_schema`, `auto_start`) | Protected field, frozen field |
| **TemplateStore** | Persistence seam for listing and mutating templates | |
| **FileTemplateStore** | Orchestration adapter that persists templates to `openflow/templates.json` | |

## Execution model

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Execution layer** | Topologically sorted group of nodes at the same depth; nodes in a layer may run in parallel | Layer, depth level, wave |
| **RunEvent** | Compact lifecycle record in `RunReport` (queued, started, retrying, completed, failed) | Event, log entry |
| **RunTelemetry** | Rich interactive run event stream (chat, tools, subagents, pauses); `ExecutionEvent` alias in orchestration | Execution event |
| **RunEventKind** | Variant of a compact `RunEvent` | Event type |
| **RunReport** | Aggregated result after a workflow run: events and per-node outputs | Summary, result, run summary |
| **NodeRunOutput** | Structured output from one node | Node output, step result |
| **EntrypointText** | Initial text for nodes with no upstream dependencies at run start | Seed input, initial prompt |
| **Project** | Folder-scoped workspace binding workflows to a repo path | Workspace, repo binding |
| **RetryPolicy** | Workflow-level retry: max attempts and backoff (`WorkflowSettings.retry_policy`) | Retry config |
| **WorkflowSchedule** | Optional cron schedule on a workflow | Cron schedule |
| **NodeInvocation** | Shared assembly of upstream inputs and `AgentRequest` for both execution engines | Request builder |

## AI boundary

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **AiPort** | Trait between the workflow engine and an AI backend | AI adapter, backend trait |
| **AgentRequest** | Payload for one AI turn on one node | AI request, turn request |
| **AgentTurnOutcome** | Result of one turn: Completed, ToolCalls, or NeedsUserInput | Turn result, AI response |
| **AgentTurnSuccess** | Completed outcome: structured output plus raw text | Success result |
| **AgentToolCallBatch** | ToolCalls outcome: batch of tool invocations from the model | Tool call batch |
| **ToolCall** | Single tool invocation from the model | Tool request, tool use |
| **ToolResult** | Result returned after a tool executes | Tool response |
| **ToolDefinition** | Schema for a tool exposed to the model | Tool schema, tool spec |
| **AgentNeedUserInput** | Signal that the model needs human input to continue | User input requested |
| **AgentError** | Error from the AI backend for the current turn | Backend error, AI failure |

## Validation outcomes

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **WorkflowValidationError** | Why a workflow graph is invalid | Validation error |
| **EmptyWorkflow** | Workflow with zero nodes | |
| **DuplicateNodeId** | Two or more nodes share an id | |
| **MissingEndpoint** | Edge references a node that does not exist | Dangling edge |
| **SelfEdge** | Edge from a node to itself | Self-loop |
| **Cycle** | Directed cycle makes topological order impossible | Circular dependency |

## Interactive execution

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **InteractiveEngine** | Stateful poll-based engine for step-through runs (desktop app path) | Step engine, interactive runner |
| **EnginePollResult** | Next action after `poll`: CallAi, AwaitInput, AwaitToolApproval, RunTools, Completed, Failed | Poll result |
| **WorkflowRunner** | Non-interactive engine: run to completion in one call; no tools or human input | Batch runner, headless runner |
| **Pause** | Suspend execution awaiting human input or tool approval | Step, break |
| **Resume** | Continue from a paused state | Continue, step forward |

## Tool approval

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **ToolApproval** | Explicit human consent before a tool call runs | Tool consent, tool permit |
| **AwaitToolApproval** | `EnginePollResult` variant when execution waits on approval | Waiting for tool approval |

## Domain modules

Map glossary buckets to `crates/domain/src/`:

| Module | Terms |
| --- | --- |
| `graph/` | Workflow, Node, Edge, ids, settings, `CallableAgent`, `resolve_callable_agent_snapshots`, `validate_workflow`, execution layers |
| `template/` | Template, LockedField, TemplateStore trait, builtin presets |
| `execution/` | WorkflowRunner, InteractiveEngine, RunReport, RunEvent, RunTelemetry, subagent_runtime, NodeInvocation |
| `conversation/` | ChatMessage, ChatRole, AgentTranscriptItem |
| `tools/` | NodeToolConfig, ApprovalMode, ToolCall, ToolResult, policy helpers |
| `ports/` | AiPort, AgentRequest, AgentTurnOutcome, human/tool input ports |

Persistence adapters outside domain:

| Location | Terms |
| --- | --- |
| `orchestration/template_store.rs` | FileTemplateStore |
| `orchestration/project_store.rs` | Project |
| `orchestration/adapters/storage/project_workflow_store.rs` | Project workflow files (`.flow/workflows/`) |
| `orchestration/adapters/storage/app_workflow_store.rs` | App-level workflows (`workflows.json`) |

Orchestration composition modules (see `docs/sections/orchestration/README.md`):

| Module | Terms |
| --- | --- |
| `orchestration/workflow/catalog.rs` | Workflow merge/split, project assign |
| `orchestration/agent_library.rs` | CallableAgent library CRUD (`FileAgentStore` adapter) |
| `orchestration/agent_store.rs` | `AgentDefinition` type alias for persisted `CallableAgent` JSON |
| `orchestration/run_coordinator.rs` | Run session, Pause/Resume host path |
| `orchestration/settings_facade.rs` | Settings, provider readiness |

## Relationships

- A **Workflow** has **Nodes** and **Edges**.
- An **Edge** passes a source **Node**'s output to a target **Node**.
- **Execution layers** come from DAG topology; same-layer nodes have no dependency on each other.
- An **AgentNodeConfig** belongs to one **Node** and may include **NodeToolConfig**.
- A **ChatMessage** or **AgentTranscriptItem** belongs to one **Node**'s transcript.
- An **AgentRequest** goes to **AiPort** for one **Node** per turn.
- **AgentTurnOutcome** is **AgentTurnSuccess**, **AgentToolCallBatch**, or **AgentNeedUserInput**.
- A **Template** instantiates a **Node** with default config and **LockedField** constraints.
- **InteractiveEngine** yields **EnginePollResult** until **Completed** or **Failed**.
- **WorkflowRunner** is the batch path; **InteractiveEngine** is the step-through path.

## Example dialogue

> **Dev:** "If a **Node** has two incoming **Edges**, how do we determine its **Execution layer**?"
>
> **Domain expert:** "Topological order — layer is one more than the max layer of upstream nodes."

> **Dev:** "What happens when the model returns **ToolCalls**?"
>
> **Domain expert:** "**InteractiveEngine** emits **AwaitToolApproval** (or **RunTools** in yolo mode). Approved calls run; **ToolResult** feeds the same **Node** for another turn."

> **Dev:** "What's the difference between **WorkflowRunner** and **InteractiveEngine**?"
>
> **Domain expert:** "**WorkflowRunner** is headless batch: one model call per node, no tools, no pause. **InteractiveEngine** drives the desktop app: multi-turn chat, tools, approval, resume."

## Flagged ambiguities

- **Layer** — in `execution_layers`, DAG depth grouping; in CSS/UI, z-index. Here: execution layer only.
- **Event** — **RunTelemetry** (interactive UI stream) vs compact **RunEvent** in `RunReport` vs OS/Tauri events.
- **Config** — **AgentNodeConfig** (agent behavior) vs **NodeToolConfig** (tool environment). Distinct concepts.
- **Template** — canonical type in `domain::template`. Legacy `NodeTemplate` JSON migrates in **FileTemplateStore**.
- **Runner** — prefer **WorkflowRunner** (batch) or **InteractiveEngine** (step-through). Do not use "runner" alone in docs or module names.
