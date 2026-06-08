use super::adjust_indentation;

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
