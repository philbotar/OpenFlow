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

## Provider readiness

- API-key providers resolve transient input, then stored profile key, then their environment variable.
- ChatGPT (Codex) requires a stored refreshable ChatGPT login and ignores `OPENAI_API_KEY`.
- Bedrock uses the AWS credential chain and configured region/profile.

## Focused verification

```bash
cargo test -p providers
cargo test -p orchestration --lib
cargo test -p desktop
npm --prefix crates/ui run typecheck
./scripts/check-architecture.sh
```

Run `./scripts/verify.sh` for the canonical full gate. Provider fixture tests cannot prove a real account's Codex entitlement; record the interactive live smoke separately.
