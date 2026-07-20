---
name: openflow-finish-change
description: >-
  Final verification playbook before handing off OpenFlow work. Use when
  finishing a change, before commit, PR, or claiming done.
---

# openflow-finish-change

Procedural gate only. Do not claim done until the matching checks pass.

## Always

```bash
./scripts/verify.sh
```

Narrow while iterating; full gate before handoff. Fail-fast: `VERIFY_FAIL_FAST=1 ./scripts/verify.sh`.

## By risk

| If you touched… | Also run |
| --- | --- |
| `crates/engine/**` execution / ports | `cargo test -p engine`; architecture check |
| Run host / tools / checkpoints | `cargo test -p orchestration --test workflow_acceptance -- --nocapture` |
| `crates/providers/**` | `cargo test -p providers` |
| `crates/ui/**` | `npm --prefix crates/ui run typecheck` and focused Vitest |
| Layer / import boundaries | `./scripts/check-architecture.sh` |
| UB-sensitive Rust | `./scripts/verify.sh --deep` or `./scripts/miri.sh` |

Fast iteration:

```bash
./scripts/test-fast.sh --execution   # and/or --desktop
```

## Docs / skills drift

If you changed paths, ports, or runtime behavior: update the matching crate `AGENTS.md` and any stale table in root `AGENTS.md` / architecture docs in the same change.

## Handoff checklist

1. Lane skill followed (engine / orchestration / provider / ui)?
2. No ghost paths (`openai_compat.rs`, etc.)?
3. Verify (or scoped equivalent) green?
4. Acceptance tests if execution semantics changed?
