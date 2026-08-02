use crate::mcp::model::{McpInstall, McpInstallHistory, McpInstallRevision, McpServerRecord};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInstallPlan {
    pub executable: String,
    pub args: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub target_dir: PathBuf,
    pub display_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackageInstallPlanError {
    #[error("external MCP servers do not have a package install plan")]
    External,
    #[error("invalid MCP package identifier")]
    InvalidPackage,
}

pub fn installed_connection(
    install: &McpInstall,
    plan: &PackageInstallPlan,
    package_args: Vec<String>,
    environment: BTreeMap<String, crate::mcp::model::PersistedValue>,
) -> Result<crate::mcp::model::McpConnection, PackageInstallPlanError> {
    match install {
        McpInstall::External => Err(PackageInstallPlanError::External),
        McpInstall::Npm { package, .. } => {
            let mut args = vec![
                "exec".to_string(),
                "--prefix".to_string(),
                plan.target_dir.display().to_string(),
                "--offline".to_string(),
                "--".to_string(),
                package.clone(),
            ];
            args.extend(package_args);
            Ok(crate::mcp::model::McpConnection::Stdio {
                command: "npm".to_string(),
                args,
                environment,
            })
        }
        McpInstall::Pypi {
            package,
            executable,
            ..
        } => {
            let executable = executable.as_deref().unwrap_or(package);
            validate_pypi_package(executable)?;
            Ok(crate::mcp::model::McpConnection::Stdio {
                command: plan
                    .target_dir
                    .join("bin")
                    .join(executable)
                    .display()
                    .to_string(),
                args: package_args,
                environment,
            })
        }
    }
}

#[must_use]
pub fn package_install_target(
    root: &Path,
    server_id: &str,
    family: &str,
    revision: &str,
) -> PathBuf {
    let server_digest = Sha256::digest(server_id.as_bytes());
    let revision_digest = Sha256::digest(revision.as_bytes());
    root.join(family)
        .join(format!("{server_digest:x}"))
        .join(format!("{revision_digest:x}"))
}

pub fn package_install_plan(
    server_id: &str,
    install: &McpInstall,
    root: &Path,
) -> Result<PackageInstallPlan, PackageInstallPlanError> {
    match install {
        McpInstall::External => Err(PackageInstallPlanError::External),
        McpInstall::Npm { package, version } => {
            validate_npm_package(package)?;
            let target_dir =
                package_install_target(root, server_id, "npm", &format!("{package}@{version}"));
            let args = vec![
                "install".to_string(),
                "--prefix".to_string(),
                target_dir.display().to_string(),
                "--no-save".to_string(),
                "--no-audit".to_string(),
                "--no-fund".to_string(),
                "--".to_string(),
                format!("{package}@{version}"),
            ];
            Ok(PackageInstallPlan {
                executable: "npm".to_string(),
                display_command: render_command("npm", &args),
                args,
                environment: BTreeMap::new(),
                target_dir,
            })
        }
        McpInstall::Pypi {
            package, version, ..
        } => {
            validate_pypi_package(package)?;
            let target_dir =
                package_install_target(root, server_id, "pypi", &format!("{package}=={version}"));
            let args = vec![
                "tool".to_string(),
                "install".to_string(),
                "--force".to_string(),
                "--".to_string(),
                format!("{package}=={version}"),
            ];
            let environment = BTreeMap::from([
                (
                    "UV_TOOL_DIR".to_string(),
                    target_dir.join("tools").display().to_string(),
                ),
                (
                    "UV_TOOL_BIN_DIR".to_string(),
                    target_dir.join("bin").display().to_string(),
                ),
            ]);
            Ok(PackageInstallPlan {
                executable: "uv".to_string(),
                display_command: render_command("uv", &args),
                args,
                environment,
                target_dir,
            })
        }
    }
}

pub fn record_install_success(
    record: &mut McpServerRecord,
    target_dir: PathBuf,
    installed_at: DateTime<Utc>,
) {
    let current = McpInstallRevision {
        install: record.install.clone(),
        connection: record.connection.clone(),
        installed_at,
        target_dir,
    };
    record.enabled = false;
    record.trust = Default::default();
    record.install_history = Some(McpInstallHistory {
        previous: record.install_history.take().map(|history| history.current),
        current,
    });
}

pub fn rollback_install(record: &mut McpServerRecord) -> bool {
    let Some(history) = record.install_history.as_mut() else {
        return false;
    };
    let Some(previous) = history.previous.take() else {
        return false;
    };
    let replaced = std::mem::replace(&mut history.current, previous);
    history.previous = Some(replaced);
    record.install.clone_from(&history.current.install);
    record.connection.clone_from(&history.current.connection);
    record.enabled = false;
    record.trust = Default::default();
    true
}

fn validate_npm_package(package: &str) -> Result<(), PackageInstallPlanError> {
    let valid = !package.is_empty()
        && !package.starts_with('-')
        && !package.chars().any(char::is_whitespace)
        && package.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'/' | b'-' | b'_' | b'.')
        })
        && (!package.starts_with('@') || package.split_once('/').is_some());
    if valid {
        Ok(())
    } else {
        Err(PackageInstallPlanError::InvalidPackage)
    }
}

fn validate_pypi_package(package: &str) -> Result<(), PackageInstallPlanError> {
    let valid = package.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'-' | b'_' | b'.'))
    });
    if !package.is_empty() && valid {
        Ok(())
    } else {
        Err(PackageInstallPlanError::InvalidPackage)
    }
}

fn render_command(executable: &str, args: &[String]) -> String {
    std::iter::once(executable)
        .chain(args.iter().map(String::as_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'@' | b'_' | b'-' | b'.' | b'=')
    }) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::model::{
        ExactPackageVersion, McpConnection, McpInstall, McpServerSource, MCP_SERVER_RECORD_VERSION,
    };

    fn npm(version: &str) -> McpInstall {
        McpInstall::Npm {
            package: "@modelcontextprotocol/server-filesystem".to_string(),
            version: ExactPackageVersion::new(version).unwrap(),
        }
    }

    #[test]
    fn npm_plan_is_exact_isolated_and_shell_free() {
        let root = Path::new("/tmp/openflow-mcp-installs");
        let plan = package_install_plan("filesystem", &npm("1.2.3"), root).unwrap();

        assert_eq!(plan.executable, "npm");
        assert_eq!(
            plan.args.last().unwrap(),
            "@modelcontextprotocol/server-filesystem@1.2.3"
        );
        assert!(plan.args.iter().any(|arg| arg == "--prefix"));
        assert!(plan.target_dir.starts_with(root.join("npm")));
        assert!(!plan.display_command.contains("latest"));
    }

    #[test]
    fn installed_npm_connection_uses_only_the_pinned_local_tree() {
        let root = Path::new("/tmp/openflow-mcp-installs");
        let install = npm("1.2.3");
        let plan = package_install_plan("filesystem", &install, root).unwrap();

        let connection = installed_connection(
            &install,
            &plan,
            vec!["/workspace".to_string()],
            BTreeMap::new(),
        )
        .unwrap();

        let McpConnection::Stdio { command, args, .. } = connection else {
            panic!("stdio connection");
        };
        assert_eq!(command, "npm");
        assert_eq!(
            &args[..5],
            [
                "exec",
                "--prefix",
                plan.target_dir.to_str().unwrap(),
                "--offline",
                "--"
            ]
        );
        assert_eq!(args[5], "@modelcontextprotocol/server-filesystem");
        assert_eq!(args[6], "/workspace");
    }

    #[test]
    fn package_versions_use_distinct_install_directories_for_rollback() {
        let root = Path::new("/tmp/openflow-mcp-installs");
        let old = package_install_plan("filesystem", &npm("1.0.0"), root).unwrap();
        let new = package_install_plan("filesystem", &npm("2.0.0"), root).unwrap();

        assert_ne!(old.target_dir, new.target_dir);
        assert_eq!(old.target_dir.parent(), new.target_dir.parent());
    }

    #[test]
    fn pypi_plan_uses_exact_uv_tool_dirs() {
        let plan = package_install_plan(
            "weather",
            &McpInstall::Pypi {
                package: "weather-mcp".to_string(),
                version: ExactPackageVersion::new("2026.8.1").unwrap(),
                executable: Some("weather-mcp".to_string()),
            },
            Path::new("/tmp/installs"),
        )
        .unwrap();

        assert_eq!(plan.executable, "uv");
        assert_eq!(plan.args.last().unwrap(), "weather-mcp==2026.8.1");
        assert!(plan.environment["UV_TOOL_DIR"].contains("/pypi/"));
        assert!(plan.environment["UV_TOOL_BIN_DIR"].ends_with("/bin"));
    }

    #[test]
    fn package_names_cannot_inject_options_or_shell_syntax() {
        for package in ["--help", "pkg;touch /tmp/pwn", "@missing-scope"] {
            let install = McpInstall::Npm {
                package: package.to_string(),
                version: ExactPackageVersion::new("1.0.0").unwrap(),
            };
            assert_eq!(
                package_install_plan("bad", &install, Path::new("/tmp")),
                Err(PackageInstallPlanError::InvalidPackage)
            );
        }
    }

    #[test]
    fn install_history_rolls_config_back_and_revokes_trust() {
        let mut record = McpServerRecord::new(
            "filesystem",
            "Filesystem",
            McpServerSource::Manual,
            npm("2.0.0"),
            McpConnection::Stdio {
                command: "new-command".to_string(),
                args: Vec::new(),
                environment: BTreeMap::new(),
            },
        );
        assert_eq!(record.schema_version, MCP_SERVER_RECORD_VERSION);
        let old_revision = McpInstallRevision {
            install: npm("1.0.0"),
            connection: McpConnection::Stdio {
                command: "old-command".to_string(),
                args: Vec::new(),
                environment: BTreeMap::new(),
            },
            installed_at: Utc::now(),
            target_dir: PathBuf::from("/tmp/old"),
        };
        record.install_history = Some(McpInstallHistory {
            current: old_revision,
            previous: None,
        });
        record_install_success(&mut record, PathBuf::from("/tmp/new"), Utc::now());

        assert!(rollback_install(&mut record));
        assert_eq!(
            record.install_history.as_ref().unwrap().current.target_dir,
            PathBuf::from("/tmp/old")
        );
        let McpConnection::Stdio { command, .. } = &record.connection else {
            panic!("stdio");
        };
        assert_eq!(command, "old-command");
        assert!(!record.enabled);
        assert!(record.trust.approved_fingerprint.is_none());
    }
}
