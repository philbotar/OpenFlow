# Build from source

Build OpenFlow locally to change the app or test a source checkout. To use the packaged macOS release, follow [Install OpenFlow](README.md).

## Prerequisites

- Rust toolchain for the workspace crates.
- Node.js 22.12+ with npm, matching the Node 22 CI setup. The locked Vite toolchain accepts `^20.19.0 || >=22.12.0`; Node 18 is unsupported even though the setup script still mentions it.
- Platform build tools required by Tauri.
- A model provider for model-backed actions: hosted-provider credentials, a ChatGPT account with Codex access, AWS credentials for Bedrock, or a running local Ollama / LM Studio server. Local profiles do not require an API key.

## Start the development app

Run these commands from the repository root.

```bash
./scripts/start.sh
```

The script installs dependencies on first run, then starts the Tauri desktop app and UI dev server together.

## Build a macOS installer

```bash
./scripts/install.sh
```

The script builds a `.dmg` installer and opens it. Drag **OpenFlow** to **Applications** to install.

## Verify changes

Use [Development commands](../contributing/commands.md) for the compile and test commands, then [Testing workflows](../contributing/testing-workflows.md) to select coverage. If compilation or a test runner fails, use [Build and test troubleshooting](../contributing/build-troubleshooting.md).
