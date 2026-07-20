# ChatGPT Codex OAuth Implementation Plan

**Goal:** Add a first-class `openai-codex` provider that signs in with a ChatGPT account, persists refreshable OAuth credentials in OpenFlow settings, and streams workflow runs through the ChatGPT Codex backend without an API key.

**Architecture:** Providers owns the OAuth protocol, credential refresh, and the `AiPort` adapter. The adapter reuses Rig 0.39's ChatGPT Responses request/stream mapping but creates a fresh model after resolving current credentials, so refresh and one-shot 401 recovery cannot use a stale token. Orchestration owns login-session state and credential persistence through a provider-defined sink. Desktop remains a thin IPC/browser-opening adapter, and UI receives only tagged login status—never tokens or account IDs.

**Tech Stack:** Rust workspace (`providers`, `orchestration`, `desktop`), Rig 0.39 ChatGPT Responses support, `reqwest`, Tokio loopback networking, PKCE S256, SolidJS/TypeScript, Wiremock, Rust unit/integration tests, and Vitest.

---

## Fixed MVP decisions

- Ship `openai-codex` in the provider catalog without a feature flag.
- Use browser PKCE at `http://localhost:1455/auth/callback`; fall back to device authorization only when port 1455 cannot be bound.
- Follow the current OpenAI Codex OAuth contract where it differs from the older oh-my-pi implementation: current connector scopes, form-encoded authorization-code exchange, JSON refresh, optional rotated-token fields, and JWT-derived expiry/account metadata.
- Keep the product decision to use device fallback instead of Codex CLI's registered port-1457 fallback.
- Reuse Rig's ChatGPT Responses transport. Do not copy oh-my-pi's multi-thousand-line Responses adapter.
- Use SSE only. Do not add WebSocket transport or the WebSocket-only beta header.
- Persist OAuth credentials in `settings.json`, matching the existing plaintext API-key policy. Redact them from IPC, logging, `Debug`, and errors.
- Use a nested `codex_oauth` settings value and preserve it across redacted settings saves.
- Refresh within five minutes of expiry and retry one unauthorized request after persisting rotated credentials.
- Use `originator: openflow`; treat private-backend compatibility as a documented, live-smoke requirement rather than impersonating the Codex CLI.
- Disconnect deletes all Codex OAuth credentials. Cancellation stops an active login without deleting an established session.

## Execution state

**Active slice:** 1 — Provider catalog and credential contract.

- [ ] 1. Add the provider catalog entry and secret-safe credential/config contract.
- [ ] 2. Implement browser PKCE, device fallback, token exchange, refresh, and JWT parsing.
- [ ] 3. Add the refreshable Rig-backed Codex `AiPort` path and request/stream contract tests.
- [ ] 4. Persist credentials, own login state in orchestration, and expose thin desktop IPC.
- [ ] 5. Add the settings UI, documentation, integration coverage, and canonical verification.

## Deferred follow-ups

- OS keychain storage.
- Importing `~/.codex/auth.json` or other external sessions.
- WebSocket transport, live model discovery, and Codex CLI thread-state parity.
- Automatic fallback to a second registered callback port.
- Other oh-my-pi OAuth providers.

