//! Line-based sequence matching for patch mode (OMP `replace.ts` seek/context APIs).

use super::normalize::{normalize_for_fuzzy, normalize_unicode};
use super::replace::{
    find_match, fuzzy_line_partial_includes, fuzzy_line_starts_with, fuzzy_sequence_score_at,
    is_dominant_fuzzy_match, line_similarity, FindMatchOptions, PARTIAL_MATCH_MIN_LENGTH,
    PARTIAL_MATCH_MIN_RATIO,
};

const SEQUENCE_FUZZY_THRESHOLD: f64 = 0.92;
const CONTEXT_FUZZY_THRESHOLD: f64 = 0.8;
const MAX_RECORDED_MATCHES: usize = 5;
const CHARACTER_MATCH_THRESHOLD: f64 = 0.92;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceMatchStrategy {
    Exact,
    TrimTrailing,
    Trim,
    CommentPrefix,
    Unicode,
    Prefix,
    Substring,
    Fuzzy,
    FuzzyDominant,
    Character,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMatchStrategy {
    Exact,
    Trim,
    Unicode,
    Prefix,
    Substring,
    Fuzzy,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SequenceSearchResult {
    pub index: Option<usize>,
    pub confidence: f64,
    pub match_count: Option<usize>,
    pub match_indices: Option<Vec<usize>>,
    pub strategy: Option<SequenceMatchStrategy>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContextLineResult {
    pub index: Option<usize>,
    pub confidence: f64,
    pub match_count: Option<usize>,
    pub match_indices: Option<Vec<usize>>,
    pub strategy: Option<ContextMatchStrategy>,
}

struct IndexedMatches {
    first_match: Option<usize>,
    match_count: usize,
    match_indices: Vec<usize>,
}

fn collect_indexed_matches(
    start: usize,
    end_inclusive: usize,
    mut predicate: impl FnMut(usize) -> bool,
) -> IndexedMatches {
    let mut first_match = None;
    let mut match_count = 0;
    let mut match_indices = Vec::new();

    for index in start..=end_inclusive {
        if !predicate(index) {
            continue;
        }
        if first_match.is_none() {
            first_match = Some(index);
        }
        match_count += 1;
        if match_indices.len() < MAX_RECORDED_MATCHES {
            match_indices.push(index);
        }
    }

    IndexedMatches {
        first_match,
        match_count,
        match_indices,
    }
}

fn matches_at(
    lines: &[String],
    pattern: &[String],
    i: usize,
    compare: &dyn Fn(&str, &str) -> bool,
) -> bool {
    pattern
        .iter()
        .enumerate()
        .all(|(j, pat_line)| compare(&lines[i + j], pat_line))
}

fn strip_comment_prefix(line: &str) -> String {
    let mut trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("/*") {
        trimmed = rest;
    } else if let Some(rest) = trimmed.strip_prefix("*/") {
        trimmed = rest;
    } else if let Some(rest) = trimmed.strip_prefix("//") {
        trimmed = rest;
    } else if let Some(rest) = trimmed.strip_prefix('*') {
        trimmed = rest;
    } else if let Some(rest) = trimmed.strip_prefix('#') {
        trimmed = rest;
    } else if let Some(rest) = trimmed.strip_prefix(';') {
        trimmed = rest;
    } else if trimmed.starts_with("/ ") {
        trimmed = &trimmed[1..];
    }
    trimmed.trim_start().to_string()
}

/// Find a sequence of pattern lines within file lines.
pub fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
    allow_fuzzy: bool,
) -> SequenceSearchResult {
    if pattern.is_empty() {
        return SequenceSearchResult {
            index: Some(start),
            confidence: 1.0,
            strategy: Some(SequenceMatchStrategy::Exact),
            ..Default::default()
        };
    }
    if pattern.len() > lines.len() {
        return SequenceSearchResult {
            confidence: 0.0,
            ..Default::default()
        };
    }

    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start
    };
    let max_start = lines.len() - pattern.len();

    let run_exact_passes = |from: usize, to: usize| -> Option<SequenceSearchResult> {
        macro_rules! try_pass {
            ($compare:expr, $confidence:expr, $strategy:expr, $ambiguous:expr) => {
                let matches =
                    collect_indexed_matches(from, to, |i| matches_at(lines, pattern, i, &$compare));
                if let Some(index) = matches.first_match {
                    return Some(SequenceSearchResult {
                        index: Some(index),
                        confidence: $confidence,
                        match_count: if $ambiguous {
                            Some(matches.match_count)
                        } else {
                            None
                        },
                        match_indices: if $ambiguous {
                            Some(matches.match_indices)
                        } else {
                            None
                        },
                        strategy: Some($strategy),
                    });
                }
            };
        }

        try_pass!(
            |a: &str, b: &str| a == b,
            1.0,
            SequenceMatchStrategy::Exact,
            true
        );
        try_pass!(
            |a: &str, b: &str| a.trim_end() == b.trim_end(),
            0.99,
            SequenceMatchStrategy::TrimTrailing,
            true
        );
        try_pass!(
            |a: &str, b: &str| a.trim() == b.trim(),
            0.98,
            SequenceMatchStrategy::Trim,
            true
        );
        try_pass!(
            |a: &str, b: &str| strip_comment_prefix(a) == strip_comment_prefix(b),
            0.975,
            SequenceMatchStrategy::CommentPrefix,
            true
        );
        try_pass!(
            |a: &str, b: &str| normalize_unicode(a) == normalize_unicode(b),
            0.97,
            SequenceMatchStrategy::Unicode,
            true
        );

        if !allow_fuzzy {
            return None;
        }

        try_pass!(
            fuzzy_line_starts_with,
            0.965,
            SequenceMatchStrategy::Prefix,
            true
        );
        try_pass!(
            fuzzy_line_partial_includes,
            0.94,
            SequenceMatchStrategy::Substring,
            true
        );

        None
    };

    if let Some(result) = run_exact_passes(search_start, max_start) {
        return result;
    }

    if eof && search_start > start {
        if let Some(result) = run_exact_passes(start, max_start) {
            return result;
        }
    }

    if !allow_fuzzy {
        return SequenceSearchResult {
            confidence: 0.0,
            ..Default::default()
        };
    }

    let mut best_score = 0.0;
    let mut second_best_score = 0.0;
    let mut best_index = None;
    let mut fuzzy_first = None;
    let mut fuzzy_count = 0;
    let mut fuzzy_indices = Vec::new();

    let mut score_fuzzy_range = |from: usize, to: usize| {
        for i in from..=to {
            let score = fuzzy_sequence_score_at(lines, pattern, i);
            if score >= SEQUENCE_FUZZY_THRESHOLD {
                if fuzzy_first.is_none() {
                    fuzzy_first = Some(i);
                }
                fuzzy_count += 1;
                if fuzzy_indices.len() < MAX_RECORDED_MATCHES {
                    fuzzy_indices.push(i);
                }
            }
            if score > best_score {
                second_best_score = best_score;
                best_score = score;
                best_index = Some(i);
            } else if score > second_best_score {
                second_best_score = score;
            }
        }
    };

    score_fuzzy_range(search_start, max_start);
    if eof && search_start > start {
        score_fuzzy_range(start, search_start.saturating_sub(1));
    }

    if let Some(index) = best_index.filter(|_| best_score >= SEQUENCE_FUZZY_THRESHOLD) {
        if is_dominant_fuzzy_match(fuzzy_count, best_score, second_best_score) {
            return SequenceSearchResult {
                index: Some(index),
                confidence: best_score,
                match_count: Some(1),
                match_indices: Some(fuzzy_indices),
                strategy: Some(SequenceMatchStrategy::FuzzyDominant),
            };
        }
        return SequenceSearchResult {
            index: Some(index),
            confidence: best_score,
            match_count: Some(fuzzy_count),
            match_indices: Some(fuzzy_indices),
            strategy: Some(SequenceMatchStrategy::Fuzzy),
        };
    }

    let pattern_text = pattern.join("\n");
    let content_text = lines[start..].join("\n");
    let match_outcome = find_match(
        &content_text,
        &pattern_text,
        &FindMatchOptions {
            allow_fuzzy: true,
            threshold: Some(CHARACTER_MATCH_THRESHOLD),
        },
    );

    if let Some(matched) = match_outcome.matched {
        let matched_prefix = &content_text[..matched.start_index];
        let line_index = start + matched_prefix.matches('\n').count();
        let fallback_count = match_outcome
            .occurrences
            .or(match_outcome.fuzzy_matches)
            .unwrap_or(1);
        return SequenceSearchResult {
            index: Some(line_index),
            confidence: matched.confidence,
            match_count: Some(fallback_count),
            strategy: Some(SequenceMatchStrategy::Character),
            ..Default::default()
        };
    }

    SequenceSearchResult {
        confidence: best_score,
        match_count: match_outcome.occurrences.or(match_outcome.fuzzy_matches),
        ..Default::default()
    }
}

/// Find the closest fuzzy sequence match for error reporting.
pub fn find_closest_sequence_match(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> SequenceSearchResult {
    if pattern.is_empty() {
        return SequenceSearchResult {
            index: Some(start),
            confidence: 1.0,
            strategy: Some(SequenceMatchStrategy::Exact),
            ..Default::default()
        };
    }
    if pattern.len() > lines.len() {
        return SequenceSearchResult {
            confidence: 0.0,
            strategy: Some(SequenceMatchStrategy::Fuzzy),
            ..Default::default()
        };
    }

    let max_start = lines.len() - pattern.len();
    let search_start = if eof && lines.len() >= pattern.len() {
        max_start
    } else {
        start
    };

    let mut best_index = None;
    let mut best_score = 0.0;

    for i in search_start..=max_start {
        let score = fuzzy_sequence_score_at(lines, pattern, i);
        if score > best_score {
            best_score = score;
            best_index = Some(i);
        }
    }

    if eof && search_start > start {
        for i in start..search_start {
            let score = fuzzy_sequence_score_at(lines, pattern, i);
            if score > best_score {
                best_score = score;
                best_index = Some(i);
            }
        }
    }

    SequenceSearchResult {
        index: best_index,
        confidence: best_score,
        strategy: Some(SequenceMatchStrategy::Fuzzy),
        ..Default::default()
    }
}

/// Find a single context line using progressive matching strategies.
pub fn find_context_line(
    lines: &[String],
    context: &str,
    start_from: usize,
    allow_fuzzy: bool,
    skip_function_fallback: bool,
) -> ContextLineResult {
    let trimmed_context = context.trim();

    let end_index = lines.len().saturating_sub(1);

    let exact_matches = collect_indexed_matches(start_from, end_index, |i| lines[i] == context);
    if let Some(index) = exact_matches.first_match {
        return ContextLineResult {
            index: Some(index),
            confidence: 1.0,
            match_count: Some(exact_matches.match_count),
            match_indices: Some(exact_matches.match_indices),
            strategy: Some(ContextMatchStrategy::Exact),
        };
    }

    let trim_matches = collect_indexed_matches(start_from, end_index, |i| {
        lines[i].trim() == trimmed_context
    });
    if let Some(index) = trim_matches.first_match {
        return ContextLineResult {
            index: Some(index),
            confidence: 0.99,
            match_count: Some(trim_matches.match_count),
            match_indices: Some(trim_matches.match_indices),
            strategy: Some(ContextMatchStrategy::Trim),
        };
    }

    let normalized_context = normalize_unicode(context);
    let unicode_matches = collect_indexed_matches(start_from, end_index, |i| {
        normalize_unicode(&lines[i]) == normalized_context
    });
    if let Some(index) = unicode_matches.first_match {
        return ContextLineResult {
            index: Some(index),
            confidence: 0.98,
            match_count: Some(unicode_matches.match_count),
            match_indices: Some(unicode_matches.match_indices),
            strategy: Some(ContextMatchStrategy::Unicode),
        };
    }

    if !allow_fuzzy {
        return ContextLineResult {
            confidence: 0.0,
            ..Default::default()
        };
    }

    let context_norm = normalize_for_fuzzy(context);
    if !context_norm.is_empty() {
        let prefix_matches = collect_indexed_matches(start_from, end_index, |i| {
            normalize_for_fuzzy(&lines[i]).starts_with(&context_norm)
        });
        if let Some(index) = prefix_matches.first_match {
            return ContextLineResult {
                index: Some(index),
                confidence: 0.96,
                match_count: Some(prefix_matches.match_count),
                match_indices: Some(prefix_matches.match_indices),
                strategy: Some(ContextMatchStrategy::Prefix),
            };
        }
    }

    if context_norm.len() >= PARTIAL_MATCH_MIN_LENGTH {
        let mut all_substring = Vec::new();
        for (i, line) in lines.iter().enumerate().skip(start_from) {
            let line_norm = normalize_for_fuzzy(line);
            if line_norm.contains(&context_norm) {
                let ratio = context_norm.len() as f64 / line_norm.len().max(1) as f64;
                all_substring.push((i, ratio));
            }
        }
        let match_indices: Vec<usize> = all_substring.iter().take(5).map(|(i, _)| *i).collect();

        if all_substring.len() == 1 {
            return ContextLineResult {
                index: Some(all_substring[0].0),
                confidence: 0.94,
                match_count: Some(1),
                match_indices: Some(match_indices),
                strategy: Some(ContextMatchStrategy::Substring),
            };
        }

        let mut first_match = None;
        let mut match_count = 0;
        for (index, ratio) in &all_substring {
            if *ratio >= PARTIAL_MATCH_MIN_RATIO {
                if first_match.is_none() {
                    first_match = Some(*index);
                }
                match_count += 1;
            }
        }
        if match_count > 0 {
            return ContextLineResult {
                index: first_match,
                confidence: 0.94,
                match_count: Some(match_count),
                match_indices: Some(match_indices),
                strategy: Some(ContextMatchStrategy::Substring),
            };
        }

        if all_substring.len() > 1 {
            return ContextLineResult {
                index: Some(all_substring[0].0),
                confidence: 0.94,
                match_count: Some(all_substring.len()),
                match_indices: Some(match_indices),
                strategy: Some(ContextMatchStrategy::Substring),
            };
        }
    }

    let mut best_index = None;
    let mut best_score = 0.0;
    let mut fuzzy_first = None;
    let mut fuzzy_count = 0;
    let mut fuzzy_indices = Vec::new();

    for (i, line) in lines.iter().enumerate().skip(start_from) {
        let line_norm = normalize_for_fuzzy(line);
        let score = line_similarity(&line_norm, &context_norm);
        if score >= CONTEXT_FUZZY_THRESHOLD {
            if fuzzy_first.is_none() {
                fuzzy_first = Some(i);
            }
            fuzzy_count += 1;
            if fuzzy_indices.len() < MAX_RECORDED_MATCHES {
                fuzzy_indices.push(i);
            }
        }
        if score > best_score {
            best_score = score;
            best_index = Some(i);
        }
    }

    if let Some(index) = best_index.filter(|_| best_score >= CONTEXT_FUZZY_THRESHOLD) {
        return ContextLineResult {
            index: Some(index),
            confidence: best_score,
            match_count: Some(fuzzy_count),
            match_indices: Some(fuzzy_indices),
            strategy: Some(ContextMatchStrategy::Fuzzy),
        };
    }

    if !skip_function_fallback && trimmed_context.ends_with("()") {
        let with_paren = trimmed_context.trim_end_matches("()").to_string() + "(";
        let without_paren = trimmed_context.trim_end_matches("()").to_string();
        let paren_result = find_context_line(lines, &with_paren, start_from, allow_fuzzy, true);
        if paren_result.index.is_some() || paren_result.match_count.unwrap_or(0) > 0 {
            return paren_result;
        }
        return find_context_line(lines, &without_paren, start_from, allow_fuzzy, true);
    }

    ContextLineResult {
        confidence: best_score,
        ..Default::default()
    }
}
