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
    let mut combined = run_git_diff(cwd, &["diff", "HEAD"])?;
    for path in list_untracked_files(cwd)? {
        let absolute = cwd.join(&path);
        if !absolute.is_file() {
            continue;
        }
        append_patch(&mut combined, &diff_untracked_file(cwd, &path)?);
    }
    Ok(combined)
}

pub fn current_branch(cwd: &Path) -> Result<String, GitError> {
    ensure_repo(cwd)?;
    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    Ok(branch)
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

fn append_patch(combined: &mut String, patch: &str) {
    let patch = patch.trim();
    if patch.is_empty() {
        return;
    }
    if !combined.is_empty() && !combined.ends_with('\n') {
        combined.push('\n');
    }
    combined.push_str(patch);
    if !combined.ends_with('\n') {
        combined.push('\n');
    }
}

fn list_untracked_files(cwd: &Path) -> Result<Vec<String>, GitError> {
    let out = run_git(cwd, &["ls-files", "--others", "--exclude-standard"])?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn diff_untracked_file(cwd: &Path, relative: &str) -> Result<String, GitError> {
    run_git_diff(cwd, &["diff", "--no-index", "--", "/dev/null", relative])
}

fn run_git_diff(cwd: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| GitError::CommandFailed {
            command: args.join(" "),
            message: error.to_string(),
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        return Ok(stdout);
    }
    if output.status.code() == Some(1) && !stdout.trim().is_empty() {
        return Ok(stdout);
    }
    if output.status.code() == Some(1) {
        return Ok(String::new());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(GitError::CommandFailed {
        command: args.join(" "),
        message: if message.is_empty() {
            format!("exit code {}", output.status)
        } else {
            message
        },
    })
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

    #[cfg_attr(miri, ignore)] // ponytail: Miri cannot emulate git subprocess (fork)
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

    #[cfg_attr(miri, ignore)] // ponytail: Miri cannot emulate git subprocess (fork)
    #[test]
    fn current_branch_returns_head_name() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        init_repo(temp.path());
        Command::new("git")
            .args(["commit", "--allow-empty", "-qm", "init"])
            .current_dir(temp.path())
            .status()
            .expect("git commit");
        Command::new("git")
            .args(["checkout", "-b", "feature/git-panel"])
            .current_dir(temp.path())
            .status()
            .expect("git checkout");
        assert_eq!(
            current_branch(temp.path()).expect("branch"),
            "feature/git-panel"
        );
    }

    #[cfg_attr(miri, ignore)] // ponytail: Miri cannot emulate git subprocess (fork)
    #[test]
    fn diff_repo_includes_untracked_new_files() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        init_repo(temp.path());
        fs::write(temp.path().join("tracked.txt"), "seed\n").expect("write tracked");
        Command::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(temp.path())
            .status()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-qm", "seed"])
            .current_dir(temp.path())
            .status()
            .expect("git commit");
        fs::write(temp.path().join("tracked.txt"), "changed\n").expect("write tracked");
        fs::write(temp.path().join("added.txt"), "new\n").expect("write added");

        let diff = diff_repo(temp.path()).expect("diff");
        assert!(diff.contains("tracked.txt"));
        assert!(diff.contains("added.txt"));
        assert!(diff.contains("--- /dev/null"));
    }
}
