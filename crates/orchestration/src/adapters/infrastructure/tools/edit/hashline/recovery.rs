//! Recover from a stale section snapshot tag via 3-way merge.

use similar::{ChangeTag, TextDiff};

use super::apply::apply_edits;
use super::messages::{
    RECOVERY_EXTERNAL_WARNING, RECOVERY_SESSION_CHAIN_WARNING, RECOVERY_SESSION_REPLAY_WARNING,
};
use super::snapshots::SnapshotStore;
use super::types::{Anchor, ApplyResult, Cursor, Edit};

pub struct RecoveryArgs<'a> {
    pub path: &'a str,
    pub current_text: &'a str,
    pub file_hash: &'a str,
    pub edits: &'a [Edit],
}

pub struct RecoveryResult {
    pub text: String,
    pub first_changed_line: Option<u32>,
    pub warnings: Vec<String>,
}

fn collect_anchor_lines(edits: &[Edit]) -> Vec<u32> {
    let mut lines = Vec::new();
    for edit in edits {
        for anchor in get_edit_anchors(edit) {
            lines.push(anchor.line);
        }
    }
    lines
}

fn get_edit_anchors(edit: &Edit) -> Vec<Anchor> {
    match edit {
        Edit::Delete { anchor, .. } | Edit::Block { anchor, .. } => {
            vec![Anchor { line: anchor.line }]
        }
        Edit::Insert { cursor, .. } => match cursor {
            Cursor::BeforeAnchor { anchor } | Cursor::AfterAnchor { anchor } => {
                vec![Anchor { line: anchor.line }]
            }
            _ => Vec::new(),
        },
    }
}

fn verify_anchor_content(previous_text: &str, current_text: &str, edits: &[Edit]) -> bool {
    let lines = collect_anchor_lines(edits);
    if lines.is_empty() {
        return true;
    }
    let prev: Vec<_> = previous_text.split('\n').collect();
    let curr: Vec<_> = current_text.split('\n').collect();
    for line in lines {
        let idx = line as usize - 1;
        if idx >= prev.len() || idx >= curr.len() {
            return false;
        }
        if prev[idx] != curr[idx] {
            return false;
        }
    }
    true
}

fn find_first_changed_line(a: &str, b: &str) -> Option<u32> {
    if a == b {
        return None;
    }
    let a_lines: Vec<_> = a.split('\n').collect();
    let b_lines: Vec<_> = b.split('\n').collect();
    let max = a_lines.len().max(b_lines.len());
    for i in 0..max {
        if a_lines.get(i) != b_lines.get(i) {
            return Some(i as u32 + 1);
        }
    }
    None
}

fn apply_line_patch_fuzz_zero(base: &str, old: &str, new: &str) -> Option<String> {
    if old == new {
        return Some(base.to_string());
    }
    let old_lines: Vec<&str> = old.split('\n').collect();
    let new_lines: Vec<&str> = new.split('\n').collect();
    let mut base_lines: Vec<String> = base.split('\n').map(String::from).collect();
    let diff = TextDiff::from_lines(old, new);
    let mut old_idx = 0usize;
    let mut new_idx = 0usize;
    let mut base_idx = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                if base_lines.get(base_idx).map(String::as_str) != Some(change.value()) {
                    return None;
                }
                old_idx += 1;
                new_idx += 1;
                base_idx += 1;
            }
            ChangeTag::Delete => {
                if old_lines.get(old_idx) != Some(&change.value()) {
                    return None;
                }
                if base_lines.get(base_idx).map(String::as_str) != Some(change.value()) {
                    return None;
                }
                base_lines.remove(base_idx);
                old_idx += 1;
            }
            ChangeTag::Insert => {
                if new_lines.get(new_idx) != Some(&change.value()) {
                    return None;
                }
                base_lines.insert(base_idx, change.value().to_string());
                base_idx += 1;
                new_idx += 1;
            }
        }
    }
    Some(base_lines.join("\n"))
}

fn apply_edits_to_snapshot(
    previous_text: &str,
    current_text: &str,
    edits: &[Edit],
    recovery_warning: &str,
) -> Option<RecoveryResult> {
    let applied = apply_edits(previous_text, edits).ok()?;
    if applied.text == previous_text {
        return None;
    }
    let merged = apply_line_patch_fuzz_zero(current_text, previous_text, &applied.text)?;
    if merged == current_text {
        return None;
    }
    let first_changed_line =
        find_first_changed_line(current_text, &merged).or(applied.first_changed_line);
    let has_net_change = first_changed_line.is_some();
    let mut warnings = if has_net_change {
        vec![recovery_warning.to_string()]
    } else {
        Vec::new()
    };
    warnings.extend(applied.warnings);
    Some(RecoveryResult {
        text: merged,
        first_changed_line,
        warnings,
    })
}

fn replay_session_chain_on_current(
    previous_text: &str,
    current_text: &str,
    edits: &[Edit],
) -> Option<RecoveryResult> {
    if previous_text.split('\n').count() != current_text.split('\n').count() {
        return None;
    }
    if !verify_anchor_content(previous_text, current_text, edits) {
        return None;
    }
    let applied = apply_edits(current_text, edits).ok()?;
    if applied.text == current_text {
        return None;
    }
    let mut warnings = vec![RECOVERY_SESSION_REPLAY_WARNING.to_string()];
    warnings.extend(applied.warnings);
    Some(RecoveryResult {
        text: applied.text,
        first_changed_line: applied.first_changed_line,
        warnings,
    })
}

pub struct Recovery<'a, S: SnapshotStore + ?Sized> {
    pub store: &'a S,
}

impl<'a, S: SnapshotStore + ?Sized> Recovery<'a, S> {
    pub fn new(store: &'a S) -> Self {
        Self { store }
    }

    pub fn try_recover(&self, args: RecoveryArgs<'_>) -> Option<RecoveryResult> {
        let snapshot = self.store.by_hash(args.path, args.file_hash)?;
        let head = self.store.head(args.path);
        let is_head = head
            .as_ref()
            .is_some_and(|h| h.hash == snapshot.hash && h.text == snapshot.text);
        let recovery_warning = if is_head {
            RECOVERY_EXTERNAL_WARNING
        } else {
            RECOVERY_SESSION_CHAIN_WARNING
        };
        if let Some(merged) = apply_edits_to_snapshot(
            &snapshot.text,
            args.current_text,
            args.edits,
            recovery_warning,
        ) {
            return Some(merged);
        }
        if !is_head {
            return replay_session_chain_on_current(&snapshot.text, args.current_text, args.edits);
        }
        None
    }
}

pub fn recovery_to_apply_result(result: RecoveryResult) -> ApplyResult {
    ApplyResult {
        text: result.text,
        first_changed_line: result.first_changed_line,
        warnings: result.warnings,
    }
}
