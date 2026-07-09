//! Helpers for the `web_search` builtin, which shells out to the
//! search-cli aggregator (https://github.com/paperfoot/search-cli).

use crate::settings::model::SearchSettings;
use crate::tool::errors::ToolError;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;

static BUNDLED_SEARCH: OnceLock<PathBuf> = OnceLock::new();

/// Desktop builds register the bundled search-cli sidecar path at app startup.
pub fn set_bundled_search_binary(path: PathBuf) -> bool {
    BUNDLED_SEARCH.set(path).is_ok()
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebSearchArgs {
    pub query: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
}

pub(crate) fn parse_args(args: Value) -> Result<WebSearchArgs, ToolError> {
    serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
        tool: "web_search".to_string(),
        problem: error.to_string(),
        hint: "required field: query (string); optional: mode (string), count (integer)"
            .to_string(),
    })
}

/// CLI arguments for one invocation: `search` subcommand, `-q`, query, `--json`, optional flags.
/// search-cli 0.8.0 requires the subcommand before `-q`; bare `-q` at top level is rejected.
pub(crate) fn cli_args(args: &WebSearchArgs) -> Vec<String> {
    let mut cli = vec![
        "search".to_string(),
        "-q".to_string(),
        args.query.clone(),
        "--json".to_string(),
    ];
    if let Some(mode) = args
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|mode| !mode.is_empty())
    {
        cli.push("-m".to_string());
        cli.push(mode.to_string());
    }
    if let Some(count) = args.count {
        cli.push("-c".to_string());
        cli.push(count.to_string());
    }
    cli
}

/// Settings keys as SEARCH_KEYS_<PROVIDER> env vars. The child process also
/// inherits the parent environment, so user-exported vars (BRAVE_API_KEY,
/// SEARCH_KEYS_BRAVE, ...) keep working; search-cli treats env vars as
/// overriding its own config file.
pub(crate) fn key_env_vars(settings: &SearchSettings) -> Vec<(String, String)> {
    settings
        .keys
        .iter()
        .filter(|(_, key)| !key.trim().is_empty())
        .map(|(provider, key)| {
            (
                format!("SEARCH_KEYS_{}", provider.trim().to_uppercase()),
                key.trim().to_string(),
            )
        })
        .collect()
}

/// Resolve the search binary: explicit setting, bundled desktop sidecar, then PATH
/// and common install locations (GUI launches on macOS get a minimal PATH).
pub(crate) fn resolve_binary(settings: &SearchSettings) -> Result<PathBuf, ToolError> {
    let configured = settings.binary_path.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Ok(path);
        }
        return Err(ToolError::NotFound {
            what: format!("search-cli binary not found at configured path: {configured}"),
            hint: "fix the binary path in Settings -> Search".to_string(),
        });
    }
    if let Some(path) = BUNDLED_SEARCH.get() {
        if path.is_file() {
            return Ok(path.clone());
        }
    }
    if let Some(path) = find_on_path("search") {
        return Ok(path);
    }
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".cargo/bin/search"));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/search"));
    candidates.push(PathBuf::from("/usr/local/bin/search"));
    if let Some(path) = candidates.into_iter().find(|candidate| candidate.is_file()) {
        return Ok(path);
    }
    Err(ToolError::NotFound {
        what: "search-cli binary not found".to_string(),
        hint: "reinstall OpenFlow or set the binary path in Settings -> Search".to_string(),
    })
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Map search-cli's semantic exit codes (1 transient, 2 auth, 3 bad input,
/// 4 rate limited) to tool errors.
pub(crate) fn map_exit_failure(code: Option<i32>, stderr: &str) -> ToolError {
    let stderr = stderr.trim();
    match code {
        Some(2) => ToolError::failed(format!(
            "web_search auth error — check search keys in Settings -> Search: {stderr}"
        )),
        Some(3) => ToolError::InvalidArgs {
            tool: "web_search".to_string(),
            problem: stderr.to_string(),
            hint: "check query, mode, and count values".to_string(),
        },
        Some(4) => ToolError::failed(format!("web_search rate limited: {stderr}")),
        _ => ToolError::failed(format!("web_search failed: {stderr}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn settings_with_keys(pairs: &[(&str, &str)]) -> SearchSettings {
        SearchSettings {
            keys: pairs
                .iter()
                .map(|(provider, key)| ((*provider).to_string(), (*key).to_string()))
                .collect::<BTreeMap<_, _>>(),
            ..SearchSettings::default()
        }
    }

    #[test]
    fn parse_args_requires_query() {
        let error = parse_args(serde_json::json!({})).unwrap_err();
        assert!(matches!(error, ToolError::InvalidArgs { .. }));
        let args = parse_args(serde_json::json!({"query": "rust rfc"})).unwrap();
        assert_eq!(args.query, "rust rfc");
        assert!(args.mode.is_none());
        assert!(args.count.is_none());
    }

    #[test]
    fn cli_args_include_json_and_optional_flags() {
        let args = parse_args(serde_json::json!({
            "query": "rust rfc",
            "mode": "news",
            "count": 5
        }))
        .unwrap();
        assert_eq!(
            cli_args(&args),
            vec!["search", "-q", "rust rfc", "--json", "-m", "news", "-c", "5"]
        );
        let bare = parse_args(serde_json::json!({"query": "rust rfc", "mode": "  "})).unwrap();
        assert_eq!(cli_args(&bare), vec!["search", "-q", "rust rfc", "--json"]);
    }

    #[test]
    fn key_env_vars_upper_cases_and_skips_blank() {
        let settings = settings_with_keys(&[("brave", " bk-1 "), ("exa", "  ")]);
        assert_eq!(
            key_env_vars(&settings),
            vec![("SEARCH_KEYS_BRAVE".to_string(), "bk-1".to_string())]
        );
    }

    #[test]
    fn resolve_binary_uses_configured_binary_path() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("search");
        std::fs::write(&binary, "#!/bin/sh\n").unwrap();
        let settings = SearchSettings {
            binary_path: binary.to_string_lossy().into_owned(),
            ..SearchSettings::default()
        };
        assert_eq!(resolve_binary(&settings).unwrap(), binary);
    }

    #[test]
    fn resolve_binary_uses_registered_bundled_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("search");
        std::fs::write(&binary, "#!/bin/sh\n").unwrap();
        if !set_bundled_search_binary(binary.clone()) {
            return;
        }
        assert_eq!(resolve_binary(&SearchSettings::default()).unwrap(), binary);
    }

    #[test]
    fn resolve_binary_prefers_configured_path_and_errors_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("search");
        std::fs::write(&binary, "#!/bin/sh\n").unwrap();

        let settings = SearchSettings {
            binary_path: binary.display().to_string(),
            ..SearchSettings::default()
        };
        assert_eq!(resolve_binary(&settings).unwrap(), binary);

        let settings = SearchSettings {
            binary_path: dir.path().join("missing").display().to_string(),
            ..SearchSettings::default()
        };
        assert!(matches!(
            resolve_binary(&settings).unwrap_err(),
            ToolError::NotFound { .. }
        ));
    }

    #[test]
    fn exit_codes_map_to_helpful_errors() {
        assert!(map_exit_failure(Some(2), "brave: 401")
            .to_string()
            .contains("auth"));
        assert!(matches!(
            map_exit_failure(Some(3), "bad mode"),
            ToolError::InvalidArgs { .. }
        ));
        assert!(map_exit_failure(Some(4), "429").to_string().contains("rate limited"));
        assert!(map_exit_failure(Some(1), "timeout").to_string().contains("failed"));
    }
}
