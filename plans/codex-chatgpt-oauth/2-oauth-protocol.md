# Slice 2: OAuth Protocol

## Goal

- Implement a cancellable, secret-safe OAuth client that supports browser PKCE on port 1455, device fallback when that port is occupied, current token refresh, and JWT metadata extraction.

## Current Question

- Question: None.
- Recommended answer: Preserve the product's port-busy device fallback while otherwise following the current official Codex OAuth behavior.
- Reason: The original plan's copied oh-my-pi refresh and scopes have drifted, but the desired UX explicitly chooses device fallback over the Codex CLI's second callback port.

## Codebase Findings

- Providers already depends on `reqwest`, `serde`, `sha2`, and Tokio, but needs direct URL-safe base64/randomness dependencies plus Tokio loopback I/O features.
- The current official browser flow validates state, uses PKCE S256, adds connector scopes, and exchanges an authorization code as form data.
- Current refresh is JSON, refresh response fields may be omitted, and expiry can be derived from JWT `exp` when `expires_in` is absent.
- Account ID is primarily read from the ID token's `https://api.openai.com/auth` claim and may also appear in access-token claims.
- Test command: `cargo test -p providers codex_oauth -- --nocapture`

## Ownership

- Create: `crates/providers/src/codex_oauth/mod.rs` for public login/refresh contracts and shared errors.
- Create: `crates/providers/src/codex_oauth/browser.rs` for PKCE, authorization URL, loopback callback, state validation, timeout, and cancellation.
- Create: `crates/providers/src/codex_oauth/device.rs` for user-code acquisition and polling semantics.
- Create: `crates/providers/src/codex_oauth/tokens.rs` for exchange, refresh, JWT parsing, and optional-field preservation.
- Modify: workspace/provider manifests and `Cargo.lock` for direct dependencies and Tokio features.
- Test: inline unit tests plus Wiremock tests under `crates/providers/tests/` for HTTP protocol behavior.

## Contract Detail

- Authorization uses client ID `app_EMoamEEZ73f0CkXaXp7hrann`, PKCE S256, a cryptographically random state, and scope `openid profile email offline_access api.connectors.read api.connectors.invoke`.
- The loopback listener binds only `127.0.0.1:1455`, accepts only `/auth/callback`, validates state before code exchange, returns a small success/failure page, and always closes on success, cancellation, timeout, or error.
- Only `AddrInUse` starts device authorization. Browser-open, callback, token, or entitlement errors remain browser-flow errors.
- Device polling treats the copied protocol's documented pending responses as pending, obeys the server interval/expiry, and supports cancellation.
- Refresh occurs through JSON `{ client_id, grant_type: "refresh_token", refresh_token }`; omitted refresh/access/ID-token fields preserve the prior value where valid.
- Errors and `Debug` output must not contain verifier, state, codes, tokens, callback query strings, or account IDs.

## Steps

- [ ] **Step 1: Write failing PKCE/JWT tests**
  - Prove verifier/challenge correctness, state mismatch rejection, account/email/expiry parsing, and secret-redacted errors.
- [ ] **Step 2: Write failing HTTP flow tests**
  - Prove exact authorize/token shapes, callback success/cancellation/timeout, occupied-port device fallback, pending polling, rotated refresh preservation, and terminal errors.
- [ ] **Step 3: Verify RED**
  - Run: `cargo test -p providers codex_oauth -- --nocapture`
  - Expected: FAIL because the OAuth module does not exist.
- [ ] **Step 4: Implement the minimal OAuth module**
  - Add secure randomness and PKCE helpers.
  - Add loopback and device flows behind injectable endpoint/port configuration for deterministic tests.
  - Add exchange, refresh, and JWT parsing with redacted diagnostics.
- [ ] **Step 5: Verify GREEN**
  - Run: `cargo test -p providers codex_oauth -- --nocapture`
  - Run: `cargo clippy -p providers --all-targets -- -D warnings`
  - Expected: PASS; no network access beyond local Wiremock fixtures.

## Maintainability Gate

- [ ] OAuth endpoints and client ID have one source of truth.
- [ ] Cryptographic values use OS-backed randomness.
- [ ] All listener exit paths release the port.
- [ ] Port conflicts are distinguishable from other login failures.
- [ ] Token-response optionality is covered by tests.

## Result

- Status: Pending
- Verification: Not run.

