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
- **Interactive node chat**: Each node has a chat log showing system messages, reasoning steps, and outputs.
- **Conditional auto-start**: Toggle "Auto-start" per node. Disabled nodes pause before execution and open a chat input bar, accepting human input as their output.
- **Background execution**: Workflow runs in a background task; the UI stays responsive and shows real-time status updates.
- **Context-aware prompting**: Chat input only appears once all upstream dependencies have completed, and the assembled context is visible in the chat history before you type.

## Setup

```bash
command -v cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup default stable
rustup component add rustfmt clippy
```

## Provider Config

The Settings screen can run workflows through:

- `ChatGPT / OpenAI`: default provider using `https://api.openai.com` and the Responses API.
- `OpenAI-compatible API`: custom base URL using either Responses API or Chat Completions API wire format.

API keys typed in Settings are transient and are not saved to disk.

Environment fallback:

```bash
export OPENAI_API_KEY="sk-your-openai-key"
export OPENAI_COMPATIBLE_API_KEY="provider-key"
```

The official OpenAI provider uses `POST /v1/responses` and sends each node output contract as `text.format.type = "json_schema"` with `strict = true`.

For OpenAI-compatible providers, choose:

- `Responses API` when the provider supports `/v1/responses`.
- `Chat Completions API` when the provider supports `/v1/chat/completions` with `response_format.type = "json_schema"`.

## Run

```bash
. "$HOME/.cargo/env"
cargo run -p agent-workflow-app
```

## Test

Default verification:

```bash
. "$HOME/.cargo/env"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Workflow acceptance tests:

```bash
cargo test -p agent-workflow-app --test workflow_acceptance -- --nocapture
```

Opt-in live AI smoke test:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
OPENAI_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="$STEP_WORKFLOW_LIVE_MODEL" \
cargo test -p agent-workflow-app --test live_workflow -- --ignored --nocapture
```

Live smoke tests assert schema-level behavior and sentinel preservation, not exact wording.

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
