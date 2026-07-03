//! Diff generation and hunk parsing (OMP `diff.ts` port).

use std::sync::OnceLock;

use regex::Regex;
use similar::{ChangeTag, TextDiff};

use super::errors::{ApplyPatchError, ParseError};

const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

static UNIFIED_HUNK_HEADER_REGEX: OnceLock<Regex> = OnceLock::new();
static LINE_HINT_REGEX: OnceLock<Regex> = OnceLock::new();
static TOP_OF_FILE_REGEX: OnceLock<Regex> = OnceLock::new();
static LINE_NUMBER_PREFIX_REGEX: OnceLock<Regex> = OnceLock::new();

fn unified_hunk_header_regex() -> &'static Regex {
    UNIFIED_HUNK_HEADER_REGEX.get_or_init(|| {
        Regex::new(r"^@@\s*-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s*@@(?:\s*(.*))?$").unwrap()
    })
}

fn line_hint_regex() -> &'static Regex {
    LINE_HINT_REGEX
        .get_or_init(|| Regex::new(r"(?i)^lines?\s+(\d+)(?:\s*-\s*(\d+))?(?:\s*@@)?$").unwrap())
}

fn top_of_file_regex() -> &'static Regex {
    TOP_OF_FILE_REGEX
        .get_or_init(|| Regex::new(r"(?i)^(top|start|beginning)\s+of\s+file$").unwrap())
}

fn line_number_prefix_regex() -> &'static Regex {
    LINE_NUMBER_PREFIX_REGEX.get_or_init(|| Regex::new(r"^\s*(\d{1,6})\s+(.+)$").unwrap())
}

const MULTI_FILE_MARKERS: &[&str] = &[
    "*** Update File:",
    "*** Add File:",
    "*** Delete File:",
    "diff --git ",
];

const DIFF_METADATA_PREFIXES: &[&str] = &[
    "*** Update File:",
    "*** Add File:",
    "*** Delete File:",
    "diff --git ",
    "index ",
    "--- ",
    "+++ ",
    "new file mode ",
    "deleted file mode ",
    "rename from ",
    "rename to ",
    "similarity index ",
    "dissimilarity index ",
    "old mode ",
    "new mode ",
];

const PATCH_WRAPPER_PREFIXES: &[&str] = &["*** Begin Patch", "*** End Patch"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub diff: String,
    pub first_changed_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub change_context: Option<String>,
    pub old_start_line: Option<usize>,
    pub new_start_line: Option<usize>,
    pub has_context_lines: bool,
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
    pub is_end_of_file: bool,
}

fn format_numbered_diff_line(prefix: char, line_num: usize, content: &str) -> String {
    format!("{prefix}{line_num}|{content}")
}

struct LineDiffPart {
    tag: ChangeTag,
    value: String,
}

fn diff_line_parts(old_content: &str, new_content: &str) -> Vec<LineDiffPart> {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut parts = Vec::new();
    let mut current_tag: Option<ChangeTag> = None;
    let mut current_value = String::new();

    for change in diff.iter_all_changes() {
        let tag = change.tag();
        if current_tag == Some(tag) {
            current_value.push_str(change.value());
        } else {
            if let Some(prev_tag) = current_tag {
                parts.push(LineDiffPart {
                    tag: prev_tag,
                    value: std::mem::take(&mut current_value),
                });
            }
            current_tag = Some(tag);
            current_value = change.value().to_string();
        }
    }
    if let Some(tag) = current_tag {
        parts.push(LineDiffPart {
            tag,
            value: current_value,
        });
    }
    parts
}

fn split_diff_part_lines(value: &str) -> Vec<String> {
    let mut raw: Vec<String> = value.split('\n').map(str::to_string).collect();
    if raw.last().is_some_and(|line| line.is_empty()) {
        raw.pop();
    }
    raw
}

/// Generate a numbered diff string with line numbers and context.
pub fn generate_diff_string(
    old_content: &str,
    new_content: &str,
    context_lines: usize,
) -> DiffResult {
    let parts = diff_line_parts(old_content, new_content);
    let mut output = Vec::new();
    let mut old_line_num = 1usize;
    let mut new_line_num = 1usize;
    let mut last_was_change = false;
    let mut first_changed_line = None;

    for (i, part) in parts.iter().enumerate() {
        let added = part.tag == ChangeTag::Insert;
        let removed = part.tag == ChangeTag::Delete;
        let raw = split_diff_part_lines(&part.value);

        if added || removed {
            if first_changed_line.is_none() {
                first_changed_line = Some(new_line_num);
            }
            for line in &raw {
                if added {
                    output.push(format_numbered_diff_line('+', new_line_num, line));
                    new_line_num += 1;
                } else {
                    output.push(format_numbered_diff_line('-', old_line_num, line));
                    old_line_num += 1;
                }
            }
            last_was_change = true;
        } else {
            let next_part_is_change = parts
                .get(i + 1)
                .is_some_and(|p| p.tag == ChangeTag::Insert || p.tag == ChangeTag::Delete);

            if last_was_change || next_part_is_change {
                let context_limit = context_lines;
                let (leading_skip, middle_skip, trailing_skip, lines_to_show) =
                    if last_was_change && next_part_is_change {
                        if raw.len() > context_limit * 2 {
                            let leading_context = &raw[..context_limit];
                            let trailing_context = &raw[raw.len() - context_limit..];
                            let middle = raw.len() - leading_context.len() - trailing_context.len();
                            (0, middle, 0, [leading_context, trailing_context].concat())
                        } else {
                            (0, 0, 0, raw.clone())
                        }
                    } else if next_part_is_change {
                        let leading_skip = raw.len().saturating_sub(context_limit);
                        (leading_skip, 0, 0, raw[leading_skip..].to_vec())
                    } else {
                        let trailing_skip = raw.len().saturating_sub(context_limit);
                        (
                            0,
                            0,
                            trailing_skip,
                            raw[..context_limit.min(raw.len())].to_vec(),
                        )
                    };

                if leading_skip > 0 {
                    old_line_num += leading_skip;
                    new_line_num += leading_skip;
                }

                let first_chunk_length = if middle_skip > 0 {
                    context_limit
                } else {
                    lines_to_show.len()
                };

                for line in lines_to_show.iter().take(first_chunk_length) {
                    output.push(format_numbered_diff_line(' ', old_line_num, line));
                    old_line_num += 1;
                    new_line_num += 1;
                }

                if middle_skip > 0 {
                    output.push(format_numbered_diff_line(' ', old_line_num, "..."));
                    old_line_num += middle_skip;
                    new_line_num += middle_skip;
                    for line in lines_to_show.iter().skip(first_chunk_length) {
                        output.push(format_numbered_diff_line(' ', old_line_num, line));
                        old_line_num += 1;
                        new_line_num += 1;
                    }
                }

                if trailing_skip > 0 {
                    old_line_num += trailing_skip;
                    new_line_num += trailing_skip;
                }
            } else {
                old_line_num += raw.len();
                new_line_num += raw.len();
            }
            last_was_change = false;
        }
    }

    DiffResult {
        diff: output.join("\n"),
        first_changed_line,
    }
}

fn is_diff_content_line(line: &str) -> bool {
    let Some(first_char) = line.chars().next() else {
        return false;
    };
    match first_char {
        ' ' => true,
        '+' => !line.starts_with("+++ "),
        '-' => !line.starts_with("--- "),
        _ => false,
    }
}

fn matches_trimmed_prefix(line: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| line.starts_with(prefix))
}

fn is_patch_wrapper_line(line: &str) -> bool {
    line == "***" || matches_trimmed_prefix(line, PATCH_WRAPPER_PREFIXES)
}

/// Strip patch wrappers and metadata lines from diff text.
pub fn normalize_diff(diff: &str) -> String {
    let mut lines: Vec<&str> = diff.split('\n').collect();

    while let Some(last_line) = lines.last() {
        if last_line.is_empty() || (last_line.trim().is_empty() && !is_diff_content_line(last_line))
        {
            lines.pop();
        } else {
            break;
        }
    }

    if lines
        .first()
        .is_some_and(|line| is_patch_wrapper_line(line.trim()))
    {
        lines.remove(0);
    }
    if lines
        .last()
        .is_some_and(|line| is_patch_wrapper_line(line.trim()))
    {
        lines.pop();
    }

    lines
        .into_iter()
        .filter(|line| {
            is_diff_content_line(line)
                || !matches_trimmed_prefix(line.trim(), DIFF_METADATA_PREFIXES)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Normalize create-file content that may use `+` line prefixes.
pub fn normalize_create_content(content: &str) -> String {
    let lines: Vec<&str> = content.split('\n').collect();
    let non_empty: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.is_empty())
        .collect();

    if !non_empty.is_empty()
        && non_empty
            .iter()
            .all(|line| line.starts_with("+ ") || line.starts_with('+'))
    {
        return lines
            .iter()
            .map(|line| {
                if let Some(stripped) = line.strip_prefix("+ ") {
                    stripped
                } else if let Some(stripped) = line.strip_prefix('+') {
                    stripped
                } else {
                    *line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    content.to_string()
}

struct UnifiedHunkHeader {
    old_start_line: usize,
    new_start_line: usize,
    change_context: Option<String>,
}

fn parse_unified_hunk_header(line: &str) -> Option<UnifiedHunkHeader> {
    let caps = unified_hunk_header_regex().captures(line.trim_end())?;
    let old_start_line: usize = caps.get(1)?.as_str().parse().ok()?;
    let new_start_line: usize = caps.get(3)?.as_str().parse().ok()?;
    let change_context = caps
        .get(5)
        .map(|m| m.as_str().trim())
        .filter(|s| !s.is_empty());

    Some(UnifiedHunkHeader {
        old_start_line,
        new_start_line,
        change_context: change_context.map(str::to_string),
    })
}

fn is_unified_diff_metadata_line(line: &str) -> bool {
    let prefixes: Vec<&str> = DIFF_METADATA_PREFIXES
        .iter()
        .copied()
        .filter(|prefix| !prefix.starts_with("*** "))
        .collect();
    matches_trimmed_prefix(line, &prefixes)
}

struct ParseHunkResult {
    hunk: DiffHunk,
    lines_consumed: usize,
}

fn parse_one_hunk(
    lines: &[&str],
    line_number: usize,
    allow_missing_context: bool,
) -> Result<ParseHunkResult, ParseError> {
    if lines.is_empty() {
        return Err(ParseError::new(
            "Diff does not contain any lines",
            Some(line_number),
        ));
    }

    let mut change_contexts: Vec<String> = Vec::new();
    let mut old_start_line = None;
    let mut new_start_line = None;

    let header_line = lines[0];
    let header_trimmed = header_line.trim_end();
    let is_header_line = header_line.starts_with("@@");
    let unified_header = if is_header_line {
        parse_unified_hunk_header(header_trimmed)
    } else {
        None
    };
    let is_empty_context_marker =
        header_trimmed == EMPTY_CHANGE_CONTEXT_MARKER || header_trimmed == "@@ @@";

    let start_index = if is_header_line
        && (header_trimmed == EMPTY_CHANGE_CONTEXT_MARKER || is_empty_context_marker)
    {
        1
    } else if let Some(header) = unified_header {
        if header.old_start_line < 1 || header.new_start_line < 1 {
            return Err(ParseError::new(
                "Line numbers in @@ header must be >= 1",
                Some(line_number),
            ));
        }
        if let Some(ctx) = header.change_context {
            change_contexts.push(ctx);
        }
        old_start_line = Some(header.old_start_line);
        new_start_line = Some(header.new_start_line);
        1
    } else if is_header_line && header_trimmed.starts_with(CHANGE_CONTEXT_MARKER) {
        let context_value = &header_trimmed[CHANGE_CONTEXT_MARKER.len()..];
        let trimmed_context_value = context_value.trim();
        let normalized_context_value = trimmed_context_value.trim_start_matches("@@ ");

        if let Some(caps) = line_hint_regex().captures(normalized_context_value) {
            let start: usize = caps
                .get(1)
                .map(|m| m.as_str().parse().unwrap_or(0))
                .unwrap_or(0);
            if start < 1 {
                return Err(ParseError::new("Line hint must be >= 1", Some(line_number)));
            }
            old_start_line = Some(start);
            new_start_line = Some(start);
        } else if top_of_file_regex().is_match(normalized_context_value) {
            old_start_line = Some(1);
            new_start_line = Some(1);
        } else if !trimmed_context_value.is_empty() {
            change_contexts.push(context_value.to_string());
        }
        1
    } else if is_header_line {
        let context_value = header_trimmed.trim_start_matches("@@").trim();
        if !context_value.is_empty() {
            change_contexts.push(context_value.to_string());
        }
        1
    } else {
        if !allow_missing_context {
            return Err(ParseError::new(
                format!(
                    "Expected hunk to start with @@ context marker, got: '{}'",
                    lines[0]
                ),
                Some(line_number),
            ));
        }
        0
    };

    if let Some(invalid) = old_start_line.filter(|&n| n < 1) {
        return Err(ParseError::new(
            format!("Line numbers must be >= 1 (got {invalid})"),
            Some(line_number),
        ));
    }
    if let Some(invalid) = new_start_line.filter(|&n| n < 1) {
        return Err(ParseError::new(
            format!("Line numbers must be >= 1 (got {invalid})"),
            Some(line_number),
        ));
    }

    let mut nested_start = start_index;
    while nested_start < lines.len() {
        let next_line = lines[nested_start];
        if !next_line.starts_with("@@") {
            break;
        }
        let trimmed = next_line.trim_end();
        if let Some(nested_context) = trimmed.strip_prefix(CHANGE_CONTEXT_MARKER) {
            if !nested_context.trim().is_empty() {
                change_contexts.push(nested_context.to_string());
            }
            nested_start += 1;
        } else if trimmed == EMPTY_CHANGE_CONTEXT_MARKER {
            nested_start += 1;
        } else {
            break;
        }
    }

    if nested_start >= lines.len() {
        return Err(ParseError::new(
            "Hunk does not contain any lines",
            Some(line_number + 1),
        ));
    }

    let change_context = if change_contexts.is_empty() {
        None
    } else {
        Some(change_contexts.join("\n"))
    };

    let mut hunk = DiffHunk {
        change_context,
        old_start_line,
        new_start_line,
        has_context_lines: false,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };

    let mut parsed_lines = 0usize;

    for i in nested_start..lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let next_line = lines.get(i + 1).copied();

        if line.is_empty()
            && parsed_lines > 0
            && next_line.is_some_and(|n| n.trim_start().starts_with("@@"))
        {
            break;
        }

        if !is_diff_content_line(line)
            && line.trim_end() == EOF_MARKER
            && line.starts_with(EOF_MARKER)
        {
            if parsed_lines == 0 {
                return Err(ParseError::new(
                    "Hunk does not contain any lines",
                    Some(line_number + 1),
                ));
            }
            hunk.is_end_of_file = true;
            parsed_lines += 1;
            break;
        }

        if trimmed == "..." || trimmed == "…" {
            hunk.has_context_lines = true;
            parsed_lines += 1;
            continue;
        }

        let first_char = line.chars().next();

        match first_char {
            None => {
                hunk.has_context_lines = true;
                hunk.old_lines.push(String::new());
                hunk.new_lines.push(String::new());
            }
            Some(' ') => {
                hunk.has_context_lines = true;
                hunk.old_lines.push(line[1..].to_string());
                hunk.new_lines.push(line[1..].to_string());
            }
            Some('+') => {
                hunk.new_lines.push(line[1..].to_string());
            }
            Some('-') => {
                hunk.old_lines.push(line[1..].to_string());
            }
            Some(_) if !line.starts_with("@@") => {
                hunk.has_context_lines = true;
                hunk.old_lines.push(line.to_string());
                hunk.new_lines.push(line.to_string());
            }
            Some(_) => {
                if parsed_lines == 0 {
                    return Err(ParseError::new(
                        format!(
                            "Unexpected line in hunk: '{line}'. Lines must start with ' ' (context), '+' (add), or '-' (remove)"
                        ),
                        Some(line_number + 1),
                    ));
                }
                break;
            }
        }
        parsed_lines += 1;
    }

    if parsed_lines == 0 {
        return Err(ParseError::new(
            "Hunk does not contain any lines",
            Some(line_number + nested_start),
        ));
    }

    strip_line_number_prefixes(&mut hunk);
    Ok(ParseHunkResult {
        hunk,
        lines_consumed: parsed_lines + nested_start,
    })
}

fn strip_line_number_prefixes(hunk: &mut DiffHunk) {
    let all_lines: Vec<String> = hunk
        .old_lines
        .iter()
        .chain(hunk.new_lines.iter())
        .filter(|line| !line.trim().is_empty())
        .cloned()
        .collect();
    if all_lines.len() < 2 {
        return;
    }

    let number_matches: Vec<_> = all_lines
        .iter()
        .filter_map(|line| line_number_prefix_regex().captures(line))
        .collect();

    if number_matches.len() < usize::max(2, (all_lines.len() * 3).div_ceil(5)) {
        return;
    }

    let numbers: Vec<usize> = number_matches
        .iter()
        .filter_map(|caps| caps.get(1).and_then(|m| m.as_str().parse().ok()))
        .collect();
    let mut sequential = 0usize;
    for window in numbers.windows(2) {
        if window[1] == window[0] + 1 {
            sequential += 1;
        }
    }
    if numbers.len() >= 3 && sequential < numbers.len().saturating_sub(2).max(1) {
        return;
    }

    let strip = |line: &str| -> String {
        line_number_prefix_regex()
            .captures(line)
            .and_then(|caps| caps.get(2).map(|m| m.as_str().to_string()))
            .unwrap_or_else(|| line.to_string())
    };

    hunk.old_lines = hunk.old_lines.iter().map(|l| strip(l)).collect();
    hunk.new_lines = hunk.new_lines.iter().map(|l| strip(l)).collect();
}

fn extract_marker_path(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("diff --git ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        let candidate = parts.get(2).or_else(|| parts.get(1))?;
        return Some(
            candidate
                .trim_start_matches('a')
                .trim_start_matches('b')
                .trim_start_matches('/')
                .to_string(),
        );
    }
    if let Some(path) = line.strip_prefix("*** Update File:") {
        return Some(path.trim().to_string());
    }
    if let Some(path) = line.strip_prefix("*** Add File:") {
        return Some(path.trim().to_string());
    }
    if let Some(path) = line.strip_prefix("*** Delete File:") {
        return Some(path.trim().to_string());
    }
    None
}

fn count_multi_file_markers(diff: &str) -> usize {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut paths = std::collections::HashSet::new();

    for line in diff.split('\n') {
        if is_diff_content_line(line) {
            continue;
        }
        let trimmed = line.trim();
        for marker in MULTI_FILE_MARKERS {
            if trimmed.starts_with(marker) {
                if let Some(file_path) = extract_marker_path(trimmed) {
                    paths.insert(file_path);
                }
                *counts.entry(marker).or_default() += 1;
                break;
            }
        }
    }

    if !paths.is_empty() {
        return paths.len();
    }
    counts.values().copied().max().unwrap_or(0)
}

/// Parse unified-diff hunks from a single-file patch.
pub fn parse_diff_hunks(diff: &str) -> Result<Vec<DiffHunk>, ApplyPatchError> {
    let multi_file_count = count_multi_file_markers(diff);
    if multi_file_count > 1 {
        return Err(ApplyPatchError(format!(
            "Diff contains {multi_file_count} file markers. Single-file patches cannot contain multi-file markers."
        )));
    }

    let normalized_diff = normalize_diff(diff);
    let lines: Vec<String> = normalized_diff.split('\n').map(str::to_string).collect();
    let mut hunks = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let line = &lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        let first_char = line.chars().next();
        let is_diff_content = matches!(first_char, Some(' ') | Some('+') | Some('-'));
        if !is_diff_content && is_unified_diff_metadata_line(trimmed) {
            i += 1;
            continue;
        }

        if trimmed.starts_with("@@") && lines[i + 1..].iter().all(|next| next.trim().is_empty()) {
            break;
        }

        let slice: Vec<&str> = lines[i..].iter().map(String::as_str).collect();
        let ParseHunkResult {
            hunk,
            lines_consumed,
        } = parse_one_hunk(&slice, i + 1, true).map_err(|e| ApplyPatchError(e.to_string()))?;
        hunks.push(hunk);
        i += lines_consumed;
    }

    Ok(hunks)
}

pub use super::text_replace::{replace_text, ReplaceOptions, ReplaceResult};
