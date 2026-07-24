//! Local JSONL dump of provider HTTP response bodies when diagnostics are on.

use bytes::Bytes;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[must_use]
pub fn debug_log_path() -> PathBuf {
    std::env::temp_dir().join(format!("openflow-debug-{}.jsonl", std::process::id()))
}

/// Append one model-response line when `enabled`. Failures are swallowed — diagnostics
/// must never break a run.
pub fn log_model_response(enabled: bool, provider: &str, status: u16, body: &Bytes) {
    if !enabled {
        return;
    }
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let path = debug_log_path();
    let _ = append_model_response(&path, provider, status, body);
}

fn append_model_response(
    path: &Path,
    provider: &str,
    status: u16,
    body: &Bytes,
) -> std::io::Result<()> {
    let payload = serde_json::to_string(&ModelResponsePayload {
        provider,
        status,
        body: body_value(body),
    })
    .map_err(std::io::Error::other)?;
    let record = DebugLogRecord {
        timestamp: now_rfc3339(),
        pid: std::process::id(),
        level: "info",
        context: "model-response",
        message: &payload,
    };
    let line = serde_json::to_string(&record).map_err(std::io::Error::other)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn body_value(body: &Bytes) -> serde_json::Value {
    serde_json::from_slice::<serde_json::Value>(body)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(body).into_owned()))
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    // ponytail: second precision is enough for local JSONL; chrono if subsecond needed
    format!("{secs}")
}

#[derive(Serialize)]
struct DebugLogRecord<'a> {
    timestamp: String,
    pid: u32,
    level: &'a str,
    context: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct ModelResponsePayload<'a> {
    provider: &'a str,
    status: u16,
    body: serde_json::Value,
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "unit tests assert log shapes with expect/unwrap"
)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;

    #[test]
    fn disabled_is_noop() {
        // Gate short-circuits before any I/O.
        log_model_response(false, "Test", 200, &Bytes::from_static(b"{}"));
    }

    #[test]
    fn append_writes_parseable_jsonl_with_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug.jsonl");
        append_model_response(
            &path,
            "Custom OpenAI-compatible API",
            200,
            &Bytes::from(r#"{"choices":[{"message":{"content":null}}]}"#),
        )
        .unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let line = text.lines().next().unwrap();
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(record["context"], "model-response");
        assert_eq!(record["level"], "info");
        let message: serde_json::Value =
            serde_json::from_str(record["message"].as_str().unwrap()).unwrap();
        assert_eq!(message["provider"], "Custom OpenAI-compatible API");
        assert_eq!(message["status"], 200);
        assert!(message["body"]["choices"].is_array());
    }

    #[test]
    fn concurrent_appends_stay_parseable() {
        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().join("debug.jsonl"));
        let mut handles = Vec::new();
        for i in 0..8 {
            let path = Arc::clone(&path);
            handles.push(std::thread::spawn(move || {
                let body = Bytes::from(format!(r#"{{"n":{i}}}"#));
                let _guard = WRITE_LOCK
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                append_model_response(&path, "Test", 200, &body).unwrap();
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let text = fs::read_to_string(&*path).unwrap();
        let lines: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines.len(), 8);
        for line in lines {
            let _: serde_json::Value = serde_json::from_str(line).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn created_file_is_owner_rw_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug.jsonl");
        append_model_response(&path, "Test", 200, &Bytes::from_static(b"{}")).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
