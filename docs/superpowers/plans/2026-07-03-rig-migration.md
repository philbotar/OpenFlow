# Rig Provider Backend Migration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hand-rolled provider wire code in `crates/providers` (Anthropic HTTP+SSE, OpenAI-compatible, Bedrock Converse) with the `rig` library (rig-core 0.39.x + rig-bedrock 0.39.x), so OpenFlow no longer maintains provider transport code.

**Architecture:** Keep the `engine::AiPort` boundary and the crate's public API (`create_provider`, `AiClientConfig`, `spec.rs` registry) exactly as-is — nothing outside `crates/providers` changes. Inside the crate, a new `rig_adapter` module translates `AgentRequest` → rig `CompletionRequest`, dispatches through an enum of rig models (rig's `CompletionModel` is not dyn-safe), and translates rig responses/streams/errors back to `AgentTurnOutcome`/`AiStreamEvent`/`AgentError`. The domain protocol in `mapping.rs` (submit_output / request_input tools, jsonrepair recovery, outcome resolution) is **kept and reused** — only the wire-format builders and parsers die.

**Tech Stack:** rig-core 0.39.x, rig-bedrock 0.39.x (feature-gated), existing wiremock test harness, existing aws-config/aws-sdk-bedrockruntime (rig-bedrock's `Client` has a `From<aws_sdk_bedrockruntime::Client>` impl so the existing `aws_runtime.rs` SSO/profile handling is reused verbatim).

---

## What rig gives us (verified against rig source @ main, 0.39.0)

- `rig::completion::CompletionModel` trait: `completion(CompletionRequest)` and `stream(CompletionRequest)`. Not object-safe (associated `Response` types) → wrap providers in an enum.
- `CompletionRequest` fields: `model: Option<String>`, `preamble: Option<String>`, `chat_history: OneOrMany<Message>`, `tools: Vec<ToolDefinition>`, `tool_choice: Option<ToolChoice>`, `temperature`, `max_tokens`, `additional_params: Option<serde_json::Value>`, `output_schema: Option<schemars::Schema>`.
- `CompletionResponse<T>`: `choice: OneOrMany<AssistantContent>` (Text / ToolCall / Reasoning), `usage: Usage` (input/output/total plus `cached_input_tokens` and `cache_creation_input_tokens`), `raw_response: T`.
- `CompletionError` variants preserve raw provider failures: `HttpError`, `ProviderResponse(ProviderResponseError)` (raw body + status), `ResponseError`, `JsonError` → enough signal to classify Transient vs Permanent.
- Streaming: `RawStreamingChoice` carries text deltas, `Reasoning`/`ReasoningDelta`, `ToolCall`/`ToolCallDelta` → maps 1:1 onto `AiStreamEvent::{AssistantDelta, ThinkingDelta}` and end-of-turn tool call collection.
- Providers: `rig::providers::anthropic` (custom `base_url`, `anthropic_version` header, message-level `cache_control: Option<CacheControl>` — native prompt-cache support), `rig::providers::openai` (Chat Completions and Responses API; custom base_url), `rig::providers::{openrouter, ollama, ...}`, and `rig-bedrock` (`Client::with_profile_name(..)`, `ClientBuilder::region(..)`, or `From<aws_sdk_bedrockruntime::Client>`).

## What we keep (do NOT delete)

| File | Why |
|---|---|
| `src/spec.rs` | Provider registry consumed by orchestration settings UI. Untouched. |
| `src/auth.rs` | `AuthConfig` is public config surface. Keep the enum; `apply_auth` for reqwest dies at the end. |
| `src/mapping.rs` (domain half) | `submit_output_tool`, `request_input_tool`, `all_tool_specs`, `parse_internal_tool_outcome`, `resolve_tool_turn_outcome`, `normalize_submit_output_arguments`, `parse_plain_json_completion`, jsonrepair recovery. This is OpenFlow's node protocol, not wire code. |
| `src/aws_runtime.rs` | SSO/home-env handling; feeds the aws sdk client we hand to rig-bedrock. |
| `src/bedrock_models.rs` | Model listing + credential verification via aws-sdk-bedrock. Out of rig's scope. |
| `src/prompt_cache.rs` (partially) | `cache_session_key` + `openai_compat_cache_key_enabled` still needed (sent via `additional_params`). Anthropic `cache_control` JSON-injection helpers die (rig has native `CacheControl`). |
| `src/client.rs` | `AiClient`/`AiClientConfig` public types stay; internals rewired to rig. |

## What dies at the end

- `src/sse.rs` (rig owns SSE)
- `src/anthropic.rs` + wire-building parts of `src/anthropic_tests.rs` (behavior tests are ported, not deleted)
- `src/openai_compat.rs`
- `src/bedrock.rs` (the Converse mapping; ~1100 lines)
- Wire-JSON builders/parsers in `mapping.rs`: `transcript_to_responses_input`, `transcript_to_chat_messages`, `tool_payload`, `extract_usage_from_openai`, `extract_usage_from_anthropic`, `parse_responses_output`, `parse_chat_completion_output`, `parse_compatible_tool_call`, `extract_chat_message_text`

## Known risks (resolve in Task 1 spike — STOP and re-plan if any fails)

1. **Custom reqwest client / read timeouts.** Current code sets a 2-minute read timeout to convert stalled SSE streams into retryable errors (`client.rs:40-45`). rig's `ClientBuilder` is generic over an HTTP client (`H`); verify we can inject a reqwest client with `connect_timeout`/`read_timeout`. If not injectable, wrap `invoke_stream` in `tokio::time::timeout` per-event as a fallback.
2. **Responses API parity.** Spec `WireApi::Responses` providers must go through rig's OpenAI Responses API path. Verify rig 0.39's `providers::openai` Responses support covers tools + streaming. If not, those providers use Chat Completions (rig) and we note the behavior change.
3. **Custom header auth.** `AuthConfig::Header { name, .. }` for OpenAI-compatible providers with non-standard header names. rig builders expose `headers_mut()` (seen in anthropic client) — verify same on the openai builder.
4. **`tool_choice: required`.** The node protocol depends on the model always calling a tool (submit_output at minimum). Verify rig's `ToolChoice` has a `Required`/`Any` variant that serializes for all three providers.
5. **rig version pinning.** All code in this plan was written against rig source at 0.39.0. Pin `=0.39.x` in Cargo.toml; expect small signature drift and fix at compile time — the shapes are right, exact paths may differ.

---

### Task 1: Dependency spike — pin rig and verify the five risks

**Files:**
- Modify: `Cargo.toml` (workspace root — add workspace deps)
- Modify: `crates/providers/Cargo.toml`
- Create: `crates/providers/examples/rig_spike.rs` (temporary, deleted in this task)

- [ ] **Step 1: Add workspace dependencies**

In the workspace root `Cargo.toml` under `[workspace.dependencies]`:

```toml
rig-core = { version = "0.39", default-features = false, features = ["reqwest-rustls"] }
rig-bedrock = "0.39"
schemars = "1"
```

(Check rig-core's feature list with `cargo info rig-core` first; pick the TLS feature matching what reqwest already uses in this workspace — look at the existing `reqwest` workspace dep features.)

In `crates/providers/Cargo.toml`:

```toml
rig-core = { workspace = true }
schemars = { workspace = true }
rig-bedrock = { workspace = true, optional = true }
```

and extend the bedrock feature:

```toml
bedrock = ["dep:rig-bedrock", "dep:aws-config", "dep:aws-sdk-bedrock", "dep:aws-sdk-bedrockruntime", "dep:aws-smithy-types", "dep:dirs"]
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p providers`
Expected: clean check (no code uses rig yet).

- [ ] **Step 3: Write a throwaway spike that exercises all five risks**

Create `crates/providers/examples/rig_spike.rs` that (compile-only, no network):

```rust
//! Throwaway spike: prove the rig API surface we depend on exists.
//! Deleted at the end of this task.
use rig::completion::{CompletionModel, ToolChoice};
use rig::providers::anthropic;

fn main() {
    // Risk 3+1: builder with custom base_url; look for a way to inject
    // a custom reqwest client and custom headers.
    let _client = anthropic::Client::builder()
        .api_key("test")
        .base_url("http://localhost:9999")
        .anthropic_version("2023-06-01")
        .build();
    // Risk 4: tool_choice required variant exists.
    let _choice = ToolChoice::Required; // adjust to actual variant name
}
```

Extend it until each of the five risks is proven or disproven — check `ToolChoice` variants (`cargo doc -p rig-core --no-deps --open` or read `~/.cargo/registry/src/*/rig-core-0.39*/src/`), the openai builder's `headers_mut()`/custom-client hooks, and the Responses API entry point (`rig::providers::openai` — look for a `responses_api` module or a client flag).

- [ ] **Step 4: Record findings and delete the spike**

Write findings as a short comparison table at the bottom of THIS plan file (section "Spike findings"), including the exact type paths (e.g. `rig::providers::openai::responses_api::ResponsesCompletionModel`). Adjust later task code blocks if names differ. Then `rm crates/providers/examples/rig_spike.rs`.

**If risk 1, 3, or 4 fails with no workaround: STOP. Report to the user — the migration premise is broken.**

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/providers/Cargo.toml docs/superpowers/plans/2026-07-03-rig-migration.md
git commit -m "feat(providers): add rig-core and rig-bedrock dependencies"
```

---

### Task 2: Request conversion — `AgentRequest` → rig `CompletionRequest`

**Files:**
- Create: `crates/providers/src/rig_adapter/mod.rs`
- Create: `crates/providers/src/rig_adapter/convert.rs`
- Modify: `crates/providers/src/lib.rs` (add `pub(crate) mod rig_adapter;`)

The conversion reuses `mapping.rs` domain helpers. Transcript mapping: `AgentTranscriptItem` (engine `conversation/mod.rs:280`) has four variants — `AssistantMessage`, `UserMessage`, `ToolCall { call }`, `ToolResult { result }` — which map onto rig `Message::assistant(..)`, `Message::user(..)`, assistant `AssistantContent::ToolCall`, and `UserContent::ToolResult` respectively. Study how the current `transcript_to_chat_messages` (`mapping.rs:144`) orders and pairs them (tool results must follow their calls) and preserve that exact pairing.

- [ ] **Step 1: Write failing tests**

In `convert.rs` `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engine::{AgentTranscriptItem, NodeId, ToolCall as EngineToolCall, WorkflowId};
    use serde_json::json;

    fn request_with_transcript(transcript: Vec<AgentTranscriptItem>) -> engine::AgentRequest {
        engine::AgentRequest {
            workflow_id: WorkflowId("wf-1".into()),
            node_id: NodeId("n1".into()),
            node_label: "Node".into(),
            model: "claude-sonnet-4-6".into(),
            system_messages: vec!["sys-a".into(), "sys-b".into()],
            task_prompt: "do the thing".into(),
            input: json!({"k": "v"}),
            output_schema: json!({"type": "object", "properties": {"r": {"type": "string"}}}),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript,
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
        }
    }

    #[test]
    fn maps_system_messages_and_task_prompt() {
        let req = to_completion_request(&request_with_transcript(Vec::new()));
        assert_eq!(req.preamble.as_deref(), Some("sys-a\n\nsys-b"));
        // First user message contains the node context + task prompt,
        // mirroring what build_node_context / transcript_to_chat_messages produce today.
        let first = req.chat_history.first();
        assert!(matches!(first, rig::message::Message::User { .. }));
    }

    #[test]
    fn always_includes_submit_output_tool_and_requires_tool_choice() {
        let req = to_completion_request(&request_with_transcript(Vec::new()));
        assert!(req.tools.iter().any(|t| t.name == "submit_output"));
        assert!(req.tool_choice.is_some());
    }

    #[test]
    fn transcript_tool_call_and_result_stay_paired() {
        let transcript = vec![
            AgentTranscriptItem::ToolCall {
                call: EngineToolCall { id: "c1".into(), name: "search".into(), arguments: json!({"q": "x"}) },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "c1".into(), tool_name: "search".into(),
                    content: "found".into(), is_error: false,
                    artifact_ids: Vec::new(), ..Default::default()
                },
            },
        ];
        let req = to_completion_request(&request_with_transcript(transcript));
        // assistant tool-call message followed by user tool-result message
        let msgs: Vec<_> = req.chat_history.iter().collect();
        // exact assertions written against rig::message::Message shape from the spike
        assert!(msgs.len() >= 3); // context msg + call + result
    }

    #[test]
    fn reasoning_params_flow_into_additional_params() {
        let mut request = request_with_transcript(Vec::new());
        request.reasoning_effort = Some("high".into());
        request.reasoning_budget_tokens = Some(2048);
        let req = to_completion_request(&request);
        let params = req.additional_params.expect("params");
        assert_eq!(params["reasoning_effort"], "high");
    }
}
```

(Adjust `engine::ToolResult` construction to its real fields — it has more than shown; check `crates/engine/src/tools/config.rs:143`.)

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p providers rig_adapter::convert -- --nocapture`
Expected: FAIL — `to_completion_request` not defined.

- [ ] **Step 3: Implement `convert.rs`**

```rust
//! `AgentRequest` → rig `CompletionRequest` translation.
use crate::mapping::{all_tool_specs, build_node_context, ToolSpec};
use engine::{AgentRequest, AgentTranscriptItem};
use rig::completion::{CompletionRequest, ToolChoice};
use rig::message::Message;
use rig::OneOrMany;
use serde_json::json;

pub fn to_completion_request(request: &AgentRequest) -> CompletionRequest {
    let mut history: Vec<Message> = vec![Message::user(build_node_context(request))];
    for item in &request.transcript {
        match item {
            AgentTranscriptItem::UserMessage { content } => history.push(Message::user(content.clone())),
            AgentTranscriptItem::AssistantMessage { content } => history.push(Message::assistant(content.clone())),
            AgentTranscriptItem::ToolCall { call } => history.push(assistant_tool_call_message(call)),
            AgentTranscriptItem::ToolResult { result } => history.push(user_tool_result_message(result)),
        }
    }
    CompletionRequest {
        model: Some(request.model.clone()),
        preamble: Some(request.system_content()),
        chat_history: OneOrMany::many(history).unwrap_or_else(|_| {
            OneOrMany::one(Message::user(build_node_context(request)))
        }),
        documents: Vec::new(),
        tools: all_tool_specs(request).into_iter().map(rig_tool).collect(),
        temperature: None,
        max_tokens: None,
        tool_choice: Some(ToolChoice::Required), // exact variant per spike findings
        additional_params: additional_params(request),
        output_schema: None, // node protocol uses the submit_output tool, not native structured output
    }
}

fn rig_tool(spec: ToolSpec) -> rig::completion::ToolDefinition {
    rig::completion::ToolDefinition {
        name: spec.name,
        description: spec.description,
        parameters: spec.parameters,
    }
}

fn additional_params(request: &AgentRequest) -> Option<serde_json::Value> {
    let mut params = serde_json::Map::new();
    if let Some(effort) = &request.reasoning_effort {
        params.insert("reasoning_effort".into(), json!(effort));
    }
    if let Some(budget) = request.reasoning_budget_tokens {
        params.insert("reasoning_budget_tokens".into(), json!(budget));
    }
    if params.is_empty() { None } else { Some(serde_json::Value::Object(params)) }
}

fn assistant_tool_call_message(call: &engine::ToolCall) -> Message {
    use rig::message::{AssistantContent, ToolCall as RigToolCall, ToolFunction};
    Message::Assistant {
        id: None,
        content: OneOrMany::one(AssistantContent::ToolCall(RigToolCall::new(
            call.id.clone(),
            ToolFunction {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        ))),
    }
}

fn user_tool_result_message(result: &engine::ToolResult) -> Message {
    // Message::tool_result(id, content) builds User { content: [ToolResult] }.
    // Tool errors travel as content text, matching today's transcript_to_chat_messages
    // behavior — check that function before changing this.
    Message::tool_result(result.tool_call_id.clone(), result.content.clone())
}
```

(rig `Message` is `System { content } | User { content: OneOrMany<UserContent> } | Assistant { id, content }` with `Message::user`, `Message::assistant`, `Message::tool_result` constructors; rig `ToolCall` is `{ id, call_id, function: ToolFunction { name, arguments }, signature, additional_params }` with a `ToolCall::new(id, function)` constructor — verified against rig 0.39 source.) `mapping.rs` needs `ToolSpec`, `all_tool_specs`, and `build_node_context` to stay `pub(crate)`-visible; they already are (crate-internal module).

Note: provider-specific `additional_params` handling (how `reasoning_budget_tokens` becomes Anthropic `thinking.budget_tokens` vs OpenAI `reasoning_effort`) is applied at dispatch time in Task 6/7, not here — this function carries the raw values.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p providers rig_adapter::convert`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/rig_adapter crates/providers/src/lib.rs
git commit -m "feat(providers): map AgentRequest to rig CompletionRequest"
```

---

### Task 3: Response conversion — rig `CompletionResponse` → `AgentTurnOutcome`

**Files:**
- Create: `crates/providers/src/rig_adapter/outcome.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rig::completion::Usage;
    use rig::message::{AssistantContent, ToolCall as RigToolCall};
    use serde_json::json;

    fn usage() -> Usage {
        let mut u = Usage::new();
        u.input_tokens = 100;
        u.output_tokens = 20;
        u.total_tokens = 120;
        u
    }

    #[test]
    fn submit_output_tool_call_becomes_completed() {
        let choice = vec![AssistantContent::tool_call(
            "c1", "submit_output",
            json!({"output": {"r": "done"}, "assistant_message": null}),
        )];
        let outcome = resolve_outcome(choice, usage(), "Test provider", Some(&json!({"type":"object"}))).unwrap();
        match outcome {
            engine::AgentTurnOutcome::Completed(s) => {
                assert_eq!(s.output, json!({"r": "done"}));
                assert_eq!(s.usage.as_ref().map(|u| u.prompt_tokens), Some(100));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn request_input_becomes_needs_user_input() {
        let choice = vec![AssistantContent::tool_call(
            "c1", "request_input", json!({"assistant_message": "Which env?"}),
        )];
        let outcome = resolve_outcome(choice, usage(), "Test provider", None).unwrap();
        assert!(matches!(outcome, engine::AgentTurnOutcome::NeedsUserInput(n) if n.assistant_message == "Which env?"));
    }

    #[test]
    fn external_tool_calls_become_tool_call_batch() {
        let choice = vec![
            AssistantContent::text("Let me search."),
            AssistantContent::tool_call("c1", "search", json!({"q": "x"})),
        ];
        let outcome = resolve_outcome(choice, usage(), "Test provider", None).unwrap();
        match outcome {
            engine::AgentTurnOutcome::ToolCalls(batch) => {
                assert_eq!(batch.tool_calls.len(), 1);
                assert_eq!(batch.tool_calls[0].name, "search");
            }
            other => panic!("expected ToolCalls, got {other:?}"),
        }
    }

    #[test]
    fn malformed_submit_output_json_recovers_via_jsonrepair() {
        // trailing comma — jsonrepair territory; must still complete
        let choice = vec![AssistantContent::tool_call_raw_args(
            "c1", "submit_output",
            r#"{"output": {"r": "done"}, "assistant_message": null,}"#,
        )];
        let outcome = resolve_outcome_raw(choice, usage(), "Test provider", None);
        assert!(outcome.is_ok());
    }

    #[test]
    fn no_tool_calls_with_plain_json_text_recovers() {
        let choice = vec![AssistantContent::text(r#"{"output": {"r": "v"}, "assistant_message": null}"#)];
        let outcome = resolve_outcome(choice, usage(), "Test provider", None).unwrap();
        assert!(matches!(outcome, engine::AgentTurnOutcome::Completed(_)));
    }
}
```

(`tool_call_raw_args` may not exist in rig — if rig's `ToolCall.arguments` is a `serde_json::Value` parsed upstream, the jsonrepair test moves to the streaming path where raw argument strings appear; check the spike findings and adjust. The behavior requirement stands: malformed-but-recoverable JSON in submit_output arguments must not fail the node.)

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p providers rig_adapter::outcome`
Expected: FAIL — `resolve_outcome` not defined.

- [ ] **Step 3: Implement `outcome.rs`**

```rust
//! rig `CompletionResponse` → `AgentTurnOutcome`, reusing the mapping.rs node protocol.
use crate::mapping::{resolve_tool_turn_outcome, NoToolCallsPolicy, ResolveToolTurnParams};
use engine::{AgentError, AgentTurnOutcome, UsageReport};
use rig::completion::Usage;
use rig::message::AssistantContent;
use serde_json::Value;

pub fn to_usage_report(usage: &Usage) -> Option<UsageReport> {
    if usage.total_tokens == 0 && usage.input_tokens == 0 && usage.output_tokens == 0 {
        return None; // rig's zero-usage sentinel for "provider reported nothing"
    }
    Some(UsageReport {
        prompt_tokens: u32::try_from(usage.input_tokens).unwrap_or(u32::MAX),
        completion_tokens: u32::try_from(usage.output_tokens).unwrap_or(u32::MAX),
        total_tokens: u32::try_from(usage.total_tokens).unwrap_or(u32::MAX),
    })
}

pub fn resolve_outcome(
    choice: Vec<AssistantContent>,
    usage: Usage,
    provider_label: &str,
    output_schema: Option<&Value>,
) -> Result<AgentTurnOutcome, AgentError> {
    // Requires loosening mapping.rs ResolveToolTurnParams.provider_label from
    // &'static str to &str (mechanical, done in Task 5 step 3).
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<engine::ToolCall> = Vec::new();
    for item in choice {
        match item {
            AssistantContent::Text(t) => text_parts.push(t.text),
            AssistantContent::ToolCall(call) => tool_calls.push(engine::ToolCall {
                id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
            }),
            AssistantContent::Reasoning(_) => {} // thinking is surfaced via streaming only
        }
    }
    let assistant_message = if text_parts.is_empty() { None } else { Some(text_parts.join("")) };
    resolve_tool_turn_outcome(ResolveToolTurnParams {
        tool_calls,
        assistant_message,
        no_tool_calls: NoToolCallsPolicy::Recover {
            allow_plain_text_follow_up: true,
            error: "provider returned neither tool calls nor recoverable output",
        },
        output_schema,
        provider_label,
        usage: to_usage_report(&usage),
        filter_assistant_on_external_batch: true,
    })
}
```

(`AssistantContent`/`ToolCall` field paths per spike findings — rig's `ToolCall` is `{ id, call_id, function: ToolFunction { name, arguments } }`-shaped in 0.39; confirm.) `resolve_tool_turn_outcome` and `ResolveToolTurnParams` are already `pub` in `mapping.rs`; `NoToolCallsPolicy` too.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p providers rig_adapter::outcome`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/rig_adapter
git commit -m "feat(providers): map rig completion responses to AgentTurnOutcome"
```

---

### Task 4: Error conversion — `CompletionError` → `AgentError`

**Files:**
- Create: `crates/providers/src/rig_adapter/error.rs`

Classification contract (must match today's behavior — `engine::AgentError::is_retryable` drives the retry policy):
- HTTP 408, 409, 429, 5xx, connect/timeout/stream-stall → `Transient`
- HTTP 401, 403 → `Permanent` (auth), 400/404/422 → `Failed`
- JSON/parse errors → `Failed`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_is_transient() {
        let err = classify_status(429, "rate limited", "Anthropic");
        assert!(err.is_retryable());
    }

    #[test]
    fn server_errors_are_transient() {
        assert!(classify_status(500, "boom", "Anthropic").is_retryable());
        assert!(classify_status(529, "overloaded", "Anthropic").is_retryable());
    }

    #[test]
    fn auth_errors_are_permanent() {
        let err = classify_status(401, "bad key", "Anthropic");
        assert!(!err.is_retryable());
        assert!(matches!(err, engine::AgentError::Permanent(_)));
    }

    #[test]
    fn bad_request_is_failed_not_retryable() {
        let err = classify_status(400, "invalid model", "Anthropic");
        assert!(matches!(err, engine::AgentError::Failed(_)));
    }

    #[test]
    fn error_message_includes_provider_label_and_body() {
        let err = classify_status(429, "rate limited", "OpenRouter");
        let msg = err.to_string();
        assert!(msg.contains("OpenRouter"));
        assert!(msg.contains("rate limited"));
    }
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p providers rig_adapter::error`
Expected: FAIL.

- [ ] **Step 3: Implement `error.rs`**

```rust
//! rig `CompletionError` → `AgentError` with retryability classification.
use engine::AgentError;
use rig::completion::CompletionError;

pub fn classify_status(status: u16, body: &str, label: &str) -> AgentError {
    match status {
        401 | 403 => AgentError::Permanent(format!("{label} auth failed ({status}): {body}")),
        408 | 409 | 429 | 500..=599 => {
            AgentError::Transient(format!("{label} transient error ({status}): {body}"))
        }
        _ => AgentError::Failed(format!("{label} request failed ({status}): {body}")),
    }
}

pub fn to_agent_error(error: CompletionError, label: &str) -> AgentError {
    match error {
        CompletionError::ProviderResponse(resp) => {
            // spike finding: ProviderResponseError exposes status + raw body accessors
            classify_status(resp.status(), &resp.body_text(), label)
        }
        CompletionError::HttpError(http) => {
            // connect errors / timeouts / stream stalls are transient
            AgentError::Transient(format!("{label} HTTP transport error: {http}"))
        }
        CompletionError::JsonError(e) => AgentError::Failed(format!("{label} response JSON error: {e}")),
        CompletionError::ResponseError(e) => AgentError::Failed(format!("{label} response error: {e}")),
        other => AgentError::Failed(format!("{label} error: {other}")),
    }
}
```

(`ProviderResponseError` accessor names per spike; if `HttpError` wraps non-2xx responses with statuses too, route those through `classify_status` — inspect `rig::http_client::Error` in the spike and match today's classification table exactly.)

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p providers rig_adapter::error`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/providers/src/rig_adapter
git commit -m "feat(providers): classify rig errors into AgentError retryability"
```

---

### Task 5: Model dispatch enum + non-streaming `invoke` for Anthropic

**Files:**
- Create: `crates/providers/src/rig_adapter/model.rs`
- Test: `crates/providers/tests/rig_anthropic.rs` (wiremock, HTTP-level — ports scenarios from `src/anthropic_tests.rs`)

- [ ] **Step 1: Write failing wiremock test**

The test speaks real Anthropic wire format because rig now produces it; wiremock asserts on what rig sends and returns canned Anthropic JSON. Port the response fixtures from `src/anthropic_tests.rs` (they already encode correct Anthropic response shapes — reuse the JSON bodies verbatim).

```rust
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn anthropic_submit_output_completes_node() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_1", "type": "message", "role": "assistant",
            "model": "claude-sonnet-4-6",
            "content": [{
                "type": "tool_use", "id": "tu_1", "name": "submit_output",
                "input": {"output": {"r": "done"}, "assistant_message": null}
            }],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        })))
        .mount(&server)
        .await;

    let client = providers::create_provider(anthropic_test_config(&server.uri()));
    let outcome = client.invoke(test_request()).await.unwrap();
    assert!(matches!(outcome, engine::AgentTurnOutcome::Completed(_)));
}

#[tokio::test]
async fn anthropic_429_maps_to_transient() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server).await;
    let client = providers::create_provider(anthropic_test_config(&server.uri()));
    let err = client.invoke(test_request()).await.unwrap_err();
    assert!(err.is_retryable());
}
```

`anthropic_test_config` / `test_request` are small local helpers building `AiClientConfig { adapter: ProviderAdapterConfig::Anthropic(AnthropicConfig { base_url: server.uri(), .. }), auth: AuthConfig::Header { name: "x-api-key".into(), api_key: Some("test-key".into()), required: true }, .. }` — copy the fixture style from `crates/providers/tests/mock_factory.rs`.

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p providers --test rig_anthropic`
Expected: FAIL (old anthropic.rs path still active, or compile error on missing model.rs — either is fine; the point is the new path doesn't exist yet).

- [ ] **Step 3: Implement `model.rs` with the dispatch enum, wire Anthropic only**

```rust
//! Enum dispatch over rig provider models (CompletionModel is not dyn-safe).
use crate::client::{AiClientConfig, AnthropicConfig, ProviderAdapterConfig};
use crate::rig_adapter::{convert, error, outcome};
use engine::{AgentError, AgentRequest, AgentTurnOutcome};
use rig::completion::CompletionModel;

pub enum RigModel {
    Anthropic(rig::providers::anthropic::completion::CompletionModel),
    // OpenAiChat(..), OpenAiResponses(..), Bedrock(..) added in Tasks 7–8
}

pub fn build_model(config: &AiClientConfig, model: &str) -> Result<RigModel, AgentError> {
    match &config.adapter {
        ProviderAdapterConfig::Anthropic(anthropic) => build_anthropic(config, anthropic, model),
        _ => Err(AgentError::Failed("provider not yet migrated to rig".into())),
    }
}

fn build_anthropic(
    config: &AiClientConfig,
    anthropic: &AnthropicConfig,
    model: &str,
) -> Result<RigModel, AgentError> {
    let api_key = match &config.auth {
        crate::auth::AuthConfig::Header { api_key, .. }
        | crate::auth::AuthConfig::Bearer { api_key, .. } => api_key.clone().unwrap_or_default(),
        _ => String::new(),
    };
    if api_key.trim().is_empty() && config.auth.requires_key() {
        return Err(AgentError::Permanent(format!("{} API key missing", config.provider_label)));
    }
    let client = rig::providers::anthropic::Client::builder()
        .api_key(&api_key)
        .base_url(&anthropic.base_url)
        .anthropic_version(&anthropic.anthropic_version)
        // spike finding: inject reqwest client with CONNECT_TIMEOUT/READ_TIMEOUT here
        .build()
        .map_err(|e| AgentError::Failed(format!("failed to build Anthropic client: {e}")))?;
    Ok(RigModel::Anthropic(client.completion_model(model)))
}

impl RigModel {
    pub async fn invoke(
        &self,
        request: &AgentRequest,
        provider_label: &str,
    ) -> Result<AgentTurnOutcome, AgentError> {
        let completion_request = convert::to_completion_request(request);
        match self {
            Self::Anthropic(model) => {
                let response = model
                    .completion(completion_request)
                    .await
                    .map_err(|e| error::to_agent_error(e, provider_label))?;
                outcome::resolve_outcome(
                    response.choice.into_iter().collect(),
                    response.usage,
                    provider_label,
                    Some(&request.output_schema),
                )
            }
        }
    }
}
```

Note `messages_path` from `AnthropicConfig`: rig hardcodes `/v1/messages`. `normalize_anthropic_base_url` in rig already handles base URLs with/without the `/v1` suffix. If any configured provider spec uses a non-standard `messages_path`, surface that in the spike findings; otherwise drop the field at the END (Task 9) since it's `pub`.

Wire it into `AiClient::invoke` for the Anthropic arm only (leave the other arms on the old code paths for now):

```rust
ProviderAdapterConfig::Anthropic(_) => {
    let model = crate::rig_adapter::model::build_model(&self.config, &request.model)?;
    model.invoke(&request, &self.config.provider_label).await
}
```

`resolve_tool_turn_outcome`'s `ResolveToolTurnParams.provider_label` is `&'static str` today; loosen it to `&str` in `mapping.rs` in this step (mechanical change, no behavior change) so runtime labels flow through without leaking strings.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p providers --test rig_anthropic && cargo test -p providers`
Expected: new tests PASS; old anthropic wire-format unit tests in `src/anthropic_tests.rs` may now fail — port any that assert *behavior* (outcome mapping, error mapping) into the new test files and delete the ones that assert hand-rolled wire JSON (rig owns that now). Full suite green at the end of this step.

- [ ] **Step 5: Commit**

```bash
git add crates/providers
git commit -m "feat(providers): route Anthropic invoke through rig"
```

---

### Task 6: Streaming bridge + Anthropic `invoke_stream` + prompt caching

**Files:**
- Create: `crates/providers/src/rig_adapter/stream.rs`
- Modify: `crates/providers/src/rig_adapter/model.rs`
- Test: extend `crates/providers/tests/rig_anthropic.rs` with SSE fixtures (port event sequences from the old `anthropic_tests.rs` streaming tests)

- [ ] **Step 1: Write failing streaming test**

Use wiremock with an SSE body (`ResponseTemplate::new(200).set_body_raw(SSE_BODY, "text/event-stream")`). Port the SSE fixture from the existing anthropic streaming tests — the event sequence (message_start, content_block_start tool_use, input_json_delta…, message_delta with usage, message_stop) is already written there.

```rust
#[tokio::test]
async fn anthropic_stream_emits_deltas_and_completes() {
    let server = MockServer::start().await;
    // SSE fixture: text deltas building "Hello", then submit_output tool_use block
    Mock::given(method("POST")).and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SSE_FIXTURE, "text/event-stream"))
        .mount(&server).await;

    let client = providers::create_provider(anthropic_test_config(&server.uri()));
    let sink = RecordingSink::default(); // Vec<AiStreamEvent> behind a Mutex
    let outcome = client.invoke_stream(test_request(), &sink).await.unwrap();

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, engine::AiStreamEvent::AssistantDelta { content } if content == "He")));
    assert!(matches!(outcome, engine::AgentTurnOutcome::Completed(_)));
}
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test -p providers --test rig_anthropic anthropic_stream`
Expected: FAIL.

- [ ] **Step 3: Implement `stream.rs`**

```rust
//! Drains a rig streaming response into an AiStreamSink and a final outcome.
use crate::rig_adapter::{error, outcome};
use engine::{AgentError, AgentTurnOutcome, AiStreamEvent, AiStreamSink};
use futures::StreamExt;
use rig::streaming::{StreamedAssistantContent, StreamingCompletionResponse};

pub async fn drain<R>(
    mut stream: StreamingCompletionResponse<R>,
    sink: &dyn AiStreamSink,
    provider_label: &str,
    output_schema: Option<&serde_json::Value>,
) -> Result<AgentTurnOutcome, AgentError>
where
    R: Clone + Unpin + serde::Serialize + serde::de::DeserializeOwned + rig::completion::GetTokenUsage,
{
    while let Some(item) = stream.next().await {
        match item.map_err(|e| error::to_agent_error(e, provider_label))? {
            StreamedAssistantContent::Text(text) => {
                sink.on_stream_event(AiStreamEvent::AssistantDelta { content: text.text });
            }
            StreamedAssistantContent::Reasoning(reasoning) => {
                sink.on_stream_event(AiStreamEvent::ThinkingDelta {
                    content: reasoning_text(reasoning),
                });
            }
            // Tool calls accumulate inside StreamingCompletionResponse; nothing to emit.
            _ => {}
        }
    }
    // After the stream ends, rig exposes the aggregated choice + usage.
    // Spike finding: exact accessors (stream.choice() / stream.response() / into_parts()).
    let (choice, usage) = final_parts(stream)?;
    outcome::resolve_outcome(choice, usage, provider_label, output_schema)
}
```

(rig's `StreamingCompletionResponse` aggregates assistant items — the `assistant_items` field seen in streaming.rs:360-380 — and implements `GetTokenUsage`. `final_parts` uses the real accessors from the spike. The old code's "no deltas after tool call starts" behavior comes free: rig emits tool-call deltas as a separate variant we ignore.)

Add `invoke_stream` to `RigModel`:

```rust
pub async fn invoke_stream(
    &self,
    request: &AgentRequest,
    sink: &dyn AiStreamSink,
    provider_label: &str,
) -> Result<AgentTurnOutcome, AgentError> {
    let completion_request = convert::to_completion_request(request);
    match self {
        Self::Anthropic(model) => {
            let stream = model.stream(completion_request).await
                .map_err(|e| error::to_agent_error(e, provider_label))?;
            crate::rig_adapter::stream::drain(stream, sink, provider_label, Some(&request.output_schema)).await
        }
    }
}
```

- [ ] **Step 4: Add Anthropic prompt-cache breakpoints**

Today: `cache_control` on the system block and on the second-to-last message (`prompt_cache.rs::second_to_last_index`). rig's Anthropic message type carries `cache_control: Option<CacheControl>` natively. In `convert.rs`, this can't be expressed on the generic `rig::message::Message` — so apply it Anthropic-side via `additional_params` if rig honors it there, OR mark the messages using whatever mechanism the spike found. If rig 0.39 offers no per-message cache-control hook from the generic request: **accept the regression on BP2, keep system-prompt caching only if reachable via `additional_params`, and record the decision in the findings table**. Do not fork rig for this.

Test (only if the hook exists): wiremock body assertion that `cache_control` appears in the outgoing request JSON at the expected positions.

- [ ] **Step 5: Run full providers suite**

Run: `cargo test -p providers`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/providers
git commit -m "feat(providers): stream Anthropic through rig with delta bridging"
```

---

### Task 7: OpenAI-compatible providers through rig

**Files:**
- Modify: `crates/providers/src/rig_adapter/model.rs`
- Test: `crates/providers/tests/rig_openai_compat.rs` (port behavior scenarios from `src/openai_compat.rs` tests / `tests/mock_factory.rs`)

- [ ] **Step 1: Write failing wiremock tests**

Cover, at minimum (port fixtures from existing tests):
- Chat Completions: submit_output tool call → Completed; external tool call batch; 429 → Transient; streaming deltas.
- `prompt_cache_key` present in request body for cloud providers, absent for `ollama`/`lmstudio` (reuse `prompt_cache::cache_session_key` + `openai_compat_cache_key_enabled`, injected via `additional_params`).
- `AuthConfig::Header` custom header name reaches the wire (wiremock `header(name, value)` matcher).
- Responses API (`WireApi::Responses`): one submit_output round-trip against rig's Responses path — exact fixture shape per spike findings.

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p providers --test rig_openai_compat`
Expected: FAIL.

- [ ] **Step 3: Implement OpenAI arms in `model.rs`**

```rust
// Added variants:
//   OpenAiChat(rig::providers::openai::CompletionModel)        — WireApi::ChatCompletions
//   OpenAiResponses(<responses model type per spike>)          — WireApi::Responses
```

Builder: `rig::providers::openai::Client::builder().api_key(..).base_url(&config.base_url)`, custom auth header via `headers_mut()` when `AuthConfig::Header` has a non-standard name (spike finding), and select chat-vs-responses model per the provider spec's `default_wire_api` (already resolved into `OpenAiCompatibleConfig` — check its fields in `src/openai_compat.rs` and carry the wire-api flag through).

`invoke`/`invoke_stream` arms are copies of the Anthropic arm shape, plus OpenAI-specific `additional_params`:

```rust
fn openai_additional_params(request: &AgentRequest, config: &OpenAiCompatibleConfig, provider_id: &ProviderId) -> Option<Value> {
    let mut params = serde_json::Map::new();
    if crate::prompt_cache::openai_compat_cache_key_enabled(provider_id) {
        params.insert("prompt_cache_key".into(), json!(crate::prompt_cache::cache_session_key(request)));
    }
    if let Some(effort) = &request.reasoning_effort {
        params.insert("reasoning_effort".into(), json!(effort));
    }
    if params.is_empty() { None } else { Some(Value::Object(params)) }
}
```

(rig merges `additional_params` into the request body JSON — confirmed pattern in rig provider impls.)

- [ ] **Step 4: Run tests, verify they pass; rewire `AiClient` OpenAI arm**

Run: `cargo test -p providers`
Expected: PASS with `AiClient::invoke`/`invoke_stream` OpenAI arms now calling `rig_adapter::model` and old `openai_compat::invoke*` no longer referenced.

- [ ] **Step 5: Commit**

```bash
git add crates/providers
git commit -m "feat(providers): route OpenAI-compatible providers through rig"
```

---

### Task 8: Bedrock through rig-bedrock

**Files:**
- Modify: `crates/providers/src/rig_adapter/model.rs`
- Modify: `crates/providers/src/aws_runtime.rs` (expose a fn returning `aws_sdk_bedrockruntime::Client`)
- Test: `crates/providers/tests/rig_bedrock.rs`

Bedrock has no HTTP mock in the current suite (SDK-based); mirror whatever test strategy `src/bedrock.rs` uses today (unit tests on mapping + optionally `aws-smithy-mocks` if already available). The heavy Converse mapping is rig-bedrock's problem now, so the test surface shrinks to: client construction resolves profile/region correctly, and outcome/error conversion (already covered by Tasks 3–4).

- [ ] **Step 1: Write failing construction test**

```rust
#[cfg(feature = "bedrock")]
#[tokio::test]
async fn bedrock_model_builds_with_profile_and_region() {
    let config = bedrock_test_config(); // region: "eu-west-1", profile: Some("dev")
    let model = providers_test_hooks::build_bedrock_model(&config, "anthropic.claude-sonnet-4").await;
    assert!(model.is_ok());
}
```

(Expose a `#[doc(hidden)] pub` test hook or make this an integration test through `create_provider` + an invoke that fails at the network layer with a classified error — pick whichever the existing bedrock tests do.)

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test -p providers --test rig_bedrock --features bedrock`
Expected: FAIL.

- [ ] **Step 3: Implement the Bedrock arm**

Reuse `aws_runtime.rs` to build the SDK config (it already handles `ensure_process_home_env` + profile resolution — SSO keeps working), then hand the SDK client to rig:

```rust
#[cfg(feature = "bedrock")]
async fn build_bedrock(config: &BedrockConfig, auth: &AuthConfig, model: &str) -> Result<RigModel, AgentError> {
    // aws_runtime already builds an SdkConfig honoring profile + region + SSO;
    // extract/reuse that path rather than duplicating it.
    let sdk_config = crate::aws_runtime::load_sdk_config(&config.region, config.aws_profile.as_deref()).await?;
    let sdk_client = aws_sdk_bedrockruntime::Client::new(&sdk_config);
    let client: rig_bedrock::client::Client = sdk_client.into(); // From impl
    Ok(RigModel::Bedrock(client.completion_model(model)))
}
```

(`load_sdk_config` may need to be factored out of the existing `bedrock.rs` client-construction code into `aws_runtime.rs` — move, don't rewrite.) `RigModel::invoke`/`invoke_stream` gain a `Bedrock` arm identical in shape to Anthropic's. Note `build_model` becomes `async` here — Bedrock client construction awaits config load; Anthropic/OpenAI arms are just not-async internally, fine.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p providers --features bedrock && cargo test -p providers --no-default-features`
Expected: PASS both (the second proves the feature gate still compiles without bedrock).

- [ ] **Step 5: Commit**

```bash
git add crates/providers
git commit -m "feat(providers): route Bedrock through rig-bedrock reusing SSO runtime"
```

---

### Task 9: Delete dead code, update snapshots, workspace green

**Files:**
- Delete: `crates/providers/src/sse.rs`, `crates/providers/src/anthropic.rs`, `crates/providers/src/anthropic_tests.rs`, `crates/providers/src/openai_compat.rs` (keep `OpenAiCompatibleConfig` — move it to `client.rs`), most of `crates/providers/src/bedrock.rs`
- Modify: `crates/providers/src/mapping.rs` (delete wire builders/parsers listed in "What dies"), `crates/providers/src/lib.rs`, `crates/providers/src/prompt_cache.rs` (delete Anthropic JSON-injection helpers if unused)
- Modify: `crates/engine/tests/snapshots/public_api.txt` (if engine-visible types changed — they shouldn't)

- [ ] **Step 1: Delete old wire modules and fix compilation**

Remove the files/functions listed above. `OpenAiCompatibleConfig` moves to `client.rs` (it's re-exported from `lib.rs` — public API must not change). Run `cargo check -p providers --all-features` and chase unused-import/dead-code errors (`warnings = "deny"` makes the compiler find every orphan for you).

- [ ] **Step 2: Confirm nothing outside providers noticed**

Run: `cargo check --workspace --all-features && cargo test --workspace`
Expected: PASS with zero changes required in orchestration/engine/desktop code. If a consumer breaks, the public API leaked — fix inside providers, do not touch consumers.

- [ ] **Step 3: Clippy + public API snapshot**

Run: `cargo clippy --workspace --all-targets --all-features` (this workspace denies warnings/unwrap/expect/panic — the new adapter code must already comply)
Run: whatever regenerates `crates/engine/tests/snapshots/public_api.txt` if it fails (check the test's own instructions; providers isn't snapshot-tracked but confirm).
Expected: clean.

- [ ] **Step 4: Line-count sanity check**

Run: `wc -l crates/providers/src/*.rs crates/providers/src/rig_adapter/*.rs`
Expected: net deletion on the order of 2,500–3,000 lines (anthropic 263 + openai_compat 764 + bedrock ~1000 + sse 246 + mapping wire half ~600, replaced by ~700 lines of adapter).

- [ ] **Step 5: Commit**

```bash
git add -A crates/providers crates/engine/tests/snapshots
git commit -m "refactor(providers): delete hand-rolled provider wire code superseded by rig"
```

---

### Task 10: End-to-end smoke test against a real provider

- [ ] **Step 1: Manual smoke run**

With a real API key configured (Anthropic or the user's usual local Ollama), run the desktop app / CLI flow that executes one workflow node with tools and one with streaming. Verify: streamed deltas render, tool round-trip works, final structured output lands, token usage appears.

- [ ] **Step 2: Report results**

Report pass/fail per provider to the user before merging. Bedrock smoke requires the user's AWS SSO session — ask them to run it.

---

## Spike findings (filled in during Task 1)

| Question | Finding |
|---|---|
| reqwest client injection / read timeout | _pending_ |
| Responses API module + model type | _pending_ |
| Custom header on openai builder | _pending_ |
| ToolChoice required variant | _pending_ |
| ProviderResponseError accessors | _pending_ |
| Anthropic per-message cache_control from generic request | _pending_ |
| rig Message/AssistantContent constructor shapes | _pending_ |
