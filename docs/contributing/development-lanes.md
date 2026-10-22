# Development lanes

Choose the row matching the behavior you are changing. Area `AGENTS.md` files define local constraints; the [architecture contract](../architecture/contract.md) defines layer rules.

## Lanes

| Change | Owner / guidance | Relevant verification |
| --- | --- | --- |
| Workflow model, validation, execution semantics, ports | [Engine](../../crates/engine/AGENTS.md) | Focused engine tests; workflow acceptance for execution; public API check for public contracts |
| Run hosting, active retry, checkpoint/replay, execution cwd | [Orchestration](../../crates/orchestration/AGENTS.md), [runtime](../architecture/end-to-end-runtime.md) | Orchestration tests and workflow acceptance |
| Catalogs, settings, storage, tools I/O | [Orchestration](../../crates/orchestration/AGENTS.md) | Focused service/adapter tests; compatibility coverage for saved data |
| Provider transport, auth, streaming, mapping | [Providers](../../crates/providers/AGENTS.md) | Provider tests; Bedrock lane for AWS changes |
| Tauri handlers, event bridge, bootstrap | [Desktop](../../crates/desktop/AGENTS.md) | Desktop command/event coverage |
| UI state, interactions, DTOs, invoke wrappers | [UI](../../crates/ui/AGENTS.md) | Typecheck and focused Vitest; native coverage for IPC changes |
| Cross-layer behavior | [Runtime](../architecture/end-to-end-runtime.md), [testing](testing-workflows.md) | Full gate plus desktop/Bedrock lanes when affected |
| Docs and agent instructions only | [Docs](../AGENTS.md) | Changed links, anchors, paths, command syntax, examples, and diff |

For async ownership consult [threading](../architecture/threading-concurrency.md); for uncertain terminology consult the [glossary](../glossary.md). Routine edits do not require a full architecture review.

## Skills

Use a skill when its guidance helps with the change. A file's location alone does not determine which skill applies; editing crate documentation does not require an implementation skill.

| Skill | Use when |
| --- | --- |
| [openflow-engine-change](../../.cursor/skills/openflow-engine-change/SKILL.md) | Changing execution semantics, validation, or port contracts |
| [openflow-orchestration-change](../../.cursor/skills/openflow-orchestration-change/SKILL.md) | Changing run hosting, persistence, or backend wiring |
| [openflow-provider-change](../../.cursor/skills/openflow-provider-change/SKILL.md) | Changing provider transport, auth, streaming, or mapping |
| [openflow-ui-change](../../.cursor/skills/openflow-ui-change/SKILL.md) | Changing UI interaction/state or desktop seam integration |
| [rust-hexarc-organizer](../../.cursor/skills/rust-hexarc-organizer/SKILL.md) | Resolving uncertain ownership or dependency violations |
| [openflow-finish-change](../../.cursor/skills/openflow-finish-change/SKILL.md) | Assessing readiness or selecting missing verification |
| [product-docs-writer](../../.cursor/skills/product-docs-writer/SKILL.md) | Explicitly requested product documentation workflow |
| [openflow-deploy-ready](../../.cursor/skills/openflow-deploy-ready/SKILL.md) | Explicit release preparation/execution; completion stops at draft Release |

## Verification

Local edits, builds, linting, and tests needed for the task may proceed without per-step approval. Use temporary persistence fixtures. Live-provider smoke is separate: it can use saved credentials and incur charges, so it needs authorization.

| Purpose | Command |
| --- | --- |
| Rust compile / lint | `./scripts/check-fast.sh <crate>` / `./scripts/check-fast.sh --clippy <crate>` |
| Engine | `./scripts/verify/test-engine.sh` |
| Providers | `./scripts/verify/test-providers.sh` |
| Orchestration | `./scripts/verify/test-orchestration-lib.sh` |
| Execution acceptance | `./scripts/test-fast.sh --execution` |
| Desktop | `./scripts/test-fast.sh --desktop` |
| Bedrock / AWS | `./scripts/verify/test-providers-bedrock.sh` |
| UI types | `npm --prefix crates/ui run typecheck` |
| Focused UI test | `npm --prefix crates/ui run test -- src/api.test.ts` (replace test path) |
| Changed layer boundaries | `./scripts/check-architecture.sh` |
| Changed engine public contract | `./scripts/check-engine-public-api.sh` |
| Code PR readiness, release, cross-layer change, or requested full gate | `./scripts/verify.sh` |
| Full workspace Rust only | `./scripts/verify.sh test` |

Docs/instruction-only changes do not need Rust/UI suites unless they affect executable examples or test/build behavior. Choose focused checks for local changes. Do not repeat passing coverage without new changes, failures, or unresolved concerns.

The default full gate includes workflow acceptance through `test-fast --execution`; no duplicate standalone acceptance run is needed. It excludes desktop/Tauri and Bedrock-specific coverage. Add those lanes when affected. `--deep` adds optional mutation testing. See [testing workflows](testing-workflows.md) for test placement, granular commands, and environment constraints.

## Completion

Verify the behavior at the code or UI that consumes it, fix failures caused by the change, and report checks and remaining limitations. Reuse results from checks run against unchanged code; report pending CI and blocked checks as such.

Stop at a requested review point, a decision that needs user input, or an external blocker, and state the exact next action. Completing verification does not authorize publication.
