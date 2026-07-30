# Reference

Use this page for commands, storage paths, and operational facts that should be easy to look up.

Agent and harness tools (built-ins, MCP, authoring): [`tools.md`](tools.md).

## Dev Commands

| Goal | Command |
| --- | --- |
| Full desktop app | `./scripts/start.sh` |
| Install desktop app (macOS) | `./scripts/install.sh` |
| Frontend only | `npm --prefix crates/ui run dev` |
| Frontend typecheck | `npm --prefix crates/ui run typecheck` |
| Fast Rust lane | `./scripts/test-fast.sh` |
| Fast lane with execution tests | `./scripts/test-fast.sh --execution` |
| Full verification gate | `./scripts/verify.sh` (default Rust tests = `test-fast`; desktop via `./scripts/verify.sh test`) |
| Leg timing | `./scripts/bench-test-legs.sh` |
| Architecture checks only | `./scripts/check-architecture.sh` |
| Workflow acceptance | `cargo nextest run -p orchestration --test workflow_acceptance --no-capture` |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 STEP_WORKFLOW_LIVE_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo nextest run -p orchestration --test live_workflow --run-ignored ignored-only --no-capture` |

## Runtime and Persistence Paths

| Data | Path |
| --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` |
| Chats | `{data_local}/openflow/chats.json` |
| Settings | `{data_local}/openflow/settings.json` |
| Projects | `{data_local}/openflow/projects.json` |
| Saved agents | `{data_local}/openflow/agents.json` |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` |
| App-managed execution folders | `{data_local}/openflow/workspaces/{workflows|chats}/{id}` |
| App-run attachments | `{data_local}/openflow/runs/{runId}/attachments/{attachmentId}.{ext}` |
| Project-run attachments | `{project}/.flow/runs/{runId}/attachments/{attachmentId}.{ext}` |
| App-run node handoffs | `{data_local}/openflow/runs/{runId}/handoffs/{nodeId}/HANDOFF.md` or `HANDOFF.json` |
| Project-run node handoffs | `{project}/.flow/runs/{runId}/handoffs/{nodeId}/HANDOFF.md` or `HANDOFF.json` |
| Provider API keys | Plaintext in `settings.json` as `ProviderProfile.api_key` |
| Web search API keys | Plaintext in `settings.json` under `search.keys` (per search-cli provider id) |
| ChatGPT Codex OAuth | Plaintext in `settings.json` as `ProviderProfile.codex_oauth`; redacted from normal settings IPC |

`AppBackend::load_all_workflows` merges app-store and project-discovered workflows. Project files win on ID collision.
Run start accepts a project ID, then the backend resolves the cwd and run root. Project runs use
the exact selected project; app workflow/chat runs use the isolated managed folders above.
OpenFlow does not use a global `~/.flow`; `.flow` stays project-local.

Chat checkpoints persist attachment refs only: generated ID, sanitized filename, media type, byte
size, SHA-256, and kind. They never persist source paths or inline attachment bytes. The
orchestration layer verifies and hydrates managed bytes immediately before provider invocation.
Browser paste/drop staging uses opaque tokens under app data; successful sends or explicit removal
clean staged copies, with 24-hour stale cleanup as fallback.

## Provider Key Resolution

OpenFlow resolves provider keys in this order:

1. Transient input panel key.
2. Stored settings key: `ProviderProfile.api_key`.
3. Provider environment variable fallback, such as `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`.

Managed cloud providers reject readiness when their required key cannot be resolved. `ollama`, `lmstudio`, and `custom_openai_compatible` can resolve without a key. Custom endpoints still receive a configured key when one exists.

Provider-specific request mapping belongs in `crates/providers`. Settings, readiness, and app-level resolution live in orchestration.

The `openai-codex` provider does not consult `OPENAI_API_KEY`. It is ready when a refreshable ChatGPT OAuth session is stored. **Disconnect** explicitly clears that session; ordinary redacted settings saves preserve it.

## Architecture Check Config

Architecture checks run through:

```bash
./scripts/check-architecture.sh
```

The machine-readable rule file is [`../../crates/workspace-checks/arch-check-rules.toml`](../../crates/workspace-checks/arch-check-rules.toml). It is kept with the workspace-check tooling instead of the docs tree.

Human-readable architecture rules live in [`../architecture/contract.md`](../architecture/contract.md).
