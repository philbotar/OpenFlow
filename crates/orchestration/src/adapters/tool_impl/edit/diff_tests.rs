//! Ported OMP diff tests from `edit-diff.test.ts` and `apply-patch.test.ts`.

use super::diff::{
    generate_diff_string, normalize_create_content, normalize_diff, parse_diff_hunks, replace_text,
    ReplaceOptions,
};

#[test]
fn generate_diff_string_collapses_unchanged_lines_between_distant_edits() {
    let old_lines: Vec<String> = (1..=20).map(|n| format!("line {n}")).collect();
    let mut new_lines = old_lines.clone();
    new_lines[1] = "line 2 changed".to_string();
    new_lines[17] = "line 18 changed".to_string();

    let result = generate_diff_string(&old_lines.join("\n"), &new_lines.join("\n"), 2);
    let diff_lines: Vec<&str> = result.diff.split('\n').collect();

    assert!(diff_lines.contains(&" 5|..."));
    assert!(diff_lines.contains(&"-2|line 2"));
    assert!(diff_lines.contains(&"+2|line 2 changed"));
    assert!(diff_lines.contains(&"-18|line 18"));
    assert!(diff_lines.contains(&"+18|line 18 changed"));
    assert!(!diff_lines.contains(&" 8|line 8"));
    assert!(!diff_lines.contains(&" 12|line 12"));
}

#[test]
fn parse_diff_hunks_simple_hunk() {
    let diff = "@@ def f():\n-    pass\n+    return 123";
    let chunks = parse_diff_hunks(diff).expect("parse");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].change_context.as_deref(), Some("def f():"));
    assert_eq!(chunks[0].old_lines, vec!["    pass".to_string()]);
    assert_eq!(chunks[0].new_lines, vec!["    return 123".to_string()]);
}

#[test]
fn parse_diff_hunks_multiple_hunks() {
    let diff = "@@\n-bar\n+BAR\n@@\n-qux\n+QUX";
    let chunks = parse_diff_hunks(diff).expect("parse");
    assert_eq!(chunks.len(), 2);
}

#[test]
fn parse_diff_hunks_context_lines() {
    let diff = "@@\n foo\n-bar\n+baz\n qux";
    let chunks = parse_diff_hunks(diff).expect("parse");
    assert_eq!(
        chunks[0].old_lines,
        vec!["foo".to_string(), "bar".to_string(), "qux".to_string()]
    );
    assert_eq!(
        chunks[0].new_lines,
        vec!["foo".to_string(), "baz".to_string(), "qux".to_string()]
    );
}

#[test]
fn parse_diff_hunks_empty_marker() {
    let diff = "@@\n+new line";
    let chunks = parse_diff_hunks(diff).expect("parse");
    assert!(chunks[0].change_context.is_none());
}

#[test]
fn parse_diff_hunks_end_of_file_marker() {
    let diff = "@@\n+line\n*** End of File";
    let chunks = parse_diff_hunks(diff).expect("parse");
    assert!(chunks[0].is_end_of_file);
}

#[test]
fn parse_diff_hunks_does_not_strip_non_sequential_line_numbers() {
    let diff = "@@\n 100 foo\n-bar\n+ 100 baz";
    let chunks = parse_diff_hunks(diff).expect("parse");
    assert_eq!(
        chunks[0].old_lines,
        vec!["100 foo".to_string(), "bar".to_string()]
    );
    assert_eq!(
        chunks[0].new_lines,
        vec!["100 foo".to_string(), " 100 baz".to_string()]
    );
}

#[test]
fn normalize_diff_strips_patch_wrappers_and_metadata() {
    let diff = "*** Begin Patch\n*** Update File: foo.txt\n@@\n-old\n+new\n*** End Patch";
    let normalized = normalize_diff(diff);
    assert_eq!(normalized, "@@\n-old\n+new");
}

#[test]
fn normalize_create_content_strips_plus_prefixes() {
    let content = "+ line one\n+line two\n";
    assert_eq!(normalize_create_content(content), "line one\nline two\n");
}

#[test]
fn replace_text_single_exact_match() {
    let result = replace_text(
        "alpha\nbeta\ngamma",
        "beta",
        "BETA",
        &ReplaceOptions {
            fuzzy: false,
            all: false,
            threshold: None,
        },
    )
    .expect("replace");
    assert_eq!(result.count, 1);
    assert_eq!(result.content, "alpha\nBETA\ngamma");
}

#[test]
fn replace_text_all_replaces_every_exact_occurrence() {
    let result = replace_text(
        "foo bar foo",
        "foo",
        "baz",
        &ReplaceOptions {
            fuzzy: false,
            all: true,
            threshold: None,
        },
    )
    .expect("replace");
    assert_eq!(result.count, 2);
    assert_eq!(result.content, "baz bar baz");
}

#[test]
fn replace_text_restores_crlf_line_endings() {
    let result = replace_text(
        "old\r\nold",
        "old",
        "new",
        &ReplaceOptions {
            fuzzy: false,
            all: true,
            threshold: None,
        },
    )
    .expect("replace");
    assert_eq!(result.content, "new\r\nnew");
}

#[test]
fn replace_text_single_errors_on_multiple_exact_occurrences() {
    let err = replace_text(
        "foo bar foo",
        "foo",
        "baz",
        &ReplaceOptions {
            fuzzy: false,
            all: false,
            threshold: None,
        },
    )
    .expect_err("expected error");
    assert!(err.contains("Found 2 occurrences"));
}

#[test]
fn replace_text_fuzzy_adjusts_indentation() {
    let result = replace_text(
        "    foo",
        "foo",
        "bar",
        &ReplaceOptions {
            fuzzy: true,
            all: false,
            threshold: None,
        },
    )
    .expect("replace");
    assert_eq!(result.count, 1);
    assert_eq!(result.content, "    bar");
}
