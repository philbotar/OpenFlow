# Architecture Patterns — Node Templates & Smart Defaults

**Domain:** AI agent workflow DAG editor — reusable node presets
**Researched:** 2026-05-30

## Recommended Architecture

```
┌─────────────────────────────────────────────────────┐
│  agent-workflow-app (UI + State + Persistence)       │
│                                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ ui/templates │  │  AppState    │  │ Template   │ │
│  │ .rs          │──│  .apply_     │──│ Store      │ │
│  │ (browser)    │  │  template()  │  │ .templates │ │
│  └──────────────┘  └──────────────┘  │ .json      │ │
│                                       └────────────┘ │
│                        │                              │
├────────────────────────┼──────────────────────────────┤
│  workflow-core         │                              │
│                        ▼                              │
│  ┌────────────────────────────────────┐              │
│  │ NodeTemplate          AgentNodeConfig             │
│  │ ├── id                ├── system_prompt            │
│  │ ├── name              ├── task_prompt              │
│  │ ├── description       ├── model                    │
│  │ ├── category          ├── output_schema            │
│  │ ├── system_prompt     ├── auto_start               │
│  │ ├── task_prompt                                   │
│  │ ├── model                Node                      │
│  │ ├── output_schema        ├── AgentNodeConfig ──────┘
│  │ └── auto_start           └── template_id: Option<String>
│  └────────────────────────────────────┘              │
└─────────────────────────────────────────────────────┘
```

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `NodeTemplate` (workflow-core) | Data definition for template presets; mirrors `AgentNodeConfig` fields plus metadata (id, name, description, category) | `TemplateStore` (serialized), `AppState` (applied to nodes) |
| `TemplateStore` (agent-workflow-app) | Load/save `templates.json`, provide built-in defaults on first run, thread-safe access via `RwLock` | Filesystem (`templates.json`), `AppState` (read templates), `ui/templates.rs` (browse templates) |
| `AppState` (agent-workflow-app) | Holds template state (last used, search query), provides `apply_template_to_node()` and `save_node_as_template()` | `TemplateStore` (load/save), `ui/templates.rs` (receive user selections), `Node` (apply config) |
| `ui/templates.rs` (agent-workflow-app) | Template browser panel: display templates, search/filter, preview, trigger apply | `AppState` (read templates, trigger apply), `ui/mod.rs` (panel management) |
| `Node` (workflow-core) | Existing node type; gains `template_id: Option<String>` to track which template was applied | `AgentNodeConfig` (fields copied from template), inspector UI (show template info) |

### Data Flow

```
1. App start → TemplateStore::load_or_default()
   - Reads templates.json OR creates from built-in defaults
   - Built-in templates are hardcoded in TemplateStore (following SettingsStore::default() pattern)

2. User opens template browser → ui/templates.rs reads AppState.templates()
   - AppState reads from TemplateStore (RwLock read)

3. User selects template → ui/templates.rs calls AppState.apply_template(template_id, position)
   - AppState looks up template by ID in TemplateStore
   - Copies AgentNodeConfig fields from template to new Node
   - Sets Node.template_id = Some(template.id)
   - Adds node to workflow at specified position

4. User saves node as template → inspector calls AppState.save_node_as_template(node_id, name, description, category)
   - AppState reads Node's AgentNodeConfig
   - Creates new NodeTemplate with user-provided metadata + node's config fields
   - Calls TemplateStore.add_template(template)
   - Calls TemplateStore.save() (atomic write)

5. App shutdown (or periodic) → TemplateStore.save()
   - Serializes all templates to JSON
   - Writes to temp file, then renames (atomic)
```

### Patterns to Follow

### Pattern 1: Atomic File Persistence (from `FileSettingsStore`)

**What:** Write to a temporary file, then rename to the target path. This prevents corrupted files on crash or partial write.

**When:** Any time you write to `templates.json`.

**Example:**
```rust
fn save_to_file(path: &Path, templates: &[NodeTemplate]) -> io::Result<()> {
    let json = serde_json::to_string_pretty(templates)?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, json)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
}
```

### Pattern 2: Legacy Migration via `#[serde(default)]`

**What:** Use `#[serde(default)]` on new fields added after initial release. Old `templates.json` files without the new field will deserialize cleanly.

**When:** Adding fields to `NodeTemplate` after Phase 1.

**Example:**
```rust
#[derive(Serialize, Deserialize)]
pub struct NodeTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    // New field added in Phase 4 — old JSON files won't have it:
    #[serde(default)]
    pub icon: String, // defaults to "" for legacy templates
}
```

### Pattern 3: Built-in Defaults on First Load

**What:** When `templates.json` doesn't exist (first run), populate the store with hardcoded built-in templates.

**When:** `TemplateStore::load_or_default()`.

**Example:**
```rust
pub fn load_or_default(path: PathBuf) -> Self {
    if path.exists() {
        // Load existing
    } else {
        // Create with built-in templates
        let defaults = vec![
            NodeTemplate {
                id: "clarify_idea".into(),
                name: "Clarify Idea".into(),
                description: "Expand a rough idea into a structured analysis".into(),
                category: "Brainstorming".into(),
                system_prompt: "You are a clarity analyst...".into(),
                task_prompt: "Given the following idea: {{input}}\n\nPlease...".into(),
                model: "gpt-4o".into(),
                output_schema: None,
                auto_start: false,
            },
            // ... more built-ins
        ];
        // Save defaults to file, return store
    }
}
```

### Pattern 4: RwLock for Template Store (from `FileSettingsStore`)

**What:** Use `RwLock<Vec<NodeTemplate>>` to allow concurrent reads (browsing templates) while serializing writes (add/remove/save).

**When:** `TemplateStore` struct definition.

**Why not `Mutex`:** The template browser needs read access while the user is applying a template (which may write to the store if it updates "last used"). `RwLock` allows these to proceed concurrently since reads are non-blocking.

**Why not `Arc<Mutex<>>`:** The store is held by `AppState` (single owner), not shared across threads. `Arc` adds unnecessary indirection.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Templates Stored Inside Workflow

**What:** Adding `templates: Vec<NodeTemplate>` to the `Workflow` struct or storing template data inside `workflows.json`.

**Why bad:** Couples templates to project state. User must copy templates between workflow files. Templates become non-reusable. Template changes affect multiple files inconsistently.

**Instead:** `TemplateStore` with independent `templates.json`. Templates are global to the app, not scoped to workflows.

### Anti-Pattern 2: Mutating Templates After Node Creation

**What:** Editing a template retroactively changes all nodes created from that template.

**Why bad:** Non-deterministic behavior. User opens a week-old workflow and all nodes changed because a template was edited. Breaks the mental model of "template = starting point."

**Instead:** Template application is a copy operation. Nodes are independent after creation. `template_id` on `Node` is informational only (shows origin, enables "Reset to template" as an explicit user action).

### Anti-Pattern 3: Skipping `#[serde(default)]` on New Fields

**What:** Adding a required field to `NodeTemplate` without `#[serde(default)]`.

**Why bad:** Users with existing `templates.json` files will get deserialization errors on app start. Corrupted state.

**Instead:** Always use `#[serde(default)]` on new fields. Provide sensible defaults (empty string, `false`, `None`).

## Scalability Considerations

| Concern | At 10 templates | At 50 templates (user-created) | At 200 templates |
|---------|----------------|-------------------------------|------------------|
| File size | <5 KB JSON | <50 KB JSON | <200 KB JSON — still negligible |
| Load time | Instant (<1ms) | Instant (<5ms) | Instant (<10ms) — no need for incremental loading |
| Browse performance | Instant | Instant | May need search/filter for usability (not performance) |
| Memory | <10 KB in RwLock | <50 KB in RwLock | <200 KB — still negligible |
| Concurrent access | RwLock is fine | RwLock is fine | RwLock is fine — no contention at this scale |

Templates are a bounded collection. Even at 200 templates (unlikely for a desktop app), the JSON file is under 200 KB and loads instantly. No need for pagination, lazy loading, or database storage.

## Sources

- `crates/workflow-core/src/model.rs` — `AgentNodeConfig`, `Node` struct (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/settings_store.rs` — `FileSettingsStore` atomic write, legacy migration, RwLock pattern (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/storage.rs` — `FileWorkflowStore` JSON persistence, separate store pattern (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/state.rs` — `AppState.add_agent_node()`, state management patterns (PRIMARY, read May 30 2026)
- `.planning/codebase/ARCHITECTURE.md` — layer separation: core → app (SECONDARY, read May 30 2026)
