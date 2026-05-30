# Domain Pitfalls — Node Templates & Smart Defaults

**Domain:** AI agent workflow DAG editor — reusable node presets
**Researched:** 2026-05-30

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Template-Node Identity Confusion
**What goes wrong:** Treating `NodeTemplate` as a subclass or variant of `Node`, populating the workflow DAG with template instances, or storing templates inside the `Workflow` struct.

**Why it happens:** Intuitive (but wrong) mental model — "a template is just a pre-configured node." Developers add a `Template` variant to the `Node` enum or add `templates: Vec<NodeTemplate>` to `Workflow`.

**Consequences:** Templates become coupled to specific workflow files. User creates a template in workflow A, can't use it in workflow B. Editing a template retroactively changes all nodes in all workflows that use it (if references are used). `templates.json` becomes `workflows.json`+templates.json merged — violates single responsibility.

**Prevention:** Always model templates as a distinct concept with their own store (`TemplateStore` → `templates.json`). Node creation copies template fields — never references. `template_id` on `Node` is informational only.

**Detection:** Code review check: does any code path store template data inside a `Workflow`? Does any code path mutate `NodeTemplate` and expect `Node`s to update?

### Pitfall 2: Corrupted templates.json on Crash
**What goes wrong:** App crashes mid-write to `templates.json`. File is truncated or contains partial JSON.

**Why it happens:** Naive `fs::write()` overwrites the file in-place. If the process dies mid-write, the file is garbage.

**Consequences:** On next app start, deserialization of `templates.json` fails. User loses all custom templates. Fallback to built-in defaults (silent data loss).

**Prevention:** Atomic write pattern: write to `templates.tmp`, call `fs::rename()` to swap it into place. `rename()` is atomic on all modern filesystems. This is the pattern `FileSettingsStore` already uses — follow it exactly.

**Detection:** Unit test: write to store, kill process at random point during save, verify file is either old (intact) or new (complete), never partial.

### Pitfall 3: Built-in Templates Overwriting User Templates
**What goes wrong:** `TemplateStore::load_or_default()` returns built-in templates when `templates.json` exists but is empty or has a deserialization error.

**Why it happens:** Error handling that treats "file not found" and "invalid JSON" the same way — both trigger the "create defaults" path.

**Consequences:** User creates custom template, app crashes during save (see Pitfall 2), on restart app sees corrupt file and overwrites with defaults. User's custom template is lost.

**Prevention:** Distinguish "file doesn't exist" (first run — create defaults) from "file exists but is invalid" (recovery — alert user, attempt to recover or keep backup). On deserialization error, rename the corrupt file to `templates.json.bak` and then create defaults. Never silently overwrite.

**Detection:** Test with malformed JSON file. Verify app doesn't silently overwrite, and that backup file is created.

## Moderate Pitfalls

### Pitfall 4: Template ID Collisions Between Built-in and User Templates
**What goes wrong:** User creates a template with ID "clarify_idea" (same as a built-in). `TemplateStore` has two templates with the same ID.

**Why it happens:** Built-in templates use human-readable IDs. User creates a template and happens to pick the same ID.

**Consequences:** `apply_template()` by ID returns the wrong template. Which one? Depends on insertion order. Non-deterministic.

**Prevention:** Prefix built-in templates with `builtin.` (e.g., `builtin.clarify_idea`) or store them in a separate list from user templates. Alternative: validate user template IDs against built-in IDs on save and reject collisions with a clear error message.

**Detection:** Unit test: add user template with same ID as built-in, verify rejection or clear disambiguation.

### Pitfall 5: Template Fields Out of Sync with AgentNodeConfig
**What goes wrong:** `AgentNodeConfig` gains a new field (e.g., `temperature`). `NodeTemplate` doesn't include it. Templates applied to new nodes are missing the field.

**Why it happens:** `NodeTemplate` mirrors `AgentNodeConfig` but doesn't derive from it. They're separate structs that must be kept in sync manually.

**Consequences:** Nodes created from templates have missing fields. Serde uses `#[serde(default)]` on `AgentNodeConfig` fields, so the missing field gets its default value — not what the template author intended. Inconsistent behavior between template-applied nodes and manually configured nodes.

**Prevention:** Add a compilation check or integration test that verifies all `AgentNodeConfig` fields have corresponding fields in `NodeTemplate`. Consider using a macro or `include!` to define the shared fields once. Alternatively, embed `AgentNodeConfig` directly in `NodeTemplate` instead of mirroring fields:
```rust
pub struct NodeTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub config: AgentNodeConfig,  // <-- composition, not mirroring
}
```
This eliminates the sync problem entirely. The downside is that `NodeTemplate` serialization now nests `config` one level deeper in JSON. Acceptable tradeoff for eliminating sync bugs.

**Detection:** Integration test: create a template with all fields set, apply to node, verify all fields transferred correctly.

### Pitfall 6: Template Browser Blocking UI During File I/O
**What goes wrong:** Template browser calls `TemplateStore.save()` on the main thread. Large JSON serialization or slow disk I/O causes the UI to freeze.

**Why it happens:** The `TemplateStore` uses `RwLock`, which allows concurrent reads but write locks block all readers. If the browser is open while saving (e.g., auto-save after adding a template), the UI thread blocks on read.

**Consequences:** UI stutter or freeze during save operations. Noticeable on slow disks (HDDs, network drives).

**Prevention:** Serialize to string first (cheap), then release the write lock, then do file I/O outside the lock. The lock is held only during the `Vec` clone + serialization, not during disk I/O. Alternatively, save on a background thread via `tokio::spawn` (but templates are not async currently — keep simple).

**Detection:** Profile save operation on large template collections (200+ templates). Verify UI thread doesn't block for >1ms.

## Minor Pitfalls

### Pitfall 7: No Template Deserialization Error Recovery
**What goes wrong:** `templates.json` has a schema change that `#[serde(default)]` doesn't cover (e.g., field type changed from `String` to `i32`). Deserialization fails.

**Consequences:** App starts with no templates (falls back to built-in defaults). User doesn't know their custom templates were lost.

**Prevention:** Log a warning on deserialization error. Show a one-time notification: "Some template data could not be loaded. Defaults have been restored. Your original file was backed up to templates.json.bak." Rename corrupt file to `.bak` before creating defaults.

### Pitfall 8: Template Names Not Unique
**What goes wrong:** User creates two templates with the same display name but different IDs.

**Consequences:** Template browser shows two entries with identical names. User can't tell which is which without inspecting configs.

**Prevention:** Don't enforce unique names (too restrictive — user should be able to have "Summarize (long)" and "Summarize (short)"). Instead, show template ID as subtitle in the browser: "Summarize (long) — `my_summarize_v2`". Or show description as tooltip on hover.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| `NodeTemplate` struct definition | Mirroring `AgentNodeConfig` fields instead of composing it (Pitfall 5) | Embed `AgentNodeConfig` in `NodeTemplate` as `config` field |
| `TemplateStore` implementation | Not using atomic writes (Pitfall 2) | Follow `FileSettingsStore::save()` pattern exactly |
| `TemplateStore` implementation | Overwriting user data on deserialization error (Pitfall 3) | Distinguish "file not found" from "invalid JSON"; backup corrupt files |
| `add_agent_node_from_template()` | Templates treated as node references, not copies (Pitfall 1) | Clone fields from template to node; never store template reference on node (only `template_id: Option<String>` for display) |
| Template browser UI | Blocking UI during save (Pitfall 6) | Hold write lock only for in-memory operations; file I/O outside lock |
| Built-in templates | ID collisions with user templates (Pitfall 4) | Prefix built-in IDs with `builtin.` or validate uniqueness on user template save |

## Sources

- `crates/agent-workflow-app/src/settings_store.rs` — atomic write pattern (write-temp-then-rename), legacy migration via `#[serde(default)]` (PRIMARY, read May 30 2026)
- `crates/agent-workflow-app/src/storage.rs` — JSON persistence, error handling on load (PRIMARY, read May 30 2026)
- `crates/workflow-core/src/model.rs` — `Node`, `AgentNodeConfig` struct definitions (PRIMARY, read May 30 2026)
- Historical pitfall research: Rust ecosystem patterns for file persistence (RwLock usage, atomic writes, serde error recovery) — MEDIUM confidence from community patterns; verified against existing codebase implementation
