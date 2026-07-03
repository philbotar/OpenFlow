//! Hunk-to-line replacement planning (`compute_replacements` and apply helpers).

use super::super::diff::DiffHunk;
use super::super::errors::ApplyPatchError;
use super::super::normalize::adjust_lines_indentation;
use super::super::replace_sequence::{
    find_closest_sequence_match, seek_sequence, SequenceMatchStrategy, SequenceSearchResult,
};
use super::hunk::{build_fallback_variants, filter_fallback_variants};
use super::search::{
    apply_character_match, apply_trailing_newline_policy, attempt_sequence_fallback,
    choose_hinted_match, find_context_relative_match, find_hierarchical_context,
    find_sequence_with_hint, format_context_strategy, format_sequence_match_preview,
    format_sequence_match_previews, format_sequence_strategy, get_hunk_hint_index,
    AMBIGUITY_HINT_WINDOW,
};

pub(super) struct Replacement {
    start_index: usize,
    old_len: usize,
    new_lines: Vec<String>,
}

pub(super) fn compute_replacements(
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
        let allow_aggressive_fallbacks =
            hunk.change_context.is_some() || line_hint.is_some() || hunk.is_end_of_file;
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
                } else if let Some(count) = result.match_count.filter(|&c| c > 1) {
                    let display_context = if change_context.contains('\n') {
                        change_context
                            .split('\n')
                            .next_back()
                            .unwrap_or(change_context)
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
                        "Found {count} matches for context '{display_context}' in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
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
                let is_hierarchical =
                    change_context.contains('\n') || change_context.split_whitespace().count() > 2;
                if first_old_line.is_some_and(|l| Some(l.trim()) == final_context)
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

        if search_result.index.is_none() && pattern.last().is_some_and(|l| l.is_empty()) {
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
                let has_shared_duplicate = hunk.new_lines.iter().any(|line| line.trim() == trimmed);
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
            if let Some(count) = search_result.match_count.filter(|&c| c > 1) {
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
                    "Found {count} matches for the text in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
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

        let Some(found) = search_result.index else {
            return Err(ApplyPatchError(format!(
                "Failed to find expected lines in {path}:\n{}",
                hunk.old_lines.join("\n")
            )));
        };

        if search_result.strategy == Some(SequenceMatchStrategy::FuzzyDominant) {
            let similarity = (search_result.confidence * 100.0).round() as i64;
            warnings.push(format!(
                "Dominant fuzzy match selected in {path} near line {} ({similarity}% similar).",
                found + 1
            ));
        }

        if let Some(count) = search_result.match_count.filter(|&c| c > 1) {
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
                "Found {count} matches for the text in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
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

pub(super) fn apply_replacements(lines: &[String], replacements: &[Replacement]) -> Vec<String> {
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

pub(super) fn apply_hunks_to_content(
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
            return Ok((
                apply_trailing_newline_policy(&content, had_final_newline),
                warnings,
            ));
        }
    }

    let mut original_lines: Vec<String> =
        original_content.split('\n').map(str::to_string).collect();
    let mut stripped_trailing_empty = false;
    if had_final_newline && original_lines.last().is_some_and(|l| l.is_empty()) {
        original_lines.pop();
        stripped_trailing_empty = true;
    }

    let (replacements, warnings) = compute_replacements(&original_lines, path, hunks, allow_fuzzy)?;
    let mut new_lines = apply_replacements(&original_lines, &replacements);

    if stripped_trailing_empty {
        new_lines.push(String::new());
    }

    let content = new_lines.join("\n");
    Ok((
        apply_trailing_newline_policy(&content, had_final_newline),
        warnings,
    ))
}
