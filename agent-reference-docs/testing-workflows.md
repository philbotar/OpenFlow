# Testing Workflows

Purpose: explain how to verify workflow behavior without manually clicking through the desktop app.

## Local Dev Loops

| Goal | Command | What It Proves |
| --- | --- | --- |
| Start full desktop app | `npm --prefix crates/desktop run start -- dev` | Tauri config, frontend dev server, and desktop bootstrap work together |
| Start frontend only | `npm --prefix crates/ui run dev` | Vite dev server and frontend rendering load without desktop runtime |
| Frontend typecheck | `npm --prefix crates/ui run typecheck` | TS/TSX surface still matches current DTOs and component usage |

## Test Layers

| Layer | Command | What It Proves |
| --- | --- | --- |
| Unit tests | `cargo test --workspace` | Domain rules, tool approval resolution, app state, persistence, provider config, OpenAI-compatible and Anthropic wire mapping |
| Deterministic workflow acceptance | `cargo test -p orchestration --test workflow_acceptance -- --nocapture` | A whole workflow can run headlessly with scripted AI outputs, tool calls, and approval pauses |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 STEP_WORKFLOW_LIVE_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo test -p orchestration --test live_workflow -- --ignored --nocapture` | A real BYOK provider can complete a small workflow and satisfy schema-level rules |
## Acceptance Rules

The deterministic acceptance tests should prove:

1. Root nodes receive `entrypoint.text`.
2. Downstream nodes receive upstream outputs in deterministic order.
3. Branch/join workflows complete with all expected node outputs.
4. Manual nodes pause before execution, carry a per-node conversation, and pass the final structured node output downstream when the model signals it is ready.
5. Tool-enabled nodes can request one or more tool calls, receive tool results back into the model loop, and still produce the final node output downstream.
6. Tool approval pauses block progress until an approval decision is supplied, and denied tools surface a structured error without corrupting the run.
7. Run trace entries expose queued, running, paused, completed, or failed state transitions.
8. Chat logs capture system, thinking, user, and assistant messages where relevant, including paused-node follow-up turns and approval prompts.

## Live AI Rules

Live AI smoke tests must avoid exact prose assertions. Model output changes naturally, so assert contracts instead:

1. The run completes.
2. Every expected node has output.
3. Output is valid JSON.
4. Output satisfies the node schema.
5. Required fields are non-empty.
6. A sentinel value such as `ORCHID-91` is preserved exactly across nodes.

## When To Run Each Layer

Run this before normal commits:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo clippy-max
cargo test --workspace
```

Run this when changing execution behavior, node input shaping, manual pauses, tool approvals, tool result routing, run trace, or chat logs:
```bash
cargo test -p orchestration --test workflow_acceptance -- --nocapture
```
Run this only when intentionally checking a real provider/model:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="gpt-4o-mini" \
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```

DeepInfra-compatible chat completions example:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
STEP_WORKFLOW_LIVE_API_KEY="$OPENAI_COMPATIBLE_API_KEY" \
STEP_WORKFLOW_LIVE_BASE_URL="https://api.deepinfra.com/v1/openai" \
STEP_WORKFLOW_LIVE_CHAT_COMPLETIONS_PATH="chat/completions" \
STEP_WORKFLOW_LIVE_MODEL="deepseek-ai/DeepSeek-V4-Flash" \
cargo test -p orchestration --test live_workflow -- --ignored --nocapture
```
