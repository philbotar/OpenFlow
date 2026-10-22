# OpenFlow overview

<a id="table-of-contents"></a>

OpenFlow is a desktop workspace for agents. Use a direct chat for one task in a conversation; build a workflow when several agents should pass results to one another.

For example, connect a planner to a checklist agent. The planner turns your request into a plan; the checklist agent waits for it, then turns it into actionable items. You can inspect both conversations and reuse the workflow with a different request.

## Start using OpenFlow

<a id="choose-a-path"></a>
<a id="get-started"></a>

1. [Install OpenFlow](getting-started/README.md). The packaged macOS app needs no development toolchain.
2. [Connect a provider](guides/provider-setup.md): hosted credentials, ChatGPT sign-in, AWS credentials, or a local model server.
3. [Start a direct chat](guides/direct-chats.md), or [build your first workflow](guides/first-workflow.md) from two prompts you paste into the editor.

[How OpenFlow works](concepts/how-openflow-works.md) explains dependencies, handoffs, and your role during a run. [Using the app](guides/using-the-app.md) locates the controls. [For new users](guides/for-new-users.md) keeps old links working.

## Work with the app

<a id="i-use-openflow-day-to-day"></a>
<a id="use-the-app"></a>

| Task | Guide |
| --- | --- |
| Chat with one agent | [Direct chats](guides/direct-chats.md) |
| Create, configure, and validate a graph | [Workflow authoring](guides/workflow-authoring.md) |
| Answer, approve, retry, replay, or resume | [Runs and recovery](guides/runs.md) |
| Work in a repository and manage saved files | [Projects and persistence](guides/projects-and-persistence.md) |
| Configure tool approval, skills, subagents, search, or MCP | [Tools and integrations](guides/tools-and-integrations.md) |
| Run a saved workflow at a time or interval | [Scheduling workflows](guides/scheduling.md) |
| Recover from a setup or run failure | [Troubleshooting](troubleshooting/README.md) |

## Look up behavior

<a id="look-up-settings-or-fix-a-problem"></a>

| Question | Reference |
| --- | --- |
| How do workflows and runs relate? | [Concepts](concepts/README.md) |
| What does a workflow, node, edge, or run mean? | [Workflows and runs](concepts/workflows-and-runs.md) |
| What does a code term mean? | [Glossary](glossary.md) |
| What are the settings, defaults, and limits? | [Configuration reference](reference/configuration.md) |
| Which endpoints, credentials, and defaults does each provider use? | [Provider profiles](reference/providers.md) |
| Which tools can an agent call? | [Tools reference](reference/tools.md) |
| Where does OpenFlow store files and credentials? | [Data locations](reference/README.md) |
| What leaves the app, and what requires trust? | [Security and data](security-and-data.md) |

## Develop OpenFlow

[Build from source](getting-started/build-from-source.md) sets up a development checkout. [Contributing](contributing/README.md) describes the change and review workflow; [Architecture](architecture/README.md) maps the implementation.

### Contribute

| Page | Covers |
| --- | --- |
| [Development commands](contributing/commands.md) | Build, check, and test commands |
| [Testing workflows](contributing/testing-workflows.md) | Which checks a change needs |
| [Development lanes](contributing/development-lanes.md) | Classifying a change |
| [Coding patterns](contributing/coding-patterns.md) | Ownership, errors, persistence, test placement |
| [Build and test troubleshooting](contributing/build-troubleshooting.md) | Source-checkout failures |
| [Releasing OpenFlow (macOS)](contributing/releasing.md) | Tagging and publishing a release |
| [Documentation website](contributing/documentation-site.md) | The site and Markdown exports |
| [README workflow demo GIF](contributing/readme-demo-gif.md) | Recording the README demo |

### Architecture pages

| Page | Covers |
| --- | --- |
| [Architecture contract](architecture/contract.md) | Layer boundaries and dependencies |
| [Run lifecycle](architecture/end-to-end-runtime.md) | UI → desktop → orchestration → engine → provider |
| [Agent execution and context](architecture/technical-overview.md) | Node scheduling and context |
| [Orchestration crate layout](architecture/orchestration-layout.md) | Module map |
| [Provider adapters](architecture/provider-adapters.md) | Model transport and routing |
| [Run persistence](architecture/run-persistence.md) | Checkpoints, resume, replay |
| [Threading and concurrency](architecture/threading-concurrency.md) | Async runtimes |
| [Callable agents in orchestration](architecture/callable-agents.md) | Saved-agent snapshots |
| [Overseer output repair](architecture/output-repair.md) | Repairing malformed node output |
| [Plan → Execute internals](architecture/plan-execute.md) | The planning gate |
| [Architecture diagrams](architecture/diagrams/README.md) | Layer diagrams |

<a id="other-material"></a>
<a id="documentation-map"></a>

The [repository map](FOLDER_STRUCTURE.md) locates source packages. Historical plans and roadmap entries do not establish current product behavior.
