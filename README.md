

# OpenFlow

**Claude Code for Workflows**

Each agent is its own session, with the ability to use multiple providers in a workflow. Use Fable for planning, Sol for implementation in one clean UI.

Use hosted or local models. Give agents tools and subagents. Keep approvals on while testing, then switch to auto-approve when the workflow is ready.



[Overview](#how-it-works) · [Install](#install) · [Use cases](#use-cases) · [Features](#whats-included) · [Providers](#models-and-providers) · [Developing](#developing) · [Contributing](#contributing)

## How it works

A node is one chat: a prompt, a model, some tools, and the context it receives. Connections set the order. Independent branches run in parallel, then pass their results forward.

You can control what context each node receives, and if you want it to run autonomously or with input.

Workflows can stay in OpenFlow or live with a project under `.flow/workflows/`. Project workflows run from that project and can be reviewed in Git with the code they work on.

## A four-agent run



*In this scripted run, the middle two agents work in parallel. The final agent turns their results into a Markdown brief.*

## Install

Download the latest build from [Releases](https://github.com/philbotar/OpenFlow/releases/latest) and open it. Rust and Node are only needed for source builds.

> **macOS:** Builds are currently unsigned. If Gatekeeper blocks the app, right-click **OpenFlow** and choose **Open**, or run `xattr -cr /path/to/OpenFlow.app`.

Building from source? Jump to [Developing](#developing).

## Use cases

- Give planning, implementation, review, and testing to separate coding agents.
- Run research tasks side by side, then send their findings to one agent for the summary.
- Stop for approval before an agent edits a file or runs a command.
- Run local models through Ollama or LM Studio.
- Keep a repeatable workflow beside the code it works on.



## What's included

- **Canvas.** Add agent nodes, draw connections, and edit settings in the inspector. OpenFlow catches cycles and invalid edges before a run starts.
- **Parallel work.** Independent branches run together. A downstream node starts when the results it needs are ready.
- **Tools and subagents.** Let agents work with files, run commands, search code or the web, call MCP tools, and delegate to saved subagents. You choose which actions need approval.
- **Project workflows.** Keep workflows in OpenFlow or save them under `.flow/workflows/` so they can be committed and reviewed with the code.
- **Interactive runs.** Open a node's chat while the graph runs. Answer questions, approve tools, inspect results, or turn on auto-approve and leave it alone.



## Models and providers

OpenFlow comes with profiles for **OpenAI**, **Anthropic**, **ChatGPT (Codex)**, **Amazon Bedrock**, **OpenRouter**, **Groq**, **Together AI**, **Fireworks AI**, **DeepSeek**, **xAI / Grok**, **Mistral AI**, **Perplexity**, and **Gemini**.

Use **Ollama** or **LM Studio** for local models. For another gateway, add a custom OpenAI-compatible endpoint.

See the [provider setup guide](docs/guides/provider-setup.md) for authentication, endpoints, model selection, and compatibility.

## Developing

Want to work on OpenFlow itself? Start here.

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- Platform build tools for [Tauri](https://v2.tauri.app/start/prerequisites/)



### Run in dev mode

```bash
./scripts/start.sh
```

The first run installs the dependencies. After that, the script starts the desktop app with hot reload.

### Build an installer

```bash
./scripts/install.sh
```

On macOS, this builds and opens a `.dmg`. Drag **OpenFlow** to **Applications**.

### Other useful commands

```bash
# Full verification gate (fmt, clippy, test, arch, UI typecheck, …)
./scripts/verify.sh

# Frontend only (hot reload, no Tauri shell)
npm --prefix crates/ui run dev

# Frontend typecheck
npm --prefix crates/ui run typecheck

# Workflow acceptance tests
cargo nextest run -p orchestration --test workflow_acceptance --no-capture
```


| Resource                          | Path                                                                                 |
| --------------------------------- | ------------------------------------------------------------------------------------ |
| New user guide                    | `[docs/guides/for-new-users.md](docs/guides/for-new-users.md)`                       |
| Install, provider, first workflow | `[docs/getting-started/README.md](docs/getting-started/README.md)`                   |
| Repo map & change paths           | `[AGENTS.md](AGENTS.md)`                                                             |
| Architecture overview             | `[docs/architecture/technical-overview.md](docs/architecture/technical-overview.md)` |
| Coding patterns                   | `[docs/contributing/coding-patterns.md](docs/contributing/coding-patterns.md)`       |
| Testing workflows                 | `[docs/contributing/testing-workflows.md](docs/contributing/testing-workflows.md)`   |
| Example workflows                 | `[examples/README.md](examples/README.md)`                                           |
| Domain glossary                   | `[docs/glossary.md](docs/glossary.md)`                                               |




## Contributing

Read `[CONTRIBUTING.md](CONTRIBUTING.md)` for the PR checklist.

Before opening a PR, choose the right development lane, run `./scripts/verify.sh`, and add user-visible changes to `[CHANGELOG.md](CHANGELOG.md)`.

The lane guide lives at `[docs/contributing/development-lanes.md](docs/contributing/development-lanes.md)`.

## License

[MIT](LICENSE)
