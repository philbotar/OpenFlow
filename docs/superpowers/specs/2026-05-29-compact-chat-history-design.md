# Compact Chat History Design

## Goal

Fix the bottom chat panel so history and composer stay inside the visible panel, and replace large uneven diagnostic bubbles with compact inline rows.

## Scope

- Change chat rendering in `crates/agent-workflow-app/src/ui/canvas.rs`.
- Keep behavior in `agent-workflow-app`; no `workflow-core` or `openai-client` contract changes.
- Reuse existing theme tokens from `ui/theme.rs` unless a small chat-specific token is needed.
- Preserve current chat data model: `ChatMessage { role, content }`.

## Chosen Layout

Use option B from the visual mockup.

Chat history renders as a compact transcript:

- `System: Node 'Idea' started`
- `Thinking: System prompt: You are a focused AI agent...`
- `Thinking: Upstream input: {"upstream":[]}`
- `Assistant: ...`
- `You: ...`

Each row has a fixed-width role label column and a wrapping text column. `System` uses dim text, `Thinking` uses accent/monospace text, `Assistant` uses bright text, and `You` uses bright or accent text. Rows should align vertically and avoid framed bubble blocks for diagnostic content.

## Panel Sizing

The bottom panel keeps a strict internal split:

- History area consumes remaining height.
- Composer area reserves a bounded height.
- Error bar renders above composer inside the composer section.

The implementation must avoid child UI allocating beyond the bottom panel height. If vertical space is limited, history scrolls first and composer remains reachable.

## Interaction

No interaction behavior changes.

- Send remains enabled only when the selected node is awaiting input, execution is running, and an API key is ready.
- Retry and copy error controls stay available.
- Run trace tab remains unchanged.

## Tests

Add focused unit coverage where practical for pure helper behavior. For visual layout, run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Manual QA:

- Run a workflow that emits `System` and `Thinking` messages.
- Confirm chat rows render inline with aligned role labels.
- Confirm bottom composer and error bar stay inside the page at the screenshot-sized viewport.
