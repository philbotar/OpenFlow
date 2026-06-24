# Documentation

OpenFlow docs are split by reader intent. Start with the product walkthroughs when you want to use the app, then move into architecture and contributing docs when you are changing code.

## Filesystem

```text
docs/
├── README.md                          # This index
├── getting-started/
│   └── README.md                      # Install, run, configure a provider, start a workflow
├── guides/
│   └── first-workflow.md              # End-to-end workflow walkthrough
├── concepts/
│   ├── README.md                      # Product vocabulary and mental model
│   ├── how-openflow-works.md          # Runtime overview for users and contributors
│   └── workflows-and-runs.md          # Workflows, nodes, runs, tools, approvals
├── reference/
│   └── README.md                      # Commands, storage paths, provider key resolution
├── troubleshooting/
│   └── README.md                      # Common setup, provider, run, and verification failures
├── glossary.md                        # Engine vocabulary
├── FOLDER_STRUCTURE.md                # Normative hexagonal folder layout
├── contributing/
│   ├── README.md                      # How to change code in this repo
│   ├── development-lanes.md           # Change classification and verification lanes
│   ├── coding-patterns.md             # Architecture rules and conventions
│   └── testing-workflows.md           # Acceptance and live-AI verification
└── architecture/
    ├── README.md                      # Maintainer architecture index
    ├── technical-overview.md          # End-to-end runtime overview
    ├── contract.md                    # Layer responsibilities and dependency rules
    ├── orchestration-layout.md        # Orchestration module map and change paths
    ├── callable-agents.md             # Saved-agent snapshot and subagent model
    ├── provider-adapters.md           # Provider adapter families and Bedrock notes
    ├── run-persistence.md             # Durable run records, checkpoints, replay, resume
    ├── threading-concurrency.md       # Async runtimes, threading, and I/O
    └── diagrams/
        ├── README.md                  # Mermaid diagram index
        ├── layers-current-vs-target.mmd
        └── layers-legacy-names.mmd    # Historical crate names (reference only)
```

Architecture check rules are machine configuration, not documentation. They live at [`../crates/workspace-checks/arch-check-rules.toml`](../crates/workspace-checks/arch-check-rules.toml) and are consumed by [`../scripts/check-architecture.sh`](../scripts/check-architecture.sh).

## Read Order

1. [`getting-started/README.md`](getting-started/README.md) - run OpenFlow locally and configure a provider.
2. [`guides/first-workflow.md`](guides/first-workflow.md) - build and run a small workflow.
3. [`concepts/how-openflow-works.md`](concepts/how-openflow-works.md) - understand the engine, orchestration host, providers, tools, and UI.
4. [`concepts/workflows-and-runs.md`](concepts/workflows-and-runs.md) - learn workflow and run vocabulary.
5. [`reference/README.md`](reference/README.md) - commands, persistence paths, provider key rules.
6. [`troubleshooting/README.md`](troubleshooting/README.md) - fix common local setup and run failures.
7. [`architecture/contract.md`](architecture/contract.md) - read before changing layer boundaries.
8. [`contributing/development-lanes.md`](contributing/development-lanes.md) - classify code changes and choose verification.

## Doc Map

| Goal | Read |
| --- | --- |
| Run the app | [`getting-started/README.md`](getting-started/README.md) |
| Build your first workflow | [`guides/first-workflow.md`](guides/first-workflow.md) |
| Understand the runtime | [`concepts/how-openflow-works.md`](concepts/how-openflow-works.md) |
| Learn workflow terms | [`concepts/workflows-and-runs.md`](concepts/workflows-and-runs.md), [`glossary.md`](glossary.md) |
| Find commands and paths | [`reference/README.md`](reference/README.md) |
| Debug setup or run failures | [`troubleshooting/README.md`](troubleshooting/README.md) |
| Change code safely | [`AGENTS.md`](../AGENTS.md), [`contributing/development-lanes.md`](contributing/development-lanes.md) |
| Check architecture boundaries | [`architecture/contract.md`](architecture/contract.md) |

## Active Crates

| Crate | Question it answers | Agent orientation |
| --- | --- | --- |
| `crates/engine` | What is a valid workflow, and how does a run behave? | [`AGENTS.md`](../crates/engine/AGENTS.md) |
| `crates/orchestration` | How does the desktop app store, load, wire, and host runs? | [`AGENTS.md`](../crates/orchestration/AGENTS.md) |
| `crates/providers` | How do we talk to model providers? | [`AGENTS.md`](../crates/providers/AGENTS.md) |
| `crates/ui` / `crates/desktop` | How does the user interact with the runtime? | [`ui/AGENTS.md`](../crates/ui/AGENTS.md), [`desktop/AGENTS.md`](../crates/desktop/AGENTS.md) |

See [`architecture/contract.md`](architecture/contract.md) for layer boundaries and dependency rules.

## Scope

- Product docs explain how to run, configure, and use OpenFlow.
- Architecture docs explain ownership, boundaries, and runtime design for maintainers.
- Contributing docs explain local development lanes and verification.
- If code and docs diverge, update docs in the same change set.
