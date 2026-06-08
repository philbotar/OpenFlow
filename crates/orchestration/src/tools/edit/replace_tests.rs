//! Ported OMP `findMatch` tests from `edit-diff.test.ts`.

use super::replace::{find_match, FindMatchOptions, MatchOutcome, DEFAULT_FUZZY_THRESHOLD};

fn opts(allow_fuzzy: bool) -> FindMatchOptions {
    FindMatchOptions {
        allow_fuzzy,
        threshold: None,
    }
}

fn opts_with_threshold(allow_fuzzy: bool, threshold: f64) -> FindMatchOptions {
    FindMatchOptions {
        allow_fuzzy,
        threshold: Some(threshold),
    }
}

#[test]
fn finds_exact_match() {
    let content = "line1\nline2\nline3";
    let target = "line2";
    let result = find_match(content, target, &opts(false));
    let m = result.matched.expect("expected match");
    assert!((m.confidence - 1.0).abs() < f64::EPSILON);
    assert_eq!(m.start_line, 2);
}

#[test]
fn reports_multiple_occurrences() {
    let content = "foo\nbar\nfoo";
    let target = "foo";
    let result = find_match(content, target, &opts(false));
    assert!(result.matched.is_none());
    assert_eq!(result.occurrences, Some(2));
}

#[test]
fn returns_empty_for_no_match() {
    let content = "line1\nline2";
    let target = "notfound";
    let result = find_match(content, target, &opts(false));
    assert!(result.matched.is_none());
    assert!(result.occurrences.is_none());
}

#[test]
fn matches_tabs_in_file_with_spaces_in_target() {
    let content = "\tfoo\n\t\tbar\n\tbaz";
    let target = "  foo\n    bar\n  baz";
    let result = find_match(content, target, &opts(true));
    let m = result.matched.expect("expected match");
    assert!(m.confidence >= DEFAULT_FUZZY_THRESHOLD);
}

#[test]
fn matches_spaces_in_file_with_tabs_in_target() {
    let content = "  foo\n    bar\n  baz";
    let target = "\tfoo\n\t\tbar\n\tbaz";
    let result = find_match(content, target, &opts(true));
    let m = result.matched.expect("expected match");
    assert!(m.confidence >= DEFAULT_FUZZY_THRESHOLD);
}

#[test]
fn matches_different_space_counts_with_same_relative_structure() {
    let content = "   foo\n      bar\n   baz";
    let target = "  foo\n    bar\n  baz";
    let result = find_match(content, target, &opts(true));
    let m = result.matched.expect("expected match");
    assert!(m.confidence >= DEFAULT_FUZZY_THRESHOLD);
}

#[test]
fn matches_single_line_with_different_indentation() {
    let content = "prefix\n\t\t\t\"value\",\nsuffix";
    let target = "          \"value\",";
    let result = find_match(content, target, &opts(true));
    let m = result.matched.expect("expected match");
    assert!(m.confidence >= DEFAULT_FUZZY_THRESHOLD);
}

#[test]
fn matches_despite_one_line_with_wrong_indentation_in_file() {
    let content = "\t\t\tline1\n\t\t\tline2\n\t\tline3\n\t\t\tline4";
    let target = "      line1\n      line2\n      line3\n      line4";
    let result = find_match(content, target, &opts(true));
    let m = result.matched.expect("expected match");
    assert!(m.confidence >= DEFAULT_FUZZY_THRESHOLD);
}

#[test]
fn matches_when_target_has_consistent_indent_but_file_varies() {
    let content = "  a\n    b\n   c\n    d";
    let target = "  a\n    b\n    c\n    d";
    let result = find_match(content, target, &opts(true));
    assert!(result.matched.is_some());
}

#[test]
fn collapses_internal_whitespace() {
    let content = "foo   bar    baz";
    let target = "foo bar baz";
    let result = find_match(content, target, &opts(true));
    let m = result.matched.expect("expected match");
    assert!(m.confidence >= DEFAULT_FUZZY_THRESHOLD);
}

#[test]
fn matches_with_trailing_whitespace_differences() {
    let content = "line1  \nline2\t";
    let target = "line1\nline2";
    let result = find_match(content, target, &opts(true));
    assert!(result.matched.is_some());
}

#[test]
fn respects_custom_similarity_threshold() {
    let content = "function foo() {}";
    let target = "function bar() {}";
    let strict = find_match(content, target, &opts_with_threshold(true, 0.99));
    assert!(strict.matched.is_none());

    let lenient = find_match(content, target, &opts_with_threshold(true, 0.7));
    assert!(lenient.matched.is_some());
}

#[test]
fn reports_fuzzy_matches_count_when_multiple_above_threshold() {
    let content = "  item1\n  item2\n  item3";
    let target = "  itemX";
    let result = find_match(content, target, &opts_with_threshold(true, 0.7));
    assert!(result.fuzzy_matches.unwrap_or(0) > 1);
}

#[test]
fn handles_empty_target() {
    let content = "some content";
    let result = find_match(content, "", &opts(true));
    assert_eq!(result, MatchOutcome::default());
}

#[test]
fn handles_empty_lines_in_content() {
    let content = "line1\n\nline3";
    let target = "line1\n\nline3";
    let result = find_match(content, target, &opts(false));
    let m = result.matched.expect("expected match");
    assert!((m.confidence - 1.0).abs() < f64::EPSILON);
}

#[test]
fn handles_target_longer_than_content() {
    let content = "short";
    let target = "this is much longer than the content";
    let result = find_match(content, target, &opts(true));
    assert!(result.matched.is_none());
}
