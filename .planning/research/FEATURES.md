# Feature Landscape — Node Templates & Smart Defaults

**Domain:** AI agent workflow DAG editor — reusable node presets
**Researched:** 2026-05-30

## Table Stakes

Features users expect. Missing = feature feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Built-in template library | Users need starting points; empty node configs are intimidating | Low | 5-8 built-in templates covering common patterns (clarify, summarize, analyze, transform, brainstorm, critique) |
| Template applied on node creation | Core value prop: add node → it already has useful defaults | Medium | Integrates with `add_agent_node()` in `state.rs`; replaces current hardcoded empty-string defaults |
| Template metadata display | Users need to know what a template does before applying it | Low | Name, description, category shown in template browser; mirrors tooltip patterns already used in inspector |
| Template persistence | Must survive app restarts; users may modify or add custom templates | Low | `templates.json` with atomic writes, following `FileSettingsStore` pattern |

## Differentiators

Features that set the product apart from a simple config file.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Template browser panel | Visual gallery to browse/search/select templates — faster than remembering names | Medium | New UI panel in `ui/templates.rs`; egui `Grid` or `ScrollArea` with cards; search bar with text filtering |
| Category grouping | Templates organized by purpose (Brainstorming, Analysis, Transformation, etc.) | Low | `category` field on `NodeTemplate`; egui `CollapsingHeader` per category in browser |
| Preview before apply | See template fields before committing to a node | Low | Click template → preview panel shows system_prompt, task_prompt, model, etc. in read-only mode |
| Keyboard shortcut (Cmd/Ctrl+T) | Power users add nodes with templates without clicking | Low | `ui/mod.rs` already handles keyboard events; add shortcut to open template browser |
| Last-used template memory | App remembers which template you used last and offers it as default next time | Medium | Store `last_used_template_id` in `AppState`; pre-select in browser on next open; fallback to "Generic Agent" if none |
| Custom user templates | Users create and save their own templates from existing node configs | Medium | "Save as Template" button in node inspector; mirrors `add_template()` on `TemplateStore` |
| Template applied indicator | Node inspector shows which template was used (with option to clear) | Low | Store `template_id: Option<String>` on `Node`; show in inspector header; "Reset to template" button |
| Smart defaults by context | Suggests templates based on connected nodes or workflow context | High | Phase 4 research item; needs heuristic engine (spike later) |

## Anti-Features

Features to explicitly NOT build.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Templates as editable workflow nodes | Templates should be immutable references; editing a template should not retroactively change existing nodes | "Apply" copies template fields to node; node is independent after creation |
| Templates embedded in workflow files | Breaks reusability; couples templates to project state | Store in separate `templates.json` with its own store |
| Template versioning | Premature complexity for MVP; users are not deploying template upgrades across projects | Template ID + name is sufficient; if user wants a variant, they create a new template |
| AI-generated templates | Requires AI call, introduces latency, adds failure modes; MVP should be deterministic | Ship built-in templates; add AI generation in a future phase when AI integration is more mature |
| Template inheritance/composition | Overengineered for this scale; templates are flat presets, not a type hierarchy | Keep templates independent; users copy-paste from existing nodes if they want to combine |
| Drag files (import/export templates) | Desktop file dialog adds platform-specific complexity for marginal value | Templates are stored in `templates.json`; users can version-control or share files manually |

## Feature Dependencies

```
TemplateStore (persistence) → NodeTemplate struct (data model)
AppState.apply_template_to_node() → TemplateStore (load templates)
Template Browser UI → AppState.apply_template_to_node() (create nodes)
Save as Template → TemplateStore.add_template() (persist)
Last-used memory → TemplateStore (load last used)
Smart defaults → Template Browser UI + Node connections data
```

## MVP Recommendation

Prioritize (Phase 1-2):
1. **`NodeTemplate` struct + `TemplateStore`** — data model and persistence with 5-8 built-in templates
2. **`apply_template_to_node()` in `AppState`** — integration point; replace hardcoded defaults with template-based defaults
3. **Template browser panel (minimal)** — simple scrollable list, click to apply, search by name

Defer:
- Category grouping, preview panel: Phase 3 (enhanced UI)
- Custom user templates, "Save as Template", applied indicator: Phase 3-4
- Smart defaults by context: Phase 4 (needs spike/prototype)
- Keyboard shortcuts: Phase 4 (polish, low effort but ties to UI completion)

## Sources

- `crates/workflow-core/src/model.rs` — `AgentNodeConfig` defines template-compatible fields (system_prompt, task_prompt, model, output_schema, auto_start)
- `crates/agent-workflow-app/src/state.rs` — `add_agent_node()` is the node creation entry point, currently uses hardcoded defaults
- `.planning/PROJECT.md` — feature description: "pre-configured, reusable blueprints for agent nodes"
- Competitor analysis: n8n, LangFlow, and Dify all use template/node-preset patterns for agent workflows — confirming this is a table-stakes feature for DAG-based AI workflow editors (pattern observed via WebSearch, LOW confidence due to no direct codebase analysis of competitors)
