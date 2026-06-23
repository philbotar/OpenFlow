# Architecture

Layer model, dependency rules, and runtime concurrency design.

## Filesystem

```text
architecture/
├── README.md                    # This index
├── technical-overview.md        # End-to-end overview with diagrams: context, caching, nodes/flow, harness design
├── contract.md                  # Layer responsibilities and dependency rules (source of truth)
├── arch-check-rules.toml        # CI architecture rules (consumed by scripts/check-architecture.sh)
├── threading-concurrency.md     # Tokio runtimes, async tasks, blocking I/O risks
├── run-persistence.md           # Durable run storage, checkpoints, replay vs resume
└── diagrams/
    ├── README.md
    ├── layers-current-vs-target.mmd
    └── layers-legacy-names.mmd
```

## Read Order

1. [`technical-overview.md`](technical-overview.md) — big-picture tour: layers, node execution, context assembly, tool-result caching, harness design.
2. [`contract.md`](contract.md) — allowed/forbidden dependencies, port rules, change checklist.
3. [`arch-check-rules.toml`](arch-check-rules.toml) — Tier 2 CI rules (Cargo graph + forbidden imports).
4. [`diagrams/layers-current-vs-target.mmd`](diagrams/layers-current-vs-target.mmd) — visual current vs target seams.
5. [`threading-concurrency.md`](threading-concurrency.md) — when changing run lifecycle, I/O, or parallelism.
6. [`run-persistence.md`](run-persistence.md) — durable run records, checkpoints, replay, and resume after restart.

## Layer Stack

```text
UI (crates/ui)
  → Desktop adapter (crates/desktop)
    → Orchestration (crates/orchestration)
      → Engine (crates/engine)
      → Provider adapters (crates/providers) via AiPort
```

Orchestration owns runtime execution state and `ToolPortImpl`. Engine owns the workflow state machine and ports. Provider adapters own wire transport.

## Related

- [`../contributing/coding-patterns.md`](../contributing/coding-patterns.md) — file-level ownership and runtime semantics.
- [`../../AGENTS.md`](../../AGENTS.md) — repo map with primary change paths.
