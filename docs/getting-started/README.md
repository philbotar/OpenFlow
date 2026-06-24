# Getting Started

Use this page to run OpenFlow locally, configure a model provider, and start the first workflow.

## Prerequisites

- Rust toolchain for the workspace crates.
- Node.js and npm for the Tauri desktop and Solid UI packages.
- A provider API key for the model you plan to use.

OpenFlow currently documents local development entry points. Use the reference page for command and storage details: [`../reference/README.md`](../reference/README.md).

## Run the Desktop App

```bash
npm --prefix crates/desktop run start -- dev
```

This starts the Tauri desktop app and the UI dev server together.

For frontend-only work, run:

```bash
npm --prefix crates/ui run dev
```

Use frontend-only mode for UI iteration. Use the desktop app when you need Tauri commands, local storage, run events, provider settings, or workflow execution.

## Configure a Provider

1. Open the app.
2. Go to Settings.
3. Choose the active provider profile.
4. Add an API key or rely on the provider environment variable.
5. Save settings.

Provider key resolution uses this order:

1. Transient key entered for the current run.
2. Stored profile key in the OpenFlow settings file.
3. Provider environment variable fallback, such as `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`.

Stored keys are plaintext in the local settings file. See [`../reference/README.md#runtime-and-persistence-paths`](../reference/README.md#runtime-and-persistence-paths).

## Create a Workflow

1. Create or open a workflow from the app sidebar.
2. Add an agent node.
3. Give the node a clear instruction.
4. Configure tools or callable agents only when that node needs them.
5. Add more nodes and connect edges when later work depends on earlier output.
6. Save the workflow.

For a complete walkthrough, see [`../guides/first-workflow.md`](../guides/first-workflow.md).

## Run and Inspect

1. Start the workflow from the editor.
2. Provide entrypoint text when the run needs user input.
3. Approve or deny tool calls when approval is required.
4. Watch the run trace and conversation output.
5. Resume a paused or durable run from run history when needed.

The deterministic acceptance tests cover the same runtime contracts without clicking through the app. See [`../contributing/testing-workflows.md`](../contributing/testing-workflows.md).

## Next

- [`../guides/first-workflow.md`](../guides/first-workflow.md) - build and run a useful starter workflow.
- [`../concepts/how-openflow-works.md`](../concepts/how-openflow-works.md) - understand what happens during a run.
- [`../troubleshooting/README.md`](../troubleshooting/README.md) - fix local setup and provider failures.
