# Contributing

Start with a [source build](../getting-started/build-from-source.md), then choose the owning area and checks for your change.

## Make a change

1. Check the working tree and preserve existing staged, unstaged, and untracked work.
2. Find the owner in [AGENTS.md](../../AGENTS.md) and read that area’s guide.
3. Implement the change in that layer. Use [Coding patterns](coding-patterns.md) for ownership, errors, persistence, and test placement.
4. Run the checks for the affected behavior, then inspect the diff. Update product docs when behavior changes.
5. Run the full verification gate before a code PR. Report local checks, any live-provider smoke, and remote CI separately.

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

Project skills under [`.cursor/skills/`](../../.cursor/skills/) provide scoped help; [Development lanes](development-lanes.md) maps them to changes.

## Verify and submit

[Development lanes](development-lanes.md) selects the checks; [Development commands](commands.md) provides the command reference. Docs-only edits need link, path, syntax, and diff checks. Runtime changes need execution coverage; desktop changes need the desktop lane.

Publishing tags and releases requires explicit authorization. The [release guide](releasing.md) describes versioning and publication.

<a id="boundary-seams"></a>
<a id="ui-layout"></a>
<a id="persistence-overview"></a>

For code orientation, use [Ports and public boundaries](../FOLDER_STRUCTURE.md#ports-and-public-boundaries), [UI layout](../FOLDER_STRUCTURE.md#ui-layout), and [Persistence locations](../architecture/orchestration-layout.md#persistence-locations).

<a id="verification-quick-path"></a>

The [verification command reference](commands.md#verification-quick-path) includes fast checks, and the full gate.
