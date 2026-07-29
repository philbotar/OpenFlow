# Troubleshooting

Use this page for setup, provider, run, and verification failures. Capture the exact error and reproduce through the narrowest command before changing code.

## ChatGPT Codex sign-in

| Symptom | Check |
| --- | --- |
| Device code appears instead of a browser callback | Another process owns loopback port `1455`. Complete the device flow or stop that process before retrying. |
| Browser opened but OpenFlow keeps waiting | Confirm the callback reached `http://localhost:1455/auth/callback`; local firewall/proxy tools must allow loopback traffic. Retry if the state/callback expired. |
| “Sign in with ChatGPT” after a previous connection | The refresh session is missing, invalidated, reused, or expired. Sign in again. |
| Workspace/entitlement rejection | The selected ChatGPT account or workspace does not have Codex access. Switch accounts or ask the workspace administrator. |
| HTTP 403 from the Codex backend | The private ChatGPT backend may have changed or rejected third-party `originator: openflow`. This is not fixed by an OpenAI API key; verify current official Codex behavior and the documented compatibility boundary. |
| Usage/rate-limit error | Check ChatGPT plan/workspace limits. ChatGPT subscription limits are separate from OpenAI API billing. |

OAuth credentials are plaintext in the local OpenFlow `settings.json`, matching stored provider API keys. Normal settings IPC and logs redact them. Use Settings → ChatGPT (Codex) → **Disconnect** to delete them.

## Provider not ready

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| Header shows API key missing or **Run** stays disabled | No resolvable key for the active API-key provider | Open **Settings → Providers**, select the active profile, add a key or set the provider env var (for example `OPENAI_API_KEY`), then save. |
| Readiness message on the authoring or run screen | Same as above, or the chosen model/profile is incomplete | Confirm the active provider row shows ready; switch profile or model if needed. |
| ChatGPT (Codex) not ready | No stored OAuth session or expired login | **Settings → Providers** with **ChatGPT (Codex)** active → **Sign in with ChatGPT**, or **Disconnect** and sign in again. See [ChatGPT Codex sign-in](#chatgpt-codex-sign-in). |
| Bedrock not ready | AWS credentials or region misconfigured | Set profile/region in the Bedrock provider panel and verify the credential chain. |

Key order for API-key providers: transient run input, stored `settings.json` key, then environment variable. Details: [`../reference/README.md#provider-key-resolution`](../reference/README.md#provider-key-resolution).

## Provider readiness

- API-key providers resolve transient input, then stored profile key, then their environment variable.
- ChatGPT (Codex) requires a stored refreshable ChatGPT login and ignores `OPENAI_API_KEY`.
- Bedrock uses the AWS credential chain and configured region/profile.

## Chat attachments

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| File stays pending with a size/type error | More than 4 files, one file over 10 MiB, total over 25 MiB, unsupported extension, empty file, or content/type mismatch | Remove the rejected file or export it as JPEG, PNG, GIF, WebP, PDF, TXT, Markdown, CSV, JSON, HTML, CSS, JavaScript, or Python, then retry. |
| Provider rejects an accepted image or document | The configured transport can serialize it, but the selected model/account does not accept that media | Switch to a media-capable model for that provider and retry. Custom models remain provider-dependent. |
| “missing or corrupt” attachment | The managed run copy is absent, changed, or fails its saved size/hash/type checks | Reattach the original file. OpenFlow rejects the request before provider HTTP. |
| Image card has no preview | The bounded local preview failed or its run data is unavailable | The message stays visible as a generic attachment card. Reattach only if the provider also reports an attachment error. |
| Chat deletion reports cleanup pending | Metadata deletion succeeded but run-directory cleanup could not finish | Restart or open run history to retry quarantined cleanup. Do not manually edit `chats.json`. |

## Focused verification

```bash
cargo nextest run -p providers
cargo nextest run -p orchestration --lib
cargo nextest run -p desktop
npm --prefix crates/ui run typecheck
./scripts/check-architecture.sh
```

Run `./scripts/verify.sh` for the canonical full gate. Provider fixture tests cannot prove a real account's Codex entitlement; record the interactive live smoke separately.

## Rust build disk usage

This repo disables Cargo incremental compilation because workspace feature/test variants previously
grew `target/debug/incremental` without a bound. Compiled dependencies and sccache remain reusable.

Scripted Cargo entrypoints refuse to start when `target/debug` exceeds 64 GiB or free space falls
below 24 GiB locally (8 GiB on GitHub Actions). Override the limits with
`OPENFLOW_MAX_DEBUG_CACHE_GIB` and `OPENFLOW_MIN_BUILD_SPACE_GIB`; use `0` only to disable the
corresponding guard intentionally.

Delete only the rebuildable debug cache when the guard trips:

```bash
./scripts/clean-rust-cache.sh --yes
```

The command preserves source files, Git state, release artifacts, `target/miri`, and cross-compiled
targets. The next Rust build is cold.
