//! Error type raised when a section snapshot tag does not match live file content.

use std::fmt;

use super::format::{
    format_numbered_line, HL_FILE_HASH_EXAMPLES, HL_FILE_HASH_SEP, HL_FILE_PREFIX,
};
use super::messages::MISMATCH_CONTEXT;

#[derive(Debug, Clone)]
pub struct MismatchError {
    pub path: Option<String>,
    pub expected_file_hash: String,
    pub actual_file_hash: String,
    pub file_lines: Vec<String>,
    pub anchor_lines: Vec<u32>,
    pub hash_recognized: bool,
    message: String,
}

#[derive(Debug, Clone)]
pub struct MismatchDetails {
    pub path: Option<String>,
    pub expected_file_hash: String,
    pub actual_file_hash: String,
    pub file_lines: Vec<String>,
    pub anchor_lines: Vec<u32>,
    pub hash_recognized: Option<bool>,
}

pub fn format_full_anchor_requirement(raw: Option<&str>) -> String {
    let received = raw.map(|r| format!(" Received {r:?}.")).unwrap_or_default();
    format!(
        "a bare line number from read/search output plus the section header content-hash tag \
         (for example {HL_FILE_PREFIX}src/foo.ts{HL_FILE_HASH_SEP}{} and line \"160\"){received}",
        HL_FILE_HASH_EXAMPLES[0]
    )
}

fn get_mismatch_display_lines(anchor_lines: &[u32], file_lines: &[String]) -> Vec<u32> {
    let mut display_lines = std::collections::BTreeSet::new();
    let len = file_lines.len() as u32;
    for &line in anchor_lines {
        if !(1..=len).contains(&line) {
            continue;
        }
        let lo = line.saturating_sub(MISMATCH_CONTEXT).max(1);
        let hi = (line + MISMATCH_CONTEXT).min(len);
        for line_num in lo..=hi {
            display_lines.insert(line_num);
        }
    }
    display_lines.into_iter().collect()
}

impl MismatchError {
    pub fn new(details: MismatchDetails) -> Self {
        let hash_recognized = details.hash_recognized.unwrap_or(true);
        let message = Self::format_message(&details, hash_recognized);
        Self {
            path: details.path,
            expected_file_hash: details.expected_file_hash,
            actual_file_hash: details.actual_file_hash,
            file_lines: details.file_lines,
            anchor_lines: details.anchor_lines,
            hash_recognized,
            message,
        }
    }

    fn rejection_header(details: &MismatchDetails, hash_recognized: bool) -> Vec<String> {
        let path_text = details
            .path
            .as_ref()
            .map(|p| format!(" for {p}"))
            .unwrap_or_default();
        if !hash_recognized {
            return vec![
                format!(
                    "Edit rejected{path_text}: hash {HL_FILE_HASH_SEP}{} is not from this session.",
                    details.expected_file_hash
                ),
                format!(
                    "The current file hashes to {HL_FILE_HASH_SEP}{}. Re-read the file with `read` to copy a current {HL_FILE_PREFIX}path{HL_FILE_HASH_SEP}tag header — never invent the tag and never reuse one from a prior session.",
                    details.actual_file_hash
                ),
            ];
        }
        vec![
            format!("Edit rejected{path_text}: file changed between read and edit."),
            format!(
                "Section is bound to {HL_FILE_HASH_SEP}{}, but the current file hashes to {HL_FILE_HASH_SEP}{}. If a prior edit in this session modified this file, copy the {HL_FILE_PREFIX}path{HL_FILE_HASH_SEP}newhash header from that edit's response; otherwise re-read the file with `read` to refresh the tag before retrying.",
                details.expected_file_hash, details.actual_file_hash
            ),
        ]
    }

    fn format_message(details: &MismatchDetails, hash_recognized: bool) -> String {
        let anchor_set: std::collections::BTreeSet<u32> =
            details.anchor_lines.iter().copied().collect();
        let mut lines = Self::rejection_header(details, hash_recognized);
        let display_lines = get_mismatch_display_lines(&details.anchor_lines, &details.file_lines);
        if display_lines.is_empty() {
            return lines.join("\n");
        }
        lines.push(String::new());
        let mut previous = 0i64;
        for line_num in display_lines {
            if previous != -1 && i64::from(line_num) > previous + 1 {
                lines.push("...".to_string());
            }
            previous = i64::from(line_num);
            let text = details
                .file_lines
                .get(line_num as usize - 1)
                .map(String::as_str)
                .unwrap_or("");
            let marker = if anchor_set.contains(&line_num) {
                "*"
            } else {
                " "
            };
            lines.push(format!("{marker}{}", format_numbered_line(line_num, text)));
        }
        lines.join("\n")
    }
}

impl fmt::Display for MismatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MismatchError {}
