# Build from source

Build locally to change the app or test a source checkout. For the packaged macOS release, see [Install OpenFlow](README.md).

## Prerequisites

- Rust toolchain for the workspace crates.
- Node.js 22.12+ with npm (CI uses Node 22). The locked Vite toolchain accepts `^20.19.0 || >=22.12.0`. Node 18 is unsupported, though the setup script still names it.
- Tauri's platform build tools.
- A model provider for model-backed actions: hosted-provider credentials, a ChatGPT account with Codex access, AWS credentials for Bedrock, or a running local Ollama or LM Studio server (no API key needed).

## Start the development app

Run both scripts on this page from the repository root.

```bash
./scripts/start.sh
```

It installs dependencies on first run, then starts the Tauri desktop app and UI dev server together.

## Build a macOS installer

```bash
./scripts/install.sh
```

It builds a `.dmg` installer and opens it. Drag **OpenFlow** to **Applications**.

## Verify changes

- Compile and test commands: [Development commands](../contributing/commands.md)
- Choosing coverage: [Testing workflows](../contributing/testing-workflows.md)
- Compiler or test runner failures: [Build and test troubleshooting](../contributing/build-troubleshooting.md)
