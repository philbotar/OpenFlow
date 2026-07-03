//! Patch application for the edit engine (OMP `modes/patch.ts` port).

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::auto_generated::assert_editable_file;
use super::diff::{normalize_create_content, parse_diff_hunks};
use super::errors::ApplyPatchError;
use super::normalize::{
    detect_line_ending, normalize_to_lf, restore_line_endings, strip_bom, BomResult,
};
use super::path::resolve_writable;
use super::replace::DEFAULT_FUZZY_THRESHOLD;

mod hunk;
mod replacements;
mod search;

use replacements::apply_hunks_to_content;
use search::read_existing_patch_file;

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
