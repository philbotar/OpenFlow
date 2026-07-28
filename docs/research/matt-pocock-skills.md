# Matt Pocock Skills: Recommended Usage

Primary source: [`mattpocock/skills`](https://github.com/mattpocock/skills) at commit
[`ed37663cc5fbef691ddfecd080dff42f7e7e350d`](https://github.com/mattpocock/skills/tree/ed37663cc5fbef691ddfecd080dff42f7e7e350d).

## Install

Run:

```bash
npx skills@latest add mattpocock/skills
```

Select the required skills, including `/setup-matt-pocock-skills`. Run that setup
skill once per repo. It configures the issue tracker, triage labels, and docs
location. The `skills.sh` installer supports Codex today; a native Codex plugin
remains on the roadmap.

Source: [README.md lines 25–40](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/README.md#L25-L40),
[lines 62–67](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/README.md#L62-L67).

## Feature-development flow

The recommended `idea → ship` flow is:

1. `/grill-with-docs` — sharpen the change by interview while updating
   `CONTEXT.md` and ADRs through `/grilling` and `/domain-modeling`.
2. When a question needs a runnable answer, detour through
   `/handoff` → fresh session → `/prototype` → `/handoff`.
3. For a multi-session build, run `/to-spec` → `/to-tickets`.
   For a small build, proceed directly to `/implement`.
4. Start a fresh `/implement` session for each unblocked ticket.
   `/implement` uses `/tdd`, runs `/code-review`, then commits the work.

Keep grilling, spec creation, and ticket creation in one unbroken context.
Start each implementation ticket with fresh context. If the planning context
approaches its useful limit, use `/handoff` instead of continuing with degraded
context.

Source: [`ask-matt/SKILL.md` lines 13–32](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/skills/engineering/ask-matt/SKILL.md#L13-L32),
[`implement/SKILL.md` lines 7–15](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/skills/engineering/implement/SKILL.md#L7-L15).

## Ticket and invocation rules

- Tickets are narrow but complete vertical slices. Each slice must be
  independently demoable or verifiable, fit one fresh context, and declare its
  blocking edges.
- The user approves ticket granularity and dependency edges before publication.
  Implement any ticket on the unblocked frontier.
- User-invoked skills run only when the human invokes them. A user-invoked skill
  may invoke model-invoked skills, but never another user-invoked skill.
- Use `/wayfinder` only for a huge, foggy effort that cannot fit one session.
  Once the route is clear, return to `/to-spec`; do not use it for a well-scoped
  feature.

Source: [`to-tickets/SKILL.md` lines 25–65](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/skills/engineering/to-tickets/SKILL.md#L25-L65),
[`invocation.md` lines 1–10](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/.agents/invocation.md#L1-L10),
[`ask-matt/SKILL.md` lines 42–46](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/skills/engineering/ask-matt/SKILL.md#L42-L46).

## Minimum skill set

For the complete feature flow, install:

- `setup-matt-pocock-skills`
- `grill-with-docs`
- `grilling`
- `domain-modeling`
- `to-spec`
- `to-tickets`
- `implement`
- `tdd`
- `code-review`

Add `handoff` and `prototype` for runnable design investigations. Add
`wayfinder` only for genuinely huge, uncertain efforts.

Do not present this flow as GSD. Matt Pocock explicitly positions these skills
as small, adaptable, composable alternatives to process-owning systems such as
GSD, BMAD, and Spec-Kit.

Source: [README.md lines 15–19](https://github.com/mattpocock/skills/blob/ed37663cc5fbef691ddfecd080dff42f7e7e350d/README.md#L15-L19).
