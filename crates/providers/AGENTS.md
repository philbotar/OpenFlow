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
│  mapping.rs │  transcript + tool-arg wire shape
│  openai_compat.rs
│  anthropic.rs
│  bedrock.rs / bedrock_models.rs
│  sse.rs     │  stream parsing
│  auth.rs    │  key/header wiring
└─────────────┘
       ▲
       │ create_provider()
  orchestration (Box<dyn AiPort>)
```

### Module map

| Path | Owns |
| --- | --- |
| `lib.rs` | `create_provider()` factory — **only public wiring entry** |
| `client.rs` | `AiClient: AiPort`, config types |
| `mapping.rs` | `AgentRequest` ↔ provider payloads; `jsonrepair-rs` for tool args |
| `openai_compat.rs` | OpenAI-compatible HTTP transport |
| `anthropic.rs` | Anthropic Messages API transport |
| `sse.rs` | Server-sent event parsing |
| `auth.rs` | `AuthConfig`, header construction |
| `spec.rs` | Provider metadata, builtin specs, `ProviderId` |
| `prompt_cache.rs` | Prompt caching hints (Anthropic) |

## Dependency rules

**Allowed:** `engine`, HTTP client (`reqwest`), `serde`, `async-trait`

**Forbidden:**
- `orchestration`, `desktop`, `ui`
- Exposing concrete provider types to orchestration (use `Box<dyn AiPort>`)

Orchestration may import only allowlisted symbols from `providers` (factory + config). **`AiClient` is banned** in orchestration — call `create_provider()`.

## Code standards

1. **Adapter-only** — map engine types to wire format; no workflow or run lifecycle logic.
2. **Factory boundary** — new provider → add `providers/src/{name}.rs`, wire in `create_provider()`.
3. **No upward leaks** — orchestration never branches on `Anthropic` vs `OpenAI`; that logic stays here.
4. **Errors** — map HTTP/auth/parsing failures to `AgentError` (engine port vocabulary).
5. **Streaming** — emit `AiStreamEvent` through `AiStreamSink`; keep delta assembly testable.

## Patterns

### Where to add code

| Change | Location |
| --- | --- |
| New provider adapter | `providers/src/{name}.rs` + `lib.rs` factory |
| Wire payload shape | `mapping.rs` + provider module |
| Auth/header quirks | `auth.rs` or provider module |
| Provider catalog metadata | `spec.rs` |
| `AiPort` contract change | `engine/src/ports/outbound.rs` first, then `client.rs` |

### Adding a provider

1. Implement transport in new module (HTTP + SSE if streaming).
2. Add `ProviderSpec` entry in `spec.rs`.
3. Branch in `AiClient` / `create_provider()` config dispatch.
4. Add mapping tests for request/response round-trips.
5. Live smoke: `STEP_WORKFLOW_LIVE_AI=1` tests in orchestration (not required in providers crate).

### Testing

| Pattern | When |
| --- | --- |
| Inline `#[cfg(test)] mod tests` | Default |
| Sibling `anthropic_tests.rs` | Large provider-specific suites |

```bash
cargo test -p providers
```

Test wire mapping and `jsonrepair` recovery. Avoid live network in unit tests.

## Change checklist

1. Does orchestration still depend only on `create_provider()` + config types?
2. Is provider-specific logic contained in this crate?
3. Do mapping tests cover new fields and tool-call shapes?
4. Run `./scripts/verify.sh test clippy arch`.

## Related docs

- [`docs/sections/providers/README.md`](../../docs/sections/providers/README.md)
- [`docs/architecture/contract.md`](../../docs/architecture/contract.md)
- [`../../AGENTS.md`](../../AGENTS.md) — workspace map
