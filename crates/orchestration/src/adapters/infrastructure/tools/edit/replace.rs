//! Fuzzy text replacement for the edit engine (OMP `modes/replace.ts` port).

use super::errors::FuzzyMatch;
use super::normalize::{count_leading_whitespace, normalize_for_fuzzy};

/// Default similarity threshold for fuzzy matching (OMP parity).
pub const DEFAULT_FUZZY_THRESHOLD: f64 = 0.95;

const FALLBACK_THRESHOLD: f64 = 0.8;
pub(crate) const DOMINANT_FUZZY_MIN_CONFIDENCE: f64 = 0.97;
pub(crate) const DOMINANT_FUZZY_DELTA: f64 = 0.08;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatchOutcome {
    pub matched: Option<FuzzyMatch>,
    pub closest: Option<FuzzyMatch>,
    pub occurrences: Option<usize>,
    pub fuzzy_matches: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct FindMatchOptions {
    pub allow_fuzzy: bool,
    pub threshold: Option<f64>,
}

/// Find a match for `target` within `content`.
pub fn find_match(content: &str, target: &str, options: &FindMatchOptions) -> MatchOutcome {
    if target.is_empty() {
        return MatchOutcome::default();
    }

    if let Some(exact) = find_exact_match_outcome(content, target) {
        return exact;
    }

    let threshold = options.threshold.unwrap_or(DEFAULT_FUZZY_THRESHOLD);
    let BestFuzzyMatchResult {
        best,
        above_threshold_count,
        second_best_score,
    } = find_best_fuzzy_match(content, target, threshold);

    let Some(best) = best else {
        return MatchOutcome::default();
    };

    if options.allow_fuzzy && best.confidence >= threshold {
        if above_threshold_count == 1 {
            return MatchOutcome {
                matched: Some(best.clone()),
                closest: Some(best),
                ..Default::default()
            };
        }
        if above_threshold_count > 1
            && best.confidence >= DOMINANT_FUZZY_MIN_CONFIDENCE
            && best.confidence - second_best_score >= DOMINANT_FUZZY_DELTA
        {
            return MatchOutcome {
                matched: Some(best.clone()),
                closest: Some(best),
                fuzzy_matches: Some(above_threshold_count),
                ..Default::default()
            };
        }
    }

    MatchOutcome {
        closest: Some(best),
        fuzzy_matches: Some(above_threshold_count),
        ..Default::default()
    }
}

fn find_exact_match_outcome(content: &str, target: &str) -> Option<MatchOutcome> {
    let exact_index = content.find(target)?;
    let occurrences = content.matches(target).count();
    if occurrences > 1 {
        return Some(MatchOutcome {
            occurrences: Some(occurrences),
            ..Default::default()
        });
    }

    let start_line = content[..exact_index].matches('\n').count() + 1;
    Some(MatchOutcome {
        matched: Some(FuzzyMatch {
            actual_text: target.to_string(),
            start_index: exact_index,
            start_line,
            confidence: 1.0,
        }),
        ..Default::default()
    })
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev = vec![0; b_len + 1];
    let mut curr = vec![0; b_len + 1];
    for (j, item) in prev.iter_mut().enumerate().take(b_len + 1) {
        *item = j;
    }

    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

pub(crate) fn line_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let max_len = a.chars().count().max(b.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    let distance = levenshtein_distance(a, b);
    1.0 - (distance as f64 / max_len as f64)
}

fn compute_relative_indent_depths(lines: &[&str]) -> Vec<usize> {
    let indents: Vec<usize> = lines
        .iter()
        .map(|line| count_leading_whitespace(line))
        .collect();
    let non_empty_indents: Vec<usize> = lines
        .iter()
        .zip(indents.iter())
        .filter(|(line, _)| !line.trim().is_empty())
        .map(|(_, indent)| *indent)
        .collect();

    let min_indent = non_empty_indents.iter().copied().min().unwrap_or(0);
    let indent_steps: Vec<usize> = non_empty_indents
        .iter()
        .map(|indent| indent.saturating_sub(min_indent))
        .filter(|step| *step > 0)
        .collect();
    let indent_unit = indent_steps.iter().copied().min().unwrap_or(1).max(1);

    lines
        .iter()
        .zip(indents.iter())
        .map(|(line, indent)| {
            if line.trim().is_empty() {
                0
            } else {
                let relative_indent = indent.saturating_sub(min_indent);
                ((relative_indent as f64) / (indent_unit as f64)).round() as usize
            }
        })
        .collect()
}

fn normalize_lines(lines: &[&str], include_depth: bool) -> Vec<String> {
    let indent_depths = if include_depth {
        Some(compute_relative_indent_depths(lines))
    } else {
        None
    };

    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let trimmed = line.trim();
            let prefix = if let Some(depths) = &indent_depths {
                format!("{}|", depths[index])
            } else {
                "|".to_string()
            };
            if trimmed.is_empty() {
                prefix
            } else {
                format!("{prefix}{}", normalize_for_fuzzy(trimmed))
            }
        })
        .collect()
}

fn compute_line_offsets(lines: &[&str]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(lines.len());
    let mut offset = 0;
    for (i, line) in lines.iter().enumerate() {
        offsets.push(offset);
        offset += line.len();
        if i + 1 < lines.len() {
            offset += 1;
        }
    }
    offsets
}

struct BestFuzzyMatchResult {
    best: Option<FuzzyMatch>,
    above_threshold_count: usize,
    second_best_score: f64,
}

fn find_best_fuzzy_match_core(
    content_lines: &[&str],
    target_lines: &[&str],
    offsets: &[usize],
    threshold: f64,
    include_depth: bool,
) -> BestFuzzyMatchResult {
    let target_normalized = normalize_lines(target_lines, include_depth);

    let mut best: Option<FuzzyMatch> = None;
    let mut best_score = -1.0;
    let mut second_best_score = -1.0;
    let mut above_threshold_count = 0;

    for start in 0..=content_lines.len().saturating_sub(target_lines.len()) {
        let window_lines = &content_lines[start..start + target_lines.len()];
        let window_normalized = normalize_lines(window_lines, include_depth);
        let mut score = 0.0;
        for i in 0..target_lines.len() {
            score += line_similarity(&target_normalized[i], &window_normalized[i]);
        }
        score /= target_lines.len() as f64;

        if score >= threshold {
            above_threshold_count += 1;
        }

        if score > best_score {
            second_best_score = best_score;
            best_score = score;
            best = Some(FuzzyMatch {
                actual_text: window_lines.join("\n"),
                start_index: offsets[start],
                start_line: start + 1,
                confidence: score,
            });
        } else if score > second_best_score {
            second_best_score = score;
        }
    }

    BestFuzzyMatchResult {
        best,
        above_threshold_count,
        second_best_score,
    }
}

fn find_best_fuzzy_match(content: &str, target: &str, threshold: f64) -> BestFuzzyMatchResult {
    let content_lines: Vec<&str> = content.split('\n').collect();
    let target_lines: Vec<&str> = target.split('\n').collect();

    if target_lines.is_empty() || target.is_empty() {
        return BestFuzzyMatchResult {
            best: None,
            above_threshold_count: 0,
            second_best_score: 0.0,
        };
    }
    if target_lines.len() > content_lines.len() {
        return BestFuzzyMatchResult {
            best: None,
            above_threshold_count: 0,
            second_best_score: 0.0,
        };
    }

    let offsets = compute_line_offsets(&content_lines);
    let mut result =
        find_best_fuzzy_match_core(&content_lines, &target_lines, &offsets, threshold, true);

    if let Some(best) = &result.best {
        if best.confidence < threshold && best.confidence >= FALLBACK_THRESHOLD {
            let no_depth_result = find_best_fuzzy_match_core(
                &content_lines,
                &target_lines,
                &offsets,
                threshold,
                false,
            );
            if let Some(no_depth_best) = &no_depth_result.best {
                if let Some(current_best) = &result.best {
                    if no_depth_best.confidence > current_best.confidence {
                        result = no_depth_result;
                    }
                }
            }
        }
    }

    result
}
