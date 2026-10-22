# OpenFlow overview

<a id="table-of-contents"></a>

OpenFlow is a desktop workspace for agents. Start a direct chat to work through one task in a conversation, or build a workflow when several agents should pass results to one another.

For example, connect a planner to a checklist agent. The planner turns your request into a plan; the checklist agent waits for that output, then turns it into actionable items. You can inspect both conversations and reuse the same workflow with a different request.

## Start using OpenFlow

<a id="choose-a-path"></a>
<a id="get-started"></a>

1. [Install OpenFlow](getting-started/README.md). Open the packaged macOS app; no development toolchain is required.
2. [Connect a provider](guides/provider-setup.md). Use hosted credentials, ChatGPT sign-in, AWS credentials, or a local model server.
3. [Start a direct chat](guides/direct-chats.md), or [build your first workflow](guides/first-workflow.md) with two prompts you can paste into the editor.

[How OpenFlow works](concepts/how-openflow-works.md) explains dependencies, handoffs, and your role during a run. [Using the app](guides/using-the-app.md) locates the controls.

## Work with the app

<a id="i-use-openflow-day-to-day"></a>
<a id="use-the-app"></a>

| Task | Guide |
| --- | --- |
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
| What does a workflow, node, edge, or run mean? | [Workflows and runs](concepts/workflows-and-runs.md) |
| What are the settings, defaults, and limits? | [Configuration reference](reference/configuration.md) |
| Which endpoints, credentials, and defaults does each provider use? | [Provider profiles](reference/providers.md) |
| Which tools can an agent call? | [Tools reference](reference/tools.md) |
| Where does OpenFlow store files and credentials? | [Data locations](reference/README.md) |
| What leaves the app, and what requires trust? | [Security and data](security-and-data.md) |

## Develop OpenFlow

[Build from source](getting-started/build-from-source.md) sets up a development checkout. [Contributing](contributing/README.md) describes the change and review workflow; [Architecture](architecture/README.md) maps the implementation.

For commands and verification, use [Development commands](contributing/commands.md) and [Testing workflows](contributing/testing-workflows.md). [Documentation website](contributing/documentation-site.md) covers the site and Markdown exports.

<a id="other-material"></a>
<a id="documentation-map"></a>

The [repository map](FOLDER_STRUCTURE.md) locates source packages. Historical plans and roadmap entries do not establish current product behavior.
