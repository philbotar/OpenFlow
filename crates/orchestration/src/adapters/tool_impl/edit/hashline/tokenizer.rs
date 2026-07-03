//! Stateful, line-oriented classifier for hashline diff text.

use super::format::{
    describe_anchor_examples, HL_BLOCK_KEYWORD, HL_DELETE_KEYWORD, HL_FILE_HASH_LENGTH,
    HL_FILE_HASH_SEP, HL_FILE_PREFIX, HL_INSERT_AFTER, HL_INSERT_BEFORE, HL_INSERT_HEAD,
    HL_INSERT_KEYWORD, HL_INSERT_TAIL, HL_REPLACE_KEYWORD,
};
use super::messages::{ABORT_MARKER, BEGIN_PATCH_MARKER, END_PATCH_MARKER};
use super::types::{Anchor, Cursor, ParsedRange};

const CHAR_LINE_FEED: u8 = b'\n';
const CHAR_CARRIAGE_RETURN: u8 = b'\r';
const CHAR_ZERO: u8 = b'0';
const CHAR_NINE: u8 = b'9';
const CHAR_HASH: u8 = b'#';
const CHAR_TAB: u8 = b'\t';
const CHAR_SPACE: u8 = b' ';
const CHAR_DOT: u8 = b'.';
const CHAR_HYPHEN: u8 = b'-';
const CHAR_PAYLOAD_REPLACE: u8 = b'+';
const CHAR_COLON: u8 = b':';
const FILE_PREFIX_LENGTH: usize = HL_FILE_PREFIX.len();

fn is_digit_code(code: u8) -> bool {
    (CHAR_ZERO..=CHAR_NINE).contains(&code)
}

fn is_non_zero_digit_code(code: u8) -> bool {
    code > CHAR_ZERO && code <= CHAR_NINE
}

fn is_hex_digit_code(code: u8) -> bool {
    is_digit_code(code) || (b'A'..=b'F').contains(&code) || (b'a'..=b'f').contains(&code)
}

fn is_whitespace_code(code: u8) -> bool {
    code == CHAR_SPACE || (CHAR_TAB..=CHAR_CARRIAGE_RETURN).contains(&code)
}

fn skip_whitespace(line: &[u8], mut index: usize, end: usize) -> usize {
    while index < end && is_whitespace_code(line[index]) {
        index += 1;
    }
    index
}

fn trim_end_index(line: &[u8]) -> usize {
    let mut end = line.len();
    while end > 0 && is_whitespace_code(line[end - 1]) {
        end -= 1;
    }
    end
}

fn is_empty_line(line: &[u8]) -> bool {
    line.is_empty()
}

fn marker_line_equals(line: &[u8], marker: &str) -> bool {
    let end = trim_end_index(line);
    end == marker.len() && line.starts_with(marker.as_bytes())
}

pub fn split_hashline_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte != CHAR_LINE_FEED {
            continue;
        }
        let mut end = index;
        if end > start && bytes[end - 1] == CHAR_CARRIAGE_RETURN {
            end -= 1;
        }
        lines.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
        start = index + 1;
    }
    if start < bytes.len() {
        let mut end = bytes.len();
        if end > start && bytes[end - 1] == CHAR_CARRIAGE_RETURN {
            end -= 1;
        }
        lines.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
    }
    lines
}

pub fn clone_cursor(cursor: &Cursor) -> Cursor {
    match cursor {
        Cursor::BeforeAnchor { anchor } => Cursor::BeforeAnchor {
            anchor: Anchor { line: anchor.line },
        },
        Cursor::AfterAnchor { anchor } => Cursor::AfterAnchor {
            anchor: Anchor { line: anchor.line },
        },
        other => other.clone(),
    }
}

struct NumberScan {
    line: u32,
    next_index: usize,
}

fn scan_line_number(line: &[u8], index: usize, end: usize) -> Option<NumberScan> {
    if index >= end || !is_non_zero_digit_code(line[index]) {
        return None;
    }
    let mut line_number = 0u32;
    let mut next_index = index;
    while next_index < end {
        let code = line[next_index];
        if !is_digit_code(code) {
            break;
        }
        line_number = line_number * 10 + u32::from(code - CHAR_ZERO);
        next_index += 1;
    }
    Some(NumberScan {
        line: line_number,
        next_index,
    })
}

pub fn parse_lid(raw: &str, line_num: u32) -> Result<Anchor, String> {
    let bytes = raw.as_bytes();
    let end = trim_end_index(bytes);
    let number_start = skip_whitespace(bytes, 0, end);
    let number = scan_line_number(bytes, number_start, end)
        .ok_or_else(|| {
            format!(
                "line {line_num}: expected a line number such as {}; got {}. Use {HL_FILE_PREFIX}PATH{HL_FILE_HASH_SEP}hash from your latest read for file-version binding.",
                describe_anchor_examples("119"),
                serde_json::to_string(raw).unwrap_or_else(|_| format!("\"{raw}\""))
            )
        })?;
    if skip_whitespace(bytes, number.next_index, end) != end {
        return Err(format!(
            "line {line_num}: expected a line number such as {}; got {}. Use {HL_FILE_PREFIX}PATH{HL_FILE_HASH_SEP}hash from your latest read for file-version binding.",
            describe_anchor_examples("119"),
            serde_json::to_string(raw).unwrap_or_else(|_| format!("\"{raw}\""))
        ));
    }
    Ok(Anchor { line: number.line })
}

struct RangeScan {
    range: ParsedRange,
    next_index: usize,
}

fn scan_range_separator(line: &[u8], mut cursor: usize, end: usize) -> Option<usize> {
    let mut consumed_separator = false;
    while cursor < end {
        let code = line[cursor];
        if is_whitespace_code(code) {
            cursor += 1;
            consumed_separator = true;
            continue;
        }
        if code == CHAR_HYPHEN || code == 0xE2 {
            // ellipsis `…` is U+2026, UTF-8 E2 80 A6 — treat first byte like TS
            cursor += 1;
            consumed_separator = true;
            continue;
        }
        if code == CHAR_DOT && cursor + 1 < end && line[cursor + 1] == CHAR_DOT {
            cursor += 2;
            consumed_separator = true;
            continue;
        }
        break;
    }
    if !consumed_separator {
        return None;
    }
    if cursor >= end || !is_non_zero_digit_code(line[cursor]) {
        return None;
    }
    Some(cursor)
}

fn scan_header_range(
    line: &[u8],
    index: usize,
    end: usize,
    allow_single: bool,
) -> Option<RangeScan> {
    let number_start = skip_whitespace(line, index, end);
    let start = scan_line_number(line, number_start, end)?;
    let after_first = scan_range_separator(line, start.next_index, end);
    if after_first.is_none() {
        if !allow_single {
            return None;
        }
        return Some(RangeScan {
            range: ParsedRange {
                start: Anchor { line: start.line },
                end: Anchor { line: start.line },
            },
            next_index: skip_whitespace(line, start.next_index, end),
        });
    }
    let end_number = scan_line_number(line, after_first?, end)?;
    Some(RangeScan {
        range: ParsedRange {
            start: Anchor { line: start.line },
            end: Anchor {
                line: end_number.line,
            },
        },
        next_index: skip_whitespace(line, end_number.next_index, end),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockTarget {
    Replace { range: ParsedRange },
    Block { anchor: Anchor },
    Delete { range: ParsedRange },
    DeleteBlock { anchor: Anchor },
    InsertBefore { anchor: Anchor },
    InsertAfter { anchor: Anchor },
    Bof,
    Eof,
}

struct TargetScan {
    target: BlockTarget,
    next_index: usize,
}

fn scan_keyword(line: &[u8], index: usize, end: usize, keyword: &str) -> Option<usize> {
    let kw = keyword.as_bytes();
    if !line[index..end].starts_with(kw) {
        return None;
    }
    let next = index + kw.len();
    if next < end {
        let code = line[next];
        if !is_whitespace_code(code) && code != CHAR_COLON {
            return None;
        }
    }
    Some(next)
}

fn consume_optional_colon(line: &[u8], index: usize, end: usize) -> usize {
    let cursor = skip_whitespace(line, index, end);
    if cursor < end && line[cursor] == CHAR_COLON {
        skip_whitespace(line, cursor + 1, end)
    } else {
        cursor
    }
}

fn scan_insert_target(line: &[u8], index: usize, end: usize) -> Option<TargetScan> {
    let cursor = skip_whitespace(line, index, end);
    if let Some(before_end) = scan_keyword(line, cursor, end, HL_INSERT_BEFORE) {
        let anchor = scan_line_number(line, skip_whitespace(line, before_end, end), end)?;
        let next_index = consume_optional_colon(line, anchor.next_index, end);
        return Some(TargetScan {
            target: BlockTarget::InsertBefore {
                anchor: Anchor { line: anchor.line },
            },
            next_index,
        });
    }
    if let Some(after_end) = scan_keyword(line, cursor, end, HL_INSERT_AFTER) {
        let anchor = scan_line_number(line, skip_whitespace(line, after_end, end), end)?;
        let next_index = consume_optional_colon(line, anchor.next_index, end);
        return Some(TargetScan {
            target: BlockTarget::InsertAfter {
                anchor: Anchor { line: anchor.line },
            },
            next_index,
        });
    }
    if let Some(head_end) = scan_keyword(line, cursor, end, HL_INSERT_HEAD) {
        return Some(TargetScan {
            target: BlockTarget::Bof,
            next_index: consume_optional_colon(line, head_end, end),
        });
    }
    if let Some(tail_end) = scan_keyword(line, cursor, end, HL_INSERT_TAIL) {
        return Some(TargetScan {
            target: BlockTarget::Eof,
            next_index: consume_optional_colon(line, tail_end, end),
        });
    }
    None
}

fn scan_hunk_anchor(line: &[u8], start: usize, end: usize) -> Option<TargetScan> {
    let cursor = skip_whitespace(line, start, end);
    if let Some(replace_end) = scan_keyword(line, cursor, end, HL_REPLACE_KEYWORD) {
        let block_cursor = skip_whitespace(line, replace_end, end);
        if let Some(block_end) = scan_keyword(line, block_cursor, end, HL_BLOCK_KEYWORD) {
            let anchor = scan_line_number(line, skip_whitespace(line, block_end, end), end)?;
            return Some(TargetScan {
                target: BlockTarget::Block {
                    anchor: Anchor { line: anchor.line },
                },
                next_index: consume_optional_colon(line, anchor.next_index, end),
            });
        }
        let range = scan_header_range(line, replace_end, end, true)?;
        return Some(TargetScan {
            target: BlockTarget::Replace { range: range.range },
            next_index: consume_optional_colon(line, range.next_index, end),
        });
    }
    if let Some(delete_end) = scan_keyword(line, cursor, end, HL_DELETE_KEYWORD) {
        let block_cursor = skip_whitespace(line, delete_end, end);
        if let Some(block_end) = scan_keyword(line, block_cursor, end, HL_BLOCK_KEYWORD) {
            let anchor = scan_line_number(line, skip_whitespace(line, block_end, end), end)?;
            let next = skip_whitespace(line, anchor.next_index, end);
            if next < end && line[next] == CHAR_COLON {
                return None;
            }
            return Some(TargetScan {
                target: BlockTarget::DeleteBlock {
                    anchor: Anchor { line: anchor.line },
                },
                next_index: next,
            });
        }
        let range = scan_header_range(line, delete_end, end, true)?;
        let next = skip_whitespace(line, range.next_index, end);
        if next < end && line[next] == CHAR_COLON {
            return None;
        }
        return Some(TargetScan {
            target: BlockTarget::Delete { range: range.range },
            next_index: next,
        });
    }
    if let Some(insert_end) = scan_keyword(line, cursor, end, HL_INSERT_KEYWORD) {
        return scan_insert_target(line, insert_end, end);
    }
    None
}

struct ParsedHunkHeader {
    target: BlockTarget,
}

fn try_parse_hunk_header(line: &str) -> Option<ParsedHunkHeader> {
    let bytes = line.as_bytes();
    let end = trim_end_index(bytes);
    let start = skip_whitespace(bytes, 0, end);
    if start >= end {
        return None;
    }
    let scan = scan_hunk_anchor(bytes, start, end)?;
    if scan.next_index != end {
        return None;
    }
    Some(ParsedHunkHeader {
        target: scan.target,
    })
}

struct ParsedHeader {
    path: String,
    file_hash: Option<String>,
}

fn try_parse_header(line: &str) -> Option<ParsedHeader> {
    if !line.starts_with(HL_FILE_PREFIX) {
        return None;
    }
    let bytes = line.as_bytes();
    let end = trim_end_index(bytes);
    let mut index = FILE_PREFIX_LENGTH;
    if index >= end {
        return None;
    }
    let path_start = index;
    while index < end {
        let code = bytes[index];
        if code == CHAR_HASH || code == CHAR_SPACE || code == CHAR_TAB {
            break;
        }
        index += 1;
    }
    if index == path_start {
        return None;
    }
    let path = line[path_start..index].to_string();
    let mut file_hash = None;
    if index < end && bytes[index] == CHAR_HASH {
        let hash_start = index + 1;
        let hash_end = hash_start + HL_FILE_HASH_LENGTH;
        if hash_end > end {
            return None;
        }
        if !bytes[hash_start..hash_end]
            .iter()
            .all(|byte| is_hex_digit_code(*byte))
        {
            return None;
        }
        file_hash = Some(line[hash_start..hash_end].to_uppercase());
        index = hash_end;
    }
    if skip_whitespace(bytes, index, end) != end {
        return None;
    }
    Some(ParsedHeader { path, file_hash })
}

#[derive(Debug, Clone)]
pub enum Token {
    Blank {
        line_num: u32,
    },
    EnvelopeBegin {
        line_num: u32,
    },
    EnvelopeEnd {
        line_num: u32,
    },
    Abort {
        line_num: u32,
    },
    Header {
        line_num: u32,
        path: String,
        file_hash: Option<String>,
    },
    OpBlock {
        line_num: u32,
        target: BlockTarget,
    },
    PayloadLiteral {
        line_num: u32,
        text: String,
    },
    Raw {
        line_num: u32,
        text: String,
    },
}

fn classify_line(line: &str, line_num: u32) -> Token {
    let bytes = line.as_bytes();
    if is_empty_line(bytes) {
        return Token::Blank { line_num };
    }
    if marker_line_equals(bytes, BEGIN_PATCH_MARKER) {
        return Token::EnvelopeBegin { line_num };
    }
    if marker_line_equals(bytes, END_PATCH_MARKER) {
        return Token::EnvelopeEnd { line_num };
    }
    if marker_line_equals(bytes, ABORT_MARKER) {
        return Token::Abort { line_num };
    }
    if line.starts_with(HL_FILE_PREFIX) {
        if let Some(header) = try_parse_header(line) {
            return Token::Header {
                line_num,
                path: header.path,
                file_hash: header.file_hash,
            };
        }
    }
    let lead = skip_whitespace(bytes, 0, bytes.len());
    let is_hunk_lead = line[lead..].starts_with(HL_REPLACE_KEYWORD)
        || line[lead..].starts_with(HL_DELETE_KEYWORD)
        || line[lead..].starts_with(HL_INSERT_KEYWORD);
    if is_hunk_lead {
        if let Some(hunk) = try_parse_hunk_header(line) {
            return Token::OpBlock {
                line_num,
                target: hunk.target,
            };
        }
    }
    if !bytes.is_empty() && bytes[0] == CHAR_PAYLOAD_REPLACE {
        return Token::PayloadLiteral {
            line_num,
            text: line[1..].to_string(),
        };
    }
    Token::Raw {
        line_num,
        text: line.to_string(),
    }
}

pub struct Tokenizer {
    buffer: String,
    next_line_num: u32,
    closed: bool,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            next_line_num: 1,
            closed: false,
        }
    }

    pub fn feed(&mut self, chunk: &str) -> Result<Vec<Token>, String> {
        if self.closed {
            return Err("Tokenizer is closed; call reset() before reusing.".to_string());
        }
        if chunk.is_empty() {
            return Ok(Vec::new());
        }
        self.buffer.push_str(chunk);
        Ok(self.drain_complete_lines())
    }

    pub fn end(&mut self) -> Vec<Token> {
        if self.closed {
            return Vec::new();
        }
        self.closed = true;
        let buf = std::mem::take(&mut self.buffer);
        if buf.is_empty() {
            return Vec::new();
        }
        let bytes = buf.as_bytes();
        let mut stop = bytes.len();
        if stop > 0 && bytes[stop - 1] == CHAR_CARRIAGE_RETURN {
            stop -= 1;
        }
        vec![classify_line(&buf[..stop], self.next_line_num)]
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.next_line_num = 1;
        self.closed = false;
    }

    pub fn tokenize(&self, line: &str, line_num: u32) -> Token {
        classify_line(line, line_num)
    }

    pub fn is_op(&self, line: &str) -> bool {
        try_parse_hunk_header(line).is_some()
    }

    fn drain_complete_lines(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let buf = self.buffer.as_bytes();
        let mut start = 0usize;
        for (index, &byte) in buf.iter().enumerate() {
            if byte != CHAR_LINE_FEED {
                continue;
            }
            let mut stop = index;
            if stop > start && buf[stop - 1] == CHAR_CARRIAGE_RETURN {
                stop -= 1;
            }
            let line = String::from_utf8_lossy(&buf[start..stop]);
            tokens.push(classify_line(&line, self.next_line_num));
            self.next_line_num += 1;
            start = index + 1;
        }
        self.buffer = if start < buf.len() {
            String::from_utf8_lossy(&buf[start..]).into_owned()
        } else {
            String::new()
        };
        tokens
    }
}
