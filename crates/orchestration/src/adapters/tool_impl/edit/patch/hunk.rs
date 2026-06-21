//! Hunk normalization and fallback variants.

use std::collections::{HashMap, HashSet};

use super::super::diff::DiffHunk;
use super::super::normalize::{
    convert_leading_tabs_to_spaces, count_leading_whitespace, get_leading_whitespace,
    leading_whitespace_byte_len,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HunkVariantKind {
    TrimCommon,
    DedupeShared,
    CollapseRepeated,
    SingleLine,
}

pub(super) struct HunkVariant {
    pub(super) old_lines: Vec<String>,
    pub(super) new_lines: Vec<String>,
    pub(super) kind: HunkVariantKind,
}

pub(super) fn is_blank_line(line: &str) -> bool {
    line.trim().is_empty()
}

pub(super) fn are_equal_lines(left: &[String], right: &[String]) -> bool {
    left == right
}

pub(super) fn are_equal_trimmed_lines(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(l, r)| l.trim() == r.trim())
}

pub(super) fn get_indent_char(lines: &[String]) -> char {
    for line in lines {
        let ws = get_leading_whitespace(line);
        if !ws.is_empty() {
            return ws.chars().next().unwrap_or(' ');
        }
    }
    ' '
}

pub(super) fn collect_indent_deltas(old_lines: &[String], actual_lines: &[String]) -> Vec<isize> {
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

pub(super) fn apply_indent_delta_line(line: &str, delta: isize, indent_char: char) -> String {
    if is_blank_line(line) {
        return line.to_string();
    }
    if delta > 0 {
        return format!("{}{line}", indent_char.to_string().repeat(delta as usize));
    }
    let to_remove = (-delta as usize).min(leading_whitespace_byte_len(line));
    line[to_remove..].to_string()
}

pub(super) fn can_convert_tabs_to_spaces(
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

pub(super) fn adjust_lines_indentation(
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

pub(super) fn resolve_tab_width(samples: &HashMap<usize, usize>) -> Option<isize> {
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

pub(super) fn convert_spaces_to_tabs_line(line: &str, tab_width: isize, offset: isize) -> String {
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

pub(super) fn trim_common_context(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
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

pub(super) fn collapse_consecutive_shared_lines(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
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

pub(super) fn collapse_repeated_blocks(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
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
        if changed {
            output
        } else {
            lines.to_vec()
        }
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

pub(super) fn reduce_to_single_line_change(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
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

pub(super) fn build_fallback_variants(hunk: &DiffHunk) -> Vec<HunkVariant> {
    let base = HunkVariant {
        old_lines: hunk.old_lines.clone(),
        new_lines: hunk.new_lines.clone(),
        kind: HunkVariantKind::TrimCommon,
    };

    let trimmed = trim_common_context(&base.old_lines, &base.new_lines);
    let deduped = collapse_consecutive_shared_lines(
        trimmed
            .as_ref()
            .map(|v| v.old_lines.as_slice())
            .unwrap_or(&base.old_lines),
        trimmed
            .as_ref()
            .map(|v| v.new_lines.as_slice())
            .unwrap_or(&base.new_lines),
    );
    let collapsed = collapse_repeated_blocks(
        deduped.as_ref().map(|v| v.old_lines.as_slice()).unwrap_or(
            trimmed
                .as_ref()
                .map(|v| v.old_lines.as_slice())
                .unwrap_or(&base.old_lines),
        ),
        deduped.as_ref().map(|v| v.new_lines.as_slice()).unwrap_or(
            trimmed
                .as_ref()
                .map(|v| v.new_lines.as_slice())
                .unwrap_or(&base.new_lines),
        ),
    );
    let single_line = reduce_to_single_line_change(
        trimmed
            .as_ref()
            .map(|v| v.old_lines.as_slice())
            .unwrap_or(&base.old_lines),
        trimmed
            .as_ref()
            .map(|v| v.new_lines.as_slice())
            .unwrap_or(&base.new_lines),
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

pub(super) fn filter_fallback_variants(
    variants: Vec<HunkVariant>,
    allow_aggressive: bool,
) -> Vec<HunkVariant> {
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
