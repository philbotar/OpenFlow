#![allow(clippy::derive_partial_eq_without_eq)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
}

fn resolve_home_dir() -> Option<PathBuf> {
    dirs::home_dir().or_else(|| {
        std::env::var_os("HOME").map(PathBuf::from).filter(|path| path.is_absolute())
    })
}

#[must_use]
pub fn default_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = resolve_home_dir() {
        roots.push(home.join(".cursor/skills"));
        roots.push(home.join(".cursor/skills-cursor"));
        roots.push(home.join(".claude/skills"));
        roots.push(home.join(".agents/skills"));
    }
    roots.push(
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".cursor/skills"),
    );
    roots
}

/// # Errors
/// Returns an error if a configured root cannot be read.
pub fn discover(extra_paths: &[String]) -> io::Result<Vec<SkillSummary>> {
    let mut roots = default_search_roots();
    for path in extra_paths {
        roots.push(PathBuf::from(path));
    }
    discover_from_roots(&roots)
}

/// # Errors
/// Returns an error if a configured root cannot be read.
pub fn discover_from_roots(roots: &[PathBuf]) -> io::Result<Vec<SkillSummary>> {
    let mut by_id = BTreeMap::<String, SkillSummary>::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root)
            // Skill install layouts commonly symlink ~/.claude/skills/* -> ~/.agents/skills/*.
            .follow_links(true)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path.file_name().and_then(|name| name.to_str()) != Some(SKILL_FILE_NAME) {
                continue;
            }
            if let Some(summary) = parse_skill_file(path) {
                by_id.insert(summary.id.clone(), summary);
            }
        }
    }

    Ok(by_id.into_values().collect())
}

fn parse_skill_file(path: &Path) -> Option<SkillSummary> {
    let content = fs::read_to_string(path).ok()?;
    let folder_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("skill")
        .to_string();
    let frontmatter = parse_frontmatter(&content);
    let display_name = frontmatter
        .as_ref()
        .and_then(|meta| meta.name.clone())
        .unwrap_or_else(|| folder_name.clone());
    // Slash commands use folder basename (/caveman), not display labels.
    let id = folder_name.clone();
    let name = display_name;
    let description = frontmatter
        .and_then(|meta| meta.description)
        .unwrap_or_default();

    Some(SkillSummary {
        id,
        name,
        description,
        path: Some(path.display().to_string()),
    })
}

fn parse_frontmatter(content: &str) -> Option<SkillFrontmatter> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let yaml = rest[..end].trim();
    if yaml.is_empty() {
        return None;
    }
    serde_yaml::from_str(yaml).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_skill(dir: &Path, folder: &str, body: &str) -> PathBuf {
        let skill_dir = dir.join(folder);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        let path = skill_dir.join(SKILL_FILE_NAME);
        fs::write(&path, body).expect("write skill");
        path
    }

    #[test]
    fn discovers_skills_from_configured_roots() {
        let root = TempDir::new().expect("temp dir");
        write_skill(
            root.path(),
            "brainstorming",
            "---\nname: brainstorming\ndescription: Explore ideas before building.\n---\n\nBody.",
        );

        let skills = discover_from_roots(&[root.path().to_path_buf()]).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "brainstorming");
        assert_eq!(skills[0].description, "Explore ideas before building.");
    }

    #[test]
    fn resolves_id_from_folder_when_frontmatter_name_missing() {
        let root = TempDir::new().expect("temp dir");
        write_skill(root.path(), "systematic-debugging", "# No frontmatter");

        let skills = discover_from_roots(&[root.path().to_path_buf()]).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "systematic-debugging");
        assert_eq!(skills[0].name, "systematic-debugging");
        assert_eq!(skills[0].description, "");
    }

    #[test]
    fn parses_folded_description_frontmatter() {
        let root = TempDir::new().expect("temp dir");
        write_skill(
            root.path(),
            "caveman",
            "---\nname: caveman\ndescription: >\n  Ultra-compressed communication mode.\n---\n",
        );

        let skills = discover_from_roots(&[root.path().to_path_buf()]).expect("discover");
        assert_eq!(skills[0].description.trim(), "Ultra-compressed communication mode.");
    }

    #[test]
    fn later_roots_override_earlier_ids() {
        let low = TempDir::new().expect("low priority");
        let high = TempDir::new().expect("high priority");
        write_skill(
            low.path(),
            "browser",
            "---\nname: browser\ndescription: Old browser skill.\n---\n",
        );
        write_skill(
            high.path(),
            "browser",
            "---\nname: browser\ndescription: New browser skill.\n---\n",
        );

        let skills = discover_from_roots(&[low.path().to_path_buf(), high.path().to_path_buf()])
            .expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "New browser skill.");
    }

    #[test]
    fn ignores_missing_roots() {
        let skills =
            discover_from_roots(&[PathBuf::from("/tmp/does-not-exist-skill-root")]).expect("discover");
        assert!(skills.is_empty());
    }

    #[test]
    fn discovers_skills_through_symlinked_directories() {
        let root = TempDir::new().expect("temp dir");
        let target = root.path().join("target-skills/caveman");
        fs::create_dir_all(&target).expect("create target");
        fs::write(
            target.join(SKILL_FILE_NAME),
            "---\nname: caveman\ndescription: Terse mode.\n---\n",
        )
        .expect("write skill");

        let link_parent = root.path().join("linked-skills");
        fs::create_dir_all(&link_parent).expect("create link parent");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&target, link_parent.join("caveman")).expect("symlink");
        }
        #[cfg(not(unix))]
        {
            return;
        }

        let skills = discover_from_roots(&[link_parent]).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "caveman");
        assert_eq!(skills[0].description, "Terse mode.");
    }
}
