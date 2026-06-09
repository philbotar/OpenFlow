//! Ported OMP `parseApplyPatch` tests from `apply-patch.test.ts`.

use super::apply_patch::{
    expand_apply_patch_to_inputs, parse_apply_patch, parse_apply_patch_streaming,
};
use super::errors::ParseError;
use super::patch::{PatchInput, PatchOp};

fn wrap(body: &str) -> String {
    format!("*** Begin Patch\n{body}\n*** End Patch")
}

#[test]
fn rejects_invalid_first_line() {
    let err = parse_apply_patch("bad").unwrap_err();
    assert!(err.to_string().contains("*** Begin Patch"));
}

#[test]
fn rejects_missing_end_marker() {
    let err = parse_apply_patch("*** Begin Patch\nbad").unwrap_err();
    assert!(err.to_string().contains("*** End Patch"));
}

#[test]
fn parses_add_file_with_whitespace_padded_markers() {
    let patch = "*** Begin Patch \n*** Add File: foo\n+hi\n *** End Patch";
    let result = parse_apply_patch(patch).expect("parse");
    assert_eq!(
        result,
        vec![PatchInput {
            path: "foo".to_string(),
            op: PatchOp::Create,
            rename: None,
            diff: Some("hi\n".to_string()),
        }]
    );
}

#[test]
fn rejects_empty_update_file_hunk() {
    let patch = wrap("*** Update File: test.py");
    let err = parse_apply_patch(&patch).unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn parses_empty_patch() {
    let patch = wrap("");
    let result = parse_apply_patch(&patch).expect("parse");
    assert!(result.is_empty());
}

#[test]
fn parses_full_patch_with_all_operations() {
    let patch = wrap(
        "*** Add File: path/add.py\n\
         +abc\n\
         +def\n\
         *** Delete File: path/delete.py\n\
         *** Update File: path/update.py\n\
         *** Move to: path/update2.py\n\
         @@ def f():\n\
         -    pass\n\
         +    return 123",
    );
    let result = parse_apply_patch(&patch).expect("parse");
    assert_eq!(result.len(), 3);

    assert_eq!(result[0].path, "path/add.py");
    assert_eq!(result[0].op, PatchOp::Create);
    assert_eq!(result[0].diff.as_deref(), Some("abc\ndef\n"));

    assert_eq!(result[1].path, "path/delete.py");
    assert_eq!(result[1].op, PatchOp::Delete);

    assert_eq!(result[2].path, "path/update.py");
    assert_eq!(result[2].op, PatchOp::Update);
    assert_eq!(result[2].rename.as_deref(), Some("path/update2.py"));
    assert!(result[2]
        .diff
        .as_ref()
        .is_some_and(|d| d.contains("-    pass")));
}

#[test]
fn parses_heredoc_wrapped_patch() {
    let patch_text = "*** Begin Patch\n*** Add File: test.txt\n+hello\n*** End Patch";
    let heredoc_patch = format!("<<'EOF'\n{patch_text}\nEOF\n");
    let result = parse_apply_patch(&heredoc_patch).expect("parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].op, PatchOp::Create);
}

#[test]
fn returns_patch_input_shape_for_add() {
    let result = parse_apply_patch(&wrap("*** Add File: foo.txt\n+hi")).expect("parse");
    assert_eq!(
        result,
        vec![PatchInput {
            path: "foo.txt".to_string(),
            op: PatchOp::Create,
            rename: None,
            diff: Some("hi\n".to_string()),
        }]
    );
}

#[test]
fn maps_update_with_rename() {
    let result = parse_apply_patch(&wrap(
        "*** Update File: a.py\n*** Move to: b.py\n@@\n-old\n+new",
    ))
    .expect("parse");
    assert_eq!(result[0].path, "a.py");
    assert_eq!(result[0].op, PatchOp::Update);
    assert_eq!(result[0].rename.as_deref(), Some("b.py"));
    assert!(result[0].diff.as_ref().is_some_and(|d| d.contains("-old")));
}

#[test]
fn zero_hunk_patch_returns_empty_array() {
    assert!(parse_apply_patch(&wrap("")).expect("parse").is_empty());
}

#[test]
fn heredoc_wrapper_with_double_quotes_is_stripped() {
    let inner = wrap("*** Add File: x.txt\n+content");
    let wrapped = format!("<<\"EOF\"\n{inner}\nEOF");
    let result = parse_apply_patch(&wrapped).expect("parse");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].op, PatchOp::Create);
}

#[test]
fn heredoc_wrapper_with_bare_eof_is_stripped() {
    let inner = wrap("*** Add File: x.txt\n+content");
    let wrapped = format!("<<EOF\n{inner}\nEOF");
    let result = parse_apply_patch(&wrapped).expect("parse");
    assert_eq!(result.len(), 1);
}

#[test]
fn mismatched_heredoc_quotes_are_not_stripped() {
    let inner = wrap("*** Add File: x.txt\n+content");
    let bad = format!("<<\"EOF'\n{inner}\nEOF");
    let err = parse_apply_patch(&bad).unwrap_err();
    assert!(matches!(err, ParseError { .. }));
}

#[test]
fn unknown_file_directive_is_rejected() {
    let err = parse_apply_patch(&wrap("*** Rename File: a")).unwrap_err();
    assert!(err.to_string().contains("is not a valid hunk header"));
}

#[test]
fn preserves_end_of_file_marker_inside_update_body() {
    let result = parse_apply_patch(&wrap("*** Update File: a.py\n@@\n-x\n+y\n*** End of File"))
        .expect("parse");
    assert!(result[0]
        .diff
        .as_ref()
        .is_some_and(|d| d.contains("*** End of File")));
}

#[test]
fn expand_apply_patch_errors_on_empty_envelope() {
    let err = expand_apply_patch_to_inputs(&wrap("")).unwrap_err();
    assert_eq!(err.0, "No files were modified.");
}

#[test]
fn streaming_parser_tolerates_missing_end_marker() {
    let partial = "*** Begin Patch\n*** Add File: foo.txt\n+partial";
    let result = parse_apply_patch_streaming(partial);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "foo.txt");
    assert_eq!(result[0].op, PatchOp::Create);
}

#[test]
fn streaming_parser_returns_empty_without_begin_marker() {
    assert!(parse_apply_patch_streaming("not a patch").is_empty());
}

#[test]
fn streaming_parser_allows_empty_update_hunk() {
    let partial = "*** Begin Patch\n*** Update File: a.py";
    let result = parse_apply_patch_streaming(partial);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].op, PatchOp::Update);
    assert_eq!(result[0].diff.as_deref(), Some(""));
}

#[test]
fn update_body_preserves_spaced_pseudo_header_lines() {
    let patch = wrap("*** Update File: a.py\n@@\n *** Update File: not-a-header\n+done");
    let result = parse_apply_patch(&patch).expect("parse");
    let diff = result[0].diff.as_ref().expect("diff");
    assert!(diff.contains("*** Update File: not-a-header"));
}
