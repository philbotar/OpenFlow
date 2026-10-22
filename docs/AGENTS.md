# Documentation ownership

Give each page one primary reader question. Keep supporting context when the reader needs it to finish that task. Put the full procedure or reference in one canonical home; other pages link to it. Do not impose the same section template on every page.

| Reader needs to… | Canonical home |
| --- | --- |
| Understand the app and choose a first task | [OpenFlow overview](README.md) |
| Install, launch, or update the packaged app | [Install OpenFlow](getting-started/README.md) |
| Connect and verify a model provider | [Provider setup](guides/provider-setup.md) |
| Work in a saved conversation | [Direct chats](guides/direct-chats.md) |
| Complete one two-agent tutorial | [Your first workflow](guides/first-workflow.md) |
| Find a UI control | [Using the app](guides/using-the-app.md) |
| Perform a recurring product task | The relevant page under `guides/` |
| Understand dependencies, handoffs, and interaction | [How OpenFlow works](concepts/how-openflow-works.md) |
| Look up graph and run terms | [Workflows and runs](concepts/workflows-and-runs.md) |
| Look up defaults, provider profiles, tool contracts, or data locations | The corresponding page under `reference/` |
| Fix a packaged-app failure | [Troubleshooting](troubleshooting/README.md) |
| Set up a source checkout | [Build from source](getting-started/build-from-source.md), shown under Contribute |
| Change and verify the code | `contributing/`: workflow, commands, test strategy, and build failures |
| Understand or modify runtime internals | `architecture/`; [Folder structure](FOLDER_STRUCTURE.md) owns the source map |

Product concepts explain visible behavior without requiring crate, IPC, or trait knowledge. Provider setup owns authentication actions; `reference/providers.md` owns the profile matrix and credential precedence; `architecture/provider-adapters.md` owns transport. Keep Rust build failures out of product troubleshooting.

The sidebar label and page title must agree. New pages need an appropriate navigation entry and an inbound link. `guides/for-new-users.md` is a compatibility page for old links, not another onboarding path. Preserve old URLs and anchors when moving content, and update current links to the canonical destination.

Before handoff, read these paths as a new user: overview → install → provider → direct chat; overview → install → provider → first workflow → run recovery. Check prerequisites, exact controls, expected results, duplicate instructions, unexplained terms, and detours into unrelated topics. Link/build checks alone do not establish that a path works.

Run the docs package's build and check after content moves. The check validates navigation titles, links, anchors, assets, and exports. Preserve verified behavior, code examples, defaults, limits, and caveats through the move.
