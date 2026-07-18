# Glossary

Engine and app vocabulary for OpenFlow. Use these terms in code, docs, and UI copy. Avoid the listed aliases.

For where terms live in code, see [Engine modules](#engine-modules), [Orchestration modules](#orchestration-modules), and [orchestration crate layout](architecture/orchestration-layout.md).

## Graph structure

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Workflow** | A directed acyclic graph (DAG) of nodes and edges defining an AI pipeline | Graph, pipeline, flowchart |
| **Node** | A vertex in the workflow graph that runs an AI agent | Step, task, stage |
| **Edge** | A directed connection between two nodes, carrying output as input | Connection, link, arrow |
| **NodeId** | A unique identifier for a node within a workflow | |
| **EdgeId** | A unique identifier for an edge within a workflow | |
| **WorkflowId** | A unique identifier for a workflow | |
| **WorkflowSettings** | Portable per-workflow configuration: shared context, retry, schedule, provider override, and optional Plan → Execute gate | Workflow config |
| **PlanMode** | Optional workflow gate that holds a run in Planning until its configured conversational review node freezes a change evidence packet | Plan-only workflow |
| **SharedContext** | Text appended to every node's system prompt for the duration of a run (`WorkflowSettings.shared_context`) | Shared node context |
| **ExecutionCwd** | Folder used as the working directory for filesystem tools during a run; chosen at run time, not stored on the workflow | Working directory, run folder |

## Node lifecycle

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **NodeKind** | The variant of a node (e.g. Agent) determining its runtime behavior | Node type, node variant |
| **AgentNodeConfig** | Configuration for an agent node: model, prompts, tools, auto-start, callable agents | Node config, agent config |
| **NodePosition** | Coordinates for rendering a node on the canvas | Position, coordinates |
| **AutoStart** | Whether a node invokes its model when its execution layer is reached (`auto_start: true`), or first waits for a human kickoff message (`false`) | Auto-execute |
| **RequestUserInput** | Whether a running node may call `openflow_request_user_input` to ask a direct follow-up question; defaults to false | AutoStart, manual start |
| **CallableAgent** | A saved agent definition a node may invoke as a subagent during a run (`engine::CallableAgent`) | Saved subagent, AgentDefinition |
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
| **ApprovalMode** | Node-level tool approval strategy: `read_only` (read-class tools only, auto-approved), `write` (all tools; read-class auto, write-class prompt - default), `always_ask` (prompt every call), `yolo` (never prompt) | Approval policy |
| **Tool capability class** | Static read/write grouping for builtins. Read: retrieval/search tools. Write: mutation, shell, subagent tools. Drives approval and `read_only` availability. | Tool tier |
| **ToolTier** | Serialized capability class on tool definitions: `read` or `write` | Tool level, access tier |
| **ToolAccessPolicy** | Run-phase capability rule. Planning permits read-tier tools plus the host-owned plan-artifact writer; Execution restores the node's normal catalog. | Approval mode |
| **ToolConcurrency** | Whether tool calls share or exclude concurrent access: `shared` or `exclusive` | Parallelism, execution mode |
| **ToolCallStatus** | Lifecycle of a tool call: proposed, awaiting_approval, running, completed, blocked, failed, aborted | Call status |
| **ToolTruncation** | Limits and strategy for truncating tool output | Output limit, size cap |
| **PendingToolApproval** | A tool call awaiting human approval before execution | Pending tool, queued call |

## Template system

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Template** | Reusable node definition with default `AgentNodeConfig` and locked fields | Node preset, blueprint |
| **LockedField** | Field name users cannot edit when a template is applied (e.g. `output_schema`, `auto_start`) | Protected field, frozen field |
| **TemplateStore** | Engine-level seam for listing and mutating templates | |

## Execution model

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Execution layer** | Topologically sorted group of nodes at the same depth; nodes in a layer may run in parallel | Layer, depth level, wave |
| **RunTelemetry** | Rich interactive run event stream (chat, tools, subagents, pauses); `ExecutionEvent` alias in orchestration | Execution event |
| **RunReport** | Aggregated result after a workflow run: per-node outputs plus read/token counters | Summary, result, run summary |
| **NodeRunOutput** | Structured output from one node | Node output, step result |
| **EntrypointText** | Initial text for nodes with no upstream dependencies at run start | Seed input, initial prompt |
| **Project** | Folder-scoped workspace binding workflows to a repo path | Workspace, repo binding |
| **RetryPolicy** | Workflow-level retry: max attempts and backoff (`WorkflowSettings.retry_policy`) | Retry config |
| **WorkflowSchedule** | Optional cron schedule on a workflow | Cron schedule |
| **NodeInvocation** | Shared assembly of upstream inputs and `AgentRequest` for `InteractiveEngine` | Request builder |
| **FrozenChangeEvidencePacket** | Immutable, hash-verified structured review output injected into later Plan Mode requests as `input.change_evidence_packet` | Plan file, system prompt |
| **Plan artifact** | Run-owned immutable Markdown evidence written by `openflow_write_plan_artifact` and referenced as `artifact:<uuid>` | Repository plan file |

## AI boundary

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **AiPort** | Trait between the workflow engine and an AI backend | AI adapter, backend trait |
| **AgentRequest** | Payload for one AI turn on one node | AI request, turn request |
| **AgentTurnPhase** | Tool-catalog phase for one model turn: Control exposes workflow control tools; Work exposes executable tools | Tool mode, request mode |
| **AgentTurnOutcome** | Result of one turn: Completed, ContinueWork, ToolCalls, NeedsUserInput, or Message | Turn result, AI response |
| **AgentTurnSuccess** | Completed outcome: structured output plus raw text | Success result |
| **AgentContinueWork** | Control outcome that advances the node to a Work turn | Continue signal, work request |
| **AgentToolCallBatch** | ToolCalls outcome: batch of tool invocations from the model | Tool call batch |
| **ToolCall** | Single tool invocation from the model | Tool request, tool use |
| **ToolResult** | Result returned after a tool executes | Tool response |
| **ToolDefinition** | Schema for a tool exposed to the model | Tool schema, tool spec |
| **AgentNeedUserInput** | Signal that the model needs human input to continue | User input requested |
| **AgentError** | Error from the AI backend for the current turn | Backend error, AI failure |
| **OutputRepairCandidate** | Redacted in-memory payload for a malformed `openflow_submit_node_output` call (raw args size-capped; omitted from `Display`/`Debug`) | Repair payload, raw tool args |
| **Overseer output repair** | One bounded same-provider AI pass (`RepairingAiPort`) that may fix a repairable candidate before engine retries | Repair node, overseer agent, CallableAgent |

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
| **InteractiveEngine** | Sans-I/O state machine for desktop and headless runs; `run()` invokes `AiPort`/`ToolPort` until terminal or `NeedsInteraction` | Step engine, interactive runner |
| **EngineRunResult** | Outcome of one `run()` step: `Completed(RunReport)`, `Failed`, `Cancelled`, or `NeedsInteraction` | Run result, poll result |
| **NeedsInteraction** | `EngineRunResult` variant batching paused nodes (`EngineAwaitInput`, `EngineAwaitApproval`, `EngineRetryableNode`) | Pause batch |
| **Pause** | Suspend execution awaiting human input, tool approval, or retry | Step, break |
| **Resume** | Continue from a paused state via `on_*` handlers and another `run()` | Continue, step forward |

## Tool approval

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **ToolApproval** | Explicit human consent before a tool call runs | Tool consent, tool permit |
| **EngineAwaitApproval** | Pause payload when a node waits on tool approval before `run()` returns | Waiting for tool approval |

## Engine modules

Map glossary buckets to `crates/engine/src/`:

| Module | Terms |
| --- | --- |
| `graph/` | Workflow, Node, Edge, ids, settings, `CallableAgent`, `resolve_callable_agent_snapshots`, `validate_workflow`, execution layers |
| `template/` | Template, LockedField, TemplateStore trait, builtin presets |
| `execution/` | InteractiveEngine, RunReport, RunTelemetry, subagent_runtime, NodeInvocation |
| `conversation/` | ChatMessage, ChatRole, AgentTranscriptItem |
| `tools/` | NodeToolConfig, ApprovalMode, ToolCall, ToolResult, policy helpers |
| `ports/` | AiPort, AgentRequest, AgentTurnOutcome, human/tool input ports |

Persistence adapters outside engine:

| Location | Terms |
| --- | --- |
| `orchestration/adapters/storage/project_store.rs` | Project |
| `orchestration/adapters/storage/project_workflow_store.rs` | Project workflow files (`.flow/workflows/`) |
| `orchestration/adapters/storage/app_workflow_store.rs` | App-level workflows (`workflows.json`) |

## Orchestration modules

See [orchestration crate layout](architecture/orchestration-layout.md) for the crate overview.

| Module | Terms |
| --- | --- |
| `orchestration/workflow/catalog.rs` | Workflow merge/split, project assign |
| `orchestration/agent.rs` | CallableAgent library CRUD (`FileAgentStore` adapter) |
| `orchestration/lib.rs` | `AgentDefinition` type alias for persisted `CallableAgent` JSON |
| `orchestration/run/coordinator/mod.rs` | Run session, Pause/Resume host path |
| `orchestration/settings/facade.rs` | Settings, provider readiness |
| `orchestration/settings/context_window.rs` | Bundled model → context-window lookup (`resources/context_window_sizes.json`) |

## Relationships

- A **Workflow** has **Nodes** and **Edges**.
- An **Edge** passes a source **Node**'s output to a target **Node**.
- **Execution layers** come from DAG topology; same-layer nodes have no dependency on each other.
- An **AgentNodeConfig** belongs to one **Node** and may include **NodeToolConfig**.
- A **ChatMessage** or **AgentTranscriptItem** belongs to one **Node**'s transcript.
- An **AgentRequest** goes to **AiPort** for one **Node** per turn.
- An **AgentRequest** has one **AgentTurnPhase**; control and executable tools are never advertised together.
- **AgentTurnOutcome** selects completion, phase transition, executable tool work, human input, or a plain message.
- A **Template** instantiates a **Node** with default config and **LockedField** constraints.
- **InteractiveEngine::run** returns **EngineRunResult** until **Completed**, **Failed**, **Cancelled**, or **NeedsInteraction**.
- Headless acceptance runs use the same **InteractiveEngine** via `run_workflow_headless` in orchestration.

## Example dialogue

> **Dev:** "If a **Node** has two incoming **Edges**, how do we determine its **Execution layer**?"
>
> **Domain expert:** "Topological order - layer is one more than the max layer of upstream nodes."

> **Dev:** "What happens when the model returns **ToolCalls**?"
>
> **Domain expert:** "**InteractiveEngine** returns **NeedsInteraction** with **EngineAwaitApproval** when policy requires a prompt (auto-allowed calls run inside **run**). Approved calls execute; **ToolResult** feeds the same **Node** for another turn."

> **Dev:** "How do headless tests run workflows?"
>
> **Domain expert:** **`run_workflow_headless`** in orchestration constructs **InteractiveEngine** with scripted inputs and approvals — same state machine as the desktop app, no separate batch runner.

## Flagged ambiguities

- **Layer** - in `execution_layers`, DAG depth grouping; in CSS/UI, z-index. Here: execution layer only.
- **Event** - **RunTelemetry** / **ExecutionEvent** (interactive UI stream) vs OS/Tauri IPC events. Lifecycle detail lives in telemetry, not **RunReport**.
- **Config** - **AgentNodeConfig** (agent behavior) vs **NodeToolConfig** (tool environment). Distinct concepts.
- **Template** - canonical type in `engine::template`. Do not confuse it with older `NodeTemplate` naming in historical docs.
- **Runner** - prefer **InteractiveEngine** or **run_workflow_headless**. Do not use "runner" alone in docs or module names.
