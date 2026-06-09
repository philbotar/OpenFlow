//! CLI formatter registry (incremental Phase 8 — rust-first).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use super::config::LspSettings;
use super::diagnostics::{FileDiagnosticsResult, FormatResult};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FormatterSpec {
    name: &'static str,
    binary: &'static str,
    extensions: &'static [&'static str],
}

const FORMATTERS: &[FormatterSpec] = &[
    FormatterSpec {
        name: "rustfmt",
        binary: "rustfmt",
        extensions: &["rs"],
    },
    FormatterSpec {
        name: "prettier",
        binary: "prettier",
        extensions: &[
            "js", "jsx", "ts", "tsx", "json", "css", "scss", "html", "md", "yaml", "yml",
        ],
    },
    FormatterSpec {
        name: "gofmt",
        binary: "gofmt",
        extensions: &["go"],
    },
    FormatterSpec {
        name: "black",
        binary: "black",
        extensions: &["py"],
    },
    FormatterSpec {
        name: "stylua",
        binary: "stylua",
        extensions: &["lua"],
    },
];

static BINARY_AVAILABILITY: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

pub(crate) fn formatter_for_path(path: &Path) -> Option<&'static FormatterSpec> {
    let ext = path.extension()?.to_str()?;
    FORMATTERS
        .iter()
        .find(|spec| spec.extensions.contains(&ext))
}

fn binary_available(binary: &str) -> bool {
    let cache = BINARY_AVAILABILITY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().expect("binary availability cache");
    if let Some(cached) = guard.get(binary) {
        return *cached;
    }
    let available = Command::new(binary)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    guard.insert(binary.to_string(), available);
    available
}

fn read_pipe(pipe: Option<std::process::ChildStdout>) -> String {
    let mut text = String::new();
    if let Some(mut reader) = pipe {
        let _ = reader.read_to_string(&mut text);
    }
    text
}

fn read_stderr_pipe(pipe: Option<std::process::ChildStderr>) -> String {
    let mut text = String::new();
    if let Some(mut reader) = pipe {
        let _ = reader.read_to_string(&mut text);
    }
    text
}

fn run_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn formatter: {error}"))?;
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("formatter wait failed: {error}"))?
        {
            let stdout = read_pipe(child.stdout.take());
            let stderr = read_stderr_pipe(child.stderr.take());
            return Ok(std::process::Output {
                status,
                stdout: stdout.into_bytes(),
                stderr: stderr.into_bytes(),
            });
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("formatter timed out after {}ms", timeout.as_millis()));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn format_failure_message(spec: &FormatterSpec, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("{} exit code {}", spec.name, output.status)
    } else {
        format!("{} failed: {stderr}", spec.name)
    }
}

pub fn format_file_in_place(path: &Path, settings: &LspSettings) -> FileDiagnosticsResult {
    let Some(spec) = formatter_for_path(path) else {
        return FileDiagnosticsResult {
            formatter: Some(FormatResult::Skipped),
            ..Default::default()
        };
    };
    if !binary_available(spec.binary) {
        return FileDiagnosticsResult {
            server: Some(spec.name.to_string()),
            formatter: Some(FormatResult::Skipped),
            summary: format!("{} not installed", spec.binary),
            ..Default::default()
        };
    }

    let before = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return FileDiagnosticsResult::formatter_failed(
                spec.name,
                format!("read before format failed: {error}"),
            );
        }
    };

    let timeout = Duration::from_millis(settings.timeout_ms);
    let output = match spec.name {
        "rustfmt" => {
            let mut command = Command::new(spec.binary);
            command.arg(path);
            run_with_timeout(command, timeout)
        }
        "prettier" => {
            let mut command = Command::new(spec.binary);
            command.arg("--write").arg(path);
            run_with_timeout(command, timeout)
        }
        "gofmt" => {
            let mut command = Command::new(spec.binary);
            command.arg("-w").arg(path);
            run_with_timeout(command, timeout)
        }
        "black" => {
            let mut command = Command::new(spec.binary);
            command.arg("--quiet").arg(path);
            run_with_timeout(command, timeout)
        }
        "stylua" => {
            let mut command = Command::new(spec.binary);
            command.arg(path);
            run_with_timeout(command, timeout)
        }
        _ => {
            let mut command = Command::new(spec.binary);
            command.arg(path);
            run_with_timeout(command, timeout)
        }
    };

    let output = match output {
        Ok(output) => output,
        Err(message) => return FileDiagnosticsResult::formatter_failed(spec.name, message),
    };
    if !output.status.success() {
        return FileDiagnosticsResult::formatter_failed(
            spec.name,
            format_failure_message(spec, &output),
        );
    }

    let after = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return FileDiagnosticsResult::formatter_failed(
                spec.name,
                format!("read after format failed: {error}"),
            );
        }
    };

    let formatter = if before == after {
        FormatResult::Unchanged
    } else {
        FormatResult::Formatted
    };
    FileDiagnosticsResult::ok(formatter, spec.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_rustfmt_for_rs_files() {
        let spec = formatter_for_path(Path::new("src/lib.rs")).expect("formatter");
        assert_eq!(spec.name, "rustfmt");
    }

    #[test]
    fn skips_unknown_extensions() {
        assert!(formatter_for_path(Path::new("data.bin")).is_none());
    }
}
