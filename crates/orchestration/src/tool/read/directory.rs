use std::fs;
use std::path::Path;

use crate::tool::errors::ToolError;

const MAX_DIRECTORY_ENTRIES: usize = 200;
const MAX_DIRECTORY_DEPTH: usize = 1;

pub fn render_directory_listing(path: &Path, label: &str) -> Result<String, ToolError> {
    let mut lines = Vec::new();
    collect_entries(path, path, 0, &mut lines)?;
    lines.sort();
    let total = lines.len();
    let shown: Vec<String> = lines.into_iter().take(MAX_DIRECTORY_ENTRIES).collect();
    let mut output = format!("¶{label} (directory)\n{}", shown.join("\n"));
    if total > MAX_DIRECTORY_ENTRIES {
        output.push_str(&format!(
            "\n… truncated at {MAX_DIRECTORY_ENTRIES} of {total} entries …"
        ));
    }
    Ok(output)
}

fn collect_entries(
    root: &Path,
    current: &Path,
    depth: usize,
    lines: &mut Vec<String>,
) -> Result<(), ToolError> {
    if depth > MAX_DIRECTORY_DEPTH {
        return Ok(());
    }
    let entries = fs::read_dir(current).map_err(|error| map_directory_error(current, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| map_directory_error(current, &error))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let suffix = if path.is_dir() { "/" } else { "" };
        lines.push(format!("{relative}{suffix}"));
        if path.is_dir() && depth < MAX_DIRECTORY_DEPTH {
            collect_entries(root, &path, depth + 1, lines)?;
        }
    }
    Ok(())
}

fn map_directory_error(path: &Path, error: &std::io::Error) -> ToolError {
    ToolError::failed(format!("read failed for {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::render_directory_listing;
    use std::fs;

    #[test]
    fn directory_listing_is_sorted_and_depth_limited() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "").unwrap();
        fs::write(dir.path().join("src/nested/deep.rs"), "").unwrap();
        fs::write(dir.path().join("README.md"), "").unwrap();
        let rendered = render_directory_listing(dir.path(), "project").unwrap();
        assert!(rendered.contains("README.md"));
        assert!(rendered.contains("src/a.rs"));
        assert!(rendered.contains("src/nested/"));
        assert!(!rendered.contains("deep.rs"));
    }
}
