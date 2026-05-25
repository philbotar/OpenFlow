# Testing Workflows

Purpose: explain how to verify workflow behavior without manually clicking through the desktop app.

## Test Layers

| Layer | Command | What It Proves |
| --- | --- | --- |
| Unit tests | `cargo test --workspace` | Domain rules, app state, persistence, provider config, UI layout contracts, OpenAI wire mapping |
| Deterministic workflow acceptance | `cargo test -p agent-workflow-app --test workflow_acceptance -- --nocapture` | A whole workflow can run headlessly with scripted AI outputs |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 OPENAI_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo test -p agent-workflow-app --test live_workflow -- --ignored --nocapture` | A real model can complete a small workflow and satisfy schema-level rules |

## Acceptance Rules

The deterministic acceptance tests should prove:

1. Root nodes receive `entrypoint.text`.
2. Downstream nodes receive upstream outputs in deterministic order.
3. Branch/join workflows complete with all expected node outputs.
4. Manual nodes pause before execution, receive scripted human input, and pass that input downstream.
5. Run trace entries expose queued, running, paused, completed, or failed state.
6. Chat logs capture system, thinking, user, and assistant messages where relevant.

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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run this when changing execution behavior, node input shaping, manual pauses, run trace, or chat logs:

```bash
cargo test -p agent-workflow-app --test workflow_acceptance -- --nocapture
```

Run this only when intentionally checking a real provider/model:

```bash
STEP_WORKFLOW_LIVE_AI=1 \
OPENAI_API_KEY="$OPENAI_API_KEY" \
STEP_WORKFLOW_LIVE_MODEL="$STEP_WORKFLOW_LIVE_MODEL" \
cargo test -p agent-workflow-app --test live_workflow -- --ignored --nocapture
```
