# App Sections

One folder per workspace section. Use each `README.md` to explain **what** that part of the app does and **why** it is structured that way.

## Filesystem

```text
sections/
├── README.md           # This index
├── domain/
├── providers/
├── orchestration/
├── desktop/
└── ui/
```

## Sections

| Folder | Crate | Start here in code |
| --- | --- | --- |
| [`domain/`](domain/) | `crates/domain` | `src/lib.rs`, `src/model.rs` |
| [`providers/`](providers/) | `crates/providers` | `src/lib.rs`, `src/client.rs` |
| [`orchestration/`](orchestration/) | `crates/orchestration` | `src/backend.rs`, `src/execution.rs` |
| [`desktop/`](desktop/) | `crates/desktop` | `src/lib.rs` |
| [`ui/`](ui/) | `crates/ui` | `src/App.tsx`, `src/context/` |

## Related

- [`../architecture/contract.md`](../architecture/contract.md) — layer dependency rules
- [`../contributing/coding-patterns.md`](../contributing/coding-patterns.md) — file ownership and conventions
- [`../../AGENTS.md`](../../AGENTS.md) — repo map
