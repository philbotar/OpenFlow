# Slice 4: Settings Persistence, Login State, and Desktop IPC

## Goal

- Make OAuth credentials durable and refreshable through orchestration, expose a cancellable login state machine, and keep desktop as a thin browser/IPC adapter.

## Current Question

- Question: None.
- Recommended answer: Use four tagged-status commands—start, status, cancel, and disconnect—and keep all credentials below the IPC boundary.
- Reason: Device fallback must publish a code while login is still running, cancellation is independent from disconnect, and returning credentials to UI would violate the settings redaction contract.

## Codebase Findings

- `SettingsStore::save` merges redacted API keys, while `save_raw` performs explicit secret deletion. OAuth must join that same preservation policy.
- Run and workflow-authoring paths resolve providers from borrowed settings stores; refresh persistence requires shareable `Arc<dyn SettingsStore>` ownership or an equivalent owned sink.
- Desktop is forbidden from importing providers and must call only `AppBackend`/orchestration.
- The shell plugin is already installed and can open the authorization URL from the desktop command.
- Test command: `cargo test -p orchestration settings codex -- --nocapture`

## Ownership

- Modify: `crates/orchestration/src/settings/model.rs` for nested OAuth storage, redaction, and renamed secret preservation.
- Modify: `crates/orchestration/src/settings/ports.rs`, `settings/facade.rs`, and backend dependency ownership for a shareable settings store.
- Create: `crates/orchestration/src/settings/codex_login.rs` for tagged state, cancellation, login task ownership, persistence, and disconnect.
- Modify: `crates/orchestration/src/settings/provider.rs` for readiness, Codex config resolution, and the settings-backed credential sink.
- Modify: run coordinator and workflow authoring call sites to attach the persistence sink.
- Modify: `crates/orchestration/src/backend/settings.rs` to expose login operations.
- Modify: `crates/desktop/src/commands/settings.rs` and `crates/desktop/src/lib.rs` for thin commands and browser opening.
- Test: settings model/store, provider resolution, login state, backend, and desktop command tests.

## Contract Detail

- `ProviderProfile.codex_oauth` is omitted from redacted settings payloads. `merge_preserved_secrets` preserves it only when an ordinary redacted settings save omits it.
- Login completion and refresh persistence use `save_raw` with a fully merged settings value; disconnect uses `save_raw` after clearing OAuth.
- `CodexLoginStatus` is a camelCase tagged enum: disconnected, starting, awaitingBrowser, awaitingDevice, connected, failed, and cancelled.
- Awaiting-device status may expose verification URL, user code, and expiry; connected may expose email. No status includes tokens or account ID.
- `start_codex_login` owns one active task, opens the authorization URL through a desktop-supplied callback, publishes intermediate status, and rejects or replaces duplicate starts deterministically.
- Readiness for `openai-codex` says to sign in when credentials are absent; refreshable expired credentials remain configured.

## Steps

- [ ] **Step 1: Write failing settings tests**
  - Prove raw round-trip, redaction, redacted-save preservation, disconnect deletion, missing-login readiness, and settings-backed refresh persistence.
- [ ] **Step 2: Write failing login/backend/desktop tests**
  - Prove every tagged state, browser/device progress, cancellation, failure, connected metadata, duplicate start behavior, and absence of secrets across serialized IPC.
- [ ] **Step 3: Verify RED**
  - Run: `cargo test -p orchestration settings codex -- --nocapture`
  - Run: `cargo test -p desktop codex -- --nocapture`
  - Expected: FAIL because storage, state, sink, and commands do not exist.
- [ ] **Step 4: Implement orchestration ownership and desktop adapters**
  - Convert settings-store ownership only as far as required for the sink.
  - Add storage/redaction/readiness, the login state machine, backend methods, and thin commands.
  - Open browser URLs in desktop without importing providers.
- [ ] **Step 5: Verify GREEN**
  - Run: `cargo test -p orchestration settings codex -- --nocapture`
  - Run: `cargo test -p orchestration --lib`
  - Run: `cargo test -p desktop codex -- --nocapture`
  - Run: `./scripts/check-architecture.sh`
  - Expected: PASS with the crate boundary unchanged.

## Maintainability Gate

- [ ] One orchestration owner controls active login state.
- [ ] Ordinary settings saves cannot erase or expose OAuth credentials.
- [ ] Disconnect and cancel have distinct behavior.
- [ ] Provider refresh persistence does not depend on desktop/UI lifetime.
- [ ] Desktop imports no provider types.

## Result

- Status: Pending
- Verification: Not run.

