# Example workflows

OpenFlow adds the Matt Pocock workflow to each local app workflow catalog once.
The seeded copy is editable. Deleting it keeps it deleted; an existing workflow
with the same ID wins.

The JSON files also support project-scoped examples. Copy one into a linked
project's `.flow/workflows/` directory, then open or reload that project in
OpenFlow.

```bash
mkdir -p .flow/workflows
cp /path/to/OpenFlow/examples/matt_pocock_idea_to_ship.workflow.json \
  .flow/workflows/
```

The nodes inherit the active provider's default model at run time. Set a node
model only when that node needs an explicit override.

## Matt Pocock skills: idea to ship

[`matt_pocock_idea_to_ship.workflow.json`](matt_pocock_idea_to_ship.workflow.json)
implements the human-steered
[idea-to-ship flow](https://github.com/mattpocock/skills/blob/main/skills/engineering/ask-matt/SKILL.md):

1. Keep `/grill-with-docs`, `/to-spec`, and `/to-tickets` in one planning
   context.
2. Require the user to select a ready ticket with `/implement`.
3. Implement that ticket in a fresh node context with `/tdd`.
4. Run independent Standards and Spec review subagents.
5. Require explicit human approval before committing.

One workflow run implements one ticket. For the next unblocked ticket, start a
new run and give the planning node the existing approved spec and ticket
references. It validates those artifacts and skips replanning. The new run gives
the implementation ticket a fresh node context.

Install the upstream skills in the project first:

```bash
npx skills@latest add mattpocock/skills
```

Select `setup-matt-pocock-skills`, `grill-with-docs`, `grilling`,
`domain-modeling`, `to-spec`, `to-tickets`, `implement`, `tdd`,
`code-review`, and `codebase-design`. Then run
`/setup-matt-pocock-skills` once for that project.

Start the workflow with an entrypoint like:

```text
/grill-with-docs Add resumable file uploads with visible progress and retry.
```

The example intentionally omits OpenFlow Plan Mode because Plan Mode currently
blocks subagent calls during planning. The implementation node tells review
subagents to operate read-only on supplied material. OpenFlow still executes
subagent calls serially today; their contexts remain independent.

Upstream is MIT licensed. This example adapts the published flow; it does not
vendor the upstream skill bodies.
