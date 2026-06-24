# Architecture

Layer model, dependency rules, and runtime concurrency design.

## Filesystem

```text
architecture/
├── README.md                    # This index
├── technical-overview.md        # End-to-end overview with diagrams: context, caching, nodes/flow, harness design
├── contract.md                  # Layer responsibilities and dependency rules (source of truth)
├── orchestration-layout.md      # Orchestration module map and change paths
├── callable-agents.md           # Saved-agent snapshot and subagent model
├── provider-adapters.md         # Provider adapter families and Bedrock notes
├── threading-concurrency.md     # Tokio runtimes, async tasks, blocking I/O risks
├── run-persistence.md           # Durable run storage, checkpoints, replay vs resume
└── diagrams/
    ├── README.md
    ├── layers-current-vs-target.mmd
    └── layers-legacy-names.mmd
```

## Read order

1. [`technical-overview.md`](technical-overview.md) - big-picture tour: layers, node execution, context assembly, tool-result caching, harness design.
2. [`contract.md`](contract.md) - allowed/forbidden dependencies, port rules, change checklist.
3. [`orchestration-layout.md`](orchestration-layout.md) - orchestration module map, service/storage split, and change paths.
4. [`callable-agents.md`](callable-agents.md) - saved-agent snapshot model and subagent lifecycle.
5. [`provider-adapters.md`](provider-adapters.md) - provider adapter families and Bedrock setup notes.
6. [`diagrams/layers-current-vs-target.mmd`](diagrams/layers-current-vs-target.mmd) - visual current vs target seams.
7. [`threading-concurrency.md`](threading-concurrency.md) - when changing run lifecycle, I/O, or parallelism.
8. [`run-persistence.md`](run-persistence.md) - durable run records, checkpoints, replay, and resume after restart.

## Layer stack

```text
UI (crates/ui)
  → Desktop adapter (crates/desktop)
    → Orchestration (crates/orchestration)
      → Engine (crates/engine)
      → Provider adapters (crates/providers) via AiPort
```

Orchestration owns runtime execution state and `ToolPortImpl`. Engine owns the workflow state machine and ports. Provider adapters own wire transport.

## Related

- [`../contributing/coding-patterns.md`](../contributing/coding-patterns.md) - file-level ownership and runtime semantics.
- [`../../AGENTS.md`](../../AGENTS.md) - repo map with primary change paths.
- [`../../crates/workspace-checks/arch-check-rules.toml`](../../crates/workspace-checks/arch-check-rules.toml) - machine-readable architecture check config.
