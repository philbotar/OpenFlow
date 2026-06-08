//! Jailed read/write helpers for edit tools (OMP `read-file.ts` + patch I/O shell).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::normalize::{detect_line_ending, normalize_to_lf, restore_line_endings, strip_bom};
use super::path::{resolve_writable, PathEscapeError};

#[derive(Debug, Error)]
pub enum EditIoError {
    #[error(transparent)]
    Path(#[from] PathEscapeError),
    #[error("notebook paths are not supported for edit I/O: {0}")]
    NotebookNotSupported(String),
    #[error("file not found: {0}")]
    NotFound(String),
    #[error("{operation} failed for {path}: {source}")]
    Io {
        operation: &'static str,
        path: String,
        source: io::Error,
    },
}

/// Edit-time file I/O confined to **ExecutionCwd**.
#[derive(Debug, Clone)]
pub struct EditIo {
    cwd: PathBuf,
}

impl EditIo {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn resolve(&self, user_path: &str) -> Result<PathBuf, PathEscapeError> {
        resolve_writable(&self.cwd, user_path)
    }

    /// Read a text file as LF-normalized UTF-8 (BOM stripped).
    pub fn read_text(&self, user_path: &str) -> Result<String, EditIoError> {
        if is_notebook_path(user_path) {
            return Err(EditIoError::NotebookNotSupported(user_path.to_string()));
        }

        let absolute = self.resolve(user_path)?;
        let raw = fs::read_to_string(&absolute).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                EditIoError::NotFound(user_path.to_string())
            } else {
                EditIoError::Io {
                    operation: "read",
                    path: user_path.to_string(),
                    source: error,
                }
            }
        })?;
        let stripped = strip_bom(&raw);
        Ok(normalize_to_lf(&stripped.text))
    }

    /// Write UTF-8 text, restoring BOM and the file's original line endings when present.
    pub fn write_text(&self, user_path: &str, content: &str) -> Result<(), EditIoError> {
        if is_notebook_path(user_path) {
            return Err(EditIoError::NotebookNotSupported(user_path.to_string()));
        }

        let absolute = self.resolve(user_path)?;
        if let Some(parent) = absolute.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| EditIoError::Io {
                    operation: "mkdir",
                    path: user_path.to_string(),
                    source,
                })?;
            }
        }

        let (bom, ending, had_final_newline, file_existed) = if absolute.exists() {
            let existing = fs::read_to_string(&absolute).map_err(|source| EditIoError::Io {
                operation: "read",
                path: user_path.to_string(),
                source,
            })?;
            let bom_result = strip_bom(&existing);
            (
                bom_result.bom,
                detect_line_ending(&bom_result.text),
                existing.ends_with('\n') || existing.ends_with("\r\n"),
                true,
            )
        } else {
            (String::new(), detect_line_ending(content), false, false)
        };

        let mut normalized = normalize_to_lf(content);
        if file_existed {
            normalized = apply_trailing_newline_policy(&normalized, had_final_newline);
        }
        let payload = restore_line_endings(&normalized, ending);
        let final_content = format!("{bom}{payload}");

        fs::write(&absolute, final_content).map_err(|source| EditIoError::Io {
            operation: "write",
            path: user_path.to_string(),
            source,
        })
    }

    /// Write new file content with a trailing newline (create semantics).
    pub fn write_text_create(&self, user_path: &str, content: &str) -> Result<(), EditIoError> {
        let mut payload = content.to_string();
        if !payload.ends_with('\n') {
            payload.push('\n');
        }
        self.write_text(user_path, &payload)
    }

    pub fn exists(&self, user_path: &str) -> Result<bool, EditIoError> {
        if is_notebook_path(user_path) {
            return Err(EditIoError::NotebookNotSupported(user_path.to_string()));
        }

        let absolute = self.resolve(user_path)?;
        Ok(absolute.exists())
    }
}

fn apply_trailing_newline_policy(content: &str, had_final_newline: bool) -> String {
    if had_final_newline {
        if content.ends_with('\n') {
            content.to_string()
        } else {
            format!("{content}\n")
        }
    } else {
        content.trim_end_matches('\n').to_string()
    }
}

fn is_notebook_path(path: &str) -> bool {
    path.ends_with(".ipynb")
        || path.contains(".ipynb:")
        || path.ends_with(".ipynb/READ_ONLY")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn write_text_restores_crlf_from_existing_file() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        let path = "crlf.txt";
        fs::write(temp.path().join(path), "old\r\n").expect("seed");

        io.write_text(path, "new\nline").expect("write");

        let bytes = fs::read(temp.path().join(path)).expect("read bytes");
        assert_eq!(bytes, b"new\r\nline\r\n");
    }

    #[test]
    fn write_text_create_uses_lf_for_new_file() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());

        io.write_text_create("new.txt", "hello").expect("write");

        let text = fs::read_to_string(temp.path().join("new.txt")).expect("read");
        assert_eq!(text, "hello\n");
    }

    #[test]
    fn read_text_normalizes_to_lf() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        fs::write(temp.path().join("mix.txt"), "a\r\nb").expect("seed");

        let text = io.read_text("mix.txt").expect("read");
        assert_eq!(text, "a\nb");
    }

    #[test]
    fn rejects_notebook_paths() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        let err = io.read_text("nb.ipynb").unwrap_err();
        assert!(matches!(err, EditIoError::NotebookNotSupported(_)));
    }

    #[test]
    fn rejects_path_escape_on_write() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        let err = io.write_text("../escape.txt", "x").unwrap_err();
        assert!(matches!(err, EditIoError::Path(_)));
    }

    #[test]
    fn rejects_path_escape_on_read() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        let err = io.read_text("../escape.txt").unwrap_err();
        assert!(matches!(err, EditIoError::Path(_)));
    }

    #[test]
    fn rejects_notebook_paths_on_exists() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        let err = io.exists("nb.ipynb").unwrap_err();
        assert!(matches!(err, EditIoError::NotebookNotSupported(_)));
    }

    #[test]
    fn write_text_strips_trailing_newline_when_file_had_none() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());
        let path = "no_final_newline.txt";
        fs::write(temp.path().join(path), "old").expect("seed");

        io.write_text(path, "new\n").expect("write");

        let text = fs::read_to_string(temp.path().join(path)).expect("read");
        assert_eq!(text, "new");
    }

    #[test]
    fn write_creates_parent_directories() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let io = EditIo::new(temp.path().to_path_buf());

        io.write_text_create("deep/nested/file.txt", "data").expect("write");

        assert!(temp.path().join("deep/nested/file.txt").is_file());
    }
}
