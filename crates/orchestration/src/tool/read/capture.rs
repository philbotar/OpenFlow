use engine::ReadRecord;
use std::fs;
use std::path::{Path, PathBuf};

use super::selector::{split_selector, ReadSelector};
use super::summary::{should_summarize_path, structural_summary};

/// Build a read record from a local read tool call and file bytes.
#[must_use]
pub fn capture_read_record(cwd: &Path, path_arg: &str) -> Option<ReadRecord> {
    let (path, selector) = split_selector(path_arg);
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("artifact:")
    {
        return None;
    }
    if !matches!(selector, ReadSelector::None) {
        return Some(ReadRecord {
            path,
            outline: None,
        });
    }
    let absolute = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        cwd.join(&path)
    };
    let metadata = fs::metadata(&absolute).ok()?;
    if !metadata.is_file() {
        return Some(ReadRecord {
            path,
            outline: None,
        });
    }
    let text = fs::read_to_string(&absolute).ok()?;
    let outline = if should_summarize_path(&path) {
        structural_summary(&path, &text)
    } else {
        None
    };
    Some(ReadRecord { path, outline })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn capture_read_record_includes_structural_outline_for_code_files() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("lib.rs");
        let mut handle = std::fs::File::create(&file).expect("create");
        writeln!(handle, "pub fn hello() {{}}").expect("write");
        let record = capture_read_record(dir.path(), "lib.rs").expect("record");
        assert_eq!(record.path, "lib.rs");
        assert!(record
            .outline
            .as_deref()
            .is_some_and(|o| o.contains("pub fn hello")));
    }
}
