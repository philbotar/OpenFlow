//! Git CLI helpers scoped to the execution folder.

use std::path::Path;
use std::process::{Command, Stdio};

use thiserror::Error;

use crate::tools::edit::path::{resolve_writable, PathEscapeError};

#[derive(Debug, Error)]
pub enum GitError {
    #[error(transparent)]
    Path(#[from] PathEscapeError),
    #[error("not a git repository")]
    NotRepo,
    #[error("git {command} failed: {message}")]
    CommandFailed { command: String, message: String },
}

pub fn is_repo(cwd: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn diff_file(cwd: &Path, user_path: &str) -> Result<String, GitError> {
    ensure_repo(cwd)?;
    let relative = relative_path(cwd, user_path)?;
    run_git(cwd, &["diff", "--", &relative])
}

pub fn diff_repo(cwd: &Path) -> Result<String, GitError> {
    ensure_repo(cwd)?;
    run_git(cwd, &["diff"])
}

pub fn stage_file(cwd: &Path, user_path: &str) -> Result<(), GitError> {
    ensure_repo(cwd)?;
    let relative = relative_path(cwd, user_path)?;
    run_git(cwd, &["add", "--", &relative]).map(|_| ())
}

pub fn restore_file(cwd: &Path, user_path: &str) -> Result<(), GitError> {
    ensure_repo(cwd)?;
    let relative = relative_path(cwd, user_path)?;
    if run_git(cwd, &["restore", "--", &relative]).is_ok() {
        return Ok(());
    }
    run_git(cwd, &["checkout", "--", &relative]).map(|_| ())
}

fn ensure_repo(cwd: &Path) -> Result<(), GitError> {
    if is_repo(cwd) {
        Ok(())
    } else {
        Err(GitError::NotRepo)
    }
}

fn relative_path(cwd: &Path, user_path: &str) -> Result<String, GitError> {
    let absolute = resolve_writable(cwd, user_path)?;
    Ok(path_relative_to_cwd(cwd, &absolute))
}

fn path_relative_to_cwd(cwd: &Path, absolute: &Path) -> String {
    absolute
        .strip_prefix(cwd)
        .map(|rel| rel.to_string_lossy().into_owned())
        .unwrap_or_else(|_| absolute.to_string_lossy().into_owned())
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| GitError::CommandFailed {
            command: args.join(" "),
            message: error.to_string(),
        })?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::CommandFailed {
            command: args.join(" "),
            message: if message.is_empty() {
                format!("exit code {}", output.status)
            } else {
                message
            },
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn init_repo(dir: &Path) {
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir)
            .status()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir)
            .status()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .status()
            .expect("git config name");
    }

    #[test]
    fn diff_file_shows_uncommitted_changes() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        init_repo(temp.path());
        fs::write(temp.path().join("note.txt"), "alpha\n").expect("write");
        Command::new("git")
            .args(["add", "note.txt"])
            .current_dir(temp.path())
            .status()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-qm", "seed"])
            .current_dir(temp.path())
            .status()
            .expect("git commit");
        fs::write(temp.path().join("note.txt"), "beta\n").expect("write");

        let diff = diff_file(temp.path(), "note.txt").expect("diff");
        assert!(diff.contains("beta") || diff.contains("-alpha") || diff.contains("+beta"));
    }
}
