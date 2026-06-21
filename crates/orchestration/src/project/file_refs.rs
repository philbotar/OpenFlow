use crate::api::{ProjectFileReference, ProjectFileReferenceContent, ProjectFileReferenceKind};
use crate::error::BackendError;
use ignore::WalkBuilder;
use std::fs;
use std::path::{Component, Path, PathBuf};

const DEFAULT_FILE_REFERENCE_LIMIT: usize = 30;
const DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES: u64 = 64 * 1024;
const DEFAULT_FILE_REFERENCE_MAX_FILES: usize = 5;
const DEFAULT_DIRECTORY_REFERENCE_MAX_ENTRIES: usize = 200;
const DEFAULT_DIRECTORY_REFERENCE_MAX_TEXT_FILES: usize = 20;
const DEFAULT_DIRECTORY_REFERENCE_TOTAL_READ_LIMIT_BYTES: usize = 128 * 1024;

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
        if path == cwd {
            continue;
        }
        let Ok(relative) = path.strip_prefix(&cwd) else {
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
        if !matches_query(&relative, normalized_query.as_deref()) {
            continue;
        }
        matches.push(ProjectFileReference {
            display_path: relative.clone(),
            path: relative,
            kind,
            size_bytes: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
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
        let requested_directory = path.trim().ends_with('/') || path.trim().ends_with('\\');
        let relative = normalize_user_relative_path(path)?;
        let absolute = resolve_existing_path(&cwd, &relative)?;
        let metadata = fs::metadata(&absolute)
            .map_err(|error| BackendError::ProjectOperation(format!("read {relative}: {error}")))?;
        if requested_directory && !metadata.is_dir() {
            return Err(BackendError::ProjectOperation(format!(
                "file reference is not a directory: {relative}"
            )));
        }
        if metadata.is_dir() {
            refs.push(read_directory_reference(&cwd, &absolute, &relative)?);
        } else if metadata.is_file() {
            refs.push(read_file_reference(&absolute, &relative)?);
        } else {
            return Err(BackendError::ProjectOperation(format!(
                "file reference is neither a file nor a directory: {relative}"
            )));
        }
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

fn ensure_trailing_slash(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

fn resolve_existing_path(cwd: &Path, relative: &str) -> Result<PathBuf, BackendError> {
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

fn read_file_reference(
    absolute: &Path,
    relative: &str,
) -> Result<ProjectFileReferenceContent, BackendError> {
    let metadata = fs::metadata(absolute)
        .map_err(|error| BackendError::ProjectOperation(format!("read {relative}: {error}")))?;
    let bytes = fs::read(absolute)
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

    Ok(ProjectFileReferenceContent {
        path: relative.to_string(),
        kind: ProjectFileReferenceKind::File,
        content,
        truncated,
        size_bytes: metadata.len(),
    })
}

fn read_directory_reference(
    cwd: &Path,
    absolute: &Path,
    relative: &str,
) -> Result<ProjectFileReferenceContent, BackendError> {
    let label = ensure_trailing_slash(relative);
    let mut tree_lines = vec!["Directory tree:".to_string(), label.clone()];
    let mut file_sections = Vec::new();
    let mut text_file_count = 0usize;
    let mut bytes_read = 0usize;
    let mut truncated = false;
    let mut entry_count = 0usize;

    let mut builder = WalkBuilder::new(absolute);
    builder.standard_filters(true).follow_links(false);

    for entry in builder.build() {
        let entry = entry.map_err(|error| BackendError::ProjectOperation(error.to_string()))?;
        let path = entry.path();
        if path == absolute {
            continue;
        }
        entry_count += 1;
        if entry_count > DEFAULT_DIRECTORY_REFERENCE_MAX_ENTRIES {
            truncated = true;
            tree_lines.push(format!(
                "... truncated after {DEFAULT_DIRECTORY_REFERENCE_MAX_ENTRIES} entries"
            ));
            break;
        }

        let Ok(local_relative) = path.strip_prefix(absolute) else {
            continue;
        };
        let depth = local_relative.components().count().saturating_sub(1);
        let indent = "  ".repeat(depth);
        let file_name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        let Ok(global_relative_path) = path.strip_prefix(cwd) else {
            continue;
        };
        let file_type = entry.file_type();

        if file_type.is_some_and(|kind| kind.is_dir()) {
            tree_lines.push(format!("{indent}- {file_name}/"));
            continue;
        }
        if !file_type.is_some_and(|kind| kind.is_file()) {
            continue;
        }

        tree_lines.push(format!("{indent}- {file_name}"));
        let global_relative = normalize_relative_path(global_relative_path, false);
        if text_file_count >= DEFAULT_DIRECTORY_REFERENCE_MAX_TEXT_FILES {
            truncated = true;
            continue;
        }
        if bytes_read >= DEFAULT_DIRECTORY_REFERENCE_TOTAL_READ_LIMIT_BYTES {
            truncated = true;
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| BackendError::ProjectOperation(error.to_string()))?;
        let remaining = DEFAULT_DIRECTORY_REFERENCE_TOTAL_READ_LIMIT_BYTES - bytes_read;
        let read_limit = remaining.min(DEFAULT_FILE_REFERENCE_READ_LIMIT_BYTES as usize);
        let bytes = fs::read(path).map_err(|error| {
            BackendError::ProjectOperation(format!("read {global_relative}: {error}"))
        })?;
        let file_truncated = bytes.len() > read_limit || metadata.len() as usize > read_limit;
        let bytes = bytes.into_iter().take(read_limit).collect::<Vec<_>>();
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(_) => {
                file_sections.push(format!("Skipped binary file: {global_relative}"));
                continue;
            }
        };
        bytes_read += content.len();
        text_file_count += 1;
        if file_truncated {
            truncated = true;
        }
        let header = if file_truncated {
            format!(
                "File: {global_relative} (truncated at {read_limit} bytes of {})",
                metadata.len()
            )
        } else {
            format!("File: {global_relative}")
        };
        file_sections.push(format!(
            "{header}\n```text\n{}\n```",
            strip_trailing_newline(&content)
        ));
    }

    if text_file_count >= DEFAULT_DIRECTORY_REFERENCE_MAX_TEXT_FILES {
        truncated = true;
        file_sections.push(format!(
            "Skipped additional text files after {DEFAULT_DIRECTORY_REFERENCE_MAX_TEXT_FILES} files."
        ));
    }

    let mut sections = tree_lines;
    if !file_sections.is_empty() {
        sections.push(String::new());
        sections.extend(file_sections);
    }

    Ok(ProjectFileReferenceContent {
        path: label,
        kind: ProjectFileReferenceKind::Directory,
        content: sections.join("\n"),
        truncated,
        size_bytes: bytes_read as u64,
    })
}

fn strip_trailing_newline(value: &str) -> &str {
    value.strip_suffix('\n').unwrap_or(value)
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
                kind: ProjectFileReferenceKind::File,
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

    #[test]
    fn reads_directory_reference_with_tree_and_file_contents() {
        let dir = TempDir::new().expect("tempdir");
        write(
            &dir.path().join("src/components/Button.tsx"),
            "export function Button() {}\n",
        );
        write(
            &dir.path().join("src/components/nested/Card.tsx"),
            "export function Card() {}\n",
        );
        fs::write(
            dir.path().join("src/components/logo.bin"),
            [0xff, 0xfe, 0xfd],
        )
        .expect("write binary");

        let refs = read_project_file_references(
            dir.path().to_str().expect("utf8 path"),
            &[String::from("src/components/")],
        )
        .expect("read refs");

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path, "src/components/");
        assert_eq!(refs[0].kind, ProjectFileReferenceKind::Directory);
        assert!(refs[0].content.contains("Directory tree:"));
        assert!(refs[0].content.contains("src/components/"));
        assert!(refs[0].content.contains("- Button.tsx"));
        assert!(refs[0].content.contains("File: src/components/Button.tsx"));
        assert!(refs[0].content.contains("export function Button() {}"));
        assert!(refs[0]
            .content
            .contains("Skipped binary file: src/components/logo.bin"));
        assert!(!refs[0].truncated);
    }
}
