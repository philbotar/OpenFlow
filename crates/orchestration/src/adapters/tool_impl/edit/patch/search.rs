//! Sequence and character match search for patch application.

use std::io;
use std::path::Path;

use super::super::diff::DiffHunk;
use super::super::errors::ApplyPatchError;
use super::super::normalize::{adjust_indentation, normalize_to_lf};
use super::super::replace::{find_match, FindMatchOptions, DOMINANT_FUZZY_MIN_CONFIDENCE};
use super::super::replace_sequence::{
    find_context_line, seek_sequence, ContextMatchStrategy, SequenceMatchStrategy,
    SequenceSearchResult,
};
use super::hunk::{build_fallback_variants, filter_fallback_variants};
use super::PatchFileSystem;

pub(super) const AMBIGUITY_HINT_WINDOW: usize = 200;
pub(super) const MATCH_PREVIEW_CONTEXT: usize = 2;
pub(super) const MATCH_PREVIEW_MAX_LEN: usize = 80;
pub(super) const CHARACTER_RELAXED_THRESHOLD: f64 = 0.92;
pub(super) const MAX_OCCURRENCE_PREVIEWS: usize = 5;

pub(super) fn find_context_relative_match(
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

pub(super) fn format_sequence_match_preview(lines: &[String], start_idx: usize) -> String {
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

pub(super) fn format_sequence_match_previews(
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
    let more_msg = match_count
        .filter(|&c| c > indices.len())
        .map(|c| format!(" (showing first {} of {c})", indices.len()))
        .unwrap_or_default();
    Some(format!("{}{}", previews.join("\n\n"), more_msg))
}

pub(super) fn choose_hinted_match(
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

pub(super) fn get_hunk_hint_index(hunk: &DiffHunk, current_index: usize) -> Option<usize> {
    let hint_index = hunk.old_start_line? - 1;
    if hint_index >= current_index {
        Some(hint_index)
    } else {
        None
    }
}

pub(super) fn format_sequence_strategy(strategy: Option<SequenceMatchStrategy>) -> Option<String> {
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

pub(super) fn format_context_strategy(strategy: Option<ContextMatchStrategy>) -> Option<String> {
    strategy.map(|s| match s {
        ContextMatchStrategy::Exact => "exact".to_string(),
        ContextMatchStrategy::Trim => "trim".to_string(),
        ContextMatchStrategy::Unicode => "unicode".to_string(),
        ContextMatchStrategy::Prefix => "prefix".to_string(),
        ContextMatchStrategy::Substring => "substring".to_string(),
        ContextMatchStrategy::Fuzzy => "fuzzy".to_string(),
    })
}

pub(super) fn find_hierarchical_context(
    lines: &[String],
    context: &str,
    start_from: usize,
    line_hint: Option<usize>,
    allow_fuzzy: bool,
) -> super::super::replace_sequence::ContextLineResult {
    use super::super::replace_sequence::ContextLineResult;

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

pub(super) fn find_sequence_with_hint(
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

pub(super) fn attempt_sequence_fallback(
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
    if let Some(fallback_index) = fallback
        .index
        .filter(|_| fallback.match_count.unwrap_or(1) <= 1)
    {
        let next_index = fallback_index + 1;
        if next_index <= lines.len().saturating_sub(hunk.old_lines.len()) {
            let second = seek_sequence(lines, &hunk.old_lines, next_index, false, allow_fuzzy);
            if second.index.is_some() {
                return None;
            }
        }
        return Some(fallback_index);
    }

    for variant in
        filter_fallback_variants(build_fallback_variants(hunk), allow_aggressive_fallbacks)
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

pub(super) fn format_character_occurrence_previews(content: &str, target: &str) -> Vec<String> {
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

pub(super) fn apply_character_match(
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
            let relaxed_outcome = find_match(
                &normalized_content,
                &normalized_old_text,
                &FindMatchOptions {
                    allow_fuzzy: true,
                    threshold: Some(relaxed),
                },
            );
            if relaxed_outcome.matched.is_some() {
                match_outcome = relaxed_outcome;
            }
        }
    }

    if let Some(occurrences) = match_outcome.occurrences {
        if occurrences > 1 {
            let previews =
                format_character_occurrence_previews(&normalized_content, &normalized_old_text);
            let more_msg = if occurrences > MAX_OCCURRENCE_PREVIEWS {
                format!(" (showing first {MAX_OCCURRENCE_PREVIEWS} of {occurrences})")
            } else {
                String::new()
            };
            return Err(ApplyPatchError(format!(
                "Found {occurrences} occurrences in {path}{more_msg}:\n\n{}\n\nAdd more context lines to disambiguate.",
                previews.join("\n\n")
            )));
        }
    }

    if let Some(fuzzy_matches) = match_outcome.fuzzy_matches {
        if fuzzy_matches > 1 {
            return Err(ApplyPatchError(format!(
                "Found {fuzzy_matches} high-confidence matches in {path}. The text must be unique. Please provide more context to make it unique."
            )));
        }
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

pub(super) fn apply_trailing_newline_policy(content: &str, had_final_newline: bool) -> String {
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

pub(super) fn read_existing_patch_file(
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
