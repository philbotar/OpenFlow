//! Apply a parsed list of edits to a text body.

use std::sync::OnceLock;

use regex::Regex;

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

fn structural_closer_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*[)\]}]+[;,]?\s*$").expect("valid regex"))
}

#[derive(Clone, Copy, Default)]
struct DelimiterBalance {
    paren: i32,
    bracket: i32,
    brace: i32,
}

fn compute_delimiter_balance(lines: &[String]) -> DelimiterBalance {
    let mut balance = DelimiterBalance::default();
    let mut in_block_comment = false;
    let mut quote = '\0';
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if in_block_comment {
                if ch == '*' && chars.get(i + 1) == Some(&'/') {
                    in_block_comment = false;
                    i += 1;
                }
                i += 1;
                continue;
            }
            if quote != '\0' {
                if ch == '\\' {
                    i += 1;
                } else if ch == quote {
                    quote = '\0';
                }
                i += 1;
                continue;
            }
            if ch == '"' || ch == '\'' || ch == '`' {
                quote = ch;
                i += 1;
                continue;
            }
            if ch == '/' && chars.get(i + 1) == Some(&'/') {
                break;
            }
            if ch == '/' && chars.get(i + 1) == Some(&'*') {
                in_block_comment = true;
                i += 1;
                i += 1;
                continue;
            }
            match ch {
                '(' => balance.paren += 1,
                ')' => balance.paren -= 1,
                '[' => balance.bracket += 1,
                ']' => balance.bracket -= 1,
                '{' => balance.brace += 1,
                '}' => balance.brace -= 1,
                _ => {}
            }
            i += 1;
        }
        if quote == '"' || quote == '\'' {
            quote = '\0';
        }
    }
    balance
}

fn balance_delta(a: DelimiterBalance, b: DelimiterBalance) -> DelimiterBalance {
    DelimiterBalance {
        paren: a.paren - b.paren,
        bracket: a.bracket - b.bracket,
        brace: a.brace - b.brace,
    }
}

fn balance_negate(a: DelimiterBalance) -> DelimiterBalance {
    DelimiterBalance {
        paren: -a.paren,
        bracket: -a.bracket,
        brace: -a.brace,
    }
}

fn balance_equal(a: DelimiterBalance, b: DelimiterBalance) -> bool {
    a.paren == b.paren && a.bracket == b.bracket && a.brace == b.brace
}

fn balance_is_zero(a: DelimiterBalance) -> bool {
    a.paren == 0 && a.bracket == 0 && a.brace == 0
}

struct ReplacementGroup {
    insert_indices: Vec<usize>,
    delete_indices: Vec<usize>,
    payload: Vec<String>,
    start_line: u32,
    end_line: u32,
}

fn find_replacement_group(edits: &[AppliedEdit], start: usize) -> Option<ReplacementGroup> {
    let first = edits.get(start)?;
    let (anchor_line, line_num) = match first {
        Edit::Insert {
            mode: Some(InsertMode::Replacement),
            cursor: Cursor::BeforeAnchor { anchor },
            line_num,
            ..
        } => (anchor.line, *line_num),
        _ => return None,
    };
    let mut insert_indices = Vec::new();
    let mut payload = Vec::new();
    let mut i = start;
    while i < edits.len() {
        match &edits[i] {
            Edit::Insert {
                mode: Some(InsertMode::Replacement),
                cursor: Cursor::BeforeAnchor { anchor },
                line_num: ln,
                text,
                ..
            } if *ln == line_num && anchor.line == anchor_line => {
                insert_indices.push(i);
                payload.push(text.clone());
                i += 1;
            }
            _ => break,
        }
    }
    let mut delete_indices = Vec::new();
    let mut expected_line = anchor_line;
    while i < edits.len() {
        match &edits[i] {
            Edit::Delete {
                anchor,
                line_num: ln,
                ..
            } if *ln == line_num && anchor.line == expected_line => {
                delete_indices.push(i);
                expected_line += 1;
                i += 1;
            }
            _ => break,
        }
    }
    if delete_indices.is_empty() {
        return None;
    }
    let delete_count = delete_indices.len() as u32;
    Some(ReplacementGroup {
        insert_indices,
        delete_indices,
        payload,
        start_line: anchor_line,
        end_line: anchor_line + delete_count - 1,
    })
}

fn find_duplicate_suffix(
    group: &ReplacementGroup,
    file_lines: &[String],
    delta: DelimiterBalance,
) -> usize {
    if balance_is_zero(delta) {
        return 0;
    }
    let max_k = group
        .payload
        .len()
        .min(file_lines.len().saturating_sub(group.end_line as usize));
    for k in (1..=max_k).rev() {
        let matches = (0..k).all(|t| {
            group.payload[group.payload.len() - k + t] == file_lines[group.end_line as usize + t]
        });
        if !matches {
            continue;
        }
        let slice = &group.payload[group.payload.len() - k..];
        if balance_equal(compute_delimiter_balance(slice), delta) {
            return k;
        }
    }
    0
}

fn find_duplicate_prefix(
    group: &ReplacementGroup,
    file_lines: &[String],
    delta: DelimiterBalance,
) -> usize {
    if balance_is_zero(delta) {
        return 0;
    }
    let max_j = group.payload.len().min(group.start_line as usize - 1);
    for j in (1..=max_j).rev() {
        let matches =
            (0..j).all(|t| group.payload[t] == file_lines[group.start_line as usize - 1 - j + t]);
        if !matches {
            continue;
        }
        if balance_equal(compute_delimiter_balance(&group.payload[..j]), delta) {
            return j;
        }
    }
    0
}

fn find_dropped_suffix_closers(
    group: &ReplacementGroup,
    file_lines: &[String],
    delta: DelimiterBalance,
) -> usize {
    let wanted = balance_negate(delta);
    let re = structural_closer_re();
    for m in 1..=group.delete_indices.len() {
        let line = file_lines
            .get(group.end_line as usize - m)
            .map(String::as_str)
            .unwrap_or("");
        if !re.is_match(line) {
            break;
        }
        let slice = &file_lines[group.end_line as usize - m..group.end_line as usize];
        if balance_equal(compute_delimiter_balance(slice), wanted) {
            return m;
        }
    }
    0
}

struct BoundaryEcho {
    leading: usize,
    trailing: usize,
}

fn has_non_whitespace(text: &str) -> bool {
    text.chars()
        .any(|ch| !matches!(ch, '\t' | '\n' | '\u{000b}' | '\u{000c}' | '\r' | ' '))
}

fn count_duplicate_leading_boundary_lines(
    group: &ReplacementGroup,
    file_lines: &[String],
) -> usize {
    let max = group.payload.len().min(group.start_line as usize - 1);
    for count in (1..=max).rev() {
        let mut matches = true;
        let mut has_content = false;
        for offset in 0..count {
            let line = &group.payload[offset];
            if line != &file_lines[group.start_line as usize - 1 - count + offset] {
                matches = false;
                break;
            }
            has_content |= has_non_whitespace(line);
        }
        if matches && has_content {
            return count;
        }
    }
    0
}

fn count_duplicate_trailing_boundary_lines(
    group: &ReplacementGroup,
    file_lines: &[String],
) -> usize {
    let max = group
        .payload
        .len()
        .min(file_lines.len().saturating_sub(group.end_line as usize));
    for count in (1..=max).rev() {
        let mut matches = true;
        let mut has_content = false;
        for offset in 0..count {
            let line = &group.payload[group.payload.len() - count + offset];
            if line != &file_lines[group.end_line as usize + offset] {
                matches = false;
                break;
            }
            has_content |= has_non_whitespace(line);
        }
        if matches && has_content {
            return count;
        }
    }
    0
}

fn find_boundary_echo(group: &ReplacementGroup, file_lines: &[String]) -> Option<BoundaryEcho> {
    let leading_max = count_duplicate_leading_boundary_lines(group, file_lines);
    if leading_max == 0 {
        return None;
    }
    let trailing_max = count_duplicate_trailing_boundary_lines(group, file_lines);
    if trailing_max == 0 {
        return None;
    }
    if leading_max + trailing_max >= group.payload.len() {
        return None;
    }
    Some(BoundaryEcho {
        leading: leading_max,
        trailing: trailing_max,
    })
}

fn repair_replacement_boundaries(
    edits: &[AppliedEdit],
    file_lines: &[String],
) -> (Vec<AppliedEdit>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    let mut i = 0;
    while i < edits.len() {
        if let Some(group) = find_replacement_group(edits, i) {
            let inserts: Vec<_> = group
                .insert_indices
                .iter()
                .map(|&idx| edits[idx].clone())
                .collect();
            let deletes: Vec<_> = group
                .delete_indices
                .iter()
                .map(|&idx| edits[idx].clone())
                .collect();
            i = group.delete_indices.last().copied().unwrap_or(i) + 1;

            if let Some(echo) = find_boundary_echo(&group, file_lines) {
                warnings.push(format!(
                    "Auto-repaired a replacement boundary echo at line {}: dropped {} leading and {} trailing payload line(s) already present outside the range. Issue the payload as the final desired content for the selected range only — never restate unchanged lines bordering the range.",
                    group.start_line, echo.leading, echo.trailing
                ));
                let total = inserts.len();
                out.extend(
                    inserts
                        .into_iter()
                        .skip(echo.leading)
                        .take(total.saturating_sub(echo.leading + echo.trailing)),
                );
                out.extend(deletes);
                continue;
            }

            let old_slice = &file_lines[group.start_line as usize - 1..group.end_line as usize];
            let delta = balance_delta(
                compute_delimiter_balance(&group.payload),
                compute_delimiter_balance(old_slice),
            );
            if balance_is_zero(delta) {
                out.extend(inserts);
                out.extend(deletes);
                continue;
            }
            let dup_suffix = find_duplicate_suffix(&group, file_lines, delta);
            if dup_suffix > 0 {
                warnings.push(format!(
                    "Auto-repaired a delimiter-balance mismatch in the replacement at line {}: dropped {} duplicated trailing payload line(s) already present below the range. Issue the payload as the final desired content only — never restate or omit a closing bracket bordering the range.",
                    group.start_line, dup_suffix
                ));
                let keep = inserts.len().saturating_sub(dup_suffix);
                out.extend(inserts.into_iter().take(keep));
                out.extend(deletes);
                continue;
            }
            let dup_prefix = find_duplicate_prefix(&group, file_lines, delta);
            if dup_prefix > 0 {
                warnings.push(format!(
                    "Auto-repaired a delimiter-balance mismatch in the replacement at line {}: dropped {} duplicated leading payload line(s) already present above the range. Issue the payload as the final desired content only — never restate or omit a closing bracket bordering the range.",
                    group.start_line, dup_prefix
                ));
                out.extend(inserts.into_iter().skip(dup_prefix));
                out.extend(deletes);
                continue;
            }
            let dropped = find_dropped_suffix_closers(&group, file_lines, delta);
            if dropped > 0 {
                warnings.push(format!(
                    "Auto-repaired a delimiter-balance mismatch in the replacement at line {}: kept {} structural closing line(s) the range deleted without restating. Issue the payload as the final desired content only — never restate or omit a closing bracket bordering the range.",
                    group.start_line, dropped
                ));
                out.extend(inserts);
                let keep = deletes.len().saturating_sub(dropped);
                out.extend(deletes.into_iter().take(keep));
                continue;
            }
            out.extend(inserts);
            out.extend(deletes);
            continue;
        }
        out.push(edits[i].clone());
        i += 1;
    }
    (out, warnings)
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
