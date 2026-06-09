use super::{
    adjust_indentation, detect_line_ending, min_indent, normalize_to_lf, normalize_unicode,
    restore_line_endings, strip_bom, LineEnding,
};

#[test]
fn adds_indentation_when_actual_text_is_more_indented() {
    let old_text = "foo\nbar";
    let actual_text = "    foo\n    bar";
    let new_text = "foo\nbaz\nbar";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "    foo\n    baz\n    bar");
}

#[test]
fn removes_indentation_when_actual_text_is_less_indented() {
    let old_text = "        foo\n        bar";
    let actual_text = "    foo\n    bar";
    let new_text = "        foo\n        baz";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "    foo\n    baz");
}

#[test]
fn preserves_empty_lines() {
    let old_text = "foo\n\nbar";
    let actual_text = "    foo\n\n    bar";
    let new_text = "foo\n\nbaz";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "    foo\n\n    baz");
}

#[test]
fn returns_unchanged_when_indentation_matches() {
    let old_text = "    foo";
    let actual_text = "    foo";
    let new_text = "    bar";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "    bar");
}

#[test]
fn uses_tab_from_actual_text_when_adding_indentation() {
    let old_text = "foo";
    let actual_text = "\t\tfoo";
    let new_text = "bar";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "\t\tbar");
}

#[test]
fn handles_mixed_content_with_different_indent_levels() {
    let old_text = "if (x) {\n  return y;\n}";
    let actual_text = "    if (x) {\n      return y;\n    }";
    let new_text = "if (x) {\n  return z;\n}";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "    if (x) {\n      return z;\n    }");
}

#[test]
fn does_not_go_negative_on_removal() {
    let old_text = "    foo";
    let actual_text = "foo";
    let new_text = "  bar";
    let result = adjust_indentation(old_text, actual_text, new_text);
    assert_eq!(result, "bar");
}

#[test]
fn detect_line_ending_lf() {
    assert_eq!(detect_line_ending("a\nb"), LineEnding::Lf);
}

#[test]
fn detect_line_ending_crlf() {
    assert_eq!(detect_line_ending("a\r\nb"), LineEnding::CrLf);
}

#[test]
fn detect_line_ending_cr_only() {
    assert_eq!(detect_line_ending("a\rb"), LineEnding::Cr);
}

#[test]
fn normalize_to_lf_converts_crlf_and_cr() {
    assert_eq!(normalize_to_lf("a\r\nb\rc"), "a\nb\nc");
}

#[test]
fn restore_line_endings_roundtrips_crlf() {
    let original = "line1\r\nline2";
    let lf = normalize_to_lf(original);
    assert_eq!(
        restore_line_endings(&lf, LineEnding::CrLf),
        "line1\r\nline2"
    );
}

#[test]
fn restore_line_endings_roundtrips_cr_only() {
    let original = "line1\rline2";
    let lf = normalize_to_lf(original);
    assert_eq!(restore_line_endings(&lf, LineEnding::Cr), original);
}

#[test]
fn restore_line_endings_normalizes_crlf_input_before_encoding() {
    assert_eq!(restore_line_endings("a\r\nb", LineEnding::CrLf), "a\r\nb");
}

#[test]
fn strip_bom_removes_prefix() {
    let result = strip_bom("\u{FEFF}hello");
    assert_eq!(result.bom, "\u{FEFF}");
    assert_eq!(result.text, "hello");
}

#[test]
fn strip_bom_leaves_content_without_bom() {
    let result = strip_bom("hello");
    assert!(result.bom.is_empty());
    assert_eq!(result.text, "hello");
}

#[test]
fn normalize_unicode_replaces_smart_quotes_and_nfc() {
    let decomposed = "e\u{0301}";
    let result = normalize_unicode(&format!("\u{201C}hi\u{201D} {decomposed}"));
    assert_eq!(result, "\"hi\" é");
}

#[test]
fn min_indent_ignores_empty_lines() {
    assert_eq!(min_indent("    foo\n\n  bar"), 2);
}
