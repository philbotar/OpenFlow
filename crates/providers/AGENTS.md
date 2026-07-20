---
description: Coding agent orientation for the providers crate
globs: crates/providers/**
alwaysApply: false
---

# AGENTS.md — Providers

**Question this crate answers:** How do we talk to OpenAI/Anthropic (and compatible APIs)?

Outbound adapter crate. Implements `engine::AiPort`. No orchestration, desktop, or UI code.

## Architecture

```
engine::AiPort (trait)
       ▲
       │ implements
┌──────┴──────┐
│  AiClient   │  client.rs — unified entry
├─────────────┤
│  mapping/   │  transcript + tool-arg wire shape
│  rig_adapter/  Rig 0.39 transport (OpenAI-compat, Anthropic, Bedrock)
│  auth.rs    │  key/header wiring
│  spec.rs    │  provider catalog metadata
└─────────────┘
       ▲
       │ create_provider()
  orchestration (Box<dyn AiPort>)
```

### Module map

| Path | Owns |
| --- | --- |
| `lib.rs` | `create_provider()` factory — **only public wiring entry** |
| `client.rs` | `AiClient: AiPort`, config types (`OpenAiCompatibleConfig`, `AnthropicConfig`, `BedrockConfig`) |
| `mapping/` | `AgentRequest` ↔ provider payloads; `jsonrepair-rs` for tool args |
| `rig_adapter/` | Rig model build, convert, stream, outcome, Anthropic HTTP extras |
| `auth.rs` | `AuthConfig`, header construction |
| `spec.rs` | Provider metadata, builtin specs, `ProviderId` |
| `prompt_cache.rs` | Prompt caching hints (Anthropic / OpenAI-compat keys) |
| `aws_runtime.rs` / `bedrock_*.rs` | Bedrock credentials, model list, errors (feature `bedrock`) |

Pre-Rig modules `openai_compat.rs`, `anthropic.rs`, and `sse.rs` are **gone**. Do not recreate them — extend `rig_adapter/` and `mapping/`.

## Dependency rules

**Allowed:** `engine`, HTTP client (`reqwest`), `serde`, `async-trait`, Rig

**Forbidden:**
- `orchestration`, `desktop`, `ui`
- Exposing concrete provider types to orchestration (use `Box<dyn AiPort>`)

Orchestration may import only allowlisted symbols from `providers` (factory + config). **`AiClient` is banned** in orchestration — call `create_provider()`.

## Code standards

1. **Adapter-only** — map engine types to wire format; no workflow or run lifecycle logic.
2. **Factory boundary** — new provider family → extend `ProviderAdapterConfig` + `rig_adapter/model.rs`, wire via `create_provider()`.
3. **No upward leaks** — orchestration never branches on `Anthropic` vs `OpenAI`; that logic stays here.
4. **Errors** — map HTTP/auth/parsing failures to `AgentError` (engine port vocabulary).
5. **Streaming** — emit `AiStreamEvent` through `AiStreamSink`; keep delta assembly testable.

## Patterns

### Where to add code

| Change | Location |
| --- | --- |
| New provider family / model build | `rig_adapter/model.rs` + `client.rs` config enum |
| Wire payload / tool-arg shape | `mapping/` + `rig_adapter/convert.rs` |
| Auth/header quirks | `auth.rs` or `rig_adapter/` |
| Provider catalog metadata | `spec.rs` |
| `AiPort` contract change | `engine/src/ports/outbound.rs` first, then `client.rs` |
| Reasoning / thinking blocks | `rig_adapter/claude_thinking.rs`, `reasoning_convert.rs` |

### Adding a provider

1. Add config variant / fields in `client.rs`.
2. Build Rig model in `rig_adapter/model.rs`.
3. Add `ProviderSpec` entry in `spec.rs`.
4. Cover with Wiremock tests under `crates/providers/tests/` (`rig_anthropic.rs`, `rig_openai_compat.rs`, …).
5. Live smoke: `STEP_WORKFLOW_LIVE_AI=1` tests in orchestration (not required in providers crate).

### Testing

| Pattern | When |
| --- | --- |
| Inline `#[cfg(test)] mod tests` | Default |
| `crates/providers/tests/rig_*.rs` | HTTP-level Wiremock suites |

```bash
cargo test -p providers
```

Test wire mapping and `jsonrepair` recovery. Avoid live network in unit tests.

## Change checklist

1. Does orchestration still depend only on `create_provider()` + config types?
2. Is provider-specific logic contained in this crate?
3. Do mapping / `rig_adapter` tests cover new fields and tool-call shapes?
4. Run `./scripts/verify.sh test clippy arch`.

## Related docs

- [`docs/architecture/provider-adapters.md`](../../docs/architecture/provider-adapters.md)
- [`docs/architecture/end-to-end-runtime.md`](../../docs/architecture/end-to-end-runtime.md) — when LLM calls sit in a run
- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`../../AGENTS.md`](../../AGENTS.md) — workspace map
