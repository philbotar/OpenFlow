---
name: openflow-provider-change
description: >-
  Procedural playbook for OpenFlow providers crate edits. Use when changing
  crates/providers/** — Rig adapters, mapping, auth, streaming, create_provider,
  Bedrock credentials, or tool-arg repair.
---

# openflow-provider-change

Procedural only. Architecture facts live in the docs below — do not invent a second model.

## Intake

1. Read `crates/providers/AGENTS.md`.
2. Read `docs/architecture/provider-adapters.md`.
3. For where LLM sits in a run: `docs/architecture/end-to-end-runtime.md`.
4. Confirm `docs/architecture/contract.md` if changing `AiPort`.

## Placement rules

- Adapter-only: no workflow or run lifecycle logic.
- Orchestration sees `create_provider()` + config types only — never concrete clients.
- Pre-Rig files `openai_compat.rs`, `anthropic.rs`, `sse.rs` are **deleted**. Extend `rig_adapter/` and `mapping/`.

## Where to edit

| Change | Path |
| --- | --- |
| Factory / public surface | `lib.rs`, `client.rs` |
| Transcript / tool args | `mapping/` |
| HTTP / stream / Rig models | `rig_adapter/` |
| Catalog metadata | `spec.rs` |
| Auth headers | `auth.rs` |
| Bedrock creds / model list | `aws_runtime.rs`, `bedrock_*.rs` |
| `AiPort` contract | `engine/src/ports/outbound.rs` first |

## Verify

```bash
./scripts/check-fast.sh providers
./scripts/verify/test-providers.sh
# If touching Bedrock / AWS:
./scripts/verify/test-providers-bedrock.sh
./scripts/verify.sh test-fast clippy arch
```

Wiremock suites: `crates/providers/tests/rig_*.rs`. Live AI only when intentional (`STEP_WORKFLOW_LIVE_AI=1`). `bedrock` feature is off by default.
