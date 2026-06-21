//! Patch application for the edit engine (OMP `modes/patch.ts` port).

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::auto_generated::assert_editable_file;
use super::diff::{normalize_create_content, parse_diff_hunks, DiffHunk};
use super::errors::ApplyPatchError;
use super::normalize::{
    detect_line_ending, normalize_to_lf, restore_line_endings, strip_bom, BomResult,
};
use super::path::resolve_writable;
use super::replace_sequence::{
    find_closest_sequence_match, seek_sequence, SequenceMatchStrategy, SequenceSearchResult,
};

mod hunk;
mod search;

use super::replace::DEFAULT_FUZZY_THRESHOLD;
use hunk::{adjust_lines_indentation, build_fallback_variants, filter_fallback_variants};
use search::{
    apply_character_match, apply_trailing_newline_policy, attempt_sequence_fallback,
    choose_hinted_match, find_context_relative_match, find_hierarchical_context,
    find_sequence_with_hint, format_context_strategy, format_sequence_match_preview,
    format_sequence_match_previews, format_sequence_strategy, get_hunk_hint_index,
    read_existing_patch_file, AMBIGUITY_HINT_WINDOW,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOp {
    Create,
    Delete,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchInput {
    pub path: String,
    pub op: PatchOp,
    pub rename: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PatchOptions {
    pub cwd: PathBuf,
    pub dry_run: bool,
    pub allow_fuzzy: bool,
    pub fuzzy_threshold: f64,
}

impl Default for PatchOptions {
    fn default() -> Self {
        Self {
            cwd: PathBuf::new(),
            dry_run: false,
            allow_fuzzy: true,
            fuzzy_threshold: DEFAULT_FUZZY_THRESHOLD,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchApplyResult {
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub dest_path: PathBuf,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct PatchVerifyError {
    pub message: String,
    pub relative_path: String,
    pub resolved_path: PathBuf,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum PatchError {
    #[error("{0}")]
    Apply(#[from] ApplyPatchError),
    #[error("{0}")]
    Verify(PatchVerifyError),
}

pub trait PatchFileSystem: Send + Sync {
    fn read(&self, path: &Path) -> io::Result<String>;
    fn read_binary(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn write(&self, path: &Path, content: &str) -> io::Result<()>;
    fn delete(&self, path: &Path) -> io::Result<()>;
    fn mkdir_all(&self, path: &Path) -> io::Result<()>;
    fn exists(&self, path: &Path) -> io::Result<bool>;
}

pub struct StdPatchFileSystem;

impl PatchFileSystem for StdPatchFileSystem {
    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn read_binary(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, content)
    }

    fn delete(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn mkdir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        Ok(path.exists())
    }
}

pub(super) struct Replacement {
    start_index: usize,
    old_len: usize,
    new_lines: Vec<String>,
}

pub(super) fn compute_replacements(
    original_lines: &[String],
    path: &str,
    hunks: &[DiffHunk],
    allow_fuzzy: bool,
) -> Result<(Vec<Replacement>, Vec<String>), ApplyPatchError> {
    let mut replacements = Vec::new();
    let mut warnings = Vec::new();
    let mut line_index = 0;

    for hunk in hunks {
        if let Some(old_start) = hunk.old_start_line.filter(|&n| n < 1) {
            return Err(ApplyPatchError(format!(
                "Line hint {old_start} is out of range for {path} (line numbers start at 1)"
            )));
        }
        if let Some(new_start) = hunk.new_start_line.filter(|&n| n < 1) {
            return Err(ApplyPatchError(format!(
                "Line hint {new_start} is out of range for {path} (line numbers start at 1)"
            )));
        }

        let line_hint = hunk.old_start_line;
        let allow_aggressive_fallbacks =
            hunk.change_context.is_some() || line_hint.is_some() || hunk.is_end_of_file;
        let fallback_variants =
            filter_fallback_variants(build_fallback_variants(hunk), allow_aggressive_fallbacks);

        if hunk.change_context.is_none() && !hunk.has_context_lines {
            if let Some(hint) = line_hint {
                line_index = hint
                    .saturating_sub(1)
                    .min(original_lines.len().saturating_sub(1));
            }
        }

        let mut context_index = None;

        if let Some(ref change_context) = hunk.change_context {
            let result = find_hierarchical_context(
                original_lines,
                change_context,
                line_index,
                line_hint,
                allow_fuzzy,
            );
            context_index = result.index;

            if result.index.is_none() || result.match_count.unwrap_or(0) > 1 {
                if let Some(fallback) = attempt_sequence_fallback(
                    original_lines,
                    hunk,
                    line_index,
                    line_hint,
                    allow_fuzzy,
                    allow_aggressive_fallbacks,
                ) {
                    line_index = fallback;
                } else if let Some(count) = result.match_count.filter(|&c| c > 1) {
                    let display_context = if change_context.contains('\n') {
                        change_context
                            .split('\n')
                            .next_back()
                            .unwrap_or(change_context)
                    } else {
                        change_context.as_str()
                    };
                    let previews = format_sequence_match_previews(
                        original_lines,
                        result.match_indices.as_deref(),
                        result.match_count,
                    );
                    let strategy_hint = format_context_strategy(result.strategy)
                        .map(|s| format!(" Matching strategy: {s}."))
                        .unwrap_or_default();
                    let preview_text = previews.map(|p| format!("\n\n{p}")).unwrap_or_default();
                    return Err(ApplyPatchError(format!(
                        "Found {count} matches for context '{display_context}' in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
                    )));
                } else {
                    let display_context = if change_context.contains('\n') {
                        change_context.split('\n').collect::<Vec<_>>().join(" > ")
                    } else {
                        change_context.clone()
                    };
                    return Err(ApplyPatchError(format!(
                        "Failed to find context '{display_context}' in {path}"
                    )));
                }
            } else if let Some(idx) = result.index {
                let first_old_line = hunk.old_lines.first();
                let final_context = if change_context.contains('\n') {
                    change_context.split('\n').next_back().map(str::trim)
                } else {
                    Some(change_context.trim())
                };
                let is_hierarchical =
                    change_context.contains('\n') || change_context.split_whitespace().count() > 2;
                if first_old_line.is_some_and(|l| Some(l.trim()) == final_context)
                    || is_hierarchical
                {
                    line_index = idx;
                } else {
                    line_index = idx + 1;
                }
            }
        }

        if hunk.old_lines.is_empty() {
            let insertion_idx = if hunk.change_context.is_some() {
                line_index
            } else {
                let line_hint_for_insertion = hunk.old_start_line.or(hunk.new_start_line);
                if let Some(hint) = line_hint_for_insertion {
                    if hint < 1 {
                        return Err(ApplyPatchError(format!(
                            "Line hint {hint} is out of range for insertion in {path} (line numbers start at 1)"
                        )));
                    }
                    if hint > original_lines.len() + 1 {
                        return Err(ApplyPatchError(format!(
                            "Line hint {hint} is out of range for insertion in {path} (file has {} lines)",
                            original_lines.len()
                        )));
                    }
                    hint.saturating_sub(1)
                } else if original_lines.last().is_some_and(|l| l.is_empty()) {
                    original_lines.len().saturating_sub(1)
                } else {
                    original_lines.len()
                }
            };
            replacements.push(Replacement {
                start_index: insertion_idx,
                old_len: 0,
                new_lines: hunk.new_lines.clone(),
            });
            continue;
        }

        let mut pattern = hunk.old_lines.clone();
        let match_hint = get_hunk_hint_index(hunk, line_index);
        let mut search_result = find_sequence_with_hint(
            original_lines,
            &pattern,
            line_index,
            match_hint,
            hunk.is_end_of_file,
            allow_fuzzy,
        );
        let mut new_slice = hunk.new_lines.clone();

        if search_result.index.is_none() && pattern.last().is_some_and(|l| l.is_empty()) {
            pattern.pop();
            if new_slice.last().is_some_and(|l| l.is_empty()) {
                new_slice.pop();
            }
            search_result = find_sequence_with_hint(
                original_lines,
                &pattern,
                line_index,
                match_hint,
                hunk.is_end_of_file,
                allow_fuzzy,
            );
        }

        if search_result.index.is_none() || search_result.match_count.unwrap_or(0) > 1 {
            for variant in &fallback_variants {
                if variant.old_lines.is_empty() {
                    continue;
                }
                let variant_result = find_sequence_with_hint(
                    original_lines,
                    &variant.old_lines,
                    line_index,
                    match_hint,
                    hunk.is_end_of_file,
                    allow_fuzzy,
                );
                if variant_result.index.is_some() && variant_result.match_count.unwrap_or(1) <= 1 {
                    pattern = variant.old_lines.clone();
                    new_slice = variant.new_lines.clone();
                    search_result = variant_result;
                    break;
                }
            }
        }

        if search_result.index.is_none() {
            if let Some(ctx_idx) = context_index {
                for variant in &fallback_variants {
                    if variant.old_lines.len() != 1 || variant.new_lines.len() != 1 {
                        continue;
                    }
                    let removed_line = &variant.old_lines[0];
                    let has_shared_duplicate = hunk
                        .new_lines
                        .iter()
                        .any(|line| line.trim() == removed_line.trim());
                    if let Some(adjacent) = find_context_relative_match(
                        original_lines,
                        removed_line,
                        ctx_idx,
                        has_shared_duplicate,
                    ) {
                        pattern = variant.old_lines.clone();
                        new_slice = variant.new_lines.clone();
                        search_result = SequenceSearchResult {
                            index: Some(adjacent),
                            confidence: 0.95,
                            ..Default::default()
                        };
                        break;
                    }
                }
            }
        }

        if search_result.index.is_some() && context_index.is_some() && pattern.len() == 1 {
            let trimmed = pattern[0].trim();
            let occurrence_count = original_lines
                .iter()
                .filter(|line| line.trim() == trimmed)
                .count();
            if occurrence_count > 1 {
                let has_shared_duplicate = hunk.new_lines.iter().any(|line| line.trim() == trimmed);
                if let Some(ctx_idx) = context_index {
                    if let Some(context_match) = find_context_relative_match(
                        original_lines,
                        &pattern[0],
                        ctx_idx,
                        has_shared_duplicate,
                    ) {
                        search_result = SequenceSearchResult {
                            index: Some(context_match),
                            confidence: search_result.confidence,
                            ..search_result
                        };
                    }
                }
            }
        }

        if search_result.match_count.unwrap_or(0) > 1 {
            let hint_index = match_hint.or(line_hint.map(|h| h.saturating_sub(1)));
            if let Some(hinted) = choose_hinted_match(
                search_result.match_indices.as_deref(),
                hint_index,
                AMBIGUITY_HINT_WINDOW,
            ) {
                search_result = SequenceSearchResult {
                    index: Some(hinted),
                    match_count: Some(1),
                    ..search_result
                };
            }
        }

        if search_result.index.is_none() {
            if let Some(count) = search_result.match_count.filter(|&c| c > 1) {
                let previews = format_sequence_match_previews(
                    original_lines,
                    search_result.match_indices.as_deref(),
                    search_result.match_count,
                );
                let strategy_hint = format_sequence_strategy(search_result.strategy)
                    .map(|s| format!(" Matching strategy: {s}."))
                    .unwrap_or_default();
                let preview_text = previews.map(|p| format!("\n\n{p}")).unwrap_or_default();
                return Err(ApplyPatchError(format!(
                    "Found {count} matches for the text in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
                )));
            }
            let closest = find_closest_sequence_match(
                original_lines,
                &pattern,
                line_index,
                hunk.is_end_of_file,
            );
            if let Some(closest_index) = closest.index.filter(|_| closest.confidence > 0.0) {
                let similarity = (closest.confidence * 100.0).round() as i64;
                let preview = format_sequence_match_preview(original_lines, closest_index);
                return Err(ApplyPatchError(format!(
                    "Failed to find expected lines in {path}:\n{}\n\nClosest match ({similarity}% similar) near line {}:\n{preview}",
                    hunk.old_lines.join("\n"),
                    closest_index + 1
                )));
            }
            return Err(ApplyPatchError(format!(
                "Failed to find expected lines in {path}:\n{}",
                hunk.old_lines.join("\n")
            )));
        }

        let Some(found) = search_result.index else {
            return Err(ApplyPatchError(format!(
                "Failed to find expected lines in {path}:\n{}",
                hunk.old_lines.join("\n")
            )));
        };

        if search_result.strategy == Some(SequenceMatchStrategy::FuzzyDominant) {
            let similarity = (search_result.confidence * 100.0).round() as i64;
            warnings.push(format!(
                "Dominant fuzzy match selected in {path} near line {} ({similarity}% similar).",
                found + 1
            ));
        }

        if let Some(count) = search_result.match_count.filter(|&c| c > 1) {
            let previews = format_sequence_match_previews(
                original_lines,
                search_result.match_indices.as_deref(),
                search_result.match_count,
            );
            let strategy_hint = format_sequence_strategy(search_result.strategy)
                .map(|s| format!(" Matching strategy: {s}."))
                .unwrap_or_default();
            let preview_text = previews.map(|p| format!("\n\n{p}")).unwrap_or_default();
            return Err(ApplyPatchError(format!(
                "Found {count} matches for the text in {path}.{strategy_hint}{preview_text}\n\nAdd more surrounding context or additional @@ anchors to make it unique.",
            )));
        }

        if hunk.change_context.is_none()
            && !hunk.has_context_lines
            && !hunk.is_end_of_file
            && line_hint.is_none()
        {
            let second_match =
                seek_sequence(original_lines, &pattern, found + 1, false, allow_fuzzy);
            if let Some(second_index) = second_match.index {
                let preview1 = format_sequence_match_preview(original_lines, found);
                let preview2 = format_sequence_match_preview(original_lines, second_index);
                return Err(ApplyPatchError(format!(
                    "Found 2 occurrences in {path}:\n\n{preview1}\n\n{preview2}\n\nAdd more context lines to disambiguate."
                )));
            }
        }

        let actual_matched_lines = original_lines[found..found + pattern.len()].to_vec();

        let mut is_no_op = pattern.len() == new_slice.len();
        if is_no_op {
            for (old, new) in pattern.iter().zip(new_slice.iter()) {
                if old != new {
                    is_no_op = false;
                    break;
                }
            }
        }

        if is_no_op {
            line_index = found + pattern.len();
            continue;
        }

        let adjusted_new_lines =
            adjust_lines_indentation(&pattern, &actual_matched_lines, &new_slice);
        replacements.push(Replacement {
            start_index: found,
            old_len: pattern.len(),
            new_lines: adjusted_new_lines,
        });
        line_index = found + pattern.len();
    }

    replacements.sort_by_key(|r| r.start_index);

    for i in 1..replacements.len() {
        let prev = &replacements[i - 1];
        let next = &replacements[i];
        let prev_end = prev.start_index + prev.old_len;
        if next.start_index < prev_end {
            let format_range = |replacement: &Replacement| -> String {
                if replacement.old_len == 0 {
                    format!("{} (insertion)", replacement.start_index + 1)
                } else {
                    format!(
                        "{}-{}",
                        replacement.start_index + 1,
                        replacement.start_index + replacement.old_len
                    )
                }
            };
            return Err(ApplyPatchError(format!(
                "Overlapping hunks detected in {path} at lines {} and {}. Split hunks or add more context to avoid overlap.",
                format_range(prev),
                format_range(next)
            )));
        }
    }

    Ok((replacements, warnings))
}

pub(super) fn apply_replacements(lines: &[String], replacements: &[Replacement]) -> Vec<String> {
    let mut result = lines.to_vec();
    for replacement in replacements.iter().rev() {
        let Replacement {
            start_index,
            old_len,
            new_lines,
        } = replacement;
        result.drain(*start_index..start_index + old_len);
        for (offset, line) in new_lines.iter().enumerate() {
            result.insert(start_index + offset, line.clone());
        }
    }
    result
}

pub(super) fn apply_hunks_to_content(
    original_content: &str,
    path: &str,
    hunks: &[DiffHunk],
    fuzzy_threshold: f64,
    allow_fuzzy: bool,
) -> Result<(String, Vec<String>), ApplyPatchError> {
    let had_final_newline = original_content.ends_with('\n');

    if hunks.len() == 1 {
        let hunk = &hunks[0];
        if hunk.change_context.is_none()
            && !hunk.has_context_lines
            && !hunk.old_lines.is_empty()
            && hunk.old_start_line.is_none()
            && !hunk.is_end_of_file
        {
            let (content, warnings) =
                apply_character_match(original_content, path, hunk, fuzzy_threshold, allow_fuzzy)?;
            return Ok((
                apply_trailing_newline_policy(&content, had_final_newline),
                warnings,
            ));
        }
    }

    let mut original_lines: Vec<String> =
        original_content.split('\n').map(str::to_string).collect();
    let mut stripped_trailing_empty = false;
    if had_final_newline && original_lines.last().is_some_and(|l| l.is_empty()) {
        original_lines.pop();
        stripped_trailing_empty = true;
    }

    let (replacements, warnings) = compute_replacements(&original_lines, path, hunks, allow_fuzzy)?;
    let mut new_lines = apply_replacements(&original_lines, &replacements);

    if stripped_trailing_empty {
        new_lines.push(String::new());
    }

    let content = new_lines.join("\n");
    Ok((
        apply_trailing_newline_policy(&content, had_final_newline),
        warnings,
    ))
}

pub(super) fn bytes_unchanged(pre: &[u8], post: &[u8]) -> bool {
    pre.len() == post.len() && pre.iter().zip(post.iter()).all(|(a, b)| a == b)
}

pub(super) fn verify_written_file(
    fs: &dyn PatchFileSystem,
    written_path: &Path,
    relative_path: &str,
    pre_edit_bytes: Option<&[u8]>,
    expected_content: &str,
    content_changed: bool,
) -> Result<(), PatchVerifyError> {
    let post_edit_bytes = fs.read_binary(written_path).map_err(|e| PatchVerifyError {
        message: format!("edit completed but could not verify write to {relative_path}: {e}"),
        relative_path: relative_path.to_string(),
        resolved_path: written_path.to_path_buf(),
    })?;

    if content_changed {
        if let Some(pre) = pre_edit_bytes {
            if bytes_unchanged(pre, &post_edit_bytes) {
                return Err(PatchVerifyError {
                    message: format!(
                        "edit appeared successful but file content did not change on disk: {relative_path}"
                    ),
                    relative_path: relative_path.to_string(),
                    resolved_path: written_path.to_path_buf(),
                });
            }
        }
    }

    if post_edit_bytes.as_slice() != expected_content.as_bytes() {
        return Err(PatchVerifyError {
            message: format!(
                "edit completed but file on disk does not match expected content: {relative_path}"
            ),
            relative_path: relative_path.to_string(),
            resolved_path: written_path.to_path_buf(),
        });
    }

    Ok(())
}

pub(super) fn verify_deleted_file(
    fs: &dyn PatchFileSystem,
    deleted_path: &Path,
    relative_path: &str,
) -> Result<(), PatchVerifyError> {
    if fs.exists(deleted_path).unwrap_or(false) {
        return Err(PatchVerifyError {
            message: format!("delete completed but file still exists: {relative_path}"),
            relative_path: relative_path.to_string(),
            resolved_path: deleted_path.to_path_buf(),
        });
    }
    Ok(())
}

pub fn apply_patch_entry(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
) -> Result<PatchApplyResult, PatchError> {
    let absolute_path = resolve_writable(&options.cwd, &input.path)
        .map_err(|e| PatchError::Apply(ApplyPatchError(e.0)))?;
    let dest_path = if let Some(rename) = &input.rename {
        resolve_writable(&options.cwd, rename)
            .map_err(|e| PatchError::Apply(ApplyPatchError(e.0)))?
    } else {
        absolute_path.clone()
    };

    if input.rename.is_some() && dest_path == absolute_path {
        return Err(PatchError::Apply(ApplyPatchError(
            "rename path is the same as source path".to_string(),
        )));
    }

    match input.op {
        PatchOp::Create => apply_create(input, options, fs, &absolute_path),
        PatchOp::Delete => apply_delete(input, options, fs, &absolute_path),
        PatchOp::Update => apply_update(input, options, fs, &absolute_path, &dest_path),
    }
}

pub(super) fn apply_create(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
) -> Result<PatchApplyResult, PatchError> {
    let diff = input.diff.as_ref().ok_or_else(|| {
        PatchError::Apply(ApplyPatchError(
            "Create operation requires diff (file content)".to_string(),
        ))
    })?;
    let normalized_content = normalize_create_content(diff);
    let content = if normalized_content.ends_with('\n') {
        normalized_content
    } else {
        format!("{normalized_content}\n")
    };

    if !options.dry_run {
        if fs
            .exists(absolute_path)
            .map_err(|error| PatchError::Apply(ApplyPatchError(error.to_string())))?
        {
            return Err(PatchError::Apply(ApplyPatchError(format!(
                "File already exists: {}",
                input.path
            ))));
        }
        if let Some(parent) = absolute_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs.mkdir_all(parent)
                    .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
            }
        }
        fs.write(absolute_path, &content)
            .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
        verify_written_file(fs, absolute_path, &input.path, None, &content, true)
            .map_err(PatchError::Verify)?;
    }

    Ok(PatchApplyResult {
        old_content: None,
        new_content: Some(content),
        dest_path: absolute_path.to_path_buf(),
        warnings: Vec::new(),
    })
}

pub(super) fn guard_editable(path: &Path, display_path: &str) -> Result<(), PatchError> {
    assert_editable_file(path, display_path)
        .map_err(|error| PatchError::Apply(ApplyPatchError(error.0)))
}

pub(super) fn apply_delete(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
) -> Result<PatchApplyResult, PatchError> {
    guard_editable(absolute_path, &input.path)?;
    let old_content =
        read_existing_patch_file(fs, absolute_path, &input.path).map_err(PatchError::Apply)?;

    if !options.dry_run {
        fs.delete(absolute_path)
            .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
        verify_deleted_file(fs, absolute_path, &input.path).map_err(PatchError::Verify)?;
    }

    Ok(PatchApplyResult {
        old_content: Some(old_content),
        new_content: None,
        dest_path: absolute_path.to_path_buf(),
        warnings: Vec::new(),
    })
}

pub(super) fn apply_update(
    input: &PatchInput,
    options: &PatchOptions,
    fs: &dyn PatchFileSystem,
    absolute_path: &Path,
    dest_path: &Path,
) -> Result<PatchApplyResult, PatchError> {
    guard_editable(absolute_path, &input.path)?;
    let is_move = input.rename.is_some() && dest_path != absolute_path;
    if is_move {
        if let Some(rename) = input.rename.as_deref() {
            guard_editable(dest_path, rename)?;
        }
    }

    let diff = input.diff.as_ref().ok_or_else(|| {
        PatchError::Apply(ApplyPatchError(
            "Update operation requires diff (hunks)".to_string(),
        ))
    })?;

    let pre_edit_bytes = if !options.dry_run {
        fs.read_binary(absolute_path).ok()
    } else {
        None
    };

    let original_content =
        read_existing_patch_file(fs, absolute_path, &input.path).map_err(PatchError::Apply)?;

    let BomResult {
        mut bom,
        text: stripped_content,
    } = strip_bom(&original_content);
    if bom.is_empty() {
        if let Ok(bytes) = fs.read_binary(absolute_path) {
            if bytes.len() >= 3 && bytes[0] == 0xef && bytes[1] == 0xbb && bytes[2] == 0xbf {
                bom = "\u{feff}".to_string();
            }
        }
    }

    let line_ending = detect_line_ending(&stripped_content);
    let normalized_content = normalize_to_lf(&stripped_content);
    let hunks = parse_diff_hunks(diff).map_err(PatchError::Apply)?;

    if hunks.is_empty() {
        return Err(PatchError::Apply(ApplyPatchError(
            "Diff contains no hunks".to_string(),
        )));
    }

    let (new_content, warnings) = apply_hunks_to_content(
        &normalized_content,
        &input.path,
        &hunks,
        options.fuzzy_threshold,
        options.allow_fuzzy,
    )
    .map_err(PatchError::Apply)?;

    let final_content = format!("{bom}{}", restore_line_endings(&new_content, line_ending));
    let content_changed = original_content != final_content;

    if !options.dry_run {
        if is_move {
            let dest_pre_edit_bytes = fs.read_binary(dest_path).ok();
            let dest_relative = input.rename.as_deref().unwrap_or(&input.path);

            if let Some(parent) = dest_path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs.mkdir_all(parent)
                        .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
                }
            }
            fs.write(dest_path, &final_content)
                .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;
            verify_written_file(
                fs,
                dest_path,
                dest_relative,
                dest_pre_edit_bytes.as_deref(),
                &final_content,
                content_changed,
            )
            .map_err(PatchError::Verify)?;

            if let Err(error) = fs.delete(absolute_path) {
                let _ = fs.delete(dest_path);
                return Err(PatchError::Apply(ApplyPatchError(format!(
                    "rename failed after writing destination; rolled back destination write: {error}"
                ))));
            }
            verify_deleted_file(fs, absolute_path, &input.path).map_err(PatchError::Verify)?;
        } else {
            fs.write(absolute_path, &final_content)
                .map_err(|e| PatchError::Apply(ApplyPatchError(e.to_string())))?;

            verify_written_file(
                fs,
                absolute_path,
                &input.path,
                pre_edit_bytes.as_deref(),
                &final_content,
                content_changed,
            )
            .map_err(PatchError::Verify)?;
        }
    }

    Ok(PatchApplyResult {
        old_content: Some(original_content),
        new_content: Some(final_content),
        dest_path: if is_move {
            dest_path.to_path_buf()
        } else {
            absolute_path.to_path_buf()
        },
        warnings,
    })
}
