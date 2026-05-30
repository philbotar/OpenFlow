# Technology Stack — Node Templates

**Project:** Step-Through Agentic Workflow
**Researched:** 2026-05-30

## Recommended Stack

### Core Framework (existing, unchanged)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Rust | Edition 2021 | Language | Already in use; no reason to change |
| egui | 0.34.x | Immediate-mode GUI | Already in use; template browser UI is a new panel, not a new UI paradigm |
| eframe | 0.34.x | Desktop app shell | Already in use; provides `epi::Frame` for storage path resolution (used by `TemplateStore`) |

### Serialization (existing, unchanged)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| serde | 1.x | Serialize/Deserialize derives | Already in use; `NodeTemplate` needs the same derives as `AgentNodeConfig` and `Workflow` |
| serde_json | 1.x | JSON file persistence | Already in use; `templates.json` uses the same format as `settings.json` and `workflows.json` |

### Storage Path Resolution (existing, unchanged)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| dirs | 5.x | Data-local directory path | Already in use by `settings_store.rs`; `TemplateStore` resolves `templates.json` to the same directory |

### No New Dependencies Required
All template functionality is additive to the existing stack. The `FileSettingsStore` pattern (atomic write via temp file + rename, legacy field migration via `#[serde(default)]`, `RwLock` for thread-safe access) provides everything `TemplateStore` needs. No new crates to audit or add to `Cargo.toml`.

### Node Template Struct — Serde Strategy
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeTemplate {
    // Metadata
    pub id: String,           // Unique identifier (e.g., "clarify_idea")
    pub name: String,         // Display name (e.g., "Clarify Idea")
    pub description: String,  // Tooltip/hover text
    pub category: String,     // Grouping (e.g., "Brainstorming", "Analysis")

    // AgentNodeConfig fields (pre-fill these when creating a node)
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub task_prompt: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub auto_start: bool,
}
```

### TemplateStore — File I/O Pattern (following `FileSettingsStore`)
```rust
pub struct TemplateStore {
    path: std::path::PathBuf,
    // RwLock<Vec<NodeTemplate>> for thread-safe mutable access
}

impl TemplateStore {
    pub fn load_or_default(path: std::path::PathBuf) -> Self;
    pub fn templates(&self) -> Vec<NodeTemplate>;
    pub fn add_template(&self, template: NodeTemplate);
    pub fn remove_template(&self, id: &str);
    pub fn save(&self) -> Result<(), std::io::Error>;  // Atomic write via temp file + rename
}
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Storage format | Separate `templates.json` | Embed in `workflows.json` per workflow | Couples templates to project state; breaks reusability across workflows |
| Storage format | Separate `templates.json` | Database (SQLite) | Overkill for ~20-50 templates; JSON file is human-readable, version-control-friendly, and follows existing patterns |
| Template struct location | `workflow-core/src/model.rs` | `agent-workflow-app/src/` | Templates are domain types; `workflow-core` is the domain layer. App layer stores and displays them, core defines them |
| Template ID strategy | String ID (e.g., "clarify_idea") | UUID | Human-readable IDs make built-in templates self-documenting; UUIDs add friction for no benefit at this scale |
| Runtime config | `RwLock<Vec<NodeTemplate>>` | `Arc<Mutex<Vec<NodeTemplate>>>` | `RwLock` allows concurrent reads (common case: browsing templates while another thread applies one). Follows `settings_store.rs` pattern |
| Built-in templates | Hardcoded in `TemplateStore::load_or_default()` | External JSON bundled with app | External JSON is nicer but requires build script or `include_str!`; hardcoded defaults are simpler and match `SettingsStore::default()` pattern |

## Installation

No new dependencies. The feature is additive to the existing workspace. Verify with:

```bash
cargo build --workspace
cargo test --workspace
```

## Sources

- `crates/workflow-core/src/model.rs` — `AgentNodeConfig` fields, serde patterns (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/settings_store.rs` — `FileSettingsStore` atomic write + legacy migration pattern (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/storage.rs` — `FileWorkflowStore` JSON persistence pattern (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/state.rs` — `add_agent_node()` integration point (PRIMARY, read May 30 2026)
- `.planning/codebase/STACK.md` — confirmed Rust/egui/serde/tokio stack (SECONDARY, read May 30 2026)
