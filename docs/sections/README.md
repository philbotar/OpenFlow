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

| Folder | Crate | Agent orientation | Start here in code |
| --- | --- | --- | --- |
| [`domain/`](domain/) | `crates/engine` | [`AGENTS.md`](../../crates/engine/AGENTS.md) | `src/lib.rs`, `src/graph/` |
| [`providers/`](providers/) | `crates/providers` | [`AGENTS.md`](../../crates/providers/AGENTS.md) | `src/lib.rs`, `src/client.rs` |
| [`orchestration/`](orchestration/) | `crates/orchestration` | [`AGENTS.md`](../../crates/orchestration/AGENTS.md) | `src/backend/mod.rs`, [`layout.md`](orchestration/layout.md) |
| [`desktop/`](desktop/) | `crates/desktop` | [`AGENTS.md`](../../crates/desktop/AGENTS.md) | `src/lib.rs` |
| [`ui/`](ui/) | `crates/ui` | [`AGENTS.md`](../../crates/ui/AGENTS.md) | `src/App.tsx`, `src/context/` |

## Related

- [`../architecture/contract.md`](../architecture/contract.md) — layer dependency rules
- [`../contributing/coding-patterns.md`](../contributing/coding-patterns.md) — file ownership and conventions
- [`../../AGENTS.md`](../../AGENTS.md) — repo map
