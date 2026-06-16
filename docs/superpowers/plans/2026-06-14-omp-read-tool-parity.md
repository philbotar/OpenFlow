# OMP Read Tool Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Execution status (2026-06-15):** Tasks 0–4 **done** — `tool/read/` module (selectors, render, summary, directory), wired through `blocking_ops` and `dispatch`.

**Goal:** Bring Step-through's `read` tool up to OMP-style behavior: high default cap, targeted selectors, structural summaries, safe truncation, artifact recovery, and one coherent read surface for local files, URLs, and run artifacts.

**Architecture:** Keep execution ownership in `crates/orchestration/src/tool/`; do not move read behavior into `engine` or UI. Extract read-specific parsing/rendering into focused modules under `crates/orchestration/src/tool/read/`, keep `ToolRunner` dispatch thin, and keep artifact spill/recovery through the existing `ArtifactStore`. The engine only receives updated prompt guidance and tool definition text.

**Tech Stack:** Rust, Tokio `spawn_blocking`, `regex`, existing `reqwest`, existing `ArtifactStore`, existing edit snapshot store, `cargo test -p orchestration`.

---

## Scope Check

OMP's `read` handles many independent domains: local files, ranges, structural summaries, directories, archives, SQLite, documents, images, URLs, internal URIs, hashline snapshots, and renderer metadata. Building all of that in one slice would be too risky.

This plan makes Step-through's current read surface OMP-grade for the domains it already supports: local files, directories, HTTP(S) URLs, and `artifact:{id}`. After this plan, separate specs can add archive, SQLite, document, and image support without rewriting the selector/rendering core.

## Current Baseline

Step-through currently has:

- `crates/orchestration/src/tool/blocking_ops.rs` with `split_selector`, `apply_read_selector`, local file reads, and directory listing.
- `crates/orchestration/src/tool/dispatch.rs` with `read` dispatch across local files, HTTP(S) URLs, and artifacts.
- `crates/orchestration/src/tool/output.rs` with 50 KB artifact spill and `artifact:{id}` recovery.
- `crates/orchestration/src/tool/registry.rs` and `crates/engine/src/execution/node_invocation.rs` with model-facing read guidance.

Immediate cap parity is already applied in this branch:

- `DEFAULT_READ_LINE_LIMIT` is now `3000`.
- Registry and node preamble now describe a 3000-line cap.
- The focused truncation test now uses 3005 lines.

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/orchestration/src/tool/read/mod.rs` | Public read module facade used by blocking local reads, artifact reads, and URL reads. |
| `crates/orchestration/src/tool/read/selector.rs` | Parse OMP-style selectors: none, raw, conflicts, single range, open range, plus range, multi-range, and range+raw. |
| `crates/orchestration/src/tool/read/render.rs` | Render selected text with line numbers, hashline header, continuation notices, byte/line caps, and per-line column cap. |
| `crates/orchestration/src/tool/read/summary.rs` | Produce structural summaries for parseable code without sending full bodies. |
| `crates/orchestration/src/tool/read/directory.rs` | Render depth-limited directory listings with deterministic sorting and truncation notices. |
| `crates/orchestration/src/tool/blocking_ops.rs` | Delegate local file and directory reads to the new read module; keep edit/search/write here. |
| `crates/orchestration/src/tool/dispatch.rs` | Delegate artifact and URL text through the same read renderer as local files. |
| `crates/orchestration/src/tool/registry.rs` | Update tool schema description with selector syntax and summary behavior. |
| `crates/engine/src/execution/node_invocation.rs` | Update `NODE_RUNTIME_PREAMBLE` so every node learns targeted re-read behavior. |
| `crates/orchestration/src/tool/runner.rs` | Add integration tests through `ToolRunner`, including local, URL, and artifact reads. |
| `docs/contributing/testing-workflows.md` | Add the focused read-tool verification commands. |

## Invariants

- `read` remains a read-tier, shared-concurrency tool.
- `:raw` returns unnumbered content but still goes through the existing final output spill path when the returned text exceeds artifact limits.
- Bare code reads should prefer structural summary when possible; plain text reads should return numbered lines up to the configured cap.
- Summary elisions must include exact re-read ranges. The model must not need to guess what `..` means.
- URL and artifact reads must accept the same selectors as local files.
- Existing edit snapshot recording must keep working for local reads.

### Task 0: Land OMP Default Cap

**Files:**
- Modify: `crates/orchestration/src/tool/blocking_ops.rs`
- Modify: `crates/orchestration/src/tool/registry.rs`
- Modify: `crates/engine/src/execution/node_invocation.rs`
- Modify: `crates/orchestration/src/tool/runner.rs`

- [x] **Step 1: Change the default read cap**

In `crates/orchestration/src/tool/blocking_ops.rs`, set:

```rust
const DEFAULT_READ_LINE_LIMIT: usize = 3000;
```

- [x] **Step 2: Update model-facing read text**

In `crates/orchestration/src/tool/registry.rs`, use:

```rust
description: "Read a local file, directory listing, HTTP(S) URL, or spilled tool artifact. Default output is numbered lines capped at 3000 lines; append :N-M for a line range (e.g. src/lib.rs:10-20) or :raw for full unnumbered content. Truncated tool output can be read via artifact:{id} (supports the same selectors).".to_string(),
```

In `crates/engine/src/execution/node_invocation.rs`, use:

```rust
output is numbered lines (3000-line cap). Append :start-end for a line range (e.g. src/lib.rs:10-20) \
```

- [x] **Step 3: Update the focused truncation test**

In `crates/orchestration/src/tool/runner.rs`, the `read_file_without_selector_announces_truncation` test should write 3005 lines and assert:

```rust
assert!(record.result.content.contains("3000:line-3000"));
assert!(!record.result.content.contains("3001:line-3001"));
assert!(record
    .result
    .content
    .contains("truncated at line 3000 of 3005"));
```

- [x] **Step 4: Verify the cap change**

Run:

```bash
cargo test -p orchestration read_file_without_selector_announces_truncation
```

Expected: PASS.

### Task 1: Extract Selector Parser

**Files:**
- Create: `crates/orchestration/src/tool/read/mod.rs`
- Create: `crates/orchestration/src/tool/read/selector.rs`
- Modify: `crates/orchestration/src/tool/blocking_ops.rs`

- [ ] **Step 1: Create failing selector tests**

Create `crates/orchestration/src/tool/read/selector.rs` with this initial content:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadSelector {
    None,
    Raw,
    Conflicts,
    Lines {
        ranges: Vec<LineRange>,
        raw: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

pub fn split_selector(path: &str) -> (String, ReadSelector) {
    (path.to_string(), ReadSelector::None)
}

#[cfg(test)]
mod tests {
    use super::{LineRange, ReadSelector, split_selector};

    #[test]
    fn parses_raw_selector() {
        assert_eq!(split_selector("src/lib.rs:raw"), ("src/lib.rs".to_string(), ReadSelector::Raw));
    }

    #[test]
    fn parses_closed_range() {
        assert_eq!(
            split_selector("src/lib.rs:50-80"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![LineRange { start: 50, end: Some(80) }],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_plus_range() {
        assert_eq!(
            split_selector("src/lib.rs:50+150"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![LineRange { start: 50, end: Some(199) }],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_multi_range() {
        assert_eq!(
            split_selector("src/lib.rs:5-16,960-973"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![
                        LineRange { start: 5, end: Some(16) },
                        LineRange { start: 960, end: Some(973) },
                    ],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_range_and_raw_selector_in_either_order() {
        assert_eq!(
            split_selector("src/lib.rs:2-4:raw").1,
            ReadSelector::Lines {
                ranges: vec![LineRange { start: 2, end: Some(4) }],
                raw: true,
            },
        );
        assert_eq!(
            split_selector("src/lib.rs:raw:2-4").1,
            ReadSelector::Lines {
                ranges: vec![LineRange { start: 2, end: Some(4) }],
                raw: true,
            },
        );
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p orchestration tool::read::selector
```

Expected: FAIL because only `:raw` and no-op parsing work.

- [ ] **Step 3: Implement selector parsing**

Replace the body of `selector.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadSelector {
    None,
    Raw,
    Conflicts,
    Lines {
        ranges: Vec<LineRange>,
        raw: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

pub fn split_selector(path: &str) -> (String, ReadSelector) {
    let Some(index) = path.rfind(':') else {
        return (path.to_string(), ReadSelector::None);
    };
    let base = &path[..index];
    let suffix = &path[index + 1..];

    if suffix.eq_ignore_ascii_case("raw") {
        let (maybe_base, selector) = split_selector(base);
        if let ReadSelector::Lines { ranges, .. } = selector {
            return (maybe_base, ReadSelector::Lines { ranges, raw: true });
        }
        return (base.to_string(), ReadSelector::Raw);
    }

    if suffix.eq_ignore_ascii_case("conflicts") {
        return (base.to_string(), ReadSelector::Conflicts);
    }

    if let Some(ranges) = parse_ranges(suffix) {
        let (maybe_base, selector) = split_selector(base);
        let raw = matches!(selector, ReadSelector::Raw);
        let base = if raw { maybe_base } else { base.to_string() };
        return (base, ReadSelector::Lines { ranges, raw });
    }

    (path.to_string(), ReadSelector::None)
}

fn parse_ranges(input: &str) -> Option<Vec<LineRange>> {
    let mut ranges = Vec::new();
    for part in input.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return None;
        }
        ranges.push(parse_range(trimmed)?);
    }
    ranges.sort_by_key(|range| range.start);
    ranges.dedup();
    Some(merge_ranges(ranges))
}

fn parse_range(input: &str) -> Option<LineRange> {
    if let Some((start, count)) = input.split_once('+') {
        let start = parse_line(start)?;
        let count = parse_line(count)?;
        return Some(LineRange {
            start,
            end: Some(start.saturating_add(count).saturating_sub(1)),
        });
    }
    if let Some((start, end)) = input.split_once('-') {
        let start = parse_line(start)?;
        let end = if end.is_empty() {
            None
        } else {
            Some(parse_line(end)?.max(start))
        };
        return Some(LineRange { start, end });
    }
    let start = parse_line(input)?;
    Some(LineRange { start, end: Some(start) })
}

fn parse_line(input: &str) -> Option<usize> {
    input.parse::<usize>().ok().filter(|value| *value > 0)
}

fn merge_ranges(ranges: Vec<LineRange>) -> Vec<LineRange> {
    let mut merged: Vec<LineRange> = Vec::new();
    for range in ranges {
        let Some(last) = merged.last_mut() else {
            merged.push(range);
            continue;
        };
        match (last.end, range.end) {
            (None, _) => {}
            (Some(last_end), None) if range.start <= last_end.saturating_add(1) => {
                last.end = None;
            }
            (Some(last_end), Some(end)) if range.start <= last_end.saturating_add(1) => {
                last.end = Some(last_end.max(end));
            }
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::{LineRange, ReadSelector, split_selector};

    #[test]
    fn parses_raw_selector() {
        assert_eq!(split_selector("src/lib.rs:raw"), ("src/lib.rs".to_string(), ReadSelector::Raw));
    }

    #[test]
    fn parses_closed_range() {
        assert_eq!(
            split_selector("src/lib.rs:50-80"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![LineRange { start: 50, end: Some(80) }],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_plus_range() {
        assert_eq!(
            split_selector("src/lib.rs:50+150"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![LineRange { start: 50, end: Some(199) }],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_multi_range() {
        assert_eq!(
            split_selector("src/lib.rs:5-16,960-973"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![
                        LineRange { start: 5, end: Some(16) },
                        LineRange { start: 960, end: Some(973) },
                    ],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_range_and_raw_selector_in_either_order() {
        assert_eq!(
            split_selector("src/lib.rs:2-4:raw").1,
            ReadSelector::Lines {
                ranges: vec![LineRange { start: 2, end: Some(4) }],
                raw: true,
            },
        );
        assert_eq!(
            split_selector("src/lib.rs:raw:2-4").1,
            ReadSelector::Lines {
                ranges: vec![LineRange { start: 2, end: Some(4) }],
                raw: true,
            },
        );
    }
}
```

Create `crates/orchestration/src/tool/read/mod.rs`:

```rust
pub mod selector;
```

Add this to `crates/orchestration/src/tool/mod.rs`:

```rust
mod read;
```

- [ ] **Step 4: Run selector tests**

Run:

```bash
cargo test -p orchestration tool::read::selector
```

Expected: PASS.

- [ ] **Step 5: Commit selector extraction**

```bash
git add crates/orchestration/src/tool/read/mod.rs crates/orchestration/src/tool/read/selector.rs crates/orchestration/src/tool/mod.rs
git commit -m "refactor: extract read selector parsing"
```

### Task 2: Add OMP-Style Render Core

**Files:**
- Create: `crates/orchestration/src/tool/read/render.rs`
- Modify: `crates/orchestration/src/tool/read/mod.rs`
- Modify: `crates/orchestration/src/tool/blocking_ops.rs`
- Modify: `crates/orchestration/src/tool/dispatch.rs`
- Test: `crates/orchestration/src/tool/runner.rs`

- [ ] **Step 1: Write render tests**

Create `crates/orchestration/src/tool/read/render.rs` with tests first:

```rust
use super::selector::{LineRange, ReadSelector};

pub const DEFAULT_READ_LINE_LIMIT: usize = 3000;
pub const DEFAULT_READ_BYTE_LIMIT: usize = 50_000;

pub fn render_text(label: &str, text: &str, selector: &ReadSelector) -> String {
    match selector {
        ReadSelector::Raw => text.to_string(),
        _ => format!("¶{label}\n{text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{LineRange, ReadSelector, render_text};

    #[test]
    fn bare_text_is_numbered_and_limited_to_3000_lines() {
        let input = (1..=3005).map(|line| format!("line-{line}")).collect::<Vec<_>>().join("\n");
        let output = render_text("big.txt", &input, &ReadSelector::None);
        assert!(output.contains("3000:line-3000"));
        assert!(!output.contains("3001:line-3001"));
        assert!(output.contains("truncated at line 3000 of 3005"));
    }

    #[test]
    fn multi_range_keeps_requested_line_numbers() {
        let input = (1..=10).map(|line| format!("line-{line}")).collect::<Vec<_>>().join("\n");
        let output = render_text(
            "note.txt",
            &input,
            &ReadSelector::Lines {
                ranges: vec![
                    LineRange { start: 2, end: Some(3) },
                    LineRange { start: 9, end: Some(10) },
                ],
                raw: false,
            },
        );
        assert!(output.contains("2:line-2"));
        assert!(output.contains("3:line-3"));
        assert!(output.contains("…"));
        assert!(output.contains("9:line-9"));
        assert!(output.contains("10:line-10"));
    }

    #[test]
    fn range_raw_returns_verbatim_without_header() {
        let input = "a\nb\nc";
        let output = render_text(
            "note.txt",
            input,
            &ReadSelector::Lines {
                ranges: vec![LineRange { start: 2, end: Some(3) }],
                raw: true,
            },
        );
        assert_eq!(output, "b\nc");
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p orchestration tool::read::render
```

Expected: FAIL because `render_text` is still a stub.

- [ ] **Step 3: Implement render core**

Replace `render.rs` with:

```rust
use super::selector::{LineRange, ReadSelector};

pub const DEFAULT_READ_LINE_LIMIT: usize = 3000;
pub const DEFAULT_READ_BYTE_LIMIT: usize = 50_000;
const RANGE_SEPARATOR: &str = "…";

pub fn render_text(label: &str, text: &str, selector: &ReadSelector) -> String {
    match selector {
        ReadSelector::Raw => text.to_string(),
        ReadSelector::Lines { ranges, raw } => render_ranges(label, text, ranges, *raw),
        ReadSelector::None | ReadSelector::Conflicts => render_default(label, text),
    }
}

fn render_default(label: &str, text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let total_lines = lines.len();
    let selected = lines
        .iter()
        .take(DEFAULT_READ_LINE_LIMIT)
        .enumerate()
        .map(|(index, line)| format!("{}:{}", index + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    let mut output = format!("¶{label}\n{selected}");
    if total_lines > DEFAULT_READ_LINE_LIMIT {
        output.push_str(&format!(
            "\n… truncated at line {DEFAULT_READ_LINE_LIMIT} of {total_lines}; use :{{start}}-{{end}} or :raw to read more …"
        ));
    }
    output
}

fn render_ranges(label: &str, text: &str, ranges: &[LineRange], raw: bool) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    for range in ranges {
        let start_index = range.start.saturating_sub(1).min(lines.len());
        let end_index = range.end.unwrap_or(lines.len()).min(lines.len());
        if start_index >= end_index {
            chunks.push(format!("[range {} is beyond end of file: {} lines]", range.start, lines.len()));
            continue;
        }
        let chunk = lines[start_index..end_index]
            .iter()
            .enumerate()
            .map(|(offset, line)| {
                if raw {
                    (*line).to_string()
                } else {
                    format!("{}:{}", start_index + offset + 1, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        chunks.push(chunk);
    }
    let body = chunks.join(&format!("\n{RANGE_SEPARATOR}\n"));
    if raw {
        body
    } else {
        format!("¶{label}\n{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::{LineRange, ReadSelector, render_text};

    #[test]
    fn bare_text_is_numbered_and_limited_to_3000_lines() {
        let input = (1..=3005).map(|line| format!("line-{line}")).collect::<Vec<_>>().join("\n");
        let output = render_text("big.txt", &input, &ReadSelector::None);
        assert!(output.contains("3000:line-3000"));
        assert!(!output.contains("3001:line-3001"));
        assert!(output.contains("truncated at line 3000 of 3005"));
    }

    #[test]
    fn multi_range_keeps_requested_line_numbers() {
        let input = (1..=10).map(|line| format!("line-{line}")).collect::<Vec<_>>().join("\n");
        let output = render_text(
            "note.txt",
            &input,
            &ReadSelector::Lines {
                ranges: vec![
                    LineRange { start: 2, end: Some(3) },
                    LineRange { start: 9, end: Some(10) },
                ],
                raw: false,
            },
        );
        assert!(output.contains("2:line-2"));
        assert!(output.contains("3:line-3"));
        assert!(output.contains("…"));
        assert!(output.contains("9:line-9"));
        assert!(output.contains("10:line-10"));
    }

    #[test]
    fn range_raw_returns_verbatim_without_header() {
        let input = "a\nb\nc";
        let output = render_text(
            "note.txt",
            input,
            &ReadSelector::Lines {
                ranges: vec![LineRange { start: 2, end: Some(3) }],
                raw: true,
            },
        );
        assert_eq!(output, "b\nc");
    }
}
```

Update `crates/orchestration/src/tool/read/mod.rs`:

```rust
pub mod render;
pub mod selector;

pub use render::render_text;
pub use selector::{ReadSelector, split_selector};
```

- [ ] **Step 4: Wire local, URL, and artifact reads through `render_text`**

In `crates/orchestration/src/tool/blocking_ops.rs`, replace imports and helpers that use the old selector:

```rust
use crate::tool::read::{render_text, split_selector};
```

In `read_local`, replace:

```rust
let (path, selector) = split_selector(path);
```

with:

```rust
let (path, selector) = split_selector(path);
```

and replace the final line:

```rust
Ok(apply_read_selector(&path, &text, selector.as_deref()))
```

with:

```rust
Ok(render_text(&path, &text, &selector))
```

Remove the old `LINE_SELECTOR`, `DEFAULT_READ_LINE_LIMIT`, `split_selector`, `apply_read_selector`, and `parse_range` definitions from `blocking_ops.rs`.

In `crates/orchestration/src/tool/dispatch.rs`, import the new read module:

```rust
use crate::tool::read::{render_text, split_selector};
```

and replace `apply_read_selector(label, &text, selector)` with:

```rust
render_text(label, &text, &selector)
```

- [ ] **Step 5: Run render and integration tests**

Run:

```bash
cargo test -p orchestration tool::read::render
cargo test -p orchestration read_file_selector_returns_numbered_lines read_file_without_selector_announces_truncation read_artifact_round_trip_and_unknown_id
```

Expected: PASS.

- [ ] **Step 6: Commit render extraction**

```bash
git add crates/orchestration/src/tool/read/render.rs crates/orchestration/src/tool/read/mod.rs crates/orchestration/src/tool/blocking_ops.rs crates/orchestration/src/tool/dispatch.rs crates/orchestration/src/tool/runner.rs
git commit -m "refactor: share read rendering across sources"
```

### Task 3: Add Structural Summaries for Code Reads

**Files:**
- Create: `crates/orchestration/src/tool/read/summary.rs`
- Modify: `crates/orchestration/src/tool/read/mod.rs`
- Modify: `crates/orchestration/src/tool/read/render.rs`
- Test: `crates/orchestration/src/tool/runner.rs`

- [ ] **Step 1: Write summary tests**

Create `crates/orchestration/src/tool/read/summary.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralSummary {
    pub text: String,
    pub elided_ranges: Vec<(usize, usize)>,
    pub elided_lines: usize,
}

pub fn summarize_code(path: &str, text: &str) -> Option<StructuralSummary> {
    let _ = (path, text);
    None
}

#[cfg(test)]
mod tests {
    use super::summarize_code;

    #[test]
    fn summarizes_large_rust_function_body_with_recovery_range() {
        let source = r#"pub fn alpha() {
    let a = 1;
    let b = 2;
    let c = 3;
    println!("{a}{b}{c}");
}

pub fn beta() {
    println!("small");
}
"#;
        let summary = summarize_code("src/lib.rs", source).expect("summary");
        assert!(summary.text.contains("pub fn alpha() {"));
        assert!(summary.text.contains(".."));
        assert!(summary.text.contains("pub fn beta() {"));
        assert!(summary.text.contains("["));
        assert!(summary.text.contains("re-read needed ranges"));
        assert_eq!(summary.elided_ranges, vec![(2, 5)]);
    }

    #[test]
    fn skips_non_code_files() {
        assert!(summarize_code("notes.txt", "hello\nworld").is_none());
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p orchestration tool::read::summary
```

Expected: FAIL because `summarize_code` returns `None`.

- [ ] **Step 3: Implement deterministic brace summary**

Replace `summary.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralSummary {
    pub text: String,
    pub elided_ranges: Vec<(usize, usize)>,
    pub elided_lines: usize,
}

const MIN_TOTAL_LINES: usize = 8;
const MIN_BODY_LINES: usize = 4;

pub fn summarize_code(path: &str, text: &str) -> Option<StructuralSummary> {
    if !is_code_path(path) {
        return None;
    }
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() < MIN_TOTAL_LINES {
        return None;
    }

    let mut output = Vec::new();
    let mut elided_ranges = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if looks_like_block_header(line) {
            let Some(end_index) = find_block_end(&lines, index) else {
                output.push(format!("{}:{}", index + 1, line));
                index += 1;
                continue;
            };
            let body_start = index + 1;
            let body_end = end_index.saturating_sub(1);
            let body_lines = body_end.saturating_sub(body_start).saturating_add(1);
            if body_lines >= MIN_BODY_LINES {
                output.push(format!("{}:{}", index + 1, line));
                output.push("..".to_string());
                output.push(format!("{}:{}", end_index + 1, lines[end_index]));
                elided_ranges.push((body_start + 1, body_end + 1));
                index = end_index + 1;
                continue;
            }
        }
        output.push(format!("{}:{}", index + 1, line));
        index += 1;
    }

    if elided_ranges.is_empty() {
        return None;
    }

    let elided_lines = elided_ranges.iter().map(|(start, end)| end - start + 1).sum::<usize>();
    let example = elided_ranges
        .iter()
        .take(3)
        .map(|(start, end)| format!("{start}-{end}"))
        .collect::<Vec<_>>()
        .join(",");
    output.push(format!(
        "[{elided_lines} lines elided; re-read needed ranges, e.g. {path}:{example}]"
    ));

    Some(StructuralSummary {
        text: output.join("\n"),
        elided_ranges,
        elided_lines,
    })
}

fn is_code_path(path: &str) -> bool {
    matches!(
        path.rsplit('.').next(),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "cpp" | "h" | "hpp")
    )
}

fn looks_like_block_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.ends_with('{')
        && (trimmed.starts_with("fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("export function ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("impl ")
            || trimmed.starts_with("mod "))
}

fn find_block_end(lines: &[&str], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::summarize_code;

    #[test]
    fn summarizes_large_rust_function_body_with_recovery_range() {
        let source = r#"pub fn alpha() {
    let a = 1;
    let b = 2;
    let c = 3;
    println!("{a}{b}{c}");
}

pub fn beta() {
    println!("small");
}
"#;
        let summary = summarize_code("src/lib.rs", source).expect("summary");
        assert!(summary.text.contains("1:pub fn alpha() {"));
        assert!(summary.text.contains(".."));
        assert!(summary.text.contains("8:pub fn beta() {"));
        assert!(summary.text.contains("re-read needed ranges"));
        assert_eq!(summary.elided_ranges, vec![(2, 5)]);
    }

    #[test]
    fn skips_non_code_files() {
        assert!(summarize_code("notes.txt", "hello\nworld").is_none());
    }
}
```

- [ ] **Step 4: Wire bare code reads to summary**

In `crates/orchestration/src/tool/read/mod.rs`, add:

```rust
pub mod summary;
```

In `render.rs`, import summary:

```rust
use super::summary::summarize_code;
```

At the top of `render_text`, before the existing match, add:

```rust
if matches!(selector, ReadSelector::None) {
    if let Some(summary) = summarize_code(label, text) {
        return format!("¶{label}\n{}", summary.text);
    }
}
```

- [ ] **Step 5: Add ToolRunner summary integration test**

Add this test to `crates/orchestration/src/tool/runner.rs`:

```rust
#[tokio::test]
async fn read_code_without_selector_returns_structural_summary() {
    let dir = tempfile::tempdir().unwrap();
    let source = r#"pub fn alpha() {
    let a = 1;
    let b = 2;
    let c = 3;
    println!("{a}{b}{c}");
}

pub fn beta() {
    println!("small");
}
"#;
    fs::write(dir.path().join("lib.rs"), source).unwrap();
    let runner = runner(dir.path());
    let record = runner
        .execute(
            ToolCall {
                id: "call-summary".to_string(),
                name: "read".to_string(),
                arguments: serde_json::json!({"path": "lib.rs"}),
            },
            None,
        )
        .await
        .unwrap();
    assert!(record.result.content.contains("pub fn alpha()"));
    assert!(record.result.content.contains(".."));
    assert!(record.result.content.contains("re-read needed ranges"));
    assert!(!record.result.content.contains("let b = 2"));
}
```

- [ ] **Step 6: Run summary tests**

Run:

```bash
cargo test -p orchestration tool::read::summary read_code_without_selector_returns_structural_summary
```

Expected: PASS.

- [ ] **Step 7: Commit structural summaries**

```bash
git add crates/orchestration/src/tool/read/summary.rs crates/orchestration/src/tool/read/mod.rs crates/orchestration/src/tool/read/render.rs crates/orchestration/src/tool/runner.rs
git commit -m "feat: summarize large code reads"
```

### Task 4: Improve Directory Listings

**Files:**
- Create: `crates/orchestration/src/tool/read/directory.rs`
- Modify: `crates/orchestration/src/tool/read/mod.rs`
- Modify: `crates/orchestration/src/tool/blocking_ops.rs`
- Test: `crates/orchestration/src/tool/runner.rs`

- [ ] **Step 1: Write directory rendering tests**

Create `crates/orchestration/src/tool/read/directory.rs`:

```rust
use std::path::Path;

pub fn render_directory(path: &Path) -> Result<String, std::io::Error> {
    let _ = path;
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::render_directory;

    #[test]
    fn renders_directories_with_slash_and_sorted_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        let output = render_directory(dir.path()).unwrap();
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines, vec!["a.txt", "b.txt", "nested/"]);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p orchestration tool::read::directory
```

Expected: FAIL because `render_directory` returns an empty string.

- [ ] **Step 3: Implement directory renderer**

Replace `directory.rs` with:

```rust
use std::path::Path;

const DIRECTORY_ENTRY_LIMIT: usize = 500;

pub fn render_directory(path: &Path) -> Result<String, std::io::Error> {
    let mut entries = std::fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| {
            let file_type = entry.file_type().ok();
            let mut name = entry.file_name().to_string_lossy().to_string();
            if file_type.as_ref().is_some_and(|kind| kind.is_dir()) {
                name.push('/');
            }
            name
        })
        .collect::<Vec<_>>();
    entries.sort();

    let total = entries.len();
    let mut shown = entries.into_iter().take(DIRECTORY_ENTRY_LIMIT).collect::<Vec<_>>();
    if total > DIRECTORY_ENTRY_LIMIT {
        shown.push(format!(
            "… directory listing truncated after {DIRECTORY_ENTRY_LIMIT} of {total} entries; narrow the path …"
        ));
    }
    Ok(shown.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::render_directory;

    #[test]
    fn renders_directories_with_slash_and_sorted_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        let output = render_directory(dir.path()).unwrap();
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines, vec!["a.txt", "b.txt", "nested/"]);
    }
}
```

- [ ] **Step 4: Wire `BlockingToolOps::read_directory`**

In `crates/orchestration/src/tool/read/mod.rs`, add:

```rust
pub mod directory;
```

In `crates/orchestration/src/tool/blocking_ops.rs`, replace the body of `read_directory` with:

```rust
crate::tool::read::directory::render_directory(path)
    .map_err(|error| map_read_io_error(&path.display().to_string(), &error))
```

- [ ] **Step 5: Run directory tests**

Run:

```bash
cargo test -p orchestration tool::read::directory
```

Expected: PASS.

- [ ] **Step 6: Commit directory renderer**

```bash
git add crates/orchestration/src/tool/read/directory.rs crates/orchestration/src/tool/read/mod.rs crates/orchestration/src/tool/blocking_ops.rs
git commit -m "refactor: isolate read directory rendering"
```

### Task 5: Update Model Guidance to Match OMP Behavior

**Files:**
- Modify: `crates/orchestration/src/tool/registry.rs`
- Modify: `crates/engine/src/execution/node_invocation.rs`
- Test: existing unit tests under `crates/engine/src/execution/node_invocation.rs` if prompt assertions exist.

- [ ] **Step 1: Update registry description**

Replace the read tool description in `crates/orchestration/src/tool/registry.rs` with:

```rust
description: "Read a local file, directory listing, HTTP(S) URL, or spilled tool artifact. Bare code reads return structural summaries when useful; plain text defaults to numbered lines capped at 3000 lines. Append selectors to path: :N, :N-M, :N+COUNT, :A-B,C-D, :raw, or :N-M:raw. Re-read only the ranges named by summary footers. Truncated tool output can be recovered via artifact:{id} with the same selectors.".to_string(),
```

- [ ] **Step 2: Update node runtime preamble**

Replace the read bullet in `crates/engine/src/execution/node_invocation.rs` with:

```rust
- read — read a local file, directory listing, HTTP(S) URL, or spilled tool artifact. Bare code \
reads may return a structural summary with elided bodies and exact re-read ranges. Plain text \
defaults to numbered lines (3000-line cap). Append :start-end, :start+count, multi-ranges \
(e.g. :5-16,960-973), or :raw. NEVER guess what summary elisions contain; re-read only the \
needed ranges. Truncated tool output is readable via artifact:{id} (same selectors apply).\n\
```

- [ ] **Step 3: Run engine and orchestration prompt/tool tests**

Run:

```bash
cargo test -p engine node_invocation
cargo test -p orchestration read_file_selector_returns_numbered_lines
```

Expected: PASS.

- [ ] **Step 4: Commit guidance**

```bash
git add crates/orchestration/src/tool/registry.rs crates/engine/src/execution/node_invocation.rs
git commit -m "docs: teach agents targeted read selectors"
```

### Task 6: Add Focused Verification Documentation

**Files:**
- Modify: `docs/contributing/testing-workflows.md`

- [ ] **Step 1: Add read-tool verification section**

Append this section to `docs/contributing/testing-workflows.md`:

```markdown
## Read Tool Verification

Use this lane after changing `crates/orchestration/src/tool/read/`, read dispatch, artifact recovery, or model-facing read guidance.

```bash
cargo test -p orchestration tool::read
cargo test -p orchestration read_file_selector_returns_numbered_lines read_file_without_selector_announces_truncation read_artifact_round_trip_and_unknown_id
cargo test -p engine node_invocation
```

Expected:

- selector tests cover `:raw`, `:N-M`, `:N+COUNT`, multi-ranges, and range+raw;
- render tests cover the 3000-line cap and continuation notices;
- artifact tests prove spilled output is recoverable through `artifact:{id}` selectors;
- node invocation tests keep every node's system prompt aligned with the tool schema.
```

- [ ] **Step 2: Run doc-adjacent verification**

Run:

```bash
cargo test -p orchestration tool::read
cargo test -p engine node_invocation
```

Expected: PASS.

- [ ] **Step 3: Commit docs**

```bash
git add docs/contributing/testing-workflows.md
git commit -m "docs: add read tool verification lane"
```

## Future Parity Specs

Create separate plans after this foundation lands:

1. **Archive reads:** `archive.zip:path/inside.rs:50-80`, using a focused archive module and avoiding shelling out to `tar` or `unzip`.
2. **SQLite reads:** table list, schema sample, row lookup, paginated table reads, and read-only query support.
3. **Document reads:** PDF/DOCX/PPTX/XLSX/RTF/EPUB to markdown through a dedicated converter boundary.
4. **Image reads:** metadata-only by default plus explicit image-inspection flow if the app adds a vision-capable model path.
5. **Internal URI reads:** first-class `artifact:`, later `skill:`, `memory:`, and run-history paths once durable run records exist.

Do not start these before Tasks 1-6 are complete; each relies on the shared selector and renderer.

## Self-Review

Spec coverage:

- 3000-line OMP cap: Task 0.
- Same selector ergonomics as OMP for the supported read domains: Tasks 1-2.
- Structural summaries with exact range recovery: Task 3.
- Directory behavior and deterministic listings: Task 4.
- Agent-facing guidance that prevents whole-file re-reads: Task 5.
- Verification lane for future changes: Task 6.
- OMP's archive/SQLite/document/image/internal URI breadth is acknowledged as separate follow-up specs because those are independent subsystems.

Placeholder scan:

- No `TBD`, `TODO`, "implement later", or unnamed edge handling.
- Every task has exact files, commands, expected results, and concrete code snippets.

Type consistency:

- `ReadSelector`, `LineRange`, `split_selector`, and `render_text` are defined before later tasks use them.
- Later tasks route through `render_text` rather than reintroducing per-source selectors.
