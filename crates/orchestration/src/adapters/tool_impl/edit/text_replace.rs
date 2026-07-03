//! Fuzzy find-and-replace splice engine (split from diff parsing/generation).

use super::errors::FuzzyMatch;
use super::normalize::{
    adjust_indentation, detect_line_ending, normalize_to_lf, restore_line_endings,
};
use super::replace::{find_match, FindMatchOptions, MatchOutcome, DEFAULT_FUZZY_THRESHOLD};

const MAX_OCCURRENCE_PREVIEWS: usize = 5;

#[derive(Debug, Clone)]
pub struct ReplaceOptions {
    pub fuzzy: bool,
    pub all: bool,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceResult {
    pub content: String,
    pub count: usize,
}

fn format_occurrence_match_error(
    occurrences: usize,
    occurrence_previews: Option<&[String]>,
    path: Option<&str>,
) -> String {
    let previews = occurrence_previews
        .map(|items| items.join("\n\n"))
        .unwrap_or_default();
    let more_msg = if occurrences > MAX_OCCURRENCE_PREVIEWS {
        format!(" (showing first {MAX_OCCURRENCE_PREVIEWS} of {occurrences})")
    } else {
        String::new()
    };
    let path_suffix = path.map(|p| format!(" in {p}")).unwrap_or_default();
    format!(
        "Found {occurrences} occurrences{path_suffix}{more_msg}:\n\n{previews}\n\nAdd more context lines to disambiguate."
    )
}

fn exact_match_at(content: &str, old_text: &str, start: usize) -> FuzzyMatch {
    FuzzyMatch {
        actual_text: content[start..start + old_text.len()].to_string(),
        start_index: start,
        start_line: content[..start].matches('\n').count() + 1,
        confidence: 1.0,
    }
}

fn resolve_matched(
    match_outcome: &MatchOutcome,
    options: &ReplaceOptions,
    threshold: f64,
) -> Option<FuzzyMatch> {
    let should_use_closest = options.fuzzy
        && match_outcome
            .closest
            .as_ref()
            .is_some_and(|c| c.confidence >= threshold)
        && match_outcome.fuzzy_matches.is_none_or(|n| n <= 1);

    match_outcome.matched.clone().or_else(|| {
        should_use_closest
            .then(|| match_outcome.closest.clone())
            .flatten()
    })
}

fn apply_one_replacement(
    content: &mut String,
    matched: &FuzzyMatch,
    old_text: &str,
    new_text: &str,
) -> bool {
    let adjusted_new_text = adjust_indentation(old_text, &matched.actual_text, new_text);
    if adjusted_new_text == matched.actual_text {
        return false;
    }

    let end = matched.start_index + matched.actual_text.len();
    *content = format!(
        "{}{}{}",
        &content[..matched.start_index],
        adjusted_new_text,
        &content[end..]
    );
    true
}

fn finish_replace_result(content: String, original: &str, count: usize) -> ReplaceResult {
    ReplaceResult {
        content: restore_line_endings(&content, detect_line_ending(original)),
        count,
    }
}

/// Find and replace text in content using fuzzy matching.
pub fn replace_text(
    content: &str,
    old_text: &str,
    new_text: &str,
    options: &ReplaceOptions,
) -> Result<ReplaceResult, String> {
    if old_text.is_empty() {
        return Err("oldText must not be empty.".to_string());
    }

    let threshold = options.threshold.unwrap_or(DEFAULT_FUZZY_THRESHOLD);
    let mut normalized_content = normalize_to_lf(content);
    let normalized_old_text = normalize_to_lf(old_text);
    let normalized_new_text = normalize_to_lf(new_text);
    let find_opts = FindMatchOptions {
        allow_fuzzy: options.fuzzy,
        threshold: Some(threshold),
    };

    if options.all {
        let mut count = 0usize;
        loop {
            let match_outcome = find_match(&normalized_content, &normalized_old_text, &find_opts);

            if match_outcome.occurrences.is_some_and(|n| n > 1) {
                let Some(start) = normalized_content.find(&normalized_old_text) else {
                    break;
                };
                let matched = exact_match_at(&normalized_content, &normalized_old_text, start);
                if !apply_one_replacement(
                    &mut normalized_content,
                    &matched,
                    &normalized_old_text,
                    &normalized_new_text,
                ) {
                    break;
                }
                count += 1;
                continue;
            }

            let Some(matched) = resolve_matched(&match_outcome, options, threshold) else {
                break;
            };

            if !apply_one_replacement(
                &mut normalized_content,
                &matched,
                &normalized_old_text,
                &normalized_new_text,
            ) {
                break;
            }
            count += 1;
        }

        return Ok(finish_replace_result(normalized_content, content, count));
    }

    let match_outcome = find_match(&normalized_content, &normalized_old_text, &find_opts);

    if let Some(occurrences) = match_outcome.occurrences {
        if occurrences > 1 {
            return Err(format_occurrence_match_error(occurrences, None, None));
        }
    }

    if options.fuzzy && match_outcome.matched.is_none() {
        if let Some(n) = match_outcome.fuzzy_matches {
            if n > 1 {
                return Err(format!(
                    "Found {n} fuzzy matches above threshold. Add more context lines to disambiguate."
                ));
            }
        }
    }

    let Some(matched) = resolve_matched(&match_outcome, options, threshold) else {
        return Ok(finish_replace_result(normalized_content, content, 0));
    };

    apply_one_replacement(
        &mut normalized_content,
        &matched,
        &normalized_old_text,
        &normalized_new_text,
    );

    Ok(finish_replace_result(normalized_content, content, 1))
}
