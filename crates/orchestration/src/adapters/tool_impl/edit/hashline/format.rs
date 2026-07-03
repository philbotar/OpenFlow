//! Hashline format primitives: sigils, separators, and display helpers.

use std::hash::Hasher;

use twox_hash::XxHash32;

use super::types::Cursor;

pub const HL_FILE_PREFIX: &str = "¶";
pub const HL_PAYLOAD_REPLACE: &str = "+";
pub const HL_REPLACE_KEYWORD: &str = "replace";
pub const HL_BLOCK_KEYWORD: &str = "block";
pub const HL_DELETE_KEYWORD: &str = "delete";
pub const HL_INSERT_KEYWORD: &str = "insert";
pub const HL_INSERT_BEFORE: &str = "before";
pub const HL_INSERT_AFTER: &str = "after";
pub const HL_INSERT_HEAD: &str = "head";
pub const HL_INSERT_TAIL: &str = "tail";
pub const HL_HEADER_COLON: &str = ":";
pub const HL_FILE_HASH_SEP: &str = "#";
pub const HL_RANGE_SEP: &str = "..";
pub const HL_LINE_BODY_SEP: &str = ":";

pub const HL_FILE_HASH_LENGTH: usize = 4;

pub const HL_FILE_HASH_EXAMPLES: [&str; 3] = ["1A2B", "3C4D", "9F3E"];

pub fn format_replace_header(start: u32, end: u32) -> String {
    format!("{HL_REPLACE_KEYWORD} {start}{HL_RANGE_SEP}{end}{HL_HEADER_COLON}")
}

pub fn format_delete_header(start: u32, end: u32) -> String {
    if start == end {
        format!("{HL_DELETE_KEYWORD} {start}")
    } else {
        format!("{HL_DELETE_KEYWORD} {start}{HL_RANGE_SEP}{end}")
    }
}

pub fn format_insert_header(cursor: &Cursor) -> String {
    match cursor {
        Cursor::BeforeAnchor { anchor } => {
            format!(
                "{HL_INSERT_KEYWORD} {HL_INSERT_BEFORE} {}{HL_HEADER_COLON}",
                anchor.line
            )
        }
        Cursor::AfterAnchor { anchor } => {
            format!(
                "{HL_INSERT_KEYWORD} {HL_INSERT_AFTER} {}{HL_HEADER_COLON}",
                anchor.line
            )
        }
        Cursor::Bof => format!("{HL_INSERT_KEYWORD} {HL_INSERT_HEAD}{HL_HEADER_COLON}"),
        Cursor::Eof => format!("{HL_INSERT_KEYWORD} {HL_INSERT_TAIL}{HL_HEADER_COLON}"),
    }
}

fn normalize_file_hash_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut line = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            trim_trailing_ws_line(&mut line);
            out.push_str(&line);
            out.push('\n');
            line.clear();
        } else {
            line.push(ch);
        }
    }
    trim_trailing_ws_line(&mut line);
    out.push_str(&line);
    out
}

fn trim_trailing_ws_line(line: &mut String) {
    while line
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, ' ' | '\t' | '\r'))
    {
        line.pop();
    }
}

/// Compute the content-derived hash tag (4-hex uppercase, XxHash32 seed 0 low 16 bits).
pub fn compute_file_hash(text: &str) -> String {
    let normalized = normalize_file_hash_text(text);
    let mut hasher = XxHash32::with_seed(0);
    hasher.write(normalized.as_bytes());
    let low16 = hasher.finish() & 0xffff;
    format!("{low16:04X}")
}

pub fn describe_anchor_examples(line_prefix: &str) -> String {
    let examples: Vec<String> = if line_prefix.is_empty() {
        vec!["160".into(), "42".into(), "7".into()]
    } else {
        let mid = if line_prefix.len() > 1 {
            format!(
                "{}{}",
                &line_prefix[..line_prefix.len().saturating_sub(1)],
                "2"
            )
        } else {
            "42".to_string()
        };
        vec![line_prefix.to_string(), mid, "7".to_string()]
    };
    examples
        .iter()
        .map(|e| format!("\"{e}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_hashline_header(file_path: &str, file_hash: &str) -> String {
    format!("{HL_FILE_PREFIX}{file_path}{HL_FILE_HASH_SEP}{file_hash}")
}

pub fn format_numbered_line(line_number: u32, line: &str) -> String {
    format!("{line_number}{HL_LINE_BODY_SEP}{line}")
}

pub fn format_numbered_lines(text: &str, start_line: u32) -> String {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| format_numbered_line(start_line + i as u32, line))
        .collect::<Vec<_>>()
        .join("\n")
}
