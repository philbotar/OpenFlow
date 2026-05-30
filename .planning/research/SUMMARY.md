# Project Research Summary

**Project:** Step-Through Agentic Workflow — Node Templates & Smart Defaults
**Domain:** AI agent workflow DAG editor — reusable node presets
**Researched:** 2026-05-30
**Confidence:** HIGH

## Executive Summary

This project adds reusable **Node Templates** — pre-configured blueprints for agent nodes — to an existing Rust/egui desktop DAG editor. When users add an AI agent node, templates supply meaningful defaults (system prompt, task prompt, model, output schema, auto-start behavior) instead of empty placeholders. The feature is purely additive: no new dependencies, no architectural rewrites.

The recommended approach follows the codebase's established patterns exactly. `NodeTemplate` lives in `workflow-core/src/model.rs` as a domain type that **composes** `AgentNodeConfig` (not mirrors its fields) — this eliminates the sync bug risk between two independent struct definitions. `TemplateStore` follows the battle-tested `FileSettingsStore` pattern: atomic writes via temp-file-then-rename, `RwLock` for concurrent reads, `#[serde(default)]` for forward compatibility, and built-in defaults populated on first run. Templates persist in their own `templates.json` file, completely independent of `workflows.json` — templates are global to the app, not scoped to any single workflow. Template application is always a **copy** operation; nodes are independent after creation, with an informational `template_id` field tracking origin.

The key risk is identity confusion: treating templates as nodes or storing them inside workflow files. This is avoided by a strict separation — `TemplateStore` owns templates, `AppState.apply_template_to_node()` copies fields into a new `Node`, and no reference to the template persists beyond creation. The implementation follows a clear 4-phase dependency chain: data model → state integration → UI → polish. Each phase is independently testable and shippable.

## Key Findings

### Recommended Stack

No new dependencies. The feature is additive to the existing Rust 2021 / egui 0.34 / serde / dirs stack. `TemplateStore` follows the `FileSettingsStore` pattern (atomic write via temp file + rename, `RwLock` for thread-safe access) which is already proven in the codebase. Template persistence uses `serde_json` for `templates.json` in the same data directory as `settings.json` and `workflows.json`.

**Core technologies (all existing):**
- **Rust Edition 2021 / egui 0.34 / eframe 0.34:** UI and app shell — template browser is a new panel, not a new UI paradigm
- **serde / serde_json:** Serialization — `NodeTemplate` needs the same derives as `AgentNodeConfig`
- **dirs 5.x:** Storage path resolution — `templates.json` lives alongside `settings.json`
- **`RwLock` (std):** Thread-safe template store access — concurrent reads (browsing) + serialized writes (saving)

### Expected Features

**Must have (table stakes):**
- **Built-in template library** — 5-8 templates covering common patterns (clarify, summarize, analyze, transform, brainstorm, critique)
- **Template-applied node creation** — replaces hardcoded empty-string defaults in `add_agent_node()` with template-based defaults
- **Template metadata display** — name, description, category visible to users before applying
- **Template persistence** — survives app restarts; `templates.json` with atomic writes

**Should have (differentiators):**
- **Template browser panel** — visual gallery with search/filter by name and category; egui `ScrollArea` or `Grid`
- **Preview before apply** — see template fields read-only before committing to a node
- **Last-used template memory** — remembers which template the user applied last and pre-selects it
- **Custom user templates** — "Save as Template" from existing node configs
- **Template origin tracking** — node inspector shows which template was applied, with "Reset to template" option

**Defer (v2+):**
- Category grouping with `CollapsingHeader` — defer until browser UI is validated (Phase 3)
- Keyboard shortcut (Cmd/Ctrl+T) — tie to UI completion (Phase 4)
- Smart defaults by context — needs heuristic engine spike (Phase 4)
- AI-generated templates — requires AI call, adds latency and failure modes; not for MVP

### Architecture Approach

Layer separation follows the existing pattern: `NodeTemplate` in `workflow-core` (domain type), `TemplateStore` in `agent-workflow-app` (persistence), template browser in `agent-workflow-app/src/ui/templates.rs` (presentation). This mirrors the `AgentNodeConfig` → `FileSettingsStore` → `ui/settings.rs` layering.

**Critical design decision — composition over mirroring:**
```rust
// RECOMMENDED (eliminates sync bugs)
pub struct NodeTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub config: AgentNodeConfig,  // composed, not mirrored
}

// AVOID (field mirroring — Pitfall 5)
pub struct NodeTemplate {
    pub id: String,
    // ... metadata ...
    pub system_prompt: String,    // mirrors AgentNodeConfig
    pub task_prompt: String,      // mirrors AgentNodeConfig
    // ... must stay in sync with AgentNodeConfig forever
}
```

**Major components:**
1. **`NodeTemplate` (workflow-core/src/model.rs)** — Domain type composing `AgentNodeConfig` plus template metadata (id, name, description, category)
2. **`TemplateStore` (agent-workflow-app/src/template_store.rs)** — Load/save `templates.json`, provide built-in defaults on first run, thread-safe access via `RwLock`, atomic writes
3. **`AppState` extension (agent-workflow-app/src/state.rs)** — `apply_template_to_node()`, `save_node_as_template()`, last-used template tracking, template ID on `Node`
4. **`ui/templates.rs` (agent-workflow-app/src/ui/)** — Template browser panel: display, search/filter, preview, apply trigger; integrates with existing panel management in `ui/mod.rs`

**Data flow:**
1. App start → `TemplateStore::load_or_default()` reads `templates.json` or seeds built-in defaults
2. User opens template browser → `ui/templates.rs` reads from `AppState` (which reads from `TemplateStore`)
3. User selects template → UI calls `AppState.apply_template(template_id, position)` → creates `Node` with `config` copied from template, sets `template_id: Option<String>`
4. User saves node as template → inspector calls `AppState.save_node_as_template()` → creates `NodeTemplate` from node's `AgentNodeConfig` → persists via `TemplateStore.add_template()`

### Critical Pitfalls

1. **Template-node identity confusion** — Treating `NodeTemplate` as a `Node` variant or storing templates in `Workflow` structs. **Prevention:** `TemplateStore` owns templates independently; node creation copies fields (never references); `template_id` on `Node` is informational only.

2. **Corrupted `templates.json` on crash** — Naive `fs::write()` leaves partial JSON on crash. **Prevention:** Atomic write pattern: serialize to `templates.tmp`, then `fs::rename()` to `templates.json`. Follows `FileSettingsStore` pattern exactly.

3. **Built-in templates overwriting user data** — Deserialization error triggers fallback-to-defaults, silently discarding user customizations. **Prevention:** Distinguish "file not found" (first run → create defaults) from "invalid JSON" (recovery → rename corrupt file to `.bak`, log warning, show notification, then create defaults).

4. **Template ID collisions** — User creates a template with the same ID as a built-in (e.g., `clarify_idea`). **Prevention:** Prefix built-in IDs with `builtin.` (e.g., `builtin.clarify_idea`) or validate user template IDs against built-in IDs on save and reject collisions.

5. **Template fields out of sync with `AgentNodeConfig`** — Mirroring fields in two separate structs means they drift when `AgentNodeConfig` changes. **Prevention:** Use composition — embed `AgentNodeConfig` as `config: AgentNodeConfig` in `NodeTemplate`. This eliminates the sync problem entirely at the cost of one extra nesting level in JSON.

## Implications for Roadmap

Based on combined research, suggested 4-phase implementation:

### Phase 1: Data Model & Persistence
**Rationale:** Foundational — nothing else works without loadable, saveable templates. Must exist before integration.
**Delivers:** `NodeTemplate` struct in `workflow-core/src/model.rs`, `TemplateStore` in `agent-workflow-app/src/`, `templates.json` persistence with atomic writes, 5-8 built-in templates seeded on first run.
**Addresses:** Template definition, serialization, persistence, atomic saves, legacy migration (`#[serde(default)]`), corrupt file recovery (backup `.bak`).
**Avoids:** Pitfall 2 (corrupted files — atomic writes), Pitfall 3 (overwriting user data — `.bak` recovery), Pitfall 5 (field sync — composition pattern).

### Phase 2: Application Integration
**Rationale:** Wires templates into the app state. State methods must work before UI binds to them. This is where the copy-from-template semantics are enforced.
**Delivers:** `apply_template_to_node(template_id, position)` on `AppState`, `template_id: Option<String>` on `Node`, `save_node_as_template()` for custom templates, last-used template tracking, fallback to `AgentNodeConfig::default()` when no template selected.
**Uses:** `TemplateStore` (load templates), `AgentNodeConfig` (copy config fields).
**Implements:** Template-to-node application; template ID origin tracking on nodes.
**Avoids:** Pitfall 1 (template-node confusion — copy, not reference), Pitfall 4 (ID collisions — `builtin.` prefix on built-in IDs).

### Phase 3: Template Browser UI
**Rationale:** User-facing. Core state must exist before UI can bind to it. Builds on Phase 1-2.
**Delivers:** New `ui/templates.rs` panel integrated with `ui/mod.rs` panel management, scrollable template list/grid, search/filter by name, preview pane showing template config read-only, integration with existing node creation flow (apply template → add node at canvas position).
**Uses:** `AppState.apply_template_to_node()` for node creation, `TemplateStore` for template list.
**Avoids:** Pitfall 6 (UI blocking — hold write lock only for in-memory operations; file I/O outside lock).

### Phase 4: Smart Defaults & Polish
**Rationale:** Differentiation and UX refinement. Only makes sense after core template flow is validated. Needs a spike for context-sensitive recommendations.
**Delivers:** Last-used template pre-selection in browser, keyboard shortcut (Cmd/Ctrl+T), category grouping with `CollapsingHeader`, enhanced template browser interactions, context-sensitive template recommendations (spike first).
**Avoids:** Premature optimization before core flow is verified.

### Phase Ordering Rationale

- **Rigid dependency chain:** Data model → state integration → UI → polish. Each phase depends on the previous. No parallelization opportunity.
- **Each phase is independently shippable:** After Phase 1, templates exist on disk but aren't used yet. After Phase 2, programmatic template application works but has no UI. After Phase 3, users can browse and apply templates visually. Phase 4 is UX refinement.
- **Risk containment:** The trickiest parts (atomic writes, error recovery, composition pattern) are in Phase 1. If anything goes wrong, it's caught early with minimal rework.
- **UI deferral:** The template browser (Phase 3) is the visually complex part. Keeping it after core state (Phase 2) means no UI is built against unstable interfaces.

### Research Flags

Phases likely needing deeper research during planning (`/gsd-plan-phase --research-phase`):
- **Phase 4:** Context-sensitive template recommendations — requires a heuristic engine. No existing pattern in the codebase. Needs a spike/prototype to determine feasibility and approach.
- **Phase 3 (partial):** Template browser interaction design — how drag-and-drop integrates with the existing canvas interaction system. The canvas uses egui's built-in node dragging, not a custom drag system. Template-to-canvas interaction needs design exploration.

Phases with standard patterns (skip research, proceed directly to plan):
- **Phase 1:** Crystal clear — `FileSettingsStore` provides a 1:1 pattern to follow. `NodeTemplate` composition pattern is straightforward.
- **Phase 2:** Well-understood — `add_agent_node()` already exists as the integration point. Template application is a field copy, which is deterministic.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | No new dependencies; additive to existing Rust/egui/serde stack. Verified against codebase. |
| Features | HIGH | All scoped features supported by existing architecture. `AgentNodeConfig` has exactly 5 fields (confirmed in model.rs). No Handlebars, no tools config, no reasoning config to account for. |
| Architecture | HIGH | Clear pattern to follow (`FileSettingsStore` → `TemplateStore`). Composition-over-mirroring design eliminates Pitfall 5. Layer separation matches existing codebase. |
| Pitfalls | HIGH | Pitfalls are well-understood from file persistence and template/naming systems. Mitigations are concrete (atomic writes, `.bak` recovery, `builtin.` prefix, composition pattern). |

**Overall confidence: HIGH**

The research is grounded in the actual codebase (all four researchers read `model.rs`, `settings_store.rs`, `storage.rs`, and `state.rs` directly). There are no speculative technology choices — every recommendation follows an existing, working pattern in the project. The `AgentNodeConfig` layout was verified to have exactly 5 fields, eliminating scope creep from the initial assumption of 9 fields with Handlebars templating and tools config.

### Gaps to Address

- **Canvas drag-and-drop for templates:** The current canvas uses egui's built-in click-and-drag for existing nodes, not a custom drag system for creating new nodes from a palette. Research in Phase 3 planning should determine whether to use egui's `DragValue`/custom drag sources or a simpler "click template then click canvas" interaction pattern.

- **Context-sensitive defaults (Phase 4):** How to recommend templates based on connected nodes or workflow context. No existing heuristic system in the codebase. Needs a spike — possibly a simple rule engine (e.g., "if connected to a node with output_schema=X, recommend template Y") but the approach is TBD.

- **Template parameterization:** Whether templates should support `{{placeholder}}` substitution in system/task prompts. The codebase has no template engine (no Handlebars, no Tera). Out of scope for this milestone but should be noted as a potential v2 feature that would require adding a templating dependency.

## Sources

### Primary (HIGH confidence — read directly from codebase)
- `crates/workflow-core/src/model.rs` — `AgentNodeConfig` (5 fields confirmed), `Node` struct, serde patterns
- `crates/agent-workflow-app/src/settings_store.rs` — `FileSettingsStore` atomic write + legacy migration + RwLock pattern
- `crates/agent-workflow-app/src/storage.rs` — `FileWorkflowStore` JSON persistence, separate store pattern
- `crates/agent-workflow-app/src/state.rs` — `AppState.add_agent_node()`, state management patterns
- `.planning/codebase/STACK.md` — confirmed Rust/egui/serde/tokio stack
- `.planning/codebase/ARCHITECTURE.md` — layer separation: core → app

### Secondary (MEDIUM confidence)
- `.planning/PROJECT.md` — feature description and scope
- Community patterns for template/node systems (n8n, LangFlow, Dify) — confirmed template-as-blueprint pattern is industry standard

### Tertiary (LOW confidence — needs validation)
- Canvas drag-and-drop interaction design — the UI integration research proposed structures (`NodePrototype`, `nav.rs`, `DragStartEvent`) that have not been verified in the current codebase. These were aspirational design directions, not existing code. Validate during Phase 3 planning.

---
*Research completed: 2026-05-30*
*Ready for roadmap: yes*
*Synthesized from: STACK.md, FEATURES.md, ARCHITECTURE.md, PITFALLS.md + user-provided research findings*
