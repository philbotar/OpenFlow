# Plan → Execute internals

Plan → Execute is an optional `WorkflowSettings.planMode` gate. Its configured review node must allow follow-up questions. Workflows without Plan → Execute keep normal tool behavior.

The [product explanation](../concepts/how-openflow-works.md#plan--execute-mode) describes what users approve. The [tools reference](../reference/tools.md#openflow_write_plan_artifact) specifies the sealing tool.

## Planning

The run is in **Planning** until the review node produces a schema-valid change evidence packet. During Planning:

- Agents may use read-tier tools, and write or edit repository-relative `docs/**/*.md` files when the node is write-enabled.
- Only the configured evidence-source node can build the run-local plan draft at `run://PLAN.md` or request its seal. A read-only evidence-source node may still mutate that virtual draft.
- Other nodes never receive the seal tool. Engine and host checks reject invented plan-draft or seal calls.
- Other repository writes, shell execution, MCP tools, and subagent calls are denied, even if a model attempts an unadvertised call.

## Sealing the plan

The agent creates the draft with `write`, refines it with replace-mode `edit`, then calls `openflow_write_plan_artifact` without passing the Markdown again. The seal call always creates an explicit human tool-approval pause.

- Denial leaves `run://PLAN.md` mutable for revision.
- Approval atomically moves the draft to an opaque UUID path under the run artifact root.
- Retrying an already successful seal returns the same artifact.
- The tool returns `artifact:<uuid>`, SHA-256, and size. It never accepts a repository path or plan payload.

## Execution

Once the evidence-source node submits its structured output, the engine freezes that exact packet with a canonical SHA-256 and moves to **Execution**. The frozen packet is checkpointed and added as `input.change_evidence_packet` to every later agent request. It is data, not a system instruction.

Keep the packet compact: scope, exclusions, criteria, decisions, implementation slices, plan artifact reference and hash, assumptions, open questions, approval summary, and optional standards or ADR references. Downstream agents treat it as authoritative and read the artifact only when they need plan detail.
