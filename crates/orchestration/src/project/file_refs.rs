use crate::api::{ProjectFileReference, ProjectFileReferenceContent};
use crate::error::BackendError;
use ignore::WalkBuilder;
use std::fs;
use std::path::{Component, Path, PathBuf};

const DEFAULT_FILE_REFERENCE_LIMIT: usize = 30;
const DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES: u64 = 64 * 1024;
const DEFAULT_FILE_REFERENCE_MAX_FILES: usize = 5;

pub fn list_project_file_references(
    execution_cwd: &str,
    query: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<ProjectFileReference>, BackendError> {
    let cwd = canonical_execution_cwd(execution_cwd)?;
    let normalized_query = normalize_query(query);
    let limit = limit
        .unwrap_or(DEFAULT_FILE_REFERENCE_LIMIT)
        .clamp(1, DEFAULT_FILE_REFERENCE_LIMIT);
    let mut matches = Vec::new();

    let mut builder = WalkBuilder::new(&cwd);
    builder.standard_filters(true).follow_links(false);

    for entry in builder.build() {
        let entry = entry.map_err(|error| BackendError::ProjectOperation(error.to_string()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(relative) = path.strip_prefix(&cwd) else {
            continue;
        };
        let relative = normalize_relative_path(relative);
        if !matches_query(&relative, normalized_query.as_deref()) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| BackendError::ProjectOperation(error.to_string()))?;
        matches.push(ProjectFileReference {
            display_path: relative.clone(),
            path: relative,
            size_bytes: metadata.len(),
        });
        if matches.len() >= limit {
            break;
        }
    }

    matches.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(matches)
}

pub fn read_project_file_references(
    execution_cwd: &str,
    paths: &[String],
) -> Result<Vec<ProjectFileReferenceContent>, BackendError> {
    if paths.len() > DEFAULT_FILE_REFERENCE_MAX_FILES {
        return Err(BackendError::ProjectOperation(format!(
            "file references support at most {DEFAULT_FILE_REFERENCE_MAX_FILES} files per message"
        )));
    }

    let cwd = canonical_execution_cwd(execution_cwd)?;
    let mut refs = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = normalize_user_relative_path(path)?;
        let absolute = resolve_existing_file(&cwd, &relative)?;
        let metadata = fs::metadata(&absolute)
            .map_err(|error| BackendError::ProjectOperation(format!("read {relative}: {error}")))?;
        if !metadata.is_file() {
            return Err(BackendError::ProjectOperation(format!(
                "file reference is not a file: {relative}"
            )));
        }

        let bytes = fs::read(&absolute)
            .map_err(|error| BackendError::ProjectOperation(format!("read {relative}: {error}")))?;
        let truncated = metadata.len() > DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES;
        let bytes = if truncated {
            bytes
                .into_iter()
                .take(DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES as usize)
                .collect::<Vec<_>>()
        } else {
            bytes
        };
        let content = String::from_utf8(bytes).map_err(|_| {
            BackendError::ProjectOperation(format!("file reference is not valid UTF-8: {relative}"))
        })?;

        refs.push(ProjectFileReferenceContent {
            path: relative,
            content,
            truncated,
            size_bytes: metadata.len(),
        });
    }
    Ok(refs)
}

fn canonical_execution_cwd(execution_cwd: &str) -> Result<PathBuf, BackendError> {
    let trimmed = execution_cwd.trim();
    if trimmed.is_empty() {
        return Err(BackendError::InvalidExecutionCwd(
            "execution folder must not be empty".to_string(),
        ));
    }
    let cwd = Path::new(trimmed)
        .canonicalize()
        .map_err(|error| BackendError::InvalidExecutionCwd(error.to_string()))?;
    if !cwd.is_dir() {
        return Err(BackendError::InvalidExecutionCwd(format!(
            "execution folder is not a directory: {}",
            cwd.display()
        )));
    }
    Ok(cwd)
}

fn normalize_query(query: Option<&str>) -> Option<String> {
    query
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn matches_query(path: &str, query: Option<&str>) -> bool {
    match query {
        Some(query) => path.to_lowercase().contains(query),
        None => true,
    }
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_user_relative_path(path: &str) -> Result<String, BackendError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(BackendError::ProjectOperation(
            "file reference path must not be empty".to_string(),
        ));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(BackendError::ProjectOperation(format!(
            "path escapes execution folder: {trimmed}"
        )));
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(BackendError::ProjectOperation(format!(
                    "path escapes execution folder: {trimmed}"
                )));
            }
        }
    }
    if parts.is_empty() {
        return Err(BackendError::ProjectOperation(
            "file reference path must not be empty".to_string(),
        ));
    }
    Ok(parts.join("/"))
}

fn resolve_existing_file(cwd: &Path, relative: &str) -> Result<PathBuf, BackendError> {
    let absolute = cwd.join(relative);
    let canonical = absolute
        .canonicalize()
        .map_err(|error| BackendError::ProjectOperation(format!("read {relative}: {error}")))?;
    if canonical.strip_prefix(cwd).is_err() {
        return Err(BackendError::ProjectOperation(format!(
            "path escapes execution folder: {relative}"
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write fixture");
    }

    #[test]
    fn lists_matching_files_under_execution_cwd() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir.path().join("src/main.rs"), "fn main() {}\n");
        write(&dir.path().join("README.md"), "# Readme\n");
        write(
            &dir.path().join("target/generated.rs"),
            "ignored by query only\n",
        );

        let refs = list_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            Some("main"),
            Some(10),
        )
        .expect("list refs");

        assert_eq!(
            refs,
            vec![ProjectFileReference {
                path: "src/main.rs".to_string(),
                display_path: "src/main.rs".to_string(),
                size_bytes: 13,
            }]
        );
    }

    #[test]
    fn respects_gitignore() {
        let dir = TempDir::new().expect("tempdir");
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("git init");
        write(&dir.path().join(".gitignore"), "ignored.txt\n");
        write(&dir.path().join("ignored.txt"), "secret\n");
        write(&dir.path().join("visible.txt"), "public\n");

        let refs = list_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            Some("txt"),
            Some(10),
        )
        .expect("list refs");

        assert_eq!(
            refs.into_iter().map(|item| item.path).collect::<Vec<_>>(),
            vec!["visible.txt"]
        );
    }

    #[test]
    fn read_rejects_path_escape() {
        let dir = TempDir::new().expect("tempdir");
        let error = read_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            &[String::from("../outside.txt")],
        )
        .unwrap_err();

        assert!(error.to_string().contains("escapes execution folder"));
    }

    #[test]
    fn reads_utf8_file_content() {
        let dir = TempDir::new().expect("tempdir");
        write(
            &dir.path().join("src/lib.rs"),
            "pub fn answer() -> u8 { 42 }\n",
        );

        let refs = read_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            &[String::from("src/lib.rs")],
        )
        .expect("read refs");

        assert_eq!(
            refs,
            vec![ProjectFileReferenceContent {
                path: "src/lib.rs".to_string(),
                content: "pub fn answer() -> u8 { 42 }\n".to_string(),
                truncated: false,
                size_bytes: 29,
            }]
        );
    }

    #[test]
    fn read_truncates_large_files() {
        let dir = TempDir::new().expect("tempdir");
        let content = "a".repeat((DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES as usize) + 8);
        write(&dir.path().join("large.txt"), &content);

        let refs = read_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            &[String::from("large.txt")],
        )
        .expect("read refs");

        assert_eq!(
            refs[0].content.len(),
            DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES as usize
        );
        assert!(refs[0].truncated);
        assert_eq!(
            refs[0].size_bytes,
            DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES + 8
        );
    }

    #[test]
    fn read_rejects_binary_files() {
        let dir = TempDir::new().expect("tempdir");
        fs::write(dir.path().join("image.bin"), [0xff, 0xfe, 0xfd]).expect("write binary");

        let error = read_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            &[String::from("image.bin")],
        )
        .unwrap_err();

        assert!(error.to_string().contains("not valid UTF-8"));
    }

    #[test]
    fn read_limits_file_count() {
        let dir = TempDir::new().expect("tempdir");
        for index in 0..6 {
            write(&dir.path().join(format!("file-{index}.txt")), "x\n");
        }
        let paths = (0..6)
            .map(|index| format!("file-{index}.txt"))
            .collect::<Vec<_>>();

        let error = read_project_file_references(dir.path().to_str().expect("utf8 path"), &paths)
            .unwrap_err();

        assert!(error.to_string().contains("at most 5 files"));
    }
}
