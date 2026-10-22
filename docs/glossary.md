# Glossary

Use these terms consistently in code, docs, and UI copy. The last column lists aliases to avoid.

[Engine modules](#engine-modules) and [Orchestration modules](#orchestration-modules) map terms to code. The [orchestration crate layout](architecture/orchestration-layout.md) has the full module map.

## Graph structure

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Workflow** | Directed acyclic graph (DAG) of nodes and edges that defines an AI pipeline | Graph, pipeline, flowchart |
| **Node** | Vertex in the workflow graph that runs an AI agent | Step, task, stage |
| **Edge** | Directed connection between two nodes that carries output as input | Connection, link, arrow |
| **NodeId** | Unique identifier for a node within a workflow | |
| **EdgeId** | Unique identifier for an edge within a workflow | |
| **WorkflowId** | Unique identifier for a workflow | |
| **WorkflowSettings** | Portable per-workflow configuration: shared context, retry, schedule, provider override, and optional Plan → Execute gate | Workflow config |
| **PlanMode** | Optional workflow gate that holds a run in Planning until its configured conversational review node freezes a change evidence packet | Plan-only workflow |
| **SharedContext** | Text appended to every node's system prompt for the whole run (`WorkflowSettings.shared_context`) | Shared node context |
| **ExecutionCwd** | Working directory for filesystem tools during a run; chosen at run time, not stored on the workflow | Working directory, run folder |

## Node lifecycle

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **NodeKind** | Node variant (e.g. Agent) that determines runtime behavior | Node type, node variant |
| **AgentNodeConfig** | Agent node configuration: model, prompts, handoff, tools, callable agents, and optional human questions | Node config, agent config |
| **HandoffSpec** | Per-node result format: heading-based Markdown template or JSON output schema | Output mode |
| **HandoffArtifact** | Immutable run-scoped `HANDOFF.md` or `HANDOFF.json` reference with media type, size, and SHA-256 digest | Handoff file |
| **NodePosition** | Canvas coordinates for rendering a node | Position, coordinates |
| **RequestUserInput** | Whether a running node may call enabled human-input harnesses to pause; defaults to false | Manual start |
| **ConversationMode** | Direct-chat lifecycle: a plain assistant message ends one turn, live user messages may queue between model/tool calls, and submit stays terminal | Workflow completion |
| **CallableAgent** | Saved agent definition a node may invoke as a subagent during a run (`engine::CallableAgent`) | Saved subagent, AgentDefinition |
| **CallableAgentSelection** | Agent IDs on `AgentNodeConfig.callable_agents`; snapshotted at run start | Allowed agents, callable agents |
| **AllowAllCallableAgents** | When true, every saved agent is snapshotted at run start instead of `callable_agents` | Allow all agents |

## Conversation

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **ChatMessage** | One message, with a role, in a node's conversation | Message, turn |
| **ChatRole** | Message sender: System, Thinking, User, or Assistant | Sender, author |
| **AgentTranscriptItem** | Chronological transcript entry: assistant/user message, ToolCall, or ToolResult | Transcript entry, log item |

## Tool configuration

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **NodeToolConfig** | Tool capabilities for a node or saved agent: approval mode plus separate free-text and structured human-input harness flags | Tool settings, tool setup |
| **ApprovalMode** | Node-level tool approval strategy: `read_only` (read-class tools only, auto-approved), `write` (all tools; read-class auto, write-class prompt - default), `always_ask` (prompt every call), `yolo` (never prompt) | Approval policy |
| **Tool capability class** | Read/write grouping for builtins; drives approval and `read_only` availability. Read: retrieval/search tools plus `ls`. Write: mutation, shell, and subagent tools. Plain `ls` and read-only `find` Bash calls use the read approval tier. | Tool tier |
| **ToolTier** | Serialized capability class on tool definitions: `read` or `write` | Tool level, access tier |
| **ToolAccessPolicy** | Run-phase capability rule. Planning permits read-tier tools and write-enabled `docs/**/*.md` mutations; only the selected evidence-source node gets `write`/`edit` access to `run://PLAN.md` plus the approval-gated plan sealer. Execution restores the node's normal catalog. | Approval mode |
| **ToolConcurrency** | Whether tool calls share or exclude concurrent access: `shared` or `exclusive` | Parallelism, execution mode |
| **ToolCallStatus** | Tool call lifecycle: proposed, awaiting_approval, running, completed, blocked, failed, aborted | Call status |
| **ToolTruncation** | Limits and strategy for truncating tool output | Output limit, size cap |
| **PendingToolApproval** | Tool call awaiting human approval before it runs | Pending tool, queued call |

## Template system

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Template** | Reusable node definition with default `AgentNodeConfig` and locked fields | Node preset, blueprint |
| **LockedField** | Field users cannot edit once a template is applied (e.g. `output_schema`) | Protected field, frozen field |

## Execution model

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **Execution layer** | Topologically sorted group of nodes at the same depth; nodes in a layer may run in parallel | Layer, depth level, wave |
| **RunTelemetry** | Rich interactive run event stream (chat, tools, subagents, pauses); `ExecutionEvent` alias in orchestration | Execution event |
| **RunReport** | Aggregated result of a workflow run: per-node outputs plus read/token counters | Summary, result, run summary |
| **NodeRunOutput** | Structured output from one node | Node output, step result |
| **EntrypointText** | Initial text for nodes with no upstream dependencies at run start | Seed input, initial prompt |
| **Project** | Folder-scoped workspace binding workflows to a repo path | Workspace, repo binding |
| **RetryPolicy** | Workflow-level retry: max attempts and backoff (`WorkflowSettings.retry_policy`) | Retry config |
| **WorkflowSchedule** | Optional cron schedule on a workflow | Cron schedule |
| **NodeInvocation** | Shared assembly of upstream inputs and `AgentRequest` for `InteractiveEngine` | Request builder |
| **Effective provider** | Provider selected for a node: node override → shared workflow provider → active settings provider | Runtime provider |
| **FrozenChangeEvidencePacket** | Immutable, hash-verified structured review output injected into later Plan Mode requests as `input.change_evidence_packet` | Plan file, system prompt |
| **Plan artifact** | Run-owned immutable Markdown evidence the selected evidence-source node builds at `run://PLAN.md`, sealed through explicit approval of `openflow_write_plan_artifact` and referenced as `artifact:<uuid>` | Repository plan file |
| **Node handoff** | Canonical run-owned node result under `run://handoffs/{node_id}/`; downstream input carries its manifest next to the compact upstream output | Node output file |

## AI boundary

| Term | Definition | Aliases to avoid |
| --- | --- | --- |
| **AiPort** | Trait between the workflow engine and an AI backend | AI adapter, backend trait |
| **AgentRequest** | Payload for one AI turn on one node, including its effective provider and model | AI request, turn request |
| **AgentTurnOutcome** | Result of one turn: Completed, ToolCalls, NeedsUserInput, or Message | Turn result, AI response |
| **AgentTurnSuccess** | Completed outcome: structured output, raw text, and typed reasoning blocks | Success result |
| **AgentToolCallBatch** | ToolCalls outcome: batch of tool invocations from the model | Tool call batch |
| **ToolCall** | Single tool invocation from the model | Tool request, tool use |
| **ToolResult** | Result returned after a tool runs | Tool response |
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
| **Cycle** | Directed cycle that makes topological order impossible | Circular dependency |

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

Modules under `crates/engine/src/` own these terms:

| Module | Terms |
| --- | --- |
| `graph/` | Workflow, Node, Edge, ids, settings, `CallableAgent`, `resolve_callable_agent_snapshots`, `validate_workflow`, execution layers |
| `template/` | Template, locked fields, builtin presets |
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

The [orchestration layout](architecture/orchestration-layout.md) explains how these modules fit together.

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
- An **Edge** passes a source **Node**'s output and optional **Node handoff** manifest to a target **Node**.
- **Execution layers** come from DAG topology; nodes in the same layer do not depend on each other.
- An **AgentNodeConfig** belongs to one **Node** and may include **NodeToolConfig**.
- A **ChatMessage** or **AgentTranscriptItem** belongs to one **Node**'s transcript.
- An **AgentRequest** goes to **AiPort** for one **Node** per turn.
- Every **AgentRequest** advertises harness tools (`openflow_submit_node_output`, optional `openflow_request_user_input`, optional `openflow_ask_user_question`) alongside executable tools. A turn uses either exactly one harness tool alone or one or more executable tools, never both in the same batch.
- **AgentTurnOutcome** selects completion, executable tool work, human input, or a plain message.
- A **Template** instantiates a **Node** with default config and **LockedField** constraints.
- **InteractiveEngine::run** returns **EngineRunResult** until **Completed**, **Failed**, **Cancelled**, or **NeedsInteraction**.
- Headless acceptance runs use the same **InteractiveEngine** through `run_workflow_headless` in orchestration.

## Examples

- **Layers:** a node with two incoming edges sits one layer above the highest layer of its upstream nodes, following the graph's topological order.
- **Tool calls:** when the model returns `ToolCalls` and policy requires a prompt, `InteractiveEngine` returns `NeedsInteraction` with `EngineAwaitApproval`. Auto-allowed calls run inside `run`. After execution, `ToolResult` feeds the same node's next turn.
- **Headless tests:** orchestration's `run_workflow_headless` constructs `InteractiveEngine` with scripted inputs and approvals. The tests exercise the desktop app's state machine without a separate batch runner.

## Terms with multiple meanings

- **Layer** - in `execution_layers`, DAG depth grouping; in CSS/UI, z-index. Here it means execution layer only.
- **Event** - **RunTelemetry** / **ExecutionEvent** (interactive UI stream) vs OS/Tauri IPC events. Lifecycle detail lives in telemetry, not **RunReport**.
- **Config** - **AgentNodeConfig** (agent behavior) vs **NodeToolConfig** (tool environment). Distinct concepts.
- **Template** - the canonical type in `engine::template`. Not the older `NodeTemplate` naming in historical docs.
- **Runner** - prefer **InteractiveEngine** or **run_workflow_headless**. Do not use "runner" alone in docs or module names.
