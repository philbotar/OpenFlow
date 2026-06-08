//! Execution-folder path jail for edit tools (OpenFlow Tier C).

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct PathEscapeError(pub String);

/// Resolve `user_path` under `cwd` (**ExecutionCwd**). Reject paths that escape the jail.
pub fn resolve_writable(cwd: &Path, user_path: &str) -> Result<PathBuf, PathEscapeError> {
    if user_path.trim().is_empty() {
        return Err(PathEscapeError("path must not be empty".to_string()));
    }

    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|error| PathEscapeError(format!("invalid cwd: {error}")))?;

    let relative = Path::new(user_path);
    if relative.is_absolute() {
        let resolved = relative
            .canonicalize()
            .map_err(|error| PathEscapeError(format!("invalid path {user_path}: {error}")))?;
        if !is_subpath(&resolved, &canonical_cwd) {
            return Err(PathEscapeError(format!(
                "path escapes execution folder: {user_path}"
            )));
        }
        return Ok(resolved);
    }

    let mut resolved = canonical_cwd.clone();
    for component in relative.components() {
        match component {
            Component::Normal(name) => {
                resolved.push(name);
                canonicalize_existing_segment(&mut resolved, &canonical_cwd, user_path)?;
            }
            Component::ParentDir => {
                if !resolved.pop() {
                    return Err(PathEscapeError(format!(
                        "path escapes execution folder: {user_path}"
                    )));
                }
                canonicalize_existing_segment(&mut resolved, &canonical_cwd, user_path)?;
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {}
        }
    }

    ensure_in_jail(&resolved, &canonical_cwd, user_path)?;

    Ok(resolved)
}

fn canonicalize_existing_segment(
    resolved: &mut PathBuf,
    canonical_cwd: &Path,
    user_path: &str,
) -> Result<(), PathEscapeError> {
    if resolved.exists() {
        *resolved = resolved
            .canonicalize()
            .map_err(|error| PathEscapeError(format!("invalid path {user_path}: {error}")))?;
        ensure_in_jail(resolved, canonical_cwd, user_path)?;
    }
    Ok(())
}

fn ensure_in_jail(
    resolved: &Path,
    canonical_cwd: &Path,
    user_path: &str,
) -> Result<(), PathEscapeError> {
    if !is_subpath(resolved, canonical_cwd) {
        return Err(PathEscapeError(format!(
            "path escapes execution folder: {user_path}"
        )));
    }
    Ok(())
}

fn is_subpath(path: &Path, base: &Path) -> bool {
    path.strip_prefix(base).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolves_relative_path_under_cwd() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let resolved = resolve_writable(&cwd, "src/main.rs").expect("resolve");
        assert_eq!(resolved, cwd.join("src/main.rs"));
    }

    #[test]
    fn rejects_parent_escape() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let err = resolve_writable(&cwd, "../outside.txt").unwrap_err();
        assert!(err.0.contains("escapes execution folder"));
    }

    #[test]
    fn rejects_absolute_path_outside_cwd() {
        let temp = TempDir::new().expect("tempdir");
        let outside = TempDir::new().expect("outside");
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, "x").expect("write");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let err = resolve_writable(&cwd, outside_file.to_str().expect("utf8")).unwrap_err();
        assert!(err.0.contains("escapes execution folder"));
    }

    #[test]
    fn allows_absolute_path_inside_cwd() {
        let temp = TempDir::new().expect("tempdir");
        let file = temp.path().join("inner.txt");
        fs::write(&file, "ok").expect("write");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let resolved = resolve_writable(&cwd, file.to_str().expect("utf8")).expect("resolve");
        assert_eq!(resolved, file.canonicalize().expect("canonicalize"));
    }

    #[test]
    fn rejects_empty_path() {
        let temp = TempDir::new().expect("tempdir");
        let cwd = temp.path().canonicalize().expect("canonicalize");
        let err = resolve_writable(&cwd, "   ").unwrap_err();
        assert!(err.0.contains("must not be empty"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_via_relative_path() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("tempdir");
        let outside = TempDir::new().expect("outside");
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, "x").expect("write");

        let link = temp.path().join("escape_link");
        symlink(outside.path(), &link).expect("symlink");

        let cwd = temp.path().canonicalize().expect("canonicalize");
        let err = resolve_writable(&cwd, "escape_link/secret.txt").unwrap_err();
        assert!(err.0.contains("escapes execution folder"));
    }
}
