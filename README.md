# Step-through Agentic Workflow

Rust desktop app for composing AI agent workflows as nodes and edges.

## What It Does

- Define agent nodes with system prompt, task prompt, model, and JSON output schema.
- Drag nodes on canvas and connect agents with explicit edge builder controls.
- Connect nodes so upstream JSON outputs become downstream inputs.
- Validate workflow graphs as DAGs.
- Run nodes by dependency layer. Branches in the same layer run together; downstream nodes wait for all required upstream outputs.
- Run workflows through OpenAI-compatible providers or Anthropic direct BYOK APIs.
- Store BYOK API keys in the OS credential store, with environment-variable fallback.
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

- OpenAI-compatible providers: OpenAI, OpenRouter, Groq, Together, Fireworks, DeepSeek, xAI/Grok, Mistral, Perplexity, Gemini OpenAI compatibility, Ollama local, LM Studio local, and one custom OpenAI-compatible profile.
- Anthropic direct Messages API.

API keys saved in Settings are stored in the OS credential store/keychain, not in `settings.json`. Local providers such as Ollama and LM Studio can run without a key.

Environment fallback:

```bash
export OPENAI_API_KEY="sk-your-openai-key"
export OPENAI_COMPATIBLE_API_KEY="provider-key"
```

OpenAI defaults to `POST /v1/responses`; other OpenAI-compatible providers default to Chat Completions. Custom OpenAI-compatible profiles can edit base URL and wire paths.

- `Responses API` when the provider supports `/v1/responses`.
- `Chat Completions API` when the provider supports `/v1/chat/completions` with `response_format.type = "json_schema"`.

## Run

```bash
npm --prefix crates/desktop run start -- dev
```

## Build macOS app

Produce a standalone `OpenFlow.app` (no Node or dev server at runtime):

```bash
npm --prefix crates/ui ci   # first time only
npm --prefix crates/desktop run build
```

Output: `target/release/bundle/macos/OpenFlow.app`

Install locally:

```bash
cp -r target/release/bundle/macos/OpenFlow.app /Applications/
open /Applications/OpenFlow.app
```

Unsigned local builds are blocked by Gatekeeper on first launch. Right-click → **Open**, or run `xattr -cr /Applications/OpenFlow.app`.

Faster debug bundle (for bundle-only iteration): `npm --prefix crates/ui run tauri build -- --debug` → `target/debug/bundle/macos/OpenFlow.app`.

## Test

Default verification (runs all steps, reports every failure, exits non-zero if any step fails):

```bash
. "$HOME/.cargo/env"
./scripts/verify.sh
```

Steps: `fmt`, `clippy` (clippy-max strictness), `doc`, `test`, `public-api`, `machete`, `typos`, `ui-typecheck`, `ui-test`, `deny`, `arch`. Optional `./scripts/verify.sh --deep` adds `mutants`. Run a subset: `./scripts/verify.sh fmt clippy`. Set `VERIFY_FAIL_FAST=1` to stop on first failure.

Extra tools (install once): `cargo install cargo-machete typos-cli cargo-mutants cargo-public-api`.

CI policy: blocking gate is `./scripts/verify.sh` (default steps, no `--deep`).

Plan review (optional): `open tools/plan-review.html` — comment on markdown plans before implementation; see [ROADMAP](docs/ROADMAP.md#interactive-plan-review-tool).

Workflow acceptance tests:

```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```

Opt-in live AI smoke test:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="gpt-4o-mini" \
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

OpenAI-compatible live smoke example:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_COMPATIBLE_API_KEY" \
STEP_WORKFLOW_LIVE_BASE_URL="https://api.deepinfra.com/v1/openai" \
STEP_WORKFLOW_LIVE_WIRE_API="chat-completions" \
STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH="chat/completions" \
STEP_WORKFLOW_LIVE_MODEL="deepseek-ai/DeepSeek-V4-Flash" \
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

Live smoke tests assert schema-level behavior and sentinel preservation, not exact wording.

## Example Workflow

`examples/feature_plan.workflow.json` contains a four-node feature planning workflow:

```text
Clarify idea -> Create plan -> Final brief
             -> Find risks  -> Final brief
```

## Ownership

- `crates/engine`: domain model, validation, execution ordering, runner, AI port.
- `crates/providers`: provider adapters for OpenAI-compatible APIs and Anthropic direct.
- `crates/orchestration`: runtime orchestration, local persistence, app state.
- `crates/ui`: frontend shell and interaction layer.
- `crates/desktop/src-tauri`: desktop adapter command surface.
- `examples`: shareable workflow JSON files.