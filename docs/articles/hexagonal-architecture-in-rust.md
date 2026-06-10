# Hexagonal Architecture in Rust: Ports, Crates, and a CI That Says No

*Lessons from building a desktop app for agentic workflows.*

I've been building a desktop application in Rust — a step-through engine for agentic AI workflows, with a Tauri shell, multiple LLM providers, and a pile of built-in tools that read and write files. It's exactly the kind of project that rots fast: HTTP clients leak into core logic, the GUI framework's types show up in your domain structs, and six months in you can't test anything without a network connection and a window manager.

Hexagonal architecture (ports and adapters) is the standard answer, but most writing about it is in Java or C#. Rust changes the trade-offs in interesting ways — some things get easier, some get harder, and the compiler can enforce rules that other ecosystems need ArchUnit for. Here's what worked.

## The shape: one crate per ring

The single biggest advantage Rust gives you is that **Cargo enforces your dependency direction for free**. Crates cannot have circular dependencies — the build fails. So instead of hexagonal architecture being a folder convention inside one giant project, make each ring of the hexagon a workspace crate:

```text
ui → desktop → orchestration → engine
                             ↘ providers → engine
```

- **`engine`** is the hexagon: the workflow model, execution semantics, and the port traits. It depends on nothing above it. No HTTP, no filesystem, no GUI.
- **`providers`** is an outbound adapter crate: it implements the engine's `AiPort` trait against real LLM APIs (Anthropic, OpenAI-compatible endpoints, SSE streaming, the works).
- **`orchestration`** is the composition root: persistence, run lifecycle, tool I/O. It wires adapters into ports.
- **`desktop`** is a thin Tauri IPC shell. It talks to orchestration through one facade and knows nothing about the engine.

If someone tries to add `orchestration` as a dependency of `engine`, the change doesn't pass review — it doesn't pass *compilation*. That's a property you simply can't get with package-by-folder in most languages.

## Ports are just traits — but be stingy with them

A port in Rust is a trait owned by the inner crate:

```rust
// engine/src/ports/outbound.rs
#[async_trait]
pub trait AiPort: Send + Sync {
    async fn invoke(&self, request: AgentRequest) -> Result<AgentResponse, AiError>;
}
```

The adapter crate implements it; the composition root constructs it and hands the engine a `Box<dyn AiPort>`. The engine never names a concrete provider. Our providers crate exposes exactly one constructor:

```rust
// providers/src/lib.rs
pub fn create_provider(config: &ProviderConfig) -> Result<Box<dyn AiPort>, ProviderError> { … }
```

That single-function surface matters more than it looks. The moment a concrete type like `AnthropicClient` is reachable from above, someone will use it, and your port becomes decorative.

The rule I'd tattoo on every Rust codebase: **add a port only when a consumer is typed against `dyn ThatPort`.** Speculative ports — traits with one implementation and no polymorphic call site — are pure ceremony. Rust makes traits cheap to write, which makes this temptation worse, not better. We audit for it: every trait in `ports.rs` must have a `dyn` consumer or it gets deleted.

A detail people miss: ports come in two directions. *Outbound* ports are what the engine calls out through (`AiPort`, `ToolPort`). *Inbound* ports are how the outside drives the engine (submit input, approve a tool call). We keep them in separate files — `ports/inbound.rs` and `ports/outbound.rs` — because the question "who implements this?" has opposite answers for each, and mixing them confuses every new contributor.

## What goes where: a folder convention you can actually keep

Inside the composition-root crate, we settled on flat entity folders plus one centralized `adapters/` directory:

```text
orchestration/src/
├── agent/          library.rs + ports.rs     (use-case logic + traits)
├── workflow/       catalog.rs + ports.rs
├── project/        registry.rs + ports.rs
├── run/            coordinator.rs, execution/, state/
├── adapters/
│   ├── storage/        File*Store implementations
│   ├── tool_impl/      edit, grep, …
│   └── infrastructure/ lsp, git
└── backend/mod.rs      composition root — wires everything
```

Two decisions here were contrarian and both paid off:

**Flat domain folders.** The canonical hexagonal layout nests `domain/`, `application/`, and `adapters/` inside every entity. We tried it. For a codebase with five entities, it produced twenty near-empty directories and constant "which layer is this?" debates. A flat folder with `ports.rs` plus one or two role-named files (`library.rs`, `catalog.rs`, `registry.rs` — named for what the service *is*, not a generic `service.rs`) carries the same information with a third of the navigation cost.

**Centralized adapters.** All concrete I/O lives under one `adapters/` tree, grouped by technology concern, not by entity. This makes the load-bearing rule trivial to state and trivial to enforce: *no file outside `adapters/` may write `use crate::adapters::`*. Domain code reaches I/O only through the port traits it owns; only `backend/` (the composition root) is allowed to name both sides and wire them together.

## The compiler can't see modules — so lint the seams

Cargo enforces crate-level direction, but it knows nothing about module boundaries *within* a crate, and nothing about which external crates belong in which ring. Two mechanisms close the gap.

**First, `clippy.toml` disallowed lists** give you in-editor enforcement, not just CI failure:

```toml
# crates/engine/clippy.toml
disallowed-methods = [
    { path = "std::fs::read_to_string", reason = "engine is I/O-free; go through ToolPort" },
    { path = "std::fs::write", reason = "engine is I/O-free; go through ToolPort" },
]
disallowed-types = [
    { path = "std::fs::File", reason = "engine is I/O-free" },
]
```

The squiggle appears the moment someone types it, which is worth ten code-review comments.

**Second, an architecture check script in CI**, driven by a rules file rather than hardcoded greps. Ours runs in tiers:

1. **Cargo.toml edges** — assert the exact allowed dependency edges between workspace members, and that `engine` has no transport or GUI dependencies (`reqwest`, `tauri`) even transitively declared.
2. **Forbidden `use` statements per crate** — `engine` may never import `orchestration::`; `desktop` may never import `engine::` directly.
3. **Intra-crate seam rules** — domain folders can't import `crate::adapters::`; only one module is allowed to construct the engine; the UI may import `@tauri-apps/*` in exactly two files.

The script is ~250 lines of Python reading a TOML rules file. Yes, it's grep-with-extra-steps. It has caught real violations weekly. The trick that makes it survive: the rules file lives next to the architecture docs, so when the architecture changes, the diff that changes the doc changes the rules in the same commit.

## Make the rules machine-readable for your AI tools too

This is the new part of the story. Half the code in this project is written with AI assistance, and an AI agent will cheerfully violate your architecture if the rules live only in your head. So the architecture contract exists in three synchronized forms:

- `docs/architecture/contract.md` — prose for humans,
- `arch-check-rules.toml` — the machine-enforced version,
- an always-applied editor rule (Cursor rule / `CLAUDE.md` section) — a one-page distillation with the dependency diagram, the forbidden imports, a "where to add code" table, and the glossary terms.

The glossary matters more than I expected. If the codebase says `engine` but half the docs say `domain`, an AI assistant will mint a third name. Pick one term per concept, write it down, and ban the synonyms explicitly ("never `domain` or `workflow_core` — the crate is **engine**").

## What I'd tell past me

1. **One crate per ring, from day one.** Splitting later is miserable; starting split is nearly free.
2. **Ports earn their existence through a `dyn` consumer.** Otherwise delete them.
3. **Flat beats nested** inside each ring. Hexagonal is about the *seams*, not about folder depth.
4. **Enforce at three levels:** Cargo for crate edges, clippy for forbidden APIs, a rules-file script for module seams. Anything not enforced is a suggestion.
5. **Write the contract for machines** — your CI and your AI tools — and let the human docs be a view of it, not the source of truth.

The payoff is concrete: the engine crate runs its full test suite with no network, no filesystem, and no GUI — fake the two outbound ports and you can step a workflow through every approval pause in milliseconds. When we swapped in a new streaming SSE path for providers, the engine didn't change by a single line. That's the architecture doing its job: the expensive, volatile stuff lives at the edges, and the part that encodes what your product *means* stays small, pure, and testable.
