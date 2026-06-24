# Reference

Use this page for commands, storage paths, and operational facts that should be easy to look up.

## Dev Commands

| Goal | Command |
| --- | --- |
| Full desktop app | `npm --prefix crates/desktop run start -- dev` |
| Frontend only | `npm --prefix crates/ui run dev` |
| Frontend typecheck | `npm --prefix crates/ui run typecheck` |
| Fast Rust lane | `./scripts/test-fast.sh` |
| Fast lane with execution tests | `./scripts/test-fast.sh --execution` |
| Full verification gate | `./scripts/verify.sh` |
| Architecture checks only | `./scripts/check-architecture.sh` |
| Workflow acceptance | `cargo test -p orchestration --test workflow_acceptance -- --nocapture` |
| Live AI smoke | `STEP_WORKFLOW_LIVE_AI=1 STEP_WORKFLOW_LIVE_API_KEY=... STEP_WORKFLOW_LIVE_MODEL=... cargo test -p orchestration --test live_workflow -- --ignored --nocapture` |

## Runtime and Persistence Paths

| Data | Path |
| --- | --- |
| App workflows | `{data_local}/openflow/workflows.json` |
| Settings | `{data_local}/openflow/settings.json` |
| Projects | `{data_local}/openflow/projects.json` |
| Saved agents | `{data_local}/openflow/agents.json` |
| Node templates | `{data_local}/openflow/templates.json` |
| Project workflows | `{project}/.flow/workflows/{workflowId}.workflow.json` |
| Provider API keys | Plaintext in `settings.json` as `ProviderProfile.api_key` |

`AppBackend::load_all_workflows` merges app-store and project-discovered workflows. Project files win on ID collision.

## Provider Key Resolution

OpenFlow resolves provider keys in this order:

1. Transient input panel key.
2. Stored settings key: `ProviderProfile.api_key`.
3. Provider environment variable fallback, such as `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`.

Provider-specific request mapping belongs in `crates/providers`. Settings, readiness, and app-level resolution live in orchestration.

## Architecture Check Config

Architecture checks run through:

```bash
./scripts/check-architecture.sh
```

The machine-readable rule file is [`../../crates/workspace-checks/arch-check-rules.toml`](../../crates/workspace-checks/arch-check-rules.toml). It is kept with the workspace-check tooling instead of the docs tree.

Human-readable architecture rules live in [`../architecture/contract.md`](../architecture/contract.md).
