# Architecture

Layer model, dependency rules, and runtime concurrency design.

## Filesystem

```text
architecture/
├── README.md                    # This index
├── contract.md                  # Layer responsibilities and dependency rules (source of truth)
├── arch-check-rules.toml        # CI architecture rules (consumed by scripts/check-architecture.sh)
├── threading-concurrency.md     # Tokio runtimes, async tasks, blocking I/O risks
└── diagrams/
    ├── README.md
    ├── layers-current-vs-target.mmd
    └── layers-legacy-names.mmd
```

## Read Order

1. [`contract.md`](contract.md) — allowed/forbidden dependencies, port rules, change checklist.
2. [`arch-check-rules.toml`](arch-check-rules.toml) — Tier 2 CI rules (Cargo graph + forbidden imports).
3. [`diagrams/layers-current-vs-target.mmd`](diagrams/layers-current-vs-target.mmd) — visual current vs target seams.
4. [`threading-concurrency.md`](threading-concurrency.md) — when changing run lifecycle, I/O, or parallelism.

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
