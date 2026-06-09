//! Strip hashline / diff line prefixes from read/search echoed content.

use std::sync::OnceLock;

use regex::Regex;

use super::format::{HL_FILE_HASH_LENGTH, HL_FILE_PREFIX};

fn hl_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*(?:>>>|>>)?\s*(?:[+*-]\s*)?\d+:").expect("valid regex"))
}

fn hl_prefix_plus_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*(?:>>>|>>)?\s*\+\s*\d+:").expect("valid regex"))
}

fn hl_header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"^\s*{HL_FILE_PREFIX}\S+#[0-9a-fA-F]{{{HL_FILE_HASH_LENGTH}}}\s*$"
        ))
        .expect("valid regex")
    })
}

fn starts_with_single_plus(line: &str) -> bool {
    line.starts_with('+') && !line.starts_with("++")
}

fn strip_diff_plus_prefix(line: &str) -> String {
    if starts_with_single_plus(line) {
        line[1..].to_string()
    } else {
        line.to_string()
    }
}

fn read_truncation_notice_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\[(?:Showing lines \d+-\d+ of \d+|\d+ more lines? in (?:file|\S+))\b.*\bUse :L?\d+",
        )
        .expect("valid regex")
    })
}

fn strip_leading_hashline_prefixes(line: &str) -> String {
    let mut result = line.to_string();
    loop {
        let previous = result.clone();
        result = hl_prefix_re().replace(&result, "").into_owned();
        if result == previous {
            break;
        }
    }
    result
}

#[derive(Default)]
struct LinePrefixStats {
    non_empty: usize,
    header_count: usize,
    hash_prefix_count: usize,
    diff_plus_hash_prefix_count: usize,
    diff_plus_count: usize,
    truncation_notice_count: usize,
}

fn collect_line_prefix_stats(lines: &[String]) -> LinePrefixStats {
    let mut stats = LinePrefixStats::default();
    let header_re = hl_header_re();
    let trunc_re = read_truncation_notice_re();
    let prefix_re = hl_prefix_re();
    let prefix_plus_re = hl_prefix_plus_re();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if trunc_re.is_match(line) {
            stats.truncation_notice_count += 1;
            continue;
        }
        if header_re.is_match(line) {
            stats.non_empty += 1;
            stats.header_count += 1;
            continue;
        }
        stats.non_empty += 1;
        if prefix_re.is_match(line) {
            stats.hash_prefix_count += 1;
        }
        if prefix_plus_re.is_match(line) {
            stats.diff_plus_hash_prefix_count += 1;
        }
        if starts_with_single_plus(line) {
            stats.diff_plus_count += 1;
        }
    }
    stats
}

pub fn strip_new_line_prefixes(lines: &[String]) -> Vec<String> {
    let stats = collect_line_prefix_stats(lines);
    if stats.non_empty == 0 {
        return lines.to_vec();
    }
    let content_line_count = stats.non_empty - stats.header_count;
    let strip_hash = content_line_count > 0 && stats.hash_prefix_count == content_line_count;
    let strip_plus = !strip_hash
        && stats.diff_plus_hash_prefix_count == 0
        && stats.diff_plus_count > 0
        && stats.diff_plus_count * 2 >= stats.non_empty;
    if !strip_hash && !strip_plus && stats.diff_plus_hash_prefix_count == 0 {
        return lines.to_vec();
    }
    let header_re = hl_header_re();
    let trunc_re = read_truncation_notice_re();
    let prefix_plus_re = hl_prefix_plus_re();
    lines
        .iter()
        .filter(|line| !(trunc_re.is_match(line) || strip_hash && header_re.is_match(line)))
        .map(|line| {
            if strip_hash {
                strip_leading_hashline_prefixes(line)
            } else if strip_plus {
                strip_diff_plus_prefix(line)
            } else if stats.diff_plus_hash_prefix_count > 0 && prefix_plus_re.is_match(line) {
                hl_prefix_re().replace(line, "").into_owned()
            } else {
                line.clone()
            }
        })
        .collect()
}

pub fn strip_hashline_prefixes(lines: &[String]) -> Vec<String> {
    let stats = collect_line_prefix_stats(lines);
    if stats.non_empty == 0 {
        return lines.to_vec();
    }
    let content_line_count = stats.non_empty - stats.header_count;
    if content_line_count == 0 || stats.hash_prefix_count != content_line_count {
        return lines.to_vec();
    }
    let header_re = hl_header_re();
    let trunc_re = read_truncation_notice_re();
    lines
        .iter()
        .filter(|line| !trunc_re.is_match(line) && !header_re.is_match(line))
        .map(|line| strip_leading_hashline_prefixes(line))
        .collect()
}

pub fn hashline_parse_text(edit: Option<&str>) -> Vec<String> {
    let Some(edit) = edit else {
        return Vec::new();
    };
    let trimmed = if let Some(stripped) = edit.strip_suffix('\n') {
        stripped
    } else {
        edit
    };
    let lines: Vec<String> = trimmed
        .replace('\r', "")
        .split('\n')
        .map(String::from)
        .collect();
    strip_new_line_prefixes(&lines)
}
