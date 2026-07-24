<p align="center">
  <img src="crates/desktop/icons/icon.png" alt="OpenFlow" width="128" height="128" />
</p>

<h1 align="center">OpenFlow</h1>

<p align="center">
  <strong>The visual IDE for multi-agent workflows.</strong><br/>
  Built for repeatable pipelines, with the extensibility and feel of Claude Code.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/rust-2021-orange?logo=rust&logoColor=white" alt="Rust 2021" />
  <img src="https://img.shields.io/badge/tauri-2.0-FFC131?logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/solidjs-1.9-2C4F7C?logo=solid&logoColor=white" alt="SolidJS" />
</p>

<p align="center">
  <a href="#what-is-openflow">What is OpenFlow?</a> ·
  <a href="#install">Install</a> ·
  <a href="#features">Features</a> ·
  <a href="#developing">Developing</a> ·
  <a href="#contributing">Contributing</a>
</p>

## What is OpenFlow?

Building a multi-agent LLM pipeline usually means gluing prompts, state, and provider SDKs together by hand, then debugging it blind. OpenFlow gives you a canvas instead: drag out agents, wire them into a pipeline, and watch each one think, call tools, and hand off to the next in real time.

Underneath the canvas is a full agent harness. Tool use, approvals, subagents, and multiple LLM providers are all built in, so what you draw is what actually runs, and runs again the same way next time.

You can also choose how it runs. Keep it interactive like a Claude Code session, pausing to chat and approve each tool call, or switch a workflow to auto-approve and let it run standalone from start to finish.

## Install

Grab the latest build from [Releases](https://github.com/philbotar/OpenFlow/releases/latest) and open it. No Rust or Node required.

> **macOS gatekeeper:** unsigned builds may be blocked on first launch. Right-click **OpenFlow** → **Open**, or run `xattr -cr /path/to/OpenFlow.app`.

Want to build the installer yourself instead? See [Developing](#developing) below.

## Features

<table>
<tr>
<td width="50%" valign="top">

### Visual workflow editor

Drag nodes onto a canvas, wire them into a DAG, and configure each agent in an inspector panel. Validation runs before every run: cycles and broken edges never reach execution.

</td>
<td width="50%" valign="top">

### Parallel agent layers

Nodes in the same topological layer run concurrently. Downstream agents receive upstream output automatically, with no manual plumbing.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Tools & subagents

Agents can read/write files, run shell commands, and search code, each gated by an approval policy you control. Hand off a sub-task to another saved agent mid-run with no manual wiring.

</td>
<td width="50%" valign="top">

### Multi-provider LLM support

Point any node at an OpenAI-compatible or Anthropic model. Mix providers in one workflow, or swap a model out, without touching the rest of the pipeline.

</td>
</tr>
<tr>
<td width="50%" valign="top">

### Project-aware persistence

Workflows save automatically, either in the app or checked into your repo, so they stay versioned alongside your code and always run from the right working directory.

</td>
<td width="50%" valign="top">

### Interactive or standalone runs

Run it like a Claude Code session, pausing to approve tools and chat with individual nodes as thinking and results stream in, or flip on auto-approve and let the whole workflow run to completion on its own.

</td>
</tr>
</table>

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

Installs dependencies on first run, then launches the desktop app with hot reload.

### Build an installer

```bash
./scripts/install.sh
```

Builds a `.dmg` (macOS) and opens it. Drag **OpenFlow** to **Applications**.

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
| Repo map & change paths | [`AGENTS.md`](AGENTS.md) |
| Architecture overview | [`docs/architecture/technical-overview.md`](docs/architecture/technical-overview.md) |
| Coding patterns | [`docs/contributing/coding-patterns.md`](docs/contributing/coding-patterns.md) |
| Testing workflows | [`docs/contributing/testing-workflows.md`](docs/contributing/testing-workflows.md) |
| Domain glossary | [`docs/glossary.md`](docs/glossary.md) |

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the PR checklist. Classify your change with [`docs/contributing/development-lanes.md`](docs/contributing/development-lanes.md), run `./scripts/verify.sh`, and update [`CHANGELOG.md`](CHANGELOG.md) for user-visible changes.

## License

[MIT](LICENSE)
