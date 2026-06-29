use crate::api::{DebugLogEntry, DebugLogWrite};
use crate::error::BackendError;
use crate::settings::model::AppSettings;
use chrono::Utc;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DebugLogRecord<'a> {
    timestamp: String,
    pid: u32,
    level: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<&'a str>,
    message: &'a str,
}

#[must_use]
pub fn debug_log_path() -> PathBuf {
    std::env::temp_dir().join(format!("openflow-debug-{}.jsonl", std::process::id()))
}

/// # Errors
/// Returns an error if diagnostics are enabled and the temp log cannot be appended.
pub fn append_debug_log(
    settings: &AppSettings,
    entry: &DebugLogEntry,
) -> Result<DebugLogWrite, BackendError> {
    if !settings.local_diagnostics.debug_output {
        return Ok(DebugLogWrite {
            enabled: false,
            path: None,
        });
    }

    let path = debug_log_path();
    append_debug_log_to(&path, entry)?;
    Ok(DebugLogWrite {
        enabled: true,
        path: Some(path.display().to_string()),
    })
}

fn append_debug_log_to(path: &Path, entry: &DebugLogEntry) -> Result<(), BackendError> {
    let record = DebugLogRecord {
        timestamp: Utc::now().to_rfc3339(),
        pid: std::process::id(),
        level: entry.level.trim(),
        context: entry
            .context
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        message: entry.message.as_str(),
    };
    let line =
        serde_json::to_string(&record).map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_debug_log_skips_when_disabled() {
        let settings = AppSettings::default();
        let entry = DebugLogEntry {
            level: "error".to_string(),
            message: "failure".to_string(),
            context: None,
        };

        let result = append_debug_log(&settings, &entry).unwrap();

        assert!(!result.enabled);
        assert_eq!(result.path, None);
    }

    #[test]
    fn append_debug_log_to_writes_jsonl_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openflow-debug-test.jsonl");
        let entry = DebugLogEntry {
            level: "error".to_string(),
            message: "Bedrock failed".to_string(),
            context: Some("Refresh from AWS".to_string()),
        };

        append_debug_log_to(&path, &entry).unwrap();

        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"level\":\"error\""));
        assert!(text.contains("\"context\":\"Refresh from AWS\""));
        assert!(text.contains("\"message\":\"Bedrock failed\""));
    }
}
