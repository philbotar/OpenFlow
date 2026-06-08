//! Patch application for the edit engine (OMP `modes/patch.ts` port).

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::diff::{normalize_create_content, parse_diff_hunks, DiffHunk};
use super::errors::ApplyPatchError;
use super::path::resolve_writable;
use super::normalize::{
    adjust_indentation, convert_leading_tabs_to_spaces, count_leading_whitespace,
    detect_line_ending, get_leading_whitespace, leading_whitespace_byte_len,
    normalize_to_lf, restore_line_endings, strip_bom, BomResult,
};
use super::replace::{
    find_match, FindMatchOptions, DOMINANT_FUZZY_MIN_CONFIDENCE, DEFAULT_FUZZY_THRESHOLD,
};
use super::replace_sequence::{
    find_closest_sequence_match, find_context_line, seek_sequence, ContextMatchStrategy,
    SequenceMatchStrategy, SequenceSearchResult,
};

const AMBIGUITY_HINT_WINDOW: usize = 200;
const MATCH_PREVIEW_CONTEXT: usize = 2;
const MATCH_PREVIEW_MAX_LEN: usize = 80;
const CHARACTER_RELAXED_THRESHOLD: f64 = 0.92;
const MAX_OCCURRENCE_PREVIEWS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOp {
    Create,
    Delete,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchInput {
    pub path: String,
    pub op: PatchOp,
    pub rename: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PatchOptions {
    pub cwd: PathBuf,
    pub dry_run: bool,
    pub allow_fuzzy: bool,
    pub fuzzy_threshold: f64,
}

impl Default for PatchOptions {
    fn default() -> Self {
        Self {
            cwd: PathBuf::new(),
            dry_run: false,
            allow_fuzzy: true,
            fuzzy_threshold: DEFAULT_FUZZY_THRESHOLD,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchApplyResult {
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub dest_path: PathBuf,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct PatchVerifyError {
    pub message: String,
    pub relative_path: String,
    pub resolved_path: PathBuf,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum PatchError {
    #[error("{0}")]
    Apply(#[from] ApplyPatchError),
    #[error("{0}")]
    Verify(PatchVerifyError),
}

pub trait PatchFileSystem: Send + Sync {
    fn read(&self, path: &Path) -> io::Result<String>;
    fn read_binary(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn write(&self, path: &Path, content: &str) -> io::Result<()>;
    fn delete(&self, path: &Path) -> io::Result<()>;
    fn mkdir_all(&self, path: &Path) -> io::Result<()>;
    fn exists(&self, path: &Path) -> io::Result<bool>;
}

pub struct StdPatchFileSystem;

impl PatchFileSystem for StdPatchFileSystem {
    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn read_binary(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, content)
    }

    fn delete(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn mkdir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        Ok(path.exists())
    }
}

struct Replacement {
    start_index: usize,
    old_len: usize,
    new_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HunkVariantKind {
    TrimCommon,
    DedupeShared,
    CollapseRepeated,
    SingleLine,
}

struct HunkVariant {
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    kind: HunkVariantKind,
}

fn is_blank_line(line: &str) -> bool {
    line.trim().is_empty()
}

fn are_equal_lines(left: &[String], right: &[String]) -> bool {
    left == right
}

fn are_equal_trimmed_lines(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(l, r)| l.trim() == r.trim())
}

fn get_indent_char(lines: &[String]) -> char {
    for line in lines {
        let ws = get_leading_whitespace(line);
        if !ws.is_empty() {
            return ws.chars().next().unwrap_or(' ');
        }
    }
    ' '
}

fn collect_indent_deltas(old_lines: &[String], actual_lines: &[String]) -> Vec<isize> {
    let line_count = old_lines.len().min(actual_lines.len());
    let mut deltas = Vec::new();
    for i in 0..line_count {
        let old_line = &old_lines[i];
        let actual_line = &actual_lines[i];
        if is_blank_line(old_line) || is_blank_line(actual_line) {
            continue;
        }
        deltas.push(
            count_leading_whitespace(actual_line) as isize
                - count_leading_whitespace(old_line) as isize,
        );
    }
    deltas
}

fn apply_indent_delta_line(line: &str, delta: isize, indent_char: char) -> String {
    if is_blank_line(line) {
        return line.to_string();
    }
    if delta > 0 {
        return format!("{}{line}", indent_char.to_string().repeat(delta as usize));
    }
    let to_remove = (-delta as usize).min(leading_whitespace_byte_len(line));
    line[to_remove..].to_string()
}

fn can_convert_tabs_to_spaces(
    old_lines: &[String],
    actual_lines: &[String],
    spaces_per_tab: usize,
) -> bool {
    let line_count = old_lines.len().min(actual_lines.len());
    for i in 0..line_count {
        let old_line = &old_lines[i];
        let actual_line = &actual_lines[i];
        if is_blank_line(old_line) || is_blank_line(actual_line) {
            continue;
        }
        let old_indent = get_leading_whitespace(old_line);
        let actual_indent = get_leading_whitespace(actual_line);
        if old_indent.is_empty() {
            continue;
        }
        if actual_indent.len() != old_indent.len() * spaces_per_tab {
            return false;
        }
    }
    true
}

fn adjust_lines_indentation(
    pattern_lines: &[String],
    actual_lines: &[String],
    new_lines: &[String],
) -> Vec<String> {
    if pattern_lines.is_empty() || actual_lines.is_empty() || new_lines.is_empty() {
        return new_lines.to_vec();
    }

    if are_equal_lines(pattern_lines, actual_lines) {
        return new_lines.to_vec();
    }

    if are_equal_trimmed_lines(pattern_lines, new_lines) {
        return new_lines.to_vec();
    }

    let indent_char = get_indent_char(actual_lines);

    let mut pattern_tab_only = true;
    let mut actual_space_only = true;
    let mut pattern_space_only = true;
    let mut actual_tab_only = true;
    let mut pattern_mixed = false;
    let mut actual_mixed = false;

    for line in pattern_lines {
        if line.trim().is_empty() {
            continue;
        }
        let ws = get_leading_whitespace(line);
        if ws.contains(' ') {
            pattern_tab_only = false;
        }
        if ws.contains('\t') {
            pattern_space_only = false;
        }
        if ws.contains(' ') && ws.contains('\t') {
            pattern_mixed = true;
        }
    }

    for line in actual_lines {
        if line.trim().is_empty() {
            continue;
        }
        let ws = get_leading_whitespace(line);
        if ws.contains('\t') {
            actual_space_only = false;
        }
        if ws.contains(' ') {
            actual_tab_only = false;
        }
        if ws.contains(' ') && ws.contains('\t') {
            actual_mixed = true;
        }
    }

    if !pattern_mixed && !actual_mixed && pattern_tab_only && actual_space_only {
        let line_count = pattern_lines.len().min(actual_lines.len());
        let mut ratio: Option<f64> = None;
        let mut consistent = true;
        for i in 0..line_count {
            let pattern_line = &pattern_lines[i];
            let actual_line = &actual_lines[i];
            if pattern_line.trim().is_empty() || actual_line.trim().is_empty() {
                continue;
            }
            let pattern_indent = count_leading_whitespace(pattern_line);
            let actual_indent = count_leading_whitespace(actual_line);
            if pattern_indent == 0 {
                continue;
            }
            if !actual_indent.is_multiple_of(pattern_indent) {
                consistent = false;
                break;
            }
            let next_ratio = actual_indent as f64 / pattern_indent as f64;
            if let Some(r) = ratio {
                if (r - next_ratio).abs() > f64::EPSILON {
                    consistent = false;
                    break;
                }
            } else {
                ratio = Some(next_ratio);
            }
        }

        if let Some(ratio) = ratio {
            if consistent
                && can_convert_tabs_to_spaces(pattern_lines, actual_lines, ratio.round() as usize)
            {
                let converted =
                    convert_leading_tabs_to_spaces(&new_lines.join("\n"), ratio.round() as usize);
                return converted.split('\n').map(str::to_string).collect();
            }
        }
    }

    if !pattern_mixed && !actual_mixed && pattern_space_only && actual_tab_only {
        let mut samples: HashMap<usize, usize> = HashMap::new();
        let line_count = pattern_lines.len().min(actual_lines.len());
        let mut consistent = true;
        for i in 0..line_count {
            let pattern_line = &pattern_lines[i];
            let actual_line = &actual_lines[i];
            if pattern_line.trim().is_empty() || actual_line.trim().is_empty() {
                continue;
            }
            let spaces = count_leading_whitespace(pattern_line);
            let tabs = count_leading_whitespace(actual_line);
            if tabs == 0 {
                continue;
            }
            if let Some(existing) = samples.get(&tabs) {
                if *existing != spaces {
                    consistent = false;
                    break;
                }
            }
            samples.insert(tabs, spaces);
        }

        if consistent && !samples.is_empty() {
            let tab_width = resolve_tab_width(&samples);
            if let Some(tab_width) = tab_width {
                let offset = samples
                    .iter()
                    .next()
                    .map(|(t, s)| *s as isize - *t as isize * tab_width)
                    .unwrap_or(0);
                return new_lines
                    .iter()
                    .map(|line| convert_spaces_to_tabs_line(line, tab_width, offset))
                    .collect();
            }
        }
    }

    let mut content_to_actual_lines: HashMap<String, Vec<String>> = HashMap::new();
    for line in actual_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        content_to_actual_lines
            .entry(trimmed.to_string())
            .or_default()
            .push(line.clone());
    }

    let mut pattern_min = usize::MAX;
    for line in pattern_lines {
        if line.trim().is_empty() {
            continue;
        }
        pattern_min = pattern_min.min(count_leading_whitespace(line));
    }
    if pattern_min == usize::MAX {
        pattern_min = 0;
    }

    let deltas = collect_indent_deltas(pattern_lines, actual_lines);
    let delta = if !deltas.is_empty() && deltas.iter().all(|d| *d == deltas[0]) {
        Some(deltas[0])
    } else {
        None
    };

    let mut used_actual_lines: HashMap<String, usize> = HashMap::new();

    new_lines
        .iter()
        .map(|new_line| {
            if new_line.trim().is_empty() {
                return new_line.clone();
            }

            let trimmed = new_line.trim();
            if let Some(matching_actual_lines) = content_to_actual_lines.get(trimmed) {
                if matching_actual_lines.len() == 1 {
                    return matching_actual_lines[0].clone();
                }
                if matching_actual_lines.contains(new_line) {
                    return new_line.clone();
                }
                let used_count = used_actual_lines.entry(trimmed.to_string()).or_insert(0);
                if *used_count < matching_actual_lines.len() {
                    let result = matching_actual_lines[*used_count].clone();
                    *used_count += 1;
                    return result;
                }
            }

            if let Some(delta) = delta.filter(|&d| d != 0) {
                let new_indent = count_leading_whitespace(new_line);
                if new_indent == pattern_min {
                    return apply_indent_delta_line(new_line, delta, indent_char);
                }
            }
            new_line.clone()
        })
        .collect()
}

fn resolve_tab_width(samples: &HashMap<usize, usize>) -> Option<isize> {
    if samples.len() == 1 {
        let (&tabs, &spaces) = samples.iter().next()?;
        if spaces % tabs == 0 {
            return Some((spaces / tabs) as isize);
        }
        return None;
    }

    let entries: Vec<(usize, usize)> = samples.iter().map(|(k, v)| (*k, *v)).collect();
    let (t1, s1) = entries[0];
    let (t2, s2) = entries[1];
    if t1 == t2 {
        return None;
    }
    let w = (s2 as isize - s1 as isize) / (t2 as isize - t1 as isize);
    if w <= 0 {
        return None;
    }
    let b = s1 as isize - t1 as isize * w;
    for (&t, &s) in samples {
        if t as isize * w + b != s as isize {
            return None;
        }
    }
    Some(w)
}

fn convert_spaces_to_tabs_line(line: &str, tab_width: isize, offset: isize) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    let ws = count_leading_whitespace(line);
    let ws_bytes = leading_whitespace_byte_len(line);
    if ws == 0 {
        return line.to_string();
    }
    let adjusted = ws as isize - offset;
    if adjusted >= 0 && adjusted % tab_width == 0 {
        return format!(
            "{}{}",
            "\t".repeat((adjusted / tab_width) as usize),
            &line[ws_bytes..]
        );
    }
    let tab_count = adjusted.div_euclid(tab_width);
    let remainder = adjusted - tab_count * tab_width;
    if tab_count >= 0 {
        return format!(
            "{}{}{}",
            "\t".repeat(tab_count as usize),
            " ".repeat(remainder as usize),
            &line[ws_bytes..]
        );
    }
    line.to_string()
}

fn trim_common_context(old_lines: &[String], new_lines: &[String]) -> Option<HunkVariant> {
    let mut start = 0;
    let mut end_old = old_lines.len();
    let mut end_new = new_lines.len();

    while start < end_old && start < end_new && old_lines[start] == new_lines[start] {
        start += 1;
    }

    while end_old > start && end_new > start && old_lines[end_old - 1] == new_lines[end_new - 1] {
        end_old -= 1;
        end_new -= 1;
    }

    if start == 0 && end_old == old_lines.len() && end_new == new_lines.len() {
        return None;
    }

    let trimmed_old = old_lines[start..end_old].to_vec();
    let trimmed_new = new_lines[start..end_new].to_vec();
    if trimmed_old.is_empty() && trimmed_new.is_empty() {
        return None;
    }
    Some(HunkVariant {
        old_lines: trimmed_old,
        new_lines: trimmed_new,
        kind: HunkVariantKind::TrimCommon,
    })
}

fn collapse_consecutive_shared_lines(old_lines: &[String], new_lines: &[String]) -> Option<HunkVariant> {
    let shared: HashSet<&str> = old_lines
        .iter()
        .filter(|line| new_lines.contains(line))
        .map(String::as_str)
        .collect();

    let collapse = |lines: &[String]| -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = &lines[i];
            out.push(line.clone());
            let mut j = i + 1;
            while j < lines.len() && lines[j] == *line && shared.contains(line.as_str()) {
                j += 1;
            }
            i = j;
        }
        out
    };

    let collapsed_old = collapse(old_lines);
    let collapsed_new = collapse(new_lines);
    if collapsed_old.len() == old_lines.len() && collapsed_new.len() == new_lines.len() {
        return None;
    }
    Some(HunkVariant {
        old_lines: collapsed_old,
        new_lines: collapsed_new,
        kind: HunkVariantKind::DedupeShared,
    })
}

fn collapse_repeated_blocks(old_lines: &[String], new_lines: &[String]) -> Option<HunkVariant> {
    let shared: HashSet<&str> = old_lines
        .iter()
        .filter(|line| new_lines.contains(line))
        .map(String::as_str)
        .collect();

    let collapse = |lines: &[String]| -> Vec<String> {
        let mut output = lines.to_vec();
        let mut changed = false;
        let mut i = 0;
        while i < output.len() {
            let mut collapsed = false;
            for size in (2..=(output.len() - i) / 2).rev() {
                let first = &output[i..i + size];
                let second = &output[i + size..i + size * 2];
                if first.len() != second.len() || first.is_empty() {
                    continue;
                }
                if !first.iter().all(|line| shared.contains(line.as_str())) {
                    continue;
                }
                if first != second {
                    continue;
                }
                output.drain(i + size..i + size * 2);
                changed = true;
                collapsed = true;
                break;
            }
            if !collapsed {
                i += 1;
            }
        }
        if changed { output } else { lines.to_vec() }
    };

    let collapsed_old = collapse(old_lines);
    let collapsed_new = collapse(new_lines);
    if collapsed_old.len() == old_lines.len() && collapsed_new.len() == new_lines.len() {
        return None;
    }
    Some(HunkVariant {
        old_lines: collapsed_old,
        new_lines: collapsed_new,
        kind: HunkVariantKind::CollapseRepeated,
    })
}

fn reduce_to_single_line_change(old_lines: &[String], new_lines: &[String]) -> Option<HunkVariant> {
    if old_lines.len() != new_lines.len() || old_lines.is_empty() {
        return None;
    }
    let mut changed_index = None;
    for (i, (old, new)) in old_lines.iter().zip(new_lines.iter()).enumerate() {
        if old != new {
            if changed_index.is_some() {
                return None;
            }
            changed_index = Some(i);
        }
    }
    let idx = changed_index?;
    Some(HunkVariant {
        old_lines: vec![old_lines[idx].clone()],
        new_lines: vec![new_lines[idx].clone()],
        kind: HunkVariantKind::SingleLine,
    })
}

fn build_fallback_variants(hunk: &DiffHunk) -> Vec<HunkVariant> {
    let base = HunkVariant {
        old_lines: hunk.old_lines.clone(),
        new_lines: hunk.new_lines.clone(),
        kind: HunkVariantKind::TrimCommon,
    };

    let trimmed = trim_common_context(&base.old_lines, &base.new_lines);
    let deduped = collapse_consecutive_shared_lines(
        trimmed.as_ref().map(|v| v.old_lines.as_slice()).unwrap_or(&base.old_lines),
        trimmed.as_ref().map(|v| v.new_lines.as_slice()).unwrap_or(&base.new_lines),
    );
    let collapsed = collapse_repeated_blocks(
        deduped.as_ref().map(|v| v.old_lines.as_slice()).unwrap_or(
            trimmed.as_ref().map(|v| v.old_lines.as_slice()).unwrap_or(&base.old_lines),
        ),
        deduped.as_ref().map(|v| v.new_lines.as_slice()).unwrap_or(
            trimmed.as_ref().map(|v| v.new_lines.as_slice()).unwrap_or(&base.new_lines),
        ),
    );
    let single_line = reduce_to_single_line_change(
        trimmed.as_ref().map(|v| v.old_lines.as_slice()).unwrap_or(&base.old_lines),
        trimmed.as_ref().map(|v| v.new_lines.as_slice()).unwrap_or(&base.new_lines),
    );

    let mut variants = Vec::new();
    if let Some(v) = trimmed {
        variants.push(v);
    }
    if let Some(v) = deduped {
        variants.push(v);
    }
    if let Some(v) = collapsed {
        variants.push(v);
    }
    if let Some(v) = single_line {
        variants.push(v);
    }

    let mut seen = HashSet::new();
    variants
        .into_iter()
        .filter(|variant| {
            if variant.old_lines.is_empty() && variant.new_lines.is_empty() {
                return false;
            }
            let key = format!(
                "{}||{}",
                variant.old_lines.join("\n"),
                variant.new_lines.join("\n")
            );
            seen.insert(key)
        })
        .collect()
}

fn filter_fallback_variants(variants: Vec<HunkVariant>, allow_aggressive: bool) -> Vec<HunkVariant> {
    if allow_aggressive {
        return variants;
    }
    variants
        .into_iter()
        .filter(|variant| {
            !matches!(
                variant.kind,
                HunkVariantKind::CollapseRepeated | HunkVariantKind::SingleLine
            )
        })
        .collect()
}

fn find_context_relative_match(
    lines: &[String],
    pattern_line: &str,
    context_index: usize,
    prefer_second_forward_match: bool,
) -> Option<usize> {
    let trimmed = pattern_line.trim();
    let mut forward_matches = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(context_index + 1) {
        if line.trim() == trimmed {
            forward_matches.push(i);
        }
    }
    if !forward_matches.is_empty() {
        if prefer_second_forward_match && forward_matches.len() > 1 {
            return Some(forward_matches[1]);
        }
        return Some(forward_matches[0]);
    }
    (0..context_index)
        .rev()
        .find(|&i| lines[i].trim() == trimmed)
}

fn format_sequence_match_preview(lines: &[String], start_idx: usize) -> String {
    let start = start_idx.saturating_sub(MATCH_PREVIEW_CONTEXT);
    let end = (start_idx + MATCH_PREVIEW_CONTEXT + 1).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let num = start + i + 1;
            let truncated = if line.len() > MATCH_PREVIEW_MAX_LEN {
                format!("{}…", &line[..MATCH_PREVIEW_MAX_LEN - 1])
            } else {
                line.clone()
            };
            format!(" {num} | {truncated}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_sequence_match_previews(
    lines: &[String],
    match_indices: Option<&[usize]>,
    match_count: Option<usize>,
) -> Option<String> {
    let indices = match_indices?;
    if indices.is_empty() {
        return None;
    }
    let previews: Vec<String> = indices
        .iter()
        .map(|index| format_sequence_match_preview(lines, *index))
        .collect();
    let more_msg = match_count.filter(|&c| c > indices.len()).map(|c| {
        format!(" (showing first {} of {c})", indices.len())
    }).unwrap_or_default();
    Some(format!("{}{}", previews.join("\n\n"), more_msg))
}

fn choose_hinted_match(
    match_indices: Option<&[usize]>,
    hint_index: Option<usize>,
    window: usize,
) -> Option<usize> {
    let indices = match_indices?;
    if indices.is_empty() || hint_index.is_none() {
        return None;
    }
    let hint_index = hint_index?;
    let candidates: Vec<usize> = indices
        .iter()
        .copied()
        .filter(|index| index.abs_diff(hint_index) <= window)
        .collect();
    if candidates.len() == 1 {
        Some(candidates[0])
    } else {
        None
    }
}

fn get_hunk_hint_index(hunk: &DiffHunk, current_index: usize) -> Option<usize> {
    let hint_index = hunk.old_start_line? - 1;
    if hint_index >= current_index {
        Some(hint_index)
    } else {
        None
    }
}

fn format_sequence_strategy(strategy: Option<SequenceMatchStrategy>) -> Option<String> {
    strategy.map(|s| match s {
        SequenceMatchStrategy::Exact => "exact".to_string(),
        SequenceMatchStrategy::TrimTrailing => "trim-trailing".to_string(),
        SequenceMatchStrategy::Trim => "trim".to_string(),
        SequenceMatchStrategy::CommentPrefix => "comment-prefix".to_string(),
        SequenceMatchStrategy::Unicode => "unicode".to_string(),
        SequenceMatchStrategy::Prefix => "prefix".to_string(),
        SequenceMatchStrategy::Substring => "substring".to_string(),
        SequenceMatchStrategy::Fuzzy => "fuzzy".to_string(),
        SequenceMatchStrategy::FuzzyDominant => "fuzzy-dominant".to_string(),
        SequenceMatchStrategy::Character => "character".to_string(),
    })
}

fn format_context_strategy(strategy: Option<ContextMatchStrategy>) -> Option<String> {
    strategy.map(|s| match s {
        ContextMatchStrategy::Exact => "exact".to_string(),
        ContextMatchStrategy::Trim => "trim".to_string(),
        ContextMatchStrategy::Unicode => "unicode".to_string(),
        ContextMatchStrategy::Prefix => "prefix".to_string(),
        ContextMatchStrategy::Substring => "substring".to_string(),
        ContextMatchStrategy::Fuzzy => "fuzzy".to_string(),
    })
}

fn find_hierarchical_context(
    lines: &[String],
    context: &str,
    start_from: usize,
    line_hint: Option<usize>,
    allow_fuzzy: bool,
) -> super::replace_sequence::ContextLineResult {
    use super::replace_sequence::ContextLineResult;

    if context.contains('\n') {
        let parts: Vec<&str> = context
            .split('\n')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        let mut current_start = start_from;

        for (i, part) in parts.iter().enumerate() {
            let is_last = i + 1 == parts.len();
            let result = find_context_line(lines, part, current_start, allow_fuzzy, false);

            if result.match_count.unwrap_or(0) > 1 {
                if is_last {
                    if let Some(hint) = line_hint {
                        let hint_start = hint.saturating_sub(1);
                        if hint_start >= current_start {
                            let hinted =
                                find_context_line(lines, part, hint_start, allow_fuzzy, false);
                            if hinted.index.is_some() {
                                return ContextLineResult {
                                    match_count: Some(1),
                                    match_indices: hinted.index.map(|idx| vec![idx]),
                                    ..hinted
                                };
                            }
                        }
                    }
                }
                return result;
            }

            let Some(idx) = result.index else {
                if is_last {
                    if let Some(hint) = line_hint {
                        let hint_start = hint.saturating_sub(1);
                        if hint_start >= current_start {
                            let hinted =
                                find_context_line(lines, part, hint_start, allow_fuzzy, false);
                            if hinted.index.is_some() {
                                return ContextLineResult {
                                    match_count: Some(1),
                                    match_indices: hinted.index.map(|idx| vec![idx]),
                                    ..hinted
                                };
                            }
                        }
                    }
                }
                return ContextLineResult {
                    index: None,
                    confidence: result.confidence,
                    ..Default::default()
                };
            };

            if is_last {
                return result;
            }
            current_start = idx + 1;
        }
        return ContextLineResult {
            confidence: 0.0,
            ..Default::default()
        };
    }

    let space_parts: Vec<&str> = context.split_whitespace().collect();
    let has_signature_chars = context.contains(['(', ')', '{', '}', '[', ']']);
    if !has_signature_chars && space_parts.len() > 2 {
        let outer = space_parts[..space_parts.len() - 1].join(" ");
        let inner = space_parts[space_parts.len() - 1];
        let outer_result = find_context_line(lines, &outer, start_from, allow_fuzzy, false);
        if outer_result.match_count.unwrap_or(0) > 1 {
            return outer_result;
        }
        if let Some(outer_idx) = outer_result.index {
            let inner_result =
                find_context_line(lines, inner, outer_idx + 1, allow_fuzzy, false);
            if inner_result.index.is_some() {
                if inner_result.match_count.unwrap_or(0) > 1 {
                    return ContextLineResult {
                        match_count: Some(1),
                        match_indices: inner_result.index.map(|idx| vec![idx]),
                        ..inner_result
                    };
                }
                return inner_result;
            }
            if inner_result.match_count.unwrap_or(0) > 1 {
                return ContextLineResult {
                    match_count: Some(1),
                    match_indices: inner_result
                        .index
                        .map(|idx| vec![idx])
                        .or(inner_result.match_indices),
                    ..inner_result
                };
            }
        }
    }

    let result = find_context_line(lines, context, start_from, allow_fuzzy, false);

    if result.index.is_none() || result.match_count.unwrap_or(0) > 1 {
        if let Some(hint) = line_hint {
            let hint_start = hint.saturating_sub(1);
            let hinted_result = find_context_line(lines, context, hint_start, allow_fuzzy, false);
            if hinted_result.index.is_some() {
                return ContextLineResult {
                    match_count: Some(1),
                    match_indices: hinted_result.index.map(|idx| vec![idx]),
                    ..hinted_result
                };
            }
        }
    }

    if result.index.is_some() && result.match_count.unwrap_or(0) <= 1 {
        return result;
    }
    if result.match_count.unwrap_or(0) > 1 {
        return result;
    }

    if result.index.is_none() && start_from != 0 {
        let from_start = find_context_line(lines, context, 0, allow_fuzzy, false);
        if from_start.index.is_some() && from_start.match_count.unwrap_or(0) <= 1 {
            return from_start;
        }
        if from_start.match_count.unwrap_or(0) > 1 {
            return from_start;
        }
    }

    if !has_signature_chars && space_parts.len() > 1 {
        let outer = space_parts[..space_parts.len() - 1].join(" ");
        let inner = space_parts[space_parts.len() - 1];
        let outer_result = find_context_line(lines, &outer, start_from, allow_fuzzy, false);

        if outer_result.match_count.unwrap_or(0) > 1 {
            return outer_result;
        }

        let Some(outer_idx) = outer_result.index else {
            return ContextLineResult {
                index: None,
                confidence: outer_result.confidence,
                ..Default::default()
            };
        };

        let inner_result = find_context_line(lines, inner, outer_idx + 1, allow_fuzzy, false);
        if inner_result.index.is_some() {
            if inner_result.match_count.unwrap_or(0) > 1 {
                return ContextLineResult {
                    match_count: Some(1),
                    match_indices: inner_result.index.map(|idx| vec![idx]),
                    ..inner_result
                };
            }
            return inner_result;
        }
        if inner_result.match_count.unwrap_or(0) > 1 {
            return ContextLineResult {
                match_count: Some(1),
                match_indices: inner_result
                    .index
                    .map(|idx| vec![idx])
                    .or(inner_result.match_indices),
                ..inner_result
            };
        }
    }

    result
}

fn find_sequence_with_hint(
    lines: &[String],
    pattern: &[String],
    current_index: usize,
    hint_index: Option<usize>,
    eof: bool,
    allow_fuzzy: bool,
) -> SequenceSearchResult {
    let primary = seek_sequence(lines, pattern, current_index, eof, allow_fuzzy);
    if primary.match_count.unwrap_or(0) > 1 {
        if let Some(hint) = hint_index.filter(|&h| h != current_index) {
            let hinted = seek_sequence(lines, pattern, hint, eof, allow_fuzzy);
            if hinted.index.is_some() && hinted.match_count.unwrap_or(1) <= 1 {
                return hinted;
            }
            if hinted.match_count.unwrap_or(0) > 1 {
                return hinted;
            }
        }
    }
    if primary.index.is_some() || primary.match_count.unwrap_or(0) > 1 {
        return primary;
    }

    if let Some(hint) = hint_index.filter(|&h| h != current_index) {
        let hinted = seek_sequence(lines, pattern, hint, eof, allow_fuzzy);
        if hinted.index.is_some() || hinted.match_count.unwrap_or(0) > 1 {
            return hinted;
        }
    }

    if current_index != 0 {
        let from_start = seek_sequence(lines, pattern, 0, eof, allow_fuzzy);
        if from_start.index.is_some() || from_start.match_count.unwrap_or(0) > 1 {
            return from_start;
        }
    }

    primary
}

fn attempt_sequence_fallback(
    lines: &[String],
    hunk: &DiffHunk,
    current_index: usize,
    line_hint: Option<usize>,
    allow_fuzzy: bool,
    allow_aggressive_fallbacks: bool,
) -> Option<usize> {
    if hunk.old_lines.is_empty() {
        return None;
    }
    let match_hint = get_hunk_hint_index(hunk, current_index);
    let fallback = find_sequence_with_hint(
        lines,
        &hunk.old_lines,
        current_index,
        match_hint.or(line_hint.map(|h| h.saturating_sub(1))),
        false,
        allow_fuzzy,
    );
    if let Some(fallback_index) = fallback.index.filter(|_| fallback.match_count.unwrap_or(1) <= 1) {
        let next_index = fallback_index + 1;
        if next_index <= lines.len().saturating_sub(hunk.old_lines.len()) {
            let second = seek_sequence(lines, &hunk.old_lines, next_index, false, allow_fuzzy);
            if second.index.is_some() {
                return None;
            }
        }
        return Some(fallback_index);
    }

    for variant in filter_fallback_variants(build_fallback_variants(hunk), allow_aggressive_fallbacks)
    {
        if variant.old_lines.is_empty() {
            continue;
        }
        let variant_result = find_sequence_with_hint(
            lines,
            &variant.old_lines,
            current_index,
            match_hint.or(line_hint.map(|h| h.saturating_sub(1))),
            false,
            allow_fuzzy,
        );
        if variant_result.index.is_some() && variant_result.match_count.unwrap_or(1) <= 1 {
            return variant_result.index;
        }
    }
    None
}

fn format_character_occurrence_previews(content: &str, target: &str) -> Vec<String> {
    let mut previews = Vec::new();
    let mut start = 0;
    while let Some(idx) = content[start..].find(target) {
        let abs = start + idx;
        let line_num = content[..abs].matches('\n').count() + 1;
        let line_start = content[..abs].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = content[abs..]
            .find('\n')
            .map(|i| abs + i)
            .unwrap_or(content.len());
        let line = &content[line_start..line_end];
        let truncated = if line.len() > MATCH_PREVIEW_MAX_LEN {
            format!("{}…", &line[..MATCH_PREVIEW_MAX_LEN - 1])
        } else {
            line.to_string()
        };
        previews.push(format!(" {line_num} | {truncated}"));
        if previews.len() >= MAX_OCCURRENCE_PREVIEWS {
            break;
        }
        start = abs + target.len();
    }
    previews
}

fn apply_character_match(
    original_content: &str,
    path: &str,
    hunk: &DiffHunk,
    fuzzy_threshold: f64,
    allow_fuzzy: bool,
) -> Result<(String, Vec<String>), ApplyPatchError> {
    let old_text = hunk.old_lines.join("\n");
    let new_text = hunk.new_lines.join("\n");

    let normalized_content = normalize_to_lf(original_content);
    let normalized_old_text = normalize_to_lf(&old_text);

    let find_opts = FindMatchOptions {
        allow_fuzzy,
        threshold: Some(fuzzy_threshold),
    };
    let mut match_outcome = find_match(&normalized_content, &normalized_old_text, &find_opts);

    if match_outcome.matched.is_none() && allow_fuzzy {
        let relaxed = fuzzy_threshold.min(CHARACTER_RELAXED_THRESHOLD);
        if relaxed < fuzzy_threshold {
            let relaxed_outcome =
                find_match(&normalized_content, &normalized_old_text, &FindMatchOptions {
                    allow_fuzzy: true,
                    threshold: Some(relaxed),
                });
            if relaxed_outcome.matched.is_some() {
                match_outcome = relaxed_outcome;
            }
        }
    }

    if match_outcome.occurrences.unwrap_or(0) > 1 {
        let previews = format_character_occurrence_previews(&normalized_content, &normalized_old_text);
        let more_msg = if match_outcome.occurrences.unwrap() > MAX_OCCURRENCE_PREVIEWS {
            format!(
                " (showing first {MAX_OCCURRENCE_PREVIEWS} of {})",
                match_outcome.occurrences.unwrap()
            )
        } else {
            String::new()
        };
        return Err(ApplyPatchError(format!(
            "Found {} occurrences in {path}{more_msg}:\n\n{}\n\nAdd more context lines to disambiguate.",
            match_outcome.occurrences.unwrap(),
            previews.join("\n\n")
        )));
    }

    if match_outcome.fuzzy_matches.unwrap_or(0) > 1 {
        return Err(ApplyPatchError(format!(
            "Found {} high-confidence matches in {path}. The text must be unique. Please provide more context to make it unique.",
            match_outcome.fuzzy_matches.unwrap()
        )));
    }

    let Some(matched) = match_outcome.matched else {
        if let Some(closest) = match_outcome.closest {
            let similarity = (closest.confidence * 100.0).round() as i64;
            return Err(ApplyPatchError(format!(
                "Could not find a close enough match in {path}. Closest match ({similarity}% similar) at line {}.",
                closest.start_line
            )));
        }
        return Err(ApplyPatchError(format!(
            "Failed to find expected lines in {path}:\n{old_text}"
        )));
    };

    let adjusted_new_text =
        adjust_indentation(&normalized_old_text, &matched.actual_text, &new_text);

    let mut warnings = Vec::new();
    if allow_fuzzy
        && matched.confidence >= DOMINANT_FUZZY_MIN_CONFIDENCE
        && match_outcome.fuzzy_matches == Some(1)
    {
        let similarity = (matched.confidence * 100.0).round() as i64;
        warnings.push(format!(
            "Dominant fuzzy match selected in {path} near line {} ({similarity}% similar).",
            matched.start_line
        ));
    }

    let end = matched.start_index + matched.actual_text.len();
    let content = format!(
        "{}{}{}",
        &normalized_content[..matched.start_index],
        adjusted_new_text,
        &normalized_content[end..]
    );
    Ok((content, warnings))
}

fn apply_trailing_newline_policy(content: &str, had_final_newline: bool) -> String {
    if had_final_newline {
        if content.ends_with('\n') {
            content.to_string()
        } else {
            format!("{content}\n")
        }
    } else {
        content.trim_end_matches('\n').to_string()
    }
}

fn read_existing_patch_file(
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
    relative_path: &str,
) -> Result<String, ApplyPatchError> {
    match fs.read(absolute_path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            Err(ApplyPatchError(format!("File not found: {relative_path}")))
        }
        Err(err) => Err(ApplyPatchError(err.to_string())),
    }
}

fn compute_replacements(
    original_lines: &[String],
    path: &str,
    hunks: &[DiffHunk],
    allow_fuzzy: bool,
) -> Result<(Vec<Replacement>, Vec<String>), ApplyPatchError> {
    let mut replacements = Vec::new();
    let mut warnings = Vec::new();
    let mut line_index = 0;

    for hunk in hunks {
        if let Some(old_start) = hunk.old_start_line.filter(|&n| n < 1) {
            return Err(ApplyPatchError(format!(
                "Line hint {old_start} is out of range for {path} (line numbers start at 1)"
            )));
        }
        if let Some(new_start) = hunk.new_start_line.filter(|&n| n < 1) {
            return Err(ApplyPatchError(format!(
                "Line hint {new_start} is out of range for {path} (line numbers start at 1)"
            )));
        }

        let line_hint = hunk.old_start_line;
        let allow_aggressive_fallbacks = hunk.change_context.is_some()
            || line_hint.is_some()
            || hunk.is_end_of_file;
        let fallback_variants =
            filter_fallback_variants(build_fallback_variants(hunk), allow_aggressive_fallbacks);

        if hunk.change_context.is_none() && !hunk.has_context_lines {
            if let Some(hint) = line_hint {
                line_index = hint
                    .saturating_sub(1)
                    .min(original_lines.len().saturating_sub(1));
            }
        }

        let mut context_index = None;

        if let Some(ref change_context) = hunk.change_context {
            let result = find_hierarchical_context(
                original_lines,
                change_context,
                line_index,
                line_hint,
                allow_fuzzy,
            );
            context_index = result.index;

            if result.index.is_none() || result.match_count.unwrap_or(0) > 1 {
                if let Some(fallback) = attempt_sequence_fallback(
                    original_lines,
                    hunk,
                    line_index,
                    line_hint,
                    allow_fuzzy,
                    allow_aggressive_fallbacks,
                ) {
                    line_index = fallback;
                } else if result.match_count.unwrap_or(0) > 1 {
                    let display_context = if change_context.contains('\n') {
                        change_context.split('\n').next_back().unwrap_or(change_context)
                    } else {
                        change_context.as_str()
                    };
                    let previews = format_sequence_match_previews(
                        original_lines,
                        result.match_indices.as_deref(),
                        result.match_count,
                    );
                    let strategy_hint = format_context_strategy(result.strategy)
                        .map(|s| format!(" Matching strategy: {s}."))
                        .unwrap_or_default();
                    let preview_text = previews.map(|p| format!("\n\n{p}")).unwrap_or_default();
                    return Err(ApplyPatchError(format!(
                        "Found {} matches for context '{display_context}' in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
                        result.match_count.unwrap()
                    )));
                } else {
                    let display_context = if change_context.contains('\n') {
                        change_context.split('\n').collect::<Vec<_>>().join(" > ")
                    } else {
                        change_context.clone()
                    };
                    return Err(ApplyPatchError(format!(
                        "Failed to find context '{display_context}' in {path}"
                    )));
                }
            } else if let Some(idx) = result.index {
                let first_old_line = hunk.old_lines.first();
                let final_context = if change_context.contains('\n') {
                    change_context.split('\n').next_back().map(str::trim)
                } else {
                    Some(change_context.trim())
                };
                let is_hierarchical = change_context.contains('\n')
                    || change_context.split_whitespace().count() > 2;
                if first_old_line
                    .is_some_and(|l| Some(l.trim()) == final_context)
                    || is_hierarchical
                {
                    line_index = idx;
                } else {
                    line_index = idx + 1;
                }
            }
        }

        if hunk.old_lines.is_empty() {
            let insertion_idx = if hunk.change_context.is_some() {
                line_index
            } else {
                let line_hint_for_insertion = hunk.old_start_line.or(hunk.new_start_line);
                if let Some(hint) = line_hint_for_insertion {
                    if hint < 1 {
                        return Err(ApplyPatchError(format!(
                            "Line hint {hint} is out of range for insertion in {path} (line numbers start at 1)"
                        )));
                    }
                    if hint > original_lines.len() + 1 {
                        return Err(ApplyPatchError(format!(
                            "Line hint {hint} is out of range for insertion in {path} (file has {} lines)",
                            original_lines.len()
                        )));
                    }
                    hint.saturating_sub(1)
                } else if original_lines.last().is_some_and(|l| l.is_empty()) {
                    original_lines.len().saturating_sub(1)
                } else {
                    original_lines.len()
                }
            };
            replacements.push(Replacement {
                start_index: insertion_idx,
                old_len: 0,
                new_lines: hunk.new_lines.clone(),
            });
            continue;
        }

        let mut pattern = hunk.old_lines.clone();
        let match_hint = get_hunk_hint_index(hunk, line_index);
        let mut search_result = find_sequence_with_hint(
            original_lines,
            &pattern,
            line_index,
            match_hint,
            hunk.is_end_of_file,
            allow_fuzzy,
        );
        let mut new_slice = hunk.new_lines.clone();

        if search_result.index.is_none()
            && pattern.last().is_some_and(|l| l.is_empty())
        {
            pattern.pop();
            if new_slice.last().is_some_and(|l| l.is_empty()) {
                new_slice.pop();
            }
            search_result = find_sequence_with_hint(
                original_lines,
                &pattern,
                line_index,
                match_hint,
                hunk.is_end_of_file,
                allow_fuzzy,
            );
        }

        if search_result.index.is_none() || search_result.match_count.unwrap_or(0) > 1 {
            for variant in &fallback_variants {
                if variant.old_lines.is_empty() {
                    continue;
                }
                let variant_result = find_sequence_with_hint(
                    original_lines,
                    &variant.old_lines,
                    line_index,
                    match_hint,
                    hunk.is_end_of_file,
                    allow_fuzzy,
                );
                if variant_result.index.is_some() && variant_result.match_count.unwrap_or(1) <= 1 {
                    pattern = variant.old_lines.clone();
                    new_slice = variant.new_lines.clone();
                    search_result = variant_result;
                    break;
                }
            }
        }

        if search_result.index.is_none() {
            if let Some(ctx_idx) = context_index {
                for variant in &fallback_variants {
                    if variant.old_lines.len() != 1 || variant.new_lines.len() != 1 {
                        continue;
                    }
                    let removed_line = &variant.old_lines[0];
                    let has_shared_duplicate = hunk
                        .new_lines
                        .iter()
                        .any(|line| line.trim() == removed_line.trim());
                    if let Some(adjacent) = find_context_relative_match(
                        original_lines,
                        removed_line,
                        ctx_idx,
                        has_shared_duplicate,
                    ) {
                        pattern = variant.old_lines.clone();
                        new_slice = variant.new_lines.clone();
                        search_result = SequenceSearchResult {
                            index: Some(adjacent),
                            confidence: 0.95,
                            ..Default::default()
                        };
                        break;
                    }
                }
            }
        }

        if search_result.index.is_some() && context_index.is_some() && pattern.len() == 1 {
            let trimmed = pattern[0].trim();
            let occurrence_count = original_lines
                .iter()
                .filter(|line| line.trim() == trimmed)
                .count();
            if occurrence_count > 1 {
                let has_shared_duplicate = hunk
                    .new_lines
                    .iter()
                    .any(|line| line.trim() == trimmed);
                if let Some(ctx_idx) = context_index {
                    if let Some(context_match) = find_context_relative_match(
                        original_lines,
                        &pattern[0],
                        ctx_idx,
                        has_shared_duplicate,
                    ) {
                        search_result = SequenceSearchResult {
                            index: Some(context_match),
                            confidence: search_result.confidence,
                            ..search_result
                        };
                    }
                }
            }
        }

        if search_result.match_count.unwrap_or(0) > 1 {
            let hint_index = match_hint.or(line_hint.map(|h| h.saturating_sub(1)));
            if let Some(hinted) = choose_hinted_match(
                search_result.match_indices.as_deref(),
                hint_index,
                AMBIGUITY_HINT_WINDOW,
            ) {
                search_result = SequenceSearchResult {
                    index: Some(hinted),
                    match_count: Some(1),
                    ..search_result
                };
            }
        }

        if search_result.index.is_none() {
            if search_result.match_count.unwrap_or(0) > 1 {
                let previews = format_sequence_match_previews(
                    original_lines,
                    search_result.match_indices.as_deref(),
                    search_result.match_count,
                );
                let strategy_hint = format_sequence_strategy(search_result.strategy)
                    .map(|s| format!(" Matching strategy: {s}."))
                    .unwrap_or_default();
                let preview_text = previews.map(|p| format!("\n\n{p}")).unwrap_or_default();
                return Err(ApplyPatchError(format!(
                    "Found {} matches for the text in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
                    search_result.match_count.unwrap()
                )));
            }
            let closest = find_closest_sequence_match(
                original_lines,
                &pattern,
                line_index,
                hunk.is_end_of_file,
            );
            if let Some(closest_index) = closest.index.filter(|_| closest.confidence > 0.0) {
                let similarity = (closest.confidence * 100.0).round() as i64;
                let preview = format_sequence_match_preview(original_lines, closest_index);
                return Err(ApplyPatchError(format!(
                    "Failed to find expected lines in {path}:\n{}\n\nClosest match ({similarity}% similar) near line {}:\n{preview}",
                    hunk.old_lines.join("\n"),
                    closest_index + 1
                )));
            }
            return Err(ApplyPatchError(format!(
                "Failed to find expected lines in {path}:\n{}",
                hunk.old_lines.join("\n")
            )));
        }

        let found = search_result.index.unwrap();

        if search_result.strategy == Some(SequenceMatchStrategy::FuzzyDominant) {
            let similarity = (search_result.confidence * 100.0).round() as i64;
            warnings.push(format!(
                "Dominant fuzzy match selected in {path} near line {} ({similarity}% similar).",
                found + 1
            ));
        }

        if search_result.match_count.unwrap_or(0) > 1 {
            let previews = format_sequence_match_previews(
                original_lines,
                search_result.match_indices.as_deref(),
                search_result.match_count,
            );
            let strategy_hint = format_sequence_strategy(search_result.strategy)
                .map(|s| format!(" Matching strategy: {s}."))
                .unwrap_or_default();
            let preview_text = previews.map(|p| format!("\n\n{p}")).unwrap_or_default();
            return Err(ApplyPatchError(format!(
                "Found {} matches for the text in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
                search_result.match_count.unwrap()
            )));
        }

        if hunk.change_context.is_none()
            && !hunk.has_context_lines
            && !hunk.is_end_of_file
            && line_hint.is_none()
        {
            let second_match =
                seek_sequence(original_lines, &pattern, found + 1, false, allow_fuzzy);
            if let Some(second_index) = second_match.index {
                let preview1 = format_sequence_match_preview(original_lines, found);
                let preview2 = format_sequence_match_preview(original_lines, second_index);
                return Err(ApplyPatchError(format!(
                    "Found 2 occurrences in {path}:\n\n{preview1}\n\n{preview2}\n\nAdd more context lines to disambiguate."
                )));
            }
        }

        let actual_matched_lines = original_lines[found..found + pattern.len()].to_vec();

        let mut is_no_op = pattern.len() == new_slice.len();
        if is_no_op {
            for (old, new) in pattern.iter().zip(new_slice.iter()) {
                if old != new {
                    is_no_op = false;
                    break;
                }
            }
        }

        if is_no_op {
            line_index = found + pattern.len();
            continue;
        }

        let adjusted_new_lines =
            adjust_lines_indentation(&pattern, &actual_matched_lines, &new_slice);
        replacements.push(Replacement {
            start_index: found,
            old_len: pattern.len(),
            new_lines: adjusted_new_lines,
        });
        line_index = found + pattern.len();
    }

    replacements.sort_by_key(|r| r.start_index);

    for i in 1..replacements.len() {
        let prev = &replacements[i - 1];
        let next = &replacements[i];
        let prev_end = prev.start_index + prev.old_len;
        if next.start_index < prev_end {
            let format_range = |replacement: &Replacement| -> String {
                if replacement.old_len == 0 {
                    format!("{} (insertion)", replacement.start_index + 1)
                } else {
                    format!(
                        "{}-{}",
                        replacement.start_index + 1,
                        replacement.start_index + replacement.old_len
                    )
                }
            };
            return Err(ApplyPatchError(format!(
                "Overlapping hunks detected in {path} at lines {} and {}. Split hunks or add more context to avoid overlap.",
                format_range(prev),
                format_range(next)
            )));
        }
    }

    Ok((replacements, warnings))
}

fn apply_replacements(lines: &[String], replacements: &[Replacement]) -> Vec<String> {
    let mut result = lines.to_vec();
    for replacement in replacements.iter().rev() {
        let Replacement {
            start_index,
            old_len,
            new_lines,
        } = replacement;
        result.drain(*start_index..start_index + old_len);
        for (offset, line) in new_lines.iter().enumerate() {
            result.insert(start_index + offset, line.clone());
        }
    }
    result
}

fn apply_hunks_to_content(
    original_content: &str,
    path: &str,
    hunks: &[DiffHunk],
    fuzzy_threshold: f64,
    allow_fuzzy: bool,
) -> Result<(String, Vec<String>), ApplyPatchError> {
    let had_final_newline = original_content.ends_with('\n');

    if hunks.len() == 1 {
        let hunk = &hunks[0];
        if hunk.change_context.is_none()
            && !hunk.has_context_lines
            && !hunk.old_lines.is_empty()
            && hunk.old_start_line.is_none()
            && !hunk.is_end_of_file
        {
            let (content, warnings) =
                apply_character_match(original_content, path, hunk, fuzzy_threshold, allow_fuzzy)?;
            return Ok((apply_trailing_newline_policy(&content, had_final_newline), warnings));
        }
    }

    let mut original_lines: Vec<String> = original_content.split('\n').map(str::to_string).collect();
    let mut stripped_trailing_empty = false;
    if had_final_newline
        && original_lines.last().is_some_and(|l| l.is_empty())
    {
        original_lines.pop();
        stripped_trailing_empty = true;
    }

    let (replacements, warnings) =
        compute_replacements(&original_lines, path, hunks, allow_fuzzy)?;
    let mut new_lines = apply_replacements(&original_lines, &replacements);

    if stripped_trailing_empty {
        new_lines.push(String::new());
    }

    let content = new_lines.join("\n");
    Ok((apply_trailing_newline_policy(&content, had_final_newline), warnings))
}

fn bytes_unchanged(pre: &[u8], post: &[u8]) -> bool {
    pre.len() == post.len() && pre.iter().zip(post.iter()).all(|(a, b)| a == b)
}

fn verify_written_file(
    fs: &dyn PatchFileSystem,
    written_path: &Path,
    relative_path: &str,
    pre_edit_bytes: Option<&[u8]>,
    expected_content: &str,
    content_changed: bool,
) -> Result<(), PatchVerifyError> {
    let post_edit_bytes = fs.read_binary(written_path).map_err(|e| PatchVerifyError {
        message: format!(
            "edit completed but could not verify write to {relative_path}: {e}"
        ),
        relative_path: relative_path.to_string(),
        resolved_path: written_path.to_path_buf(),
    })?;

    if content_changed {
        if let Some(pre) = pre_edit_bytes {
            if bytes_unchanged(pre, &post_edit_bytes) {
                return Err(PatchVerifyError {
                    message: format!(
                        "edit appeared successful but file content did not change on disk: {relative_path}"
                    ),
                    relative_path: relative_path.to_string(),
                    resolved_path: written_path.to_path_buf(),
                });
            }
        }
    }

    if post_edit_bytes.as_slice() != expected_content.as_bytes() {
        return Err(PatchVerifyError {
            message: format!(
                "edit completed but file on disk does not match expected content: {relative_path}"
            ),
            relative_path: relative_path.to_string(),
            resolved_path: written_path.to_path_buf(),
        });
    }

    Ok(())
}

fn verify_deleted_file(
    fs: &dyn PatchFileSystem,
    deleted_path: &Path,
    relative_path: &str,
) -> Result<(), PatchVerifyError> {
    if fs.exists(deleted_path).unwrap_or(false) {
        return Err(PatchVerifyError {
            message: format!("delete completed but file still exists: {relative_path}"),
            relative_path: relative_path.to_string(),
            resolved_path: deleted_path.to_path_buf(),
        });
    }
    Ok(())
}

pub fn apply_patch_entry(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
) -> Result<PatchApplyResult, PatchError> {
    let absolute_path =
        resolve_writable(&options.cwd, &input.path).map_err(|e| PatchError::Apply(ApplyPatchError(e.0)))?;
    let dest_path = if let Some(rename) = &input.rename {
        resolve_writable(&options.cwd, rename).map_err(|e| PatchError::Apply(ApplyPatchError(e.0)))?
    } else {
        absolute_path.clone()
    };

    if input.rename.is_some() && dest_path == absolute_path {
        return Err(PatchError::Apply(ApplyPatchError(
            "rename path is the same as source path".to_string(),
        )));
    }

    match input.op {
        PatchOp::Create => apply_create(input, options, fs, &absolute_path),
        PatchOp::Delete => apply_delete(input, options, fs, &absolute_path),
        PatchOp::Update => apply_update(input, options, fs, &absolute_path, &dest_path),
    }
}

fn apply_create(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
) -> Result<PatchApplyResult, PatchError> {
    let diff = input.diff.as_ref().ok_or_else(|| {
        PatchError::Apply(ApplyPatchError(
            "Create operation requires diff (file content)".to_string(),
        ))
    })?;
    let normalized_content = normalize_create_content(diff);
    let content = if normalized_content.ends_with('\n') {
        normalized_content
    } else {
        format!("{normalized_content}\n")
    };

    if !options.dry_run {
        if fs
            .exists(absolute_path)
            .map_err(|error| PatchError::Apply(ApplyPatchError(error.to_string())))?
        {
            return Err(PatchError::Apply(ApplyPatchError(format!(
                "File already exists: {}",
                input.path
            ))));
        }
        if let Some(parent) = absolute_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs.mkdir_all(parent).map_err(|e| {
                    PatchError::Apply(ApplyPatchError(e.to_string()))
                })?;
            }
        }
        fs.write(absolute_path, &content)
            .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
        verify_written_file(
            fs,
            absolute_path,
            &input.path,
            None,
            &content,
            true,
        )
        .map_err(PatchError::Verify)?;
    }

    Ok(PatchApplyResult {
        old_content: None,
        new_content: Some(content),
        dest_path: absolute_path.to_path_buf(),
        warnings: Vec::new(),
    })
}

fn apply_delete(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
) -> Result<PatchApplyResult, PatchError> {
    let old_content = read_existing_patch_file(fs, absolute_path, &input.path)
        .map_err(PatchError::Apply)?;

    if !options.dry_run {
        fs.delete(absolute_path)
            .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
        verify_deleted_file(fs, absolute_path, &input.path).map_err(PatchError::Verify)?;
    }

    Ok(PatchApplyResult {
        old_content: Some(old_content),
        new_content: None,
        dest_path: absolute_path.to_path_buf(),
        warnings: Vec::new(),
    })
}

fn apply_update(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
    dest_path: &Path,
) -> Result<PatchApplyResult, PatchError> {
    let diff = input.diff.as_ref().ok_or_else(|| {
        PatchError::Apply(ApplyPatchError(
            "Update operation requires diff (hunks)".to_string(),
        ))
    })?;

    let pre_edit_bytes = if !options.dry_run {
        fs.read_binary(absolute_path).ok()
    } else {
        None
    };

    let original_content = read_existing_patch_file(fs, absolute_path, &input.path)
        .map_err(PatchError::Apply)?;

    let BomResult {
        mut bom,
        text: stripped_content,
    } = strip_bom(&original_content);
    if bom.is_empty() {
        if let Ok(bytes) = fs.read_binary(absolute_path) {
            if bytes.len() >= 3 && bytes[0] == 0xef && bytes[1] == 0xbb && bytes[2] == 0xbf {
                bom = "\u{feff}".to_string();
            }
        }
    }

    let line_ending = detect_line_ending(&stripped_content);
    let normalized_content = normalize_to_lf(&stripped_content);
    let hunks = parse_diff_hunks(diff).map_err(PatchError::Apply)?;

    if hunks.is_empty() {
        return Err(PatchError::Apply(ApplyPatchError(
            "Diff contains no hunks".to_string(),
        )));
    }

    let (new_content, warnings) = apply_hunks_to_content(
        &normalized_content,
        &input.path,
        &hunks,
        options.fuzzy_threshold,
        options.allow_fuzzy,
    )
    .map_err(PatchError::Apply)?;

    let final_content = format!("{bom}{}", restore_line_endings(&new_content, line_ending));
    let is_move = input.rename.is_some() && dest_path != absolute_path;
    let content_changed = original_content != final_content;

    if !options.dry_run {
        if is_move {
            let dest_pre_edit_bytes = fs.read_binary(dest_path).ok();
            let dest_relative = input.rename.as_deref().unwrap_or(&input.path);

            if let Some(parent) = dest_path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs.mkdir_all(parent).map_err(|e| {
                        PatchError::Apply(ApplyPatchError(e.to_string()))
                    })?;
                }
            }
            fs.write(dest_path, &final_content)
                .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
            verify_written_file(
                fs,
                dest_path,
                dest_relative,
                dest_pre_edit_bytes.as_deref(),
                &final_content,
                content_changed,
            )
            .map_err(PatchError::Verify)?;

            if let Err(error) = fs.delete(absolute_path) {
                let _ = fs.delete(dest_path);
                return Err(PatchError::Apply(ApplyPatchError(format!(
                    "rename failed after writing destination; rolled back destination write: {error}"
                ))));
            }
            verify_deleted_file(fs, absolute_path, &input.path).map_err(PatchError::Verify)?;
        } else {
            fs.write(absolute_path, &final_content)
                .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;

            verify_written_file(
                fs,
                absolute_path,
                &input.path,
                pre_edit_bytes.as_deref(),
                &final_content,
                content_changed,
            )
            .map_err(PatchError::Verify)?;
        }
    }

    Ok(PatchApplyResult {
        old_content: Some(original_content),
        new_content: Some(final_content),
        dest_path: if is_move {
            dest_path.to_path_buf()
        } else {
            absolute_path.to_path_buf()
        },
        warnings,
    })
}
