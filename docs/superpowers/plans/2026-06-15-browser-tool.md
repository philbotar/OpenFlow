# Browser Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an OMP-style `browser` builtin with named tabs, `open`/`close`/`run` actions, accessibility-oriented text extraction, and screenshot artifacts.

**Architecture:** Own browser lifecycle in `orchestration/src/browser/` using `chromiumoxide` (headless Chromium over CDP). One `BrowserSupervisor` per active run on `ToolRunner`. Tabs keyed by `name` (default `"main"`). `run` executes async JS in page context; `display()` calls become text blocks. Screenshots spill to run artifacts. Exclusive concurrency. Cleanup all tabs on run end.

**Tech Stack:** Rust, `chromiumoxide`, Tokio, existing `ArtifactStore`, `ToolUpdated` (optional progress for long `run`).

**Reference:** `oh-my-pi/docs/tools/browser.md`, `oh-my-pi/docs/omp-tool-analysis/07-browser-tool.md`.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/orchestration/Cargo.toml` | `chromiumoxide` dependency |
| `crates/orchestration/src/browser/mod.rs` | `BrowserSupervisor`, tab registry |
| `crates/orchestration/src/browser/tab.rs` | Single tab: navigate, eval, screenshot, extract text |
| `crates/orchestration/src/adapters/tool_impl/browser.rs` | `execute_browser` dispatch |
| `crates/orchestration/src/tool/registry.rs` | `browser` schema |
| `crates/orchestration/src/tool/runner.rs` | Supervisor on runner; cleanup hook |
| `crates/engine/src/execution/node_invocation.rs` | Preamble: prefer browser over bash+curl for pages |

## V1 Scope

- Headless Chromium only (no CDP attach to user Chrome).
- Actions: `open`, `close`, `run`.
- `open`: optional `url`, `viewport`, `wait_until` (`load` default).
- `run`: `code` string evaluated as async IIFE with `page`, `tab`, `display` in scope.
- `tab.extract()` → main content text (readability-lite: strip scripts/styles, return innerText).
- `tab.screenshot()` → PNG artifact + image reference in result text.
- Wall timeout default 30s, clamp 5–120s.
- Close tab or `close` with `all: true` on run stop.

## Out of Scope

- Stealth injection pack (OMP puppeteer scripts).
- CDP attach to spawned desktop apps.
- Worker-thread isolation (run in Tokio task with timeout).
- TUI-specific renderers.

---

### Task 1: Browser Supervisor

**Files:**
- Create: `crates/orchestration/src/browser/mod.rs`
- Create: `crates/orchestration/src/browser/tab.rs`
- Modify: `crates/orchestration/src/lib.rs`
- Test: `crates/orchestration/src/browser/mod.rs`

- [ ] **Step 1: Write failing supervisor test**

```rust
#[tokio::test]
#[ignore = "requires chromium; set STEP_BROWSER_LIVE=1"]
async fn supervisor_opens_named_tab() {
    let mut supervisor = BrowserSupervisor::launch().await.expect("launch");
    let tab = supervisor.open_tab("main", OpenTabOptions {
        url: Some("data:text/html,<h1>hello</h1>".to_string()),
        ..Default::default()
    }).await.expect("open");
    let title = tab.title().await.expect("title");
    assert!(title.contains("hello") || !title.is_empty());
    supervisor.close_all().await;
}
```

- [ ] **Step 2: Implement `BrowserSupervisor`**

```rust
pub struct BrowserSupervisor {
    browser: chromiumoxide::Browser,
    tabs: HashMap<String, BrowserTabHandle>,
}

impl BrowserSupervisor {
    pub async fn launch() -> Result<Self, BrowserError> {
        let (browser, mut handler) = chromiumoxide::Browser::launch(
            chromiumoxide::browser::LaunchOptions::default_builder()
                .headless(true)
                .build()
                .map_err(BrowserError::launch)?,
        )
        .await?;
        tokio::spawn(async move {
            while let Some(evt) = handler.next().await {
                let _ = evt;
            }
        });
        Ok(Self { browser, tabs: HashMap::new() })
    }

    pub async fn open_tab(&mut self, name: &str, opts: OpenTabOptions) -> Result<&BrowserTabHandle, BrowserError> { /* reuse or create */ }

    pub async fn close_tab(&mut self, name: &str) -> Result<(), BrowserError> { /* ... */ }

    pub async fn close_all(&mut self) { /* close every tab + browser */ }
}
```

- [ ] **Step 3: Run unit test for tab name reuse (no network)**

```rust
#[test]
fn tab_names_default_to_main() {
    assert_eq!(normalize_tab_name(None), "main");
    assert_eq!(normalize_tab_name(Some("checkout")), "checkout");
}
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(browser): add BrowserSupervisor with named tabs"
```

---

### Task 2: Browser Tool Schema + Open/Close

**Files:**
- Modify: `crates/orchestration/src/tool/registry.rs`
- Create: `crates/orchestration/src/adapters/tool_impl/browser.rs`
- Modify: `crates/orchestration/src/adapters/tool_impl/mod.rs`

- [ ] **Step 1: Register browser tool**

```rust
fn browser_tool() -> RegisteredTool {
    RegisteredTool {
        definition: ToolDefinition {
            name: "browser".to_string(),
            description: "Control a headless browser tab. Actions: open (navigate), close, run (execute JS). Prefer over bash+curl for web pages.".to_string(),
            input_schema: with_intent_field(serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "action": { "type": "string", "enum": ["open", "close", "run"] },
                    "name": { "type": "string", "description": "Tab id (default main)" },
                    "url": { "type": "string" },
                    "timeout": { "type": "integer", "description": "Seconds (default 30)" },
                    "all": { "type": "boolean", "description": "Close all tabs (close action)" },
                    "code": { "type": "string", "description": "JS for run action" }
                },
                "required": ["action"]
            })),
            tier: ToolTier::Exec,
            concurrency: ToolConcurrency::Exclusive,
        },
        kind: BuiltinToolKind::Browser,
    }
}
```

- [ ] **Step 2: Implement open/close handlers**

```rust
pub async fn execute_browser(
    supervisor: &mut BrowserSupervisor,
    args: Value,
    artifacts: &ArtifactStore,
    cancel: &CancellationToken,
) -> Result<String, ToolError> {
    let action = args.get("action").and_then(|v| v.as_str()).ok_or(/* InvalidArgs */)?;
    match action {
        "open" => { /* open_tab + optional navigate */ }
        "close" => { /* close one or all */ }
        "run" => execute_browser_run(supervisor, args, artifacts, cancel).await,
        _ => Err(ToolError::InvalidArgs { /* ... */ }),
    }
}
```

- [ ] **Step 3: Write open/close tests with mock supervisor trait**

Use a `BrowserPort` trait for unit tests without Chromium.

- [ ] **Step 4: Commit**

---

### Task 3: Run Action + JS Eval

**Files:**
- Modify: `crates/orchestration/src/browser/tab.rs`
- Modify: `crates/orchestration/src/adapters/tool_impl/browser.rs`

- [ ] **Step 1: Write failing run test (live)**

```rust
#[tokio::test]
#[ignore = "requires STEP_BROWSER_LIVE=1"]
async fn browser_run_evaluates_expression() {
    let mut supervisor = BrowserSupervisor::launch().await.unwrap();
    supervisor.open_tab("main", OpenTabOptions::default()).await.unwrap();
    let out = execute_browser(
        &mut supervisor,
        serde_json::json!({
            "action": "run",
            "code": "return await tab.extract();"
        }),
        &artifact_store_fixture(),
        &CancellationToken::new(),
    ).await.unwrap();
    assert!(!out.is_empty());
}
```

- [ ] **Step 2: Implement `execute_browser_run`**

Wrap user `code` as:

```javascript
(async () => {
  const display = (value) => { __DISPLAY__.push(String(value)); };
  const tab = { extract: () => document.body.innerText, screenshot: async () => { /* ... */ } };
  const page = globalThis;
  const displays = [];
  const result = await (async () => { CODE })();
  return { displays, result };
})()
```

Use `Page::evaluate` with timeout from args.

- [ ] **Step 3: Screenshot → artifact**

On `tab.screenshot()`, capture PNG bytes, `artifacts.store_bytes("browser", bytes)`, append `artifact:{id}` to result.

- [ ] **Step 4: Commit**

---

### Task 4: Wire ToolRunner + Cleanup

**Files:**
- Modify: `crates/orchestration/src/tool/runner.rs`
- Modify: `crates/orchestration/src/run/execution/drive.rs`

- [ ] **Step 1: Add `BrowserSupervisor` to `ToolRunner`**

Lazy-init on first `browser` call; `Mutex` for exclusive access.

- [ ] **Step 2: `Drop` or explicit cleanup on run end**

In `drive.rs` finally block, call `tool_runner.close_browser().await`.

- [ ] **Step 3: Update preamble**

Add browser when-to-use guidance to `NODE_RUNTIME_PREAMBLE`.

- [ ] **Step 4: Verify**

Run: `./scripts/verify.sh`

- [ ] **Step 5: Commit**

---

## Self-Review

| OMP feature | Task |
| --- | --- |
| Named tabs | Task 1 |
| open/close/run | Tasks 2–3 |
| extract text | Task 3 |
| screenshots | Task 3 |
| Exclusive concurrency | Task 2 schema |
| Stealth/CDP attach | Out of scope |
