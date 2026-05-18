# Step-through Agentic Workflow

Rust desktop app for composing AI agent workflows as nodes and edges.

## What It Does

- Define agent nodes with system prompt, task prompt, model, and JSON output schema.
- Drag nodes on canvas and connect agents with explicit edge builder controls.
- Connect nodes so upstream JSON outputs become downstream inputs.
- Validate workflow graphs as DAGs.
- Run nodes by dependency layer. Branches in the same layer run together; downstream nodes wait for all required upstream outputs.
- Call OpenAI Responses API with Structured Outputs.
- Provide OpenAI API key in-app (secure input) or fall back to `OPENAI_API_KEY`.
- Provide entrypoint input text routed to root agents only.
- Inspect per-agent execution status chips plus run events and node outputs in the UI.
- Use keyboard QoL: `Cmd/Ctrl+Enter` run, `Cmd/Ctrl+S` save, delete selected node, clear run trace.
- Save workflows as local JSON.

## Setup

```bash
command -v cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup default stable
rustup component add rustfmt clippy
```

## OpenAI Config

```bash
export OPENAI_API_KEY="sk-your-key"
```

The app uses the Responses API endpoint `POST /v1/responses` and sends each node output contract as `text.format.type = "json_schema"` with `strict = true`.

## Run

```bash
. "$HOME/.cargo/env"
cargo run -p agent-workflow-app
```

## Test

```bash
. "$HOME/.cargo/env"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Example Workflow

`examples/feature_plan.workflow.json` contains a four-node feature planning workflow:

```text
Clarify idea -> Create plan -> Final brief
             -> Find risks  -> Final brief
```

## Ownership

- `crates/workflow-core`: domain model, validation, execution ordering, runner, AI port.
- `crates/openai-client`: OpenAI Responses API adapter.
- `crates/agent-workflow-app`: desktop UI, local workflow persistence, edit state.
- `examples`: shareable workflow JSON files.
