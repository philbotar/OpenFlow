# For new users

OpenFlow is a desktop app for building and running multi-agent workflows on a canvas. This page maps what the product does to the first tasks you should try and where each topic is documented.

You do not need Rust or Node to use a [release build](https://github.com/philbotar/OpenFlow/releases/latest). Use [`../getting-started/README.md`](../getting-started/README.md) when building from source.

## Suggested first hour

| Step | Goal | Doc |
| --- | --- | --- |
| 1 | Install and open the app | [`../getting-started/README.md`](../getting-started/README.md) (Releases or `./scripts/install.sh`) |
| 2 | Finish onboarding; set up a provider | In-app carousel → **Set up provider →**; [`provider-setup.md`](provider-setup.md) |
| 3 | Build a two-node workflow and run it | [`first-workflow.md`](first-workflow.md) |
| 4 | Learn the sidebar, dock, and run controls | [`using-the-app.md`](using-the-app.md) |
| 5 | When something breaks | [`../troubleshooting/README.md`](../troubleshooting/README.md) |

The in-app onboarding shows **Build with AI**, inspecting nodes, and **Run** / **Stop**. Docs carry the detail onboarding skips: provider types, projects, schedules, tool approval, and persistence paths.

## What OpenFlow does (and where to read more)

| Capability | What you do in the app | Read |
| --- | --- | --- |
| Visual workflow editor | Add nodes on the canvas, connect edges, configure the inspector, fix validation errors before **Run** | [`first-workflow.md`](first-workflow.md), [`using-the-app.md`](using-the-app.md) |
| Parallel agent layers | Wire independent branches from the same upstream node; same-depth nodes run concurrently | [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md#parallel-layers) |
| Multi-provider models | **Settings → Providers** for credentials; per-node model in the inspector | [`provider-setup.md`](provider-setup.md) |
| Tools (files, shell, search, …) | Inspector **Tools** → **Approval mode**; approve or deny in chat when prompted | [`using-the-app.md`](using-the-app.md#tools-and-approval), [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md#tool-call) |
| Subagents (callable agents) | **Agents** screen to save configs; inspector **Subagents** to attach them to a node | [`using-the-app.md`](using-the-app.md#saved-agents-and-callable-agents), [`../architecture/callable-agents.md`](../architecture/callable-agents.md) |
| Interactive vs hands-off runs | Approval mode **Always ask** vs **Auto-approve all** on each node | [`using-the-app.md`](using-the-app.md#tools-and-approval) |
| Project-aware workflows | **Projects** → add repo folder; workflows under `.flow/workflows/`; `@` file refs in chat | [`using-the-app.md`](using-the-app.md), [`../reference/README.md#runtime-and-persistence-paths`](../reference/README.md#runtime-and-persistence-paths) |
| Build with AI | Sidebar **Build with AI** (sparkles) to draft a graph from chat | [`using-the-app.md`](using-the-app.md#build-with-ai) |
| Schedules | Sidebar **Schedule** on saved workflows | [`using-the-app.md`](using-the-app.md#schedule) |
| Run history & resume | Dock **History** → replay or resume | [`using-the-app.md`](using-the-app.md#run-trace-history-and-replay), [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md#run) |
| Post-run suggestions | After success, **Run review** in chat | [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md#run) |
| Plan → Execute | Workflow settings → **Plan → Execute** | [`../concepts/workflows-and-runs.md`](../concepts/workflows-and-runs.md#plan--execute) |
| MCP servers | **Settings → MCP Servers** | [`using-the-app.md`](using-the-app.md#settings-beyond-providers) |
| Web search keys | **Settings → Search** (for search-cli during runs) | [`using-the-app.md`](using-the-app.md#settings-beyond-providers) |
| Terminal in the dock | Dock **Terminal** tab | [`using-the-app.md`](using-the-app.md#editor-layout) |

## Without a provider

You can still create workflows, edit nodes, and save graphs when no API key or Codex session is configured. **Run**, **Build with AI**, and other model-backed actions stay disabled until readiness shows **Ready**. See [`provider-setup.md`](provider-setup.md#verify).

## After the basics

| If you want to… | Read |
| --- | --- |
| Understand runs end-to-end (without Rust) | [`../concepts/how-openflow-works.md`](../concepts/how-openflow-works.md) |
| Look up paths, env vars, dev commands | [`../reference/README.md`](../reference/README.md) |
| Change OpenFlow itself | [`../contributing/README.md`](../contributing/README.md), root [`AGENTS.md`](../../AGENTS.md) |

Do not use [`../ROADMAP.md`](../ROADMAP.md) as product documentation; it lists planned work, not current behavior.
