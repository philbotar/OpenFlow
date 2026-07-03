//! Apply a parsed list of edits to a text body.

use super::boundary_repair::repair_replacement_boundaries;
use super::messages::UNRESOLVED_BLOCK_INTERNAL;
use super::tokenizer::clone_cursor;
use super::types::{Anchor, ApplyResult, Cursor, Edit, InsertMode};

type AppliedEdit = Edit;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineOrigin {
    Original,
    Insert,
    Replacement,
}

#[derive(Clone)]
struct IndexedEdit {
    edit: AppliedEdit,
    idx: usize,
}

fn get_cursor_anchors(cursor: &Cursor) -> Vec<Anchor> {
    match cursor {
        Cursor::BeforeAnchor { anchor } | Cursor::AfterAnchor { anchor } => {
            vec![Anchor { line: anchor.line }]
        }
        _ => Vec::new(),
    }
}

fn get_edit_anchors(edit: &AppliedEdit) -> Vec<Anchor> {
    match edit {
        Edit::Delete { anchor, .. } => vec![Anchor { line: anchor.line }],
        Edit::Insert { cursor, .. } => get_cursor_anchors(cursor),
        Edit::Block { .. } => Vec::new(),
    }
}

fn validate_line_bounds(edits: &[AppliedEdit], file_lines: &[String]) -> Result<(), String> {
    let len = file_lines.len() as u32;
    for edit in edits {
        for anchor in get_edit_anchors(edit) {
            if anchor.line < 1 || anchor.line > len {
                return Err(format!(
                    "Line {} does not exist (file has {len} lines)",
                    anchor.line
                ));
            }
        }
    }
    Ok(())
}

fn clone_applied_edit(edit: &AppliedEdit, index: u32) -> AppliedEdit {
    match edit {
        Edit::Delete {
            anchor,
            line_num,
            old_assertion,
            ..
        } => Edit::Delete {
            anchor: Anchor { line: anchor.line },
            line_num: *line_num,
            index,
            old_assertion: old_assertion.clone(),
        },
        Edit::Insert {
            cursor,
            text,
            line_num,
            mode,
            ..
        } => Edit::Insert {
            cursor: clone_cursor(cursor),
            text: text.clone(),
            line_num: *line_num,
            index,
            mode: *mode,
        },
        Edit::Block {
            anchor,
            payloads,
            line_num,
            ..
        } => Edit::Block {
            anchor: Anchor { line: anchor.line },
            payloads: payloads.clone(),
            line_num: *line_num,
            index,
        },
    }
}

fn insert_at_start(
    file_lines: &mut Vec<String>,
    line_origins: &mut Vec<LineOrigin>,
    lines: &[String],
) {
    if lines.is_empty() {
        return;
    }
    let origins = vec![LineOrigin::Insert; lines.len()];
    if file_lines.len() == 1 && file_lines[0].is_empty() {
        *file_lines = lines.to_vec();
        *line_origins = origins;
        return;
    }
    let mut new_lines = lines.to_vec();
    new_lines.append(file_lines);
    *file_lines = new_lines;
    let mut new_origins = origins;
    new_origins.append(line_origins);
    *line_origins = new_origins;
}

fn insert_at_end(
    file_lines: &mut Vec<String>,
    line_origins: &mut Vec<LineOrigin>,
    lines: &[String],
) -> Option<u32> {
    if lines.is_empty() {
        return None;
    }
    let origins = vec![LineOrigin::Insert; lines.len()];
    if file_lines.len() == 1 && file_lines[0].is_empty() {
        *file_lines = lines.to_vec();
        *line_origins = origins;
        return Some(1);
    }
    let has_trailing_newline = file_lines.last().is_some_and(String::is_empty);
    let insert_index = if has_trailing_newline {
        file_lines.len().saturating_sub(1)
    } else {
        file_lines.len()
    };
    file_lines.splice(insert_index..insert_index, lines.iter().cloned());
    line_origins.splice(insert_index..insert_index, origins);
    Some(insert_index as u32 + 1)
}

fn bucket_anchor_edits_by_line(
    edits: &[IndexedEdit],
) -> std::collections::BTreeMap<u32, Vec<IndexedEdit>> {
    let mut by_line = std::collections::BTreeMap::new();
    for entry in edits {
        let line = match &entry.edit {
            Edit::Delete { anchor, .. } => anchor.line,
            Edit::Insert { cursor, .. } => match cursor {
                Cursor::BeforeAnchor { anchor } | Cursor::AfterAnchor { anchor } => anchor.line,
                _ => 0,
            },
            Edit::Block { .. } => 0,
        };
        by_line
            .entry(line)
            .or_insert_with(Vec::new)
            .push(IndexedEdit {
                edit: entry.edit.clone(),
                idx: entry.idx,
            });
    }
    by_line
}

pub fn apply_edits(text: &str, edits: &[Edit]) -> Result<ApplyResult, String> {
    if edits.is_empty() {
        return Ok(ApplyResult {
            text: text.to_string(),
            first_changed_line: None,
            warnings: Vec::new(),
        });
    }
    for edit in edits {
        if matches!(edit, Edit::Block { .. }) {
            return Err(UNRESOLVED_BLOCK_INTERNAL.to_string());
        }
    }
    let applied_edits: Vec<AppliedEdit> = edits.to_vec();
    let mut file_lines: Vec<String> = text.split('\n').map(String::from).collect();
    let mut line_origins = vec![LineOrigin::Original; file_lines.len()];
    let mut first_changed_line: Option<u32> = None;
    let track = |line: u32, fcl: &mut Option<u32>| {
        if fcl.is_none_or(|existing| line < existing) {
            *fcl = Some(line);
        }
    };
    let target_edits: Vec<_> = applied_edits
        .iter()
        .enumerate()
        .map(|(index, edit)| clone_applied_edit(edit, index as u32))
        .collect();
    validate_line_bounds(&target_edits, &file_lines)?;
    let (repaired, repair_warnings) = repair_replacement_boundaries(&target_edits, &file_lines);
    let warnings = repair_warnings;
    let mut bof_lines = Vec::new();
    let mut eof_lines = Vec::new();
    let mut anchor_edits = Vec::new();
    for (idx, edit) in repaired.iter().enumerate() {
        match edit {
            Edit::Insert {
                cursor: Cursor::Bof,
                text,
                ..
            } => bof_lines.push(text.clone()),
            Edit::Insert {
                cursor: Cursor::Eof,
                text,
                ..
            } => eof_lines.push(text.clone()),
            _ => anchor_edits.push(IndexedEdit {
                edit: edit.clone(),
                idx,
            }),
        }
    }
    let by_line = bucket_anchor_edits_by_line(&anchor_edits);
    for line in by_line.keys().copied().rev() {
        let Some(bucket) = by_line.get(&line) else {
            continue;
        };
        let mut sorted = bucket.clone();
        sorted.sort_by_key(|e| e.idx);
        let idx = line as usize - 1;
        let current_line = file_lines.get(idx).cloned().unwrap_or_default();
        let mut before_insert_lines = Vec::new();
        let mut after_insert_lines = Vec::new();
        let mut replacement_lines = Vec::new();
        let mut delete_line = false;
        for entry in &sorted {
            match &entry.edit {
                Edit::Insert {
                    text,
                    mode: Some(InsertMode::Replacement),
                    ..
                } => {
                    replacement_lines.push(text.clone());
                }
                Edit::Insert {
                    cursor: Cursor::AfterAnchor { .. },
                    text,
                    ..
                } => after_insert_lines.push(text.clone()),
                Edit::Insert { text, .. } => before_insert_lines.push(text.clone()),
                Edit::Delete { .. } => delete_line = true,
                Edit::Block { .. } => {}
            }
        }
        if before_insert_lines.is_empty()
            && replacement_lines.is_empty()
            && after_insert_lines.is_empty()
            && !delete_line
        {
            continue;
        }
        let before_len = before_insert_lines.len();
        let replacement_len = replacement_lines.len();
        let after_len = after_insert_lines.len();
        let replacement: Vec<String> = if delete_line {
            before_insert_lines
                .into_iter()
                .chain(replacement_lines)
                .chain(after_insert_lines)
                .collect()
        } else {
            let mut r = before_insert_lines;
            r.extend(replacement_lines);
            r.push(current_line);
            r.extend(after_insert_lines);
            r
        };
        let mut origins = Vec::new();
        for _ in 0..before_len {
            origins.push(LineOrigin::Insert);
        }
        for _ in 0..replacement_len {
            origins.push(if delete_line {
                LineOrigin::Replacement
            } else {
                LineOrigin::Insert
            });
        }
        if !delete_line {
            origins.push(
                line_origins
                    .get(idx)
                    .copied()
                    .unwrap_or(LineOrigin::Original),
            );
        }
        for _ in 0..after_len {
            origins.push(LineOrigin::Insert);
        }
        file_lines.splice(idx..idx + 1, replacement.iter().cloned());
        line_origins.splice(idx..idx + 1, origins);
        track(line, &mut first_changed_line);
    }
    if !bof_lines.is_empty() {
        insert_at_start(&mut file_lines, &mut line_origins, &bof_lines);
        track(1, &mut first_changed_line);
    }
    if let Some(eof_line) = insert_at_end(&mut file_lines, &mut line_origins, &eof_lines) {
        track(eof_line, &mut first_changed_line);
    }
    Ok(ApplyResult {
        text: file_lines.join("\n"),
        first_changed_line,
        warnings,
    })
}
