use crate::mcp::model::{McpConnection, McpTransportKind};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum McpPreflight {
    Ready {
        executable: PathBuf,
    },
    RemoteReady {
        endpoint: String,
    },
    Missing {
        command: String,
        searched_paths: Vec<PathBuf>,
    },
    UnsupportedTransport {
        transport: McpTransportKind,
    },
    InvalidRemote {
        reason: String,
    },
}

/// Resolves a stdio executable without spawning it or performing installation.
///
/// `path` is injected to keep the check deterministic. Pass the effective `PATH`
/// value from the caller when resolving a bare command name.
#[must_use]
pub fn preflight(connection: &McpConnection, path: Option<&OsStr>) -> McpPreflight {
    let command = match connection {
        McpConnection::Stdio { command, .. } => command,
        McpConnection::StreamableHttp {
            url,
            allow_localhost,
            ..
        }
        | McpConnection::LegacySse {
            url,
            allow_localhost,
            ..
        } => {
            return crate::adapters::mcp::http_security::validate_endpoint_url(
                url,
                *allow_localhost,
            )
            .map_or_else(
                |error| McpPreflight::InvalidRemote {
                    reason: error.to_string(),
                },
                |endpoint| McpPreflight::RemoteReady {
                    endpoint: endpoint.to_string(),
                },
            );
        }
    };

    let command_path = Path::new(command);
    if command_path.is_absolute() || command_path.components().count() > 1 {
        return resolve_candidates(command_path)
            .into_iter()
            .find(|candidate| is_executable(candidate))
            .map_or_else(
                || McpPreflight::Missing {
                    command: command.clone(),
                    searched_paths: Vec::new(),
                },
                |executable| McpPreflight::Ready { executable },
            );
    }

    let searched_paths: Vec<PathBuf> = path
        .map(std::env::split_paths)
        .map(Iterator::collect)
        .unwrap_or_default();
    for directory in &searched_paths {
        let candidate = directory.join(command_path);
        if let Some(executable) = resolve_candidates(&candidate)
            .into_iter()
            .find(|candidate| is_executable(candidate))
        {
            return McpPreflight::Ready { executable };
        }
    }

    McpPreflight::Missing {
        command: command.clone(),
        searched_paths,
    }
}

fn resolve_candidates(path: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        if path.extension().is_some() {
            return vec![path.to_path_buf()];
        }
        let mut candidates = vec![path.to_path_buf()];
        for extension in ["exe", "com", "cmd", "bat"] {
            candidates.push(path.with_extension(extension));
        }
        candidates
    }
    #[cfg(not(windows))]
    {
        vec![path.to_path_buf()]
    }
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{preflight, McpPreflight};
    use crate::mcp::model::{McpAuth, McpConnection};
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn stdio(command: &Path) -> McpConnection {
        McpConnection::Stdio {
            command: command.to_string_lossy().into_owned(),
            args: Vec::new(),
            environment: BTreeMap::new(),
        }
    }

    fn create_executable(path: &Path) {
        fs::write(path, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn executable_name(stem: &str) -> String {
        if cfg!(windows) {
            format!("{stem}.cmd")
        } else {
            stem.to_string()
        }
    }

    #[test]
    fn direct_executable_path_is_ready() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join(executable_name("mcp-direct"));
        create_executable(&executable);

        assert_eq!(
            preflight(&stdio(&executable), None),
            McpPreflight::Ready { executable }
        );
    }

    #[test]
    fn executable_is_resolved_from_injected_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let executable = second.path().join(executable_name("mcp-path"));
        create_executable(&executable);
        let path = std::env::join_paths([first.path(), second.path()]).unwrap();

        assert_eq!(
            preflight(&stdio(Path::new("mcp-path")), Some(path.as_os_str())),
            McpPreflight::Ready { executable }
        );
    }

    #[test]
    fn missing_executable_reports_command_and_search_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([first.path(), second.path()]).unwrap();

        let result = preflight(
            &stdio(Path::new("not-installed-mcp")),
            Some(path.as_os_str()),
        );
        let McpPreflight::Missing {
            command,
            searched_paths,
        } = result
        else {
            panic!("missing executable must return a structured Missing result");
        };
        assert_eq!(command, "not-installed-mcp");
        assert_eq!(
            searched_paths,
            vec![PathBuf::from(first.path()), PathBuf::from(second.path())]
        );
    }

    #[test]
    fn valid_https_transport_is_ready_for_remote_connect() {
        let connection = McpConnection::StreamableHttp {
            url: "https://mcp.example.test".to_string(),
            allow_localhost: false,
            headers: BTreeMap::new(),
            auth: McpAuth::None,
        };

        assert_eq!(
            preflight(&connection, Some(OsString::new().as_os_str())),
            McpPreflight::RemoteReady {
                endpoint: "https://mcp.example.test/".to_string(),
            }
        );
    }
}
