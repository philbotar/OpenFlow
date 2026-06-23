# Documentation

Standards and architecture references for contributors and coding agents.

## Filesystem

```text
docs/
├── README.md                          # This index
├── FOLDER_STRUCTURE.md                # Normative hexagonal folder layout
├── glossary.md                        # Engine vocabulary (ubiquitous language)
├── contributing/
│   ├── README.md                      # How to change code in this repo
│   ├── coding-patterns.md             # Architecture rules and conventions
│   └── testing-workflows.md           # Acceptance and live-AI verification
├── sections/                          # What each part does and why (author-owned)
│   ├── README.md
│   ├── domain/
│   ├── providers/
│   ├── orchestration/
│   ├── desktop/
│   └── ui/
└── architecture/
    ├── README.md                      # Layer model and runtime design
    ├── contract.md                    # Layer responsibilities and dependency rules
    ├── threading-concurrency.md       # Async runtimes, threading, and I/O
    └── diagrams/
        ├── README.md                  # Mermaid diagram index
        ├── layers-current-vs-target.mmd
        └── layers-legacy-names.mmd    # Historical crate names (reference only)
```

## Read Order

1. [`AGENTS.md`](../AGENTS.md) — repo map, ownership, and common change paths.
2. [`contributing/coding-patterns.md`](contributing/coding-patterns.md) — architecture rules and implementation conventions.
3. [`contributing/testing-workflows.md`](contributing/testing-workflows.md) — workflow acceptance and live-AI verification.
4. [`glossary.md`](glossary.md) — domain terms (workflows, projects, callable agents, shared context).
5. [`architecture/contract.md`](architecture/contract.md) — layer boundaries when a change crosses crates.

## Sections

| Section | Index |
| --- | --- |
| Glossary | [`glossary.md`](glossary.md) |
| Folder layout | [`FOLDER_STRUCTURE.md`](FOLDER_STRUCTURE.md) |
| App sections (what & why) | [`sections/README.md`](sections/README.md) |
| Contributing | [`contributing/README.md`](contributing/README.md) |
| Architecture | [`architecture/README.md`](architecture/README.md) |

## Active Crates

| Crate | Question it answers | Agent orientation |
| --- | --- | --- |
| `crates/engine` | What is a valid workflow, and how does a run behave? | [`AGENTS.md`](../crates/engine/AGENTS.md) |
| `crates/orchestration` | How does the desktop app store, load, wire, and host runs? | [`AGENTS.md`](../crates/orchestration/AGENTS.md) |
| `crates/providers` | How do we talk to OpenAI/Anthropic? | [`AGENTS.md`](../crates/providers/AGENTS.md) |
| `crates/ui` / `crates/desktop` | How does the user interact? | [`ui/AGENTS.md`](../crates/ui/AGENTS.md), [`desktop/AGENTS.md`](../crates/desktop/AGENTS.md) |

See [`architecture/contract.md`](architecture/contract.md) for layer boundaries and dependency rules.

## Dev Entry Points

- Desktop app: `npm --prefix crates/desktop run start -- dev`
- Frontend only: `npm --prefix crates/ui run dev`
- Frontend typecheck: `npm --prefix crates/ui run typecheck`
- Full verify gate: `./scripts/verify.sh`
- Miri (UB): `./scripts/miri.sh` — see [`contributing/testing-workflows.md`](contributing/testing-workflows.md#miri)

## Scope

- These docs define how we change code in this repository.
- If code and docs diverge, update docs in the same change set.
- Keep docs explicit and scan-friendly; prefer concrete file paths and exact token values.
