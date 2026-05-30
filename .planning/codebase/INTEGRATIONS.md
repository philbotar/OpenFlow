# External Integrations

**Analysis Date:** 2026-05-30

## APIs & External Services

**OpenAI API:**
- Purpose: Primary AI inference backend for agent workflow nodes
- Default endpoint: `https://api.openai.com`
- SDK/Client: Custom Rust adapter in `crates/openai-client/src/lib.rs` using `reqwest`
- Auth: Bearer token via `Authorization` header
- Supported wire APIs:
  - **Responses API** — `v1/responses` (default for OpenAI provider)
  - **Chat Completions API** — `v1/chat/completions` (default for OpenAI-compatible provider)
- Key implementation files:
  - `crates/openai-client/src/lib.rs` — HTTP client, endpoint normalization, response parsing
  - `crates/openai-client/src/lib.rs:186-213` — Responses API payload construction with `json_schema` format
  - `crates/openai-client/src/lib.rs:215-250` — Chat Completions API payload construction with `response_format`

**OpenAI-Compatible APIs:**
- Purpose: Support for third-party/OpenAI-compatible endpoints (e.g., Ollama, DeepInfra, local LLMs)
- Default endpoint: `http://localhost:11434` (suggesting Ollama default)
- Configuration: Custom base URL, transport type, and endpoint paths via `ProviderProfile`
- Auth: Same Bearer token pattern, env key `OPENAI_COMPATIBLE_API_KEY`

## Data Storage

**Databases:**
- None detected. No relational, document, or key-value database in use.

**Local File Storage:**
- Workflow persistence: JSON file via `serde_json`
  - Location: `dirs::data_local_dir()/step-through-agentic-workflow/workflows.json`
  - Implementation: `crates/agent-workflow-app/src/storage.rs`
  - Format: `{ "workflows": [...] }`
- Settings persistence: JSON file via `serde_json`
  - Location: `dirs::data_local_dir()/step-through-agentic-workflow/settings.json`
  - Implementation: `crates/agent-workflow-app/src/settings_store.rs`
  - Format: Provider profiles, model lists, active provider selection

**File Storage:**
- Local filesystem only (via `std::fs` and `dirs`)

**Caching:**
- None detected

## Authentication & Identity

**Auth Provider:**
- API-key based (custom, not OAuth or SSO)
- Two provider kinds defined in `crates/agent-workflow-app/src/settings_store.rs`:
  - `OpenAi` → env key `OPENAI_API_KEY`
  - `OpenAiCompatible` → env key `OPENAI_COMPATIBLE_API_KEY`
- API key precedence: UI settings value first, environment variable fallback
- No user accounts, session management, or identity provider integration

## Monitoring & Observability

**Error Tracking:**
- None detected. No Sentry, Rollbar, or similar service.

**Logs:**
- None detected. No structured logging framework (no `tracing`, `log`, or `slog`).
- Errors propagate via `thiserror`-derived enums and Rust `Result` types.

## CI/CD & Deployment

**Hosting:**
- Desktop-native binary; no hosted server or cloud deployment detected.

**CI Pipeline:**
- GitHub Actions: `.github/workflows/ci.yml`
  - Blocking job: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`
  - Non-blocking job: `cargo clippy-max` (pedantic + nursery + cargo lints)
  - Rust toolchain: `dtolnay/rust-toolchain@stable`
  - Build cache: `swatinem/rust-cache@v2`

## Environment Configuration

**Required env vars:**
- `OPENAI_API_KEY` — Primary OpenAI API key (optional if set via UI)
- `OPENAI_COMPATIBLE_API_KEY` — Compatible provider API key (optional if set via UI)

**Secrets location:**
- `.env` file present at repo root (not read for security)
- API keys may also be stored in the UI settings JSON file (local filesystem)

## Webhooks & Callbacks

**Incoming:**
- None detected. No HTTP server or webhook receiver endpoints.

**Outgoing:**
- None detected. No webhook callbacks registered with external services.

## Dependency Graph

```
agent-workflow-app
├── openai-client
│   └── workflow-core
└── workflow-core
```

- `workflow-core` — Pure domain logic, no external HTTP or filesystem dependencies
- `openai-client` — Depends on `workflow-core` for `AiPort` trait and DTOs
- `agent-workflow-app` — Depends on both internal crates plus egui/eframe stack

---

*Integration audit: 2026-05-30*
