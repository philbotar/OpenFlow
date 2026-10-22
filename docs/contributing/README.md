# Contributing

Start with a [source build](../getting-started/build-from-source.md), then find the owning area and checks for your change.

## Make a change

1. Check the working tree. Preserve existing staged, unstaged, and untracked work.
2. Find the owner in [AGENTS.md](../../AGENTS.md) and read that area's guide.
3. Implement in that layer, following [Coding patterns](coding-patterns.md) for ownership, errors, persistence, and test placement.
4. Run checks for the affected behavior and inspect the diff. Update product docs when behavior changes.
5. Run the full verification gate before a code PR. Report local checks, live-provider smoke, and remote CI separately.

<a id="filesystem"></a>

## Choose guidance for the change

| Task | Guide |
| --- | --- |
| Launch a local development build | [Build from source](../getting-started/build-from-source.md) |
| Look up commands | [Development commands](commands.md) |
| Fix compilation, cache, or test failures | [Build and test troubleshooting](build-troubleshooting.md) |
| Find the owner and checks | [Development lanes](development-lanes.md) |
| Place logic and follow runtime conventions | [Coding patterns](coding-patterns.md) |
| Verify behavior without the desktop UI | [Testing workflows](testing-workflows.md) |
| Bump versions, tag, and publish a release | [Releasing](releasing.md) |
| Build or publish the documentation website | [Documentation website](documentation-site.md) |
| Trace a run across layers | [End-to-end runtime](../architecture/end-to-end-runtime.md) |
| Check dependency rules | [Architecture contract](../architecture/contract.md) |
| Look up engine and app vocabulary | [Glossary](../glossary.md) |

Project skills in [`.cursor/skills/`](../../.cursor/skills/) give scoped help; [Development lanes](development-lanes.md) maps them to changes.

## Verify and submit

[Development lanes](development-lanes.md) picks the checks; [Development commands](commands.md) lists them. Docs-only edits need link, path, syntax, and diff checks. Runtime changes need execution coverage; desktop changes need the desktop lane.

Publishing tags and releases needs explicit authorization. The [release guide](releasing.md) covers versioning and publication.

<a id="boundary-seams"></a>
<a id="ui-layout"></a>
<a id="persistence-overview"></a>

Code orientation: [Ports and public boundaries](../FOLDER_STRUCTURE.md#ports-and-public-boundaries), [UI layout](../FOLDER_STRUCTURE.md#ui-layout), [Persistence locations](../architecture/orchestration-layout.md#persistence-locations).

<a id="verification-quick-path"></a>

Fast checks and the full gate: [verification command reference](commands.md#verification-quick-path).
