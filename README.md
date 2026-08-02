<p align="center">
  <img src="crates/desktop/icons/icon.png" alt="OpenFlow logo" width="128" height="128" />
</p>

<h1 align="center">OpenFlow</h1>

<p align="center">
  <strong>Agent harness for creating multi-agent workflows</strong>
</p>

<p align="center">
  Design workflows where different provider agents can work autonomously or with human intervention, with defined handoffs, context availablility and parallel execution.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/rust-2021-orange?logo=rust&logoColor=white" alt="Rust 2021" />
  <img src="https://img.shields.io/badge/tauri-2.0-FFC131?logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/solidjs-1.9-2C4F7C?logo=solid&logoColor=white" alt="SolidJS" />
</p>

<p align="center">
  <a href="#manage-multi-step-ai-processes-with-ease">Overview</a> ·
  <a href="#install">Install</a> ·
  <a href="#use-cases">Use cases</a> ·
  <a href="#features">Features</a> ·
  <a href="#supported-ai-model-providers">Providers</a> ·
  <a href="#developing">Developing</a> ·
  <a href="#contributing">Contributing</a>
</p>

## Manage multi-step AI processes with ease

Create workflows manually or with AI, you can replicate the workflow you do daily with skills easily. It acts like a sequential Claude code sessions, where you can have dependencies and handoffs between mutliple agents, and allow them to work in parallel to speed up your development.

Aswell, OpenFlow watches your runs, providing post-run reports to identify where in your workflow the agent is getting stuck, and reccomends ways to improve the quality and speed of your workflow. 

<p align="center">
  <img src="docs/assets/openflow-workflow-demo.gif" alt="OpenFlow running a four-agent feature-planning workflow" width="100%" />
</p>

<p align="center">
  <em>The middle agents run in parallel, then the final agent turns their results into a Markdown brief.</em>
</p>

## Install

Grab the latest build from [Releases](https://github.com/philbotar/OpenFlow/releases/latest) and open it. No prior installs required. 

> **macOS Gatekeeper:** If macOS blocks the app on first launch, right-click **OpenFlow** → **Open**, or run `xattr -cr /path/to/OpenFlow.app`.

Want to build the installer yourself instead? See [Developing](#developing) below.

## Use cases

- **AI coding workflows** — connect planning, implementation, code review, testing, and release agents in one visible pipeline.
- **Parallel research** — fan work out across specialist agents, then merge their findings into one structured result.
- **Human-in-the-loop automation** — pause for questions or tool approval before an agent edits files or runs commands.
- **Multi-provider workflows** — route individual nodes through different hosted or local model providers.
- **Repeatable LLM pipelines** — version prompts, agent settings, graph structure, and project context alongside your code.

## Features

- **Canvas editor** — Add nodes and edges, configure each agent in the Inspector. Validation blocks cycles and broken edges before **Run**.
- **Build with AI** — Draft or edit a workflow graph from chat, then save with **Create Workflow** or **Apply Changes**.
- **Parallel layers** — Nodes in the same dependency layer run concurrently. Downstream nodes receive upstream output and optional handoff artifacts.
- **Per-node models** — Each node can use its own model from the same provider or a different one.
- **Tools and subagents** — Built-in tools (files, shell, search, MCP, and others) plus callable saved agents. **Approval mode** controls whether tool calls pause for confirmation.
- **Chat and approvals** — Per-node chat during a run; approve or deny tool calls, or set **Auto-approve all**.
- **Run review** — After a successful run, chat may show post-run review suggestions (prompts, tools, structure, models, coordination). Advisory only; does not change run success.

## Supported AI model providers

Built-in profiles: **OpenAI**, **Anthropic**, **ChatGPT (Codex)**, **Amazon Bedrock**, **OpenRouter**, **Groq**, **Together AI**, **Fireworks AI**, **DeepSeek**, **xAI / Grok**, **Mistral AI**, **Perplexity**, and **Gemini**.

Run local models through **Ollama** or **LM Studio**. Connect other gateways with a custom OpenAI-compatible endpoint.

See the [provider setup guide](docs/guides/provider-setup.md) for auth, endpoints, model selection, and the full compatibility matrix.

## Developing

Making changes to OpenFlow itself? Build and run it from source.

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

| Resource | Path |
| --- | --- |
| New users — feature map & first hour | [`docs/guides/for-new-users.md`](docs/guides/for-new-users.md) |
| Install, provider, first workflow | [`docs/getting-started/README.md`](docs/getting-started/README.md) |
| Repo map & change paths | [`AGENTS.md`](AGENTS.md) |
| Architecture overview | [`docs/architecture/technical-overview.md`](docs/architecture/technical-overview.md) |
| Coding patterns | [`docs/contributing/coding-patterns.md`](docs/contributing/coding-patterns.md) |
| Testing workflows | [`docs/contributing/testing-workflows.md`](docs/contributing/testing-workflows.md) |
| Example workflows | [`examples/README.md`](examples/README.md) |
| Domain glossary | [`docs/glossary.md`](docs/glossary.md) |

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the PR checklist.

Classify your change with [`docs/contributing/development-lanes.md`](docs/contributing/development-lanes.md), run `./scripts/verify.sh`, and update [`CHANGELOG.md`](CHANGELOG.md) for user-visible changes.

## License

[MIT](LICENSE)
