//! Pure abs→rel rewrite for tool-call path args under a known root.
//! No filesystem I/O — string prefix only (engine stays pure).

use serde_json::Value;

#[cfg(test)]
use serde_json::json;

/// Relativize `path` / `paths` / `cwd` string fields when they are absolute under `root`.
/// Leaves relative paths, URLs, `artifact:…`, and out-of-root absolutes unchanged.
#[must_use]
pub fn relativize_tool_call_arguments(arguments: Value, root: Option<&str>) -> Value {
    let Some(root) = root.map(str::trim).filter(|r| !r.is_empty()) else {
        return arguments;
    };
    let Value::Object(mut map) = arguments else {
        return arguments;
    };
    for key in ["path", "cwd"] {
        if let Some(Value::String(s)) = map.get(key).cloned() {
            map.insert(key.to_string(), Value::String(relativize_path_string(&s, root)));
        }
    }
    if let Some(paths_val) = map.get("paths").cloned() {
        map.insert("paths".to_string(), relativize_paths_value(paths_val, root));
    }
    Value::Object(map)
}

fn relativize_paths_value(value: Value, root: &str) -> Value {
    match value {
        Value::String(s) => Value::String(relativize_path_string(&s, root)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| match item {
                    Value::String(s) => Value::String(relativize_path_string(&s, root)),
                    other => other,
                })
                .collect(),
        ),
        other => other,
    }
}

fn relativize_path_string(raw: &str, root: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return raw.to_string();
    }
    if looks_like_url_or_artifact(trimmed) {
        return raw.to_string();
    }
    let path = normalize_separators(trimmed);
    if !is_absolute_path(&path) {
        return raw.to_string();
    }
    let root_norm = trim_trailing_slashes(&normalize_separators(root));
    if root_norm.is_empty() {
        return raw.to_string();
    }
    if path == root_norm {
        return ".".to_string();
    }
    let prefix = format!("{root_norm}/");
    if let Some(rest) = path.strip_prefix(&prefix) {
        if rest.is_empty() {
            return ".".to_string();
        }
        return rest.to_string();
    }
    raw.to_string()
}

fn looks_like_url_or_artifact(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("artifact:")
}

fn normalize_separators(s: &str) -> String {
    s.replace('\\', "/")
}

fn trim_trailing_slashes(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

fn is_absolute_path(s: &str) -> bool {
    s.starts_with('/') || (s.len() >= 3 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/Users/philipbotar/Developer/DailyPlanner";

    #[test]
    fn abs_under_root_becomes_relative() {
        let args = json!({"path": "/Users/philipbotar/Developer/DailyPlanner/package.json"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["path"], "package.json");
    }

    #[test]
    fn abs_outside_root_unchanged() {
        let args = json!({"path": "/tmp/other/file.txt"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["path"], "/tmp/other/file.txt");
    }

    #[test]
    fn relative_unchanged() {
        let args = json!({"path": "src/App.tsx"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["path"], "src/App.tsx");
    }

    #[test]
    fn url_and_artifact_untouched() {
        let args = json!({
            "path": "https://example.com/a",
        });
        assert_eq!(
            relativize_tool_call_arguments(args, Some(ROOT))["path"],
            "https://example.com/a"
        );
        let args = json!({"path": "artifact:abc-123"});
        assert_eq!(
            relativize_tool_call_arguments(args, Some(ROOT))["path"],
            "artifact:abc-123"
        );
    }

    #[test]
    fn paths_array_and_string() {
        let args = json!({
            "paths": [
                "/Users/philipbotar/Developer/DailyPlanner/a.rs",
                "rel.rs",
                "/tmp/x"
            ]
        });
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(
            out["paths"],
            json!(["a.rs", "rel.rs", "/tmp/x"])
        );

        let args = json!({"paths": "/Users/philipbotar/Developer/DailyPlanner/src"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["paths"], "src");
    }

    #[test]
    fn cwd_field_and_root_equals_dot() {
        let args = json!({"cwd": "/Users/philipbotar/Developer/DailyPlanner/crates/ui"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["cwd"], "crates/ui");

        let args = json!({"path": "/Users/philipbotar/Developer/DailyPlanner"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["path"], ".");

        let args = json!({"path": "/Users/philipbotar/Developer/DailyPlanner/"});
        let out = relativize_tool_call_arguments(args, Some(ROOT));
        assert_eq!(out["path"], ".");
    }

    #[test]
    fn no_root_is_noop() {
        let args = json!({"path": "/Users/philipbotar/Developer/DailyPlanner/x"});
        let out = relativize_tool_call_arguments(args.clone(), None);
        assert_eq!(out, args);
        let out = relativize_tool_call_arguments(args.clone(), Some("  "));
        assert_eq!(out, args);
    }

    #[test]
    fn trailing_slash_on_root_and_windows_separators() {
        let args_win = json!({"path": r"C:\proj\src\a.ts"});
        let out = relativize_tool_call_arguments(args_win, Some(r"C:\proj"));
        assert_eq!(out["path"], "src/a.ts");

        let args = json!({"path": "/Users/philipbotar/Developer/DailyPlanner/x"});
        let out = relativize_tool_call_arguments(
            args,
            Some("/Users/philipbotar/Developer/DailyPlanner/"),
        );
        assert_eq!(out["path"], "x");
    }
}
