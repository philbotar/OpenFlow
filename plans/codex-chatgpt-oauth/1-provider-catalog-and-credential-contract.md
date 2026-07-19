# Slice 1: Provider Catalog and Credential Contract

## Goal

- Introduce `openai-codex` as a non-API-key provider and establish the secret-safe types required by OAuth, settings, and inference without changing runtime behavior yet.

## Current Question

- Question: None.
- Recommended answer: Model Codex credentials as one nested value and add a provider-owned persistence trait; keep the catalog/auth metadata distinct from bearer API-key auth.
- Reason: Access tokens rotate during runs, while redacted settings snapshots and the provider catalog must never expose or overwrite them.

## Codebase Findings

- `ProviderSpec` currently distinguishes OpenAI-compatible, Anthropic, and Bedrock providers, while `AuthSpec` has no interactive-login variant.
- `ProviderAdapterConfig` and `AiClient` assume immutable credentials and derive equality/debug traits that cannot safely include a persistence trait object.
- Rig 0.39 already includes a ChatGPT Responses provider and current bundled model constants, so the catalog can use its supported current model names without adding a custom stream implementation.
- Test command: `cargo test -p providers spec codex -- --nocapture`

## Ownership

- Modify: `crates/providers/src/spec.rs` for the builtin provider, interactive auth metadata, models, and reasoning options.
- Modify: `crates/providers/src/auth.rs` for `CodexOAuthCredentials` and manual secret-redacted formatting.
- Modify: `crates/providers/src/client.rs` for `ProviderAdapterConfig::OpenAiCodex` and `CodexCredentialSink` configuration.
- Modify: `crates/providers/src/lib.rs` for the smallest public exports required by orchestration.
- Modify: `crates/workspace-checks/arch-check-rules.toml` only if the new provider configuration types require a narrow allowlist update.
- Test: inline provider catalog, serialization, and redaction tests.

## Contract Detail

- `CodexOAuthCredentials` stores access token, refresh token, optional ID token, expiry as Unix seconds, account ID, and optional email.
- Manual `Debug` shows presence and safe metadata only; it must not render token values or account ID.
- `CodexCredentialSink` persists a complete rotated credential set and is implemented outside providers.
- `openai-codex` uses an interactive ChatGPT auth spec, `https://chatgpt.com/backend-api/codex` as its provider base, a current static Codex model list, and the reasoning-effort options supported by the Responses path.
- `ProviderAdapterConfig::OpenAiCodex` carries credentials plus an optional shared sink. It is not represented as an API-key bearer config.

## Steps

- [ ] **Step 1: Write failing provider contract tests**
  - Prove the builtin catalog contains `openai-codex`, advertises interactive login, and exposes the selected current model set.
  - Prove serialized credentials round-trip while `Debug` omits sentinel token/account values.
  - Prove the Codex adapter config can carry a persistence sink without exposing it through formatting.
- [ ] **Step 2: Verify RED**
  - Run: `cargo test -p providers spec codex -- --nocapture`
  - Expected: FAIL because the provider, auth variant, and credential contract do not exist.
- [ ] **Step 3: Implement the minimal catalog and config contract**
  - Add the credential and sink types with secret-safe formatting.
  - Add the auth/provider spec and current static models.
  - Add the adapter-config variant without dispatching inference yet.
- [ ] **Step 4: Verify GREEN**
  - Run: `cargo test -p providers spec codex -- --nocapture`
  - Run: `cargo clippy -p providers --all-targets -- -D warnings`
  - Expected: PASS with no tokens in test diagnostics.
- [ ] **Step 5: Verify the public/layer seam**
  - Run: `./scripts/check-architecture.sh`
  - Expected: PASS; orchestration may consume the exported config later, while desktop and UI remain isolated from providers.

## Maintainability Gate

- [ ] OAuth credentials have one canonical type.
- [ ] No secret-bearing type uses derived `Debug`.
- [ ] Interactive auth is not confused with API-key bearer auth.
- [ ] Only symbols required across the providers/orchestration seam are public.

## Result

- Status: Pending
- Verification: Not run.

