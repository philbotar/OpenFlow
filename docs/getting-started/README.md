# Getting Started

Use this page to run OpenFlow locally, configure a model provider, and start the first workflow.

New to the product? Start with [`../guides/for-new-users.md`](../guides/for-new-users.md) for a feature map and first-hour path.

## Install (release build)

Download the latest macOS build from [GitHub Releases](https://github.com/philbotar/OpenFlow/releases/latest) and open **OpenFlow**. No Rust or Node required.

If macOS blocks the app, right-click **OpenFlow** → **Open**, or run `xattr -cr /path/to/OpenFlow.app` on the `.app` bundle.

## Prerequisites (from source)

- Rust toolchain for the workspace crates.
- Node.js and npm for the Tauri desktop and Solid UI packages.
- Provider credentials: an API key, AWS credentials for Bedrock, or a ChatGPT account for the ChatGPT (Codex) provider.

OpenFlow currently documents local development entry points. Use the reference page for command and storage details: [`../reference/README.md`](../reference/README.md).

## Run the Desktop App

```bash
./scripts/start.sh
```

This installs dependencies on first run, then starts the Tauri desktop app and UI dev server together.

## Install the Desktop App (macOS)

```bash
./scripts/install.sh
```

This builds a `.dmg` installer and opens it. Drag **OpenFlow** to **Applications** to install.

## Configure a Provider

Runs need a ready model provider. On first launch, use **Set up provider →** on the last onboarding slide, or open **Settings → Providers**.

Follow [`../guides/provider-setup.md`](../guides/provider-setup.md) for API keys, ChatGPT (Codex) sign-in, Bedrock, verification, and how orchestration wires providers through Rig at run time.

Key resolution and storage paths: [`../reference/README.md#provider-key-resolution`](../reference/README.md#provider-key-resolution) and [`../reference/README.md#runtime-and-persistence-paths`](../reference/README.md#runtime-and-persistence-paths).

## Create a Workflow

1. Create or open a workflow from the app sidebar.
2. Add an agent node.
3. Give the node a clear instruction.
4. Configure tools or callable agents only when that node needs them.
5. Add more nodes and connect edges when later work depends on earlier output.
6. Save the workflow.

For a complete walkthrough, see [`../guides/first-workflow.md`](../guides/first-workflow.md).

## Run and Inspect

1. Start the workflow from the editor (**Run** in the top bar).
2. Provide entrypoint text in the chat composer when the run needs user input.
3. Approve or deny tool calls when approval is required.
4. Use the bottom dock: **Chat** for conversation, **Run trace** for the event timeline, **History** for past runs (replay or resume).
5. After a successful run, check **Run review** suggestions in chat when present.

See [`../guides/using-the-app.md`](../guides/using-the-app.md) for sidebar screens, projects, schedules, and settings sections.

The deterministic acceptance tests cover the same runtime contracts without clicking through the app. See [`../contributing/testing-workflows.md`](../contributing/testing-workflows.md).

## Next

- [`../guides/provider-setup.md`](../guides/provider-setup.md) - configure providers and readiness.
- [`../guides/first-workflow.md`](../guides/first-workflow.md) - build and run a useful starter workflow.
- [`../guides/using-the-app.md`](../guides/using-the-app.md) - editor, runs, projects, and schedules.
- [`../concepts/how-openflow-works.md`](../concepts/how-openflow-works.md) - understand what happens during a run.
- [`../troubleshooting/README.md`](../troubleshooting/README.md) - fix local setup and provider failures.
