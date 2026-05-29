# Compact Chat History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render chat history as compact inline transcript rows and keep the bottom chat composer inside the panel bounds.

**Architecture:** Keep the change inside `agent-workflow-app` UI. Add small pure helpers in `crates/agent-workflow-app/src/ui/canvas.rs` for role presentation metadata and chat section sizing, then update `show_chat_message` and `show_chat_content` to use those helpers.

**Tech Stack:** Rust, egui/eframe, existing `ui/theme.rs` tokens, existing `workflow_core::ChatRole`.

---

## File Structure

- Modify: `crates/agent-workflow-app/src/ui/canvas.rs`
  - Replace framed chat bubbles with inline transcript rows.
  - Add role metadata helper for labels, colors, monospace behavior.
  - Add chat sizing helper so history and composer heights stay bounded.
  - Add focused unit tests in the existing `#[cfg(test)] mod tests`.

No domain, transport, storage, or settings files change.

### Task 1: Add Pure Helpers And Tests

**Files:**
- Modify: `crates/agent-workflow-app/src/ui/canvas.rs`

- [ ] **Step 1: Write failing tests**

Add these tests to the existing test module in `crates/agent-workflow-app/src/ui/canvas.rs`:

```rust
#[test]
fn chat_role_presentations_match_inline_transcript_contract() {
    let system = chat_role_presentation(&ChatRole::System);
    assert_eq!(system.label, "System:");
    assert_eq!(system.text_color, TEXT_DIM);
    assert!(!system.monospace);

    let thinking = chat_role_presentation(&ChatRole::Thinking);
    assert_eq!(thinking.label, "Thinking:");
    assert_eq!(thinking.text_color, ACCENT);
    assert!(thinking.monospace);

    let assistant = chat_role_presentation(&ChatRole::Assistant);
    assert_eq!(assistant.label, "Assistant:");
    assert_eq!(assistant.text_color, TEXT_BRIGHT);
    assert!(!assistant.monospace);

    let user = chat_role_presentation(&ChatRole::User);
    assert_eq!(user.label, "You:");
    assert_eq!(user.text_color, TEXT_BRIGHT);
    assert!(!user.monospace);
}

#[test]
fn chat_section_heights_keep_composer_inside_available_height() {
    let heights = chat_section_heights(300.0, false);

    assert_eq!(heights.composer, CHAT_COMPOSER_RESERVED_HEIGHT);
    assert_eq!(heights.history, 300.0 - CHAT_COMPOSER_RESERVED_HEIGHT);
    assert!(heights.history + heights.composer <= 300.0);
}

#[test]
fn chat_section_heights_reserve_error_space_inside_composer_section() {
    let heights = chat_section_heights(300.0, true);

    assert_eq!(
        heights.composer,
        CHAT_COMPOSER_RESERVED_HEIGHT + CHAT_ERROR_RESERVED_HEIGHT
    );
    assert_eq!(
        heights.history,
        300.0 - CHAT_COMPOSER_RESERVED_HEIGHT - CHAT_ERROR_RESERVED_HEIGHT
    );
    assert!(heights.history + heights.composer <= 300.0);
}

#[test]
fn chat_section_heights_never_exceed_small_available_height() {
    let heights = chat_section_heights(72.0, true);

    assert_eq!(heights.history, 0.0);
    assert_eq!(heights.composer, 72.0);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p agent-workflow-app chat_ -- --nocapture
```

Expected: FAIL because `chat_role_presentation`, `chat_section_heights`, `CHAT_ERROR_RESERVED_HEIGHT`, and helper types do not exist yet.

- [ ] **Step 3: Add helper types and constants**

Add near the top-level chat constants in `crates/agent-workflow-app/src/ui/canvas.rs`:

```rust
const CHAT_ERROR_RESERVED_HEIGHT: f32 = 52.0;
const CHAT_ROLE_LABEL_WIDTH: f32 = 78.0;
const CHAT_ROW_GAP: f32 = 10.0;

#[derive(Debug, Clone, Copy)]
struct ChatRolePresentation {
    label: &'static str,
    label_color: egui::Color32,
    text_color: egui::Color32,
    monospace: bool,
}

#[derive(Debug, Clone, Copy)]
struct ChatSectionHeights {
    history: f32,
    composer: f32,
}
```

Add these helper functions near `chat_status_text`:

```rust
fn chat_role_presentation(role: &ChatRole) -> ChatRolePresentation {
    match role {
        ChatRole::User => ChatRolePresentation {
            label: "You:",
            label_color: ACCENT,
            text_color: TEXT_BRIGHT,
            monospace: false,
        },
        ChatRole::Assistant => ChatRolePresentation {
            label: "Assistant:",
            label_color: SUCCESS,
            text_color: TEXT_BRIGHT,
            monospace: false,
        },
        ChatRole::System => ChatRolePresentation {
            label: "System:",
            label_color: TEXT_DIM,
            text_color: TEXT_DIM,
            monospace: false,
        },
        ChatRole::Thinking => ChatRolePresentation {
            label: "Thinking:",
            label_color: ACCENT,
            text_color: ACCENT,
            monospace: true,
        },
    }
}

fn chat_section_heights(available_height: f32, has_error: bool) -> ChatSectionHeights {
    let reserved = CHAT_COMPOSER_RESERVED_HEIGHT
        + if has_error {
            CHAT_ERROR_RESERVED_HEIGHT
        } else {
            0.0
        };
    let composer = reserved.min(available_height);

    ChatSectionHeights {
        history: (available_height - composer).max(0.0),
        composer,
    }
}
```

- [ ] **Step 4: Run tests to verify pass**

Run:

```bash
cargo test -p agent-workflow-app chat_ -- --nocapture
```

Expected: PASS.

### Task 2: Render Inline Transcript Rows

**Files:**
- Modify: `crates/agent-workflow-app/src/ui/canvas.rs`

- [ ] **Step 1: Replace `show_chat_message` implementation**

Replace the existing bubble-based `show_chat_message` function with:

```rust
fn show_chat_message(ui: &mut egui::Ui, role: &ChatRole, content: &str) {
    let presentation = chat_role_presentation(role);
    let available_width = ui.available_width();
    let text_width = (available_width - CHAT_ROLE_LABEL_WIDTH - CHAT_ROW_GAP).max(120.0);

    ui.horizontal_top(|ui| {
        ui.add_sized(
            [CHAT_ROLE_LABEL_WIDTH, 0.0],
            egui::Label::new(
                egui::RichText::new(presentation.label)
                    .size(TS_LABEL)
                    .color(presentation.label_color)
                    .strong(),
            ),
        );
        ui.add_space(CHAT_ROW_GAP);

        let text = egui::RichText::new(content)
            .size(TS_BODY)
            .color(presentation.text_color);
        let label = if presentation.monospace {
            egui::Label::new(text.monospace())
        } else {
            egui::Label::new(text)
        }
        .wrap()
        .selectable(true);

        ui.add_sized([text_width, 0.0], label);
    });
}
```

- [ ] **Step 2: Tighten message spacing**

In the `for msg in &messages` loop inside `show_chat_content`, change:

```rust
ui.add_space(6.0);
```

to:

```rust
ui.add_space(4.0);
```

- [ ] **Step 3: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS or report required formatting.

### Task 3: Keep Chat Sections Inside Panel Bounds

**Files:**
- Modify: `crates/agent-workflow-app/src/ui/canvas.rs`

- [ ] **Step 1: Use helper for history/composer split**

In `show_chat_content`, replace:

```rust
let composer_h = CHAT_COMPOSER_RESERVED_HEIGHT.min(available_h);
let history_h = (available_h - composer_h).max(0.0);
```

with:

```rust
let heights = chat_section_heights(available_h, last_error.is_some());
let history_h = heights.history;
let composer_h = heights.composer;
```

- [ ] **Step 2: Keep error bar visually compact**

In `show_chat_error`, keep the frame, icon, copy button, and retry button, but make sure it remains inside the composer allocation by leaving it above `show_chat_composer` and using the existing `horizontal_wrapped` layout.

- [ ] **Step 3: Run targeted tests**

Run:

```bash
cargo test -p agent-workflow-app chat_ -- --nocapture
```

Expected: PASS.

### Task 4: Full Verification

**Files:**
- Verify only.

- [ ] **Step 1: Format code**

Run:

```bash
cargo fmt --all
```

Expected: command exits 0.

- [ ] **Step 2: Check formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit implementation**

Run:

```bash
git add crates/agent-workflow-app/src/ui/canvas.rs docs/superpowers/plans/2026-05-29-compact-chat-history.md
git commit -m "fix: compact chat history layout"
```
