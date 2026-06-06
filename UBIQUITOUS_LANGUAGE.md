# Ubiquitous Language

## Graph structure

| Term              | Definition                                                                 | Aliases to avoid           |
| ----------------- | -------------------------------------------------------------------------- | -------------------------- |
| **Workflow**      | A directed acyclic graph (DAG) of nodes and edges defining an AI pipeline   | Graph, pipeline, flowchart |
| **Node**          | A vertex in the workflow graph that runs an AI agent                       | Step, task, stage          |
| **Edge**          | A directed connection between two nodes, carrying output as input           | Connection, link, arrow    |
| **NodeId**        | A unique identifier for a node within a workflow                           |                            |
| **EdgeId**        | A unique identifier for an edge within a workflow                          |                            |
| **WorkflowId**    | A unique identifier for a workflow                                         |                            |

## Node lifecycle

| Term                   | Definition                                                                  | Aliases to avoid       |
| ---------------------- | -------------------------------------------------------------------------- | ---------------------- |
| **NodeKind**           | The variant of a node (e.g., Agent) determining its runtime behavior       | Node type, node variant |
| **AgentNodeConfig**    | Configuration for an agent node: model, system prompt, tools, auto-start   | Node config, agent config |
| **NodeTemplate**       | A reusable, parameterized node definition used to instantiate nodes        | Template, node blueprint |
| **NodePosition**       | Coordinates for rendering a node on the canvas                              | Position, coordinates   |

## Conversation

| Term                     | Definition                                                            | Aliases to avoid          |
| ------------------------ | --------------------------------------------------------------------- | ------------------------- |
| **ChatMessage**          | A single message in a node's conversation with a given role            | Message, turn             |
| **ChatRole**             | The sender of a message: User or Assistant                             | Sender, author            |
| **AgentTranscriptItem** | A chronological item in the node's full conversation transcript         | Transcript entry, log item |

## Execution model

| Term                 | Definition                                                                          | Aliases to avoid           |
| -------------------  | ----------------------------------------------------------------------------------- | -------------------------- |
| **Execution layer**  | A topologically sorted group of nodes at the same depth, executed in parallel       | Layer, depth level, wave   |
| **RunEvent**         | A single atomic event during workflow execution (started, completed, failed, etc.)  | Event, log entry           |
| **RunEventKind**     | The variant of a run event (started, completed, failed, user_input_required, etc.)  | Event type                 |
| **RunReport**        | The final aggregated report after a complete workflow run                            | Summary, result, run summary |
| **NodeRunOutput**    | The output produced by a single node during its run                                   | Node output, step result   |

## AI boundary

| Term                    | Definition                                                                   | Aliases to avoid         |
| ----------------------  | ---------------------------------------------------------------------------- | ------------------------ |
| **AiPort**              | The trait defining the contract between the workflow engine and an AI backend | AI adapter, backend trait |
| **AgentRequest**        | The request payload sent to an AI backend for a node's turn                  | AI request, turn request  |
| **AgentTurnOutcome**    | The result of one AI turn: Completed, ToolCalls, or NeedsUserInput            | Turn result, AI response |
| **ToolCall**            | A single tool invocation issued by the AI                                    | Tool request, tool use    |
| **ToolResult**          | The result returned from a tool call                                           | Tool response            |
| **ToolDefinition**      | The schema definition of a tool available to the AI                           | Tool schema, tool spec   |
| **AgentNeedUserInput**  | Signal that the AI requires human input to continue                           | User input requested      |

## Interactive execution

| Term                | Definition                                                                          | Aliases to avoid           |
| ------------------  | ----------------------------------------------------------------------------------- | -------------------------- |
| **InteractiveEngine** | The stateful, poll-based engine for step-through workflow execution               | Step engine, interactive runner |
| **EnginePollResult**  | The result of polling the interactive engine: CallAi, AwaitInput, AwaitToolApproval, Completed, or Failed | Poll result |
| **WorkflowRunner**   | The non-interactive runner that executes a workflow to completion in one call       | Batch runner, run to completion |
| **Pause**            | The act of suspending execution at a node, awaiting human input or tool approval   | Step, break                |
| **Resume**           | The act of continuing execution from a paused state                                 | Continue, step forward     |

## Tool approval

| Term                  | Definition                                                                    | Aliases to avoid         |
| --------------------- | ----------------------------------------------------------------------------- | ------------------------ |
| **ToolApproval**      | Explicit human consent for a tool call before it executes                     | Tool consent, tool permit |
| **AwaitToolApproval** | The EnginePollResult variant indicating execution is paused pending approval | Waiting for tool approval |

## Relationships

- A **Workflow** consists of zero or more **Nodes** and zero or more **Edges**
- An **Edge** connects a source **Node** to a target **Node**, passing the source's output as the target's input
- **Execution layers** are derived from the DAG topology; nodes in the same layer have no dependency on each other and can run in parallel
- An **AgentNodeConfig** belongs to exactly one **Node**
- A **ChatMessage** belongs to exactly one **Node**'s transcript
- An **AgentRequest** is sent to an **AiPort** for exactly one **Node** per turn
- **InteractiveEngine** yields **EnginePollResult** variants until the workflow **Completed** or **Failed**

## Example dialogue

> **Dev:** "If a **Node** has two incoming **Edges**, how do we determine its **Execution layer**?"
>
> **Domain expert:** "The layer is determined by topological order — a node's layer is one more than the maximum layer of all its upstream nodes. If Node A is in layer 2 and Node B is in layer 3, and both feed into Node C, then Node C lands in layer 4."
>
> **Dev:** "What happens if the AI calls a **ToolCall** during a turn?"
>
> **Domain expert:** "The **AgentTurnOutcome** is `ToolCalls`, and the **InteractiveEngine** emits `AwaitToolApproval` until the user approves. On approval, the tool executes and the result is fed back into the same **Node** for another AI turn."
>
> **Dev:** "Can a **Node** produce multiple **ChatMessages** in one turn?"
>
> **Domain expert:** "Yes — the AI may produce several messages during a single turn. Each is appended to the **AgentTranscriptItem** list in order, with the appropriate **ChatRole**."
>
> **Dev:** "What stops a cycle in the graph?"
>
> **Domain expert:** "**validate_workflow** returns `Cycle` if not all nodes are visited during topological sorting. The **WorkflowRunner** only accepts valid **Workflows**."

## Flagged ambiguities

- **"Layer"** is overloaded: in `execution_layers` it means a topological depth-grouping; in CSS/UI it means a z-index stacking context. In this domain, "execution layer" always refers to the DAG depth grouping.
- **"Event"** could mean a `RunEvent` (execution telemetry) or an OS-level event (file watch, Tauri event). **RunEvent** is the domain-specific term; OS events are outside this glossary.
- **"Config"** appears in both `AgentNodeConfig` and `NodeToolConfig`. The former configures the AI agent behavior; the latter configures tool availability — they are distinct concepts despite sharing the word.