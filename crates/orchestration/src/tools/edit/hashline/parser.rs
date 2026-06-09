//! Token-driven state machine that turns tokens into a flat list of edits.

use super::format::HL_PAYLOAD_REPLACE;
use super::messages::{
    BARE_BODY_AUTO_PIPED_WARNING, DELETE_BLOCK_TAKES_NO_BODY, DELETE_TAKES_NO_BODY, EMPTY_BLOCK,
    EMPTY_INSERT, MINUS_ROW_REJECTED,
};
use super::tokenizer::{BlockTarget, Token, Tokenizer};
use super::types::{Anchor, Cursor, Edit, InsertMode, ParsedRange};

fn validate_range_order(range: &ParsedRange, line_num: u32) -> Result<(), String> {
    if range.end.line < range.start.line {
        return Err(format!(
            "line {line_num}: range {}..{} ends before it starts.",
            range.start.line, range.end.line
        ));
    }
    Ok(())
}

fn expand_range(range: &ParsedRange) -> Vec<Anchor> {
    (range.start.line..=range.end.line)
        .map(|line| Anchor { line })
        .collect()
}

fn is_skippable_comment_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn detect_apply_patch_contamination(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("*** Update File:")
        || trimmed.starts_with("*** Add File:")
        || trimmed.starts_with("*** Delete File:")
        || trimmed.starts_with("*** Move to:")
    {
        let preview = if trimmed.len() > 48 {
            format!("{}…", &trimmed[..48])
        } else {
            trimmed.to_string()
        };
        return Some(format!(
            "apply_patch sentinel {} is not valid in hashline. File sections start with `¶path#HASH` (no `Update File:` / `Add File:` keyword). Use `replace N..M:`, `delete N..M`, or `insert before|after|head|tail:` ops.",
            serde_json::to_string(&preview).unwrap_or_else(|_| format!("\"{preview}\""))
        ));
    }
    if regex::Regex::new(r"^@@\s+[-+]?\d+,\d+\s+[-+]?\d+,\d+\s+@@")
        .ok()
        .is_some_and(|re| re.is_match(trimmed))
    {
        return Some(
            "unified-diff hunk header (`@@ -N,M +N,M @@`) is not valid in hashline. Use `replace N..M:`, `delete N..M`, or `insert before|after|head|tail:` ops.".to_string(),
        );
    }
    if trimmed.starts_with("@@") {
        let preview = if trimmed.len() > 48 {
            format!("{}…", &trimmed[..48])
        } else {
            trimmed.to_string()
        };
        return Some(format!(
            "`@@`-bracketed hunk header {} is not valid in hashline. Drop the `@@ ... @@` brackets and write a verb header such as `replace N..M:`.",
            serde_json::to_string(&preview).unwrap_or_else(|_| format!("\"{preview}\""))
        ));
    }
    if regex::Regex::new(r"^delete\s+[1-9]\d*(?:\s*(?:\.\.|-|…|\s)\s*[1-9]\d*)?\s*:")
        .ok()
        .is_some_and(|re| re.is_match(trimmed))
    {
        return Some(
            "`delete N..M` has no colon and no body. Remove the colon and body rows.".to_string(),
        );
    }
    if regex::Regex::new(r"^[1-9]\d*\s*$")
        .ok()
        .is_some_and(|re| re.is_match(trimmed))
    {
        return Some(format!(
            "hunk headers need a verb. Use `replace {trimmed}..{trimmed}:` to replace, or `delete {trimmed}` to delete."
        ));
    }
    if let Some(caps) = regex::Regex::new(r"^([1-9]\d*)\s*[-. …]+\s*([1-9]\d*)\s*:?$")
        .ok()
        .and_then(|re| re.captures(trimmed))
    {
        return Some(format!(
            "bare range hunk header {} is not valid. Hunk headers need a verb: write `replace {}..{}:` or `delete {}..{}`.",
            serde_json::to_string(trimmed).unwrap_or_else(|_| format!("\"{trimmed}\"")),
            &caps[1],
            &caps[2],
            &caps[1],
            &caps[2]
        ));
    }
    None
}

struct PendingComment {
    line_num: u32,
    text: String,
}

struct PayloadRow {
    text: String,
}

struct Pending {
    target: BlockTarget,
    line_num: u32,
    payloads: Vec<PayloadRow>,
}

pub struct Executor {
    edits: Vec<Edit>,
    warnings: Vec<String>,
    edit_index: u32,
    pending: Option<Pending>,
    terminated: bool,
    skippable_comments: Vec<PendingComment>,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            warnings: Vec::new(),
            edit_index: 0,
            pending: None,
            terminated: false,
            skippable_comments: Vec::new(),
        }
    }

    fn discard_pending_skippable_comments(&mut self) {
        self.skippable_comments.clear();
    }

    fn consume_pending_skippable_comments(&mut self) -> Result<(), String> {
        if self.skippable_comments.is_empty() {
            return Ok(());
        }
        let comments = std::mem::take(&mut self.skippable_comments);
        for comment in comments {
            self.handle_raw(&comment.text, comment.line_num)?;
        }
        Ok(())
    }

    pub fn feed(&mut self, token: Token) -> Result<(), String> {
        if self.terminated {
            return Ok(());
        }
        match token {
            Token::EnvelopeBegin { .. } => {
                self.consume_pending_skippable_comments()?;
            }
            Token::EnvelopeEnd { .. } => {
                self.consume_pending_skippable_comments()?;
                self.terminated = true;
            }
            Token::Abort { .. } => {
                self.terminated = true;
            }
            Token::Header { .. } => {
                self.consume_pending_skippable_comments()?;
                self.flush_pending()?;
            }
            Token::Blank { .. } => {
                self.consume_pending_skippable_comments()?;
            }
            Token::PayloadLiteral { line_num, text } => {
                self.consume_pending_skippable_comments()?;
                self.handle_literal_payload(&text, line_num)?;
            }
            Token::Raw { line_num, text } => {
                if self.pending.is_none() && is_skippable_comment_line(&text) {
                    self.skippable_comments
                        .push(PendingComment { line_num, text });
                    return Ok(());
                }
                self.consume_pending_skippable_comments()?;
                self.handle_raw(&text, line_num)?;
            }
            Token::OpBlock { line_num, target } => {
                self.discard_pending_skippable_comments();
                if matches!(
                    target,
                    BlockTarget::Replace { .. } | BlockTarget::Delete { .. }
                ) {
                    if let BlockTarget::Replace { range } | BlockTarget::Delete { range } = &target
                    {
                        validate_range_order(range, line_num)?;
                    }
                }
                self.flush_pending()?;
                self.pending = Some(Pending {
                    target,
                    line_num,
                    payloads: Vec::new(),
                });
            }
        }
        Ok(())
    }

    pub fn end(mut self) -> Result<ParseResult, String> {
        self.consume_pending_skippable_comments()?;
        self.flush_pending()?;
        self.validate_no_overlapping_deletes()?;
        Ok(ParseResult {
            edits: self.edits,
            warnings: self.warnings,
        })
    }

    pub fn end_streaming(mut self) -> Result<ParseResult, String> {
        self.consume_pending_skippable_comments()?;
        if let Some(pending) = &self.pending {
            if !pending.payloads.is_empty()
                || matches!(
                    pending.target,
                    BlockTarget::Delete { .. } | BlockTarget::DeleteBlock { .. }
                )
            {
                self.flush_pending()?;
            } else {
                self.pending = None;
            }
        }
        self.validate_no_overlapping_deletes()?;
        Ok(ParseResult {
            edits: self.edits,
            warnings: self.warnings,
        })
    }

    pub fn reset(&mut self) {
        self.edits.clear();
        self.warnings.clear();
        self.edit_index = 0;
        self.pending = None;
        self.skippable_comments.clear();
        self.terminated = false;
    }

    fn validate_no_overlapping_deletes(&self) -> Result<(), String> {
        let mut source_lines_by_anchor: std::collections::HashMap<u32, Vec<u32>> =
            std::collections::HashMap::new();
        for edit in &self.edits {
            if let Edit::Delete {
                anchor, line_num, ..
            } = edit
            {
                source_lines_by_anchor
                    .entry(anchor.line)
                    .or_default()
                    .push(*line_num);
            }
        }
        for (anchor_line, mut source_lines) in source_lines_by_anchor {
            if source_lines.len() < 2 {
                continue;
            }
            source_lines.sort_unstable();
            let first_block = source_lines[0];
            let second_block = source_lines[1];
            return Err(format!(
                "line {second_block}: anchor line {anchor_line} is already targeted by another hunk on line {first_block}. Issue ONE hunk per range; payload is only the final desired content, never a before/after pair."
            ));
        }
        Ok(())
    }

    fn handle_literal_payload(&mut self, text: &str, line_num: u32) -> Result<(), String> {
        let pending = self.pending.as_mut().ok_or_else(|| {
            format!(
                "line {line_num}: payload line has no preceding hunk header. Got {}.",
                serde_json::to_string(&format!("{HL_PAYLOAD_REPLACE}{text}")).unwrap_or_default()
            )
        })?;
        if matches!(pending.target, BlockTarget::Delete { .. }) {
            return Err(format!("line {line_num}: {DELETE_TAKES_NO_BODY}"));
        }
        if matches!(pending.target, BlockTarget::DeleteBlock { .. }) {
            return Err(format!("line {line_num}: {DELETE_BLOCK_TAKES_NO_BODY}"));
        }
        pending.payloads.push(PayloadRow {
            text: text.to_string(),
        });
        Ok(())
    }

    fn handle_raw(&mut self, text: &str, line_num: u32) -> Result<(), String> {
        if let Some(contamination) = detect_apply_patch_contamination(text) {
            return Err(format!("line {line_num}: {contamination}"));
        }
        if let Some(pending) = self.pending.as_mut() {
            if text.trim().is_empty() {
                return Ok(());
            }
            if matches!(pending.target, BlockTarget::Delete { .. }) {
                return Err(format!("line {line_num}: {DELETE_TAKES_NO_BODY}"));
            }
            if matches!(pending.target, BlockTarget::DeleteBlock { .. }) {
                return Err(format!("line {line_num}: {DELETE_BLOCK_TAKES_NO_BODY}"));
            }
            if text.trim_start().starts_with('-') {
                return Err(format!("line {line_num}: {MINUS_ROW_REJECTED}"));
            }
            if !self
                .warnings
                .iter()
                .any(|w| w == BARE_BODY_AUTO_PIPED_WARNING)
            {
                self.warnings.push(BARE_BODY_AUTO_PIPED_WARNING.to_string());
            }
            pending.payloads.push(PayloadRow {
                text: text.to_string(),
            });
            return Ok(());
        }
        if text.trim().is_empty() {
            return Ok(());
        }
        Err(format!(
            "line {line_num}: payload line has no preceding hunk header. Use `replace N..M:`, `delete N..M`, or `insert before|after|head|tail:` above the body. Got {}.",
            serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""))
        ))
    }

    fn push_insert(
        &mut self,
        cursor: Cursor,
        text: String,
        line_num: u32,
        mode: Option<InsertMode>,
    ) {
        self.edits.push(Edit::Insert {
            cursor: super::tokenizer::clone_cursor(&cursor),
            text,
            line_num,
            index: self.edit_index,
            mode,
        });
        self.edit_index += 1;
    }

    fn push_delete(&mut self, anchor: Anchor, line_num: u32) {
        self.edits.push(Edit::Delete {
            anchor: Anchor { line: anchor.line },
            line_num,
            index: self.edit_index,
            old_assertion: None,
        });
        self.edit_index += 1;
    }

    fn push_block(&mut self, anchor: Anchor, payloads: &[PayloadRow], line_num: u32) {
        self.edits.push(Edit::Block {
            anchor: Anchor { line: anchor.line },
            payloads: payloads.iter().map(|p| p.text.clone()).collect(),
            line_num,
            index: self.edit_index,
        });
        self.edit_index += 1;
    }

    fn emit_payload_rows(
        &mut self,
        cursor: Cursor,
        payloads: &[PayloadRow],
        line_num: u32,
        mode: Option<InsertMode>,
    ) {
        for payload in payloads {
            self.push_insert(cursor.clone(), payload.text.clone(), line_num, mode);
        }
    }

    fn flush_pending(&mut self) -> Result<(), String> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        let Pending {
            target,
            line_num,
            payloads,
        } = pending;
        match target {
            BlockTarget::Delete { range } => {
                for anchor in expand_range(&range) {
                    self.push_delete(anchor, line_num);
                }
            }
            BlockTarget::DeleteBlock { anchor } => {
                self.push_block(anchor, &[], line_num);
            }
            BlockTarget::Block { anchor } => {
                if payloads.is_empty() {
                    return Err(format!("line {line_num}: {EMPTY_BLOCK}"));
                }
                self.push_block(anchor, &payloads, line_num);
            }
            BlockTarget::Replace { range } => {
                if payloads.is_empty() {
                    for anchor in expand_range(&range) {
                        self.push_delete(anchor, line_num);
                    }
                    return Ok(());
                }
                let cursor = Cursor::BeforeAnchor {
                    anchor: Anchor {
                        line: range.start.line,
                    },
                };
                self.emit_payload_rows(cursor, &payloads, line_num, Some(InsertMode::Replacement));
                for anchor in expand_range(&range) {
                    self.push_delete(anchor, line_num);
                }
            }
            BlockTarget::InsertBefore { anchor } => {
                if payloads.is_empty() {
                    return Err(format!("line {line_num}: {EMPTY_INSERT}"));
                }
                self.emit_payload_rows(Cursor::BeforeAnchor { anchor }, &payloads, line_num, None);
            }
            BlockTarget::InsertAfter { anchor } => {
                if payloads.is_empty() {
                    return Err(format!("line {line_num}: {EMPTY_INSERT}"));
                }
                self.emit_payload_rows(Cursor::AfterAnchor { anchor }, &payloads, line_num, None);
            }
            BlockTarget::Bof => {
                if payloads.is_empty() {
                    return Err(format!("line {line_num}: {EMPTY_INSERT}"));
                }
                self.emit_payload_rows(Cursor::Bof, &payloads, line_num, None);
            }
            BlockTarget::Eof => {
                if payloads.is_empty() {
                    return Err(format!("line {line_num}: {EMPTY_INSERT}"));
                }
                self.emit_payload_rows(Cursor::Eof, &payloads, line_num, None);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub edits: Vec<Edit>,
    pub warnings: Vec<String>,
}

pub fn parse_patch(diff: &str) -> Result<ParseResult, String> {
    let mut tokenizer = Tokenizer::new();
    let mut executor = Executor::new();
    for token in tokenizer.feed(diff)? {
        executor.feed(token)?;
    }
    for token in tokenizer.end() {
        executor.feed(token)?;
    }
    executor.end()
}

pub fn parse_patch_streaming(diff: &str) -> Result<ParseResult, String> {
    let mut tokenizer = Tokenizer::new();
    let mut executor = Executor::new();
    for token in tokenizer.feed(diff)? {
        executor.feed(token)?;
    }
    for token in tokenizer.end() {
        executor.feed(token)?;
    }
    executor.end_streaming()
}
