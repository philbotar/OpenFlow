use crate::api::{ProjectFileReference, ProjectFileReferenceKind};
use crate::error::BackendError;
use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DEFAULT_FILE_REFERENCE_LIMIT: usize = 30;
const FILE_LIST_CACHE_TTL: Duration = Duration::from_secs(10);

struct FileListCache {
    cwd: PathBuf,
    built_at: Instant,
    entries: Vec<ProjectFileReference>,
}

// ponytail: single-entry global cache; make it a per-cwd map if users hop projects mid-keystroke.
// New files are invisible for up to FILE_LIST_CACHE_TTL — acceptable for a typeahead.
static FILE_LIST_CACHE: Mutex<Option<FileListCache>> = Mutex::new(None);

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

    let mut cache = FILE_LIST_CACHE.lock().expect("file list cache lock");
    let fresh = cache
        .as_ref()
        .is_some_and(|entry| entry.cwd == cwd && entry.built_at.elapsed() < FILE_LIST_CACHE_TTL);
    if !fresh {
        *cache = Some(FileListCache {
            entries: walk_project_files(&cwd)?,
            cwd: cwd.clone(),
            built_at: Instant::now(),
        });
    }
    let entries = &cache.as_ref().expect("cache just filled").entries;
    Ok(entries
        .iter()
        .filter(|reference| matches_query(&reference.path, normalized_query.as_deref()))
        .take(limit)
        .cloned()
        .collect())
}

fn walk_project_files(cwd: &Path) -> Result<Vec<ProjectFileReference>, BackendError> {
    let mut entries = Vec::new();
    let mut builder = WalkBuilder::new(cwd);
    builder.standard_filters(true).follow_links(false);

    for entry in builder.build() {
        let entry = entry.map_err(|error| BackendError::ProjectOperation(error.to_string()))?;
        let path = entry.path();
        if path == cwd {
            continue;
        }
        let Ok(relative) = path.strip_prefix(cwd) else {
            continue;
        };
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() && !metadata.is_dir() {
            continue;
        }
        let kind = if metadata.is_dir() {
            ProjectFileReferenceKind::Directory
        } else {
            ProjectFileReferenceKind::File
        };
        let relative =
            normalize_relative_path(relative, kind == ProjectFileReferenceKind::Directory);
        entries.push(ProjectFileReference {
            display_path: relative.clone(),
            path: relative,
            kind,
            size_bytes: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
        });
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
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

fn normalize_relative_path(path: &Path, directory: bool) -> String {
    let mut relative = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if directory {
        relative.push('/');
    }
    relative
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
                kind: ProjectFileReferenceKind::File,
                size_bytes: 13,
            }]
        );
    }

    #[test]
    fn lists_matching_directories_with_trailing_slash() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir.path().join("src/components/Button.tsx"), "export {}\n");
        write(&dir.path().join("src/hooks/useThing.ts"), "export {}\n");

        let refs = list_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            Some("components"),
            Some(10),
        )
        .expect("list refs");

        assert!(refs.contains(&ProjectFileReference {
            path: "src/components/".to_string(),
            display_path: "src/components/".to_string(),
            kind: ProjectFileReferenceKind::Directory,
            size_bytes: 0,
        }));
    }

    #[cfg_attr(miri, ignore)] // ponytail: Miri cannot emulate git subprocess (fork)
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
    fn serves_different_queries_from_one_walk() {
        let dir = TempDir::new().expect("tempdir");
        write(&dir.path().join("alpha.txt"), "a\n");
        write(&dir.path().join("beta.txt"), "b\n");
        let cwd = dir.path().to_str().expect("utf8 path");

        let first = list_project_file_references(cwd, Some("alpha"), Some(10)).expect("list");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].path, "alpha.txt");

        let second = list_project_file_references(cwd, Some("beta"), Some(10)).expect("list");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].path, "beta.txt");
    }
}
