use ignore::WalkBuilder;
use std::path::Path;

use crate::tool::errors::ToolError;

const MAX_DIRECTORY_ENTRIES: usize = 200;
const MAX_DIRECTORY_DEPTH: usize = 1;

pub fn render_directory_listing(path: &Path, label: &str) -> Result<String, ToolError> {
    let mut lines = Vec::new();
    let max_components = MAX_DIRECTORY_DEPTH + 1;
    let mut builder = WalkBuilder::new(path);
    builder.standard_filters(true).follow_links(false);

    for entry in builder.build() {
        let entry = entry.map_err(|error| {
            ToolError::failed(format!("read failed for {}: {error}", path.display()))
        })?;
        let entry_path = entry.path();
        if entry_path == path {
            continue;
        }
        let relative = entry_path.strip_prefix(path).unwrap_or(entry_path);
        if relative.components().count() > max_components {
            continue;
        }
        let display = relative.to_string_lossy().replace('\\', "/");
        let suffix = if entry_path.is_dir() { "/" } else { "" };
        lines.push(format!("{display}{suffix}"));
    }

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
