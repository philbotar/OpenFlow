//! Post-write format-on-write pipeline.

use std::path::Path;

use super::config::LspSettings;
use super::diagnostics::FileDiagnosticsResult;
use super::formatters::format_file_in_place;

/// Run format-on-write (and future diagnostics) after a file is written to disk.
#[must_use]
pub fn after_write(absolute: &Path, settings: &LspSettings) -> Option<FileDiagnosticsResult> {
    if !settings.writethrough_active() {
        return None;
    }

    let result = if settings.format_on_write {
        format_file_in_place(absolute, settings)
    } else {
        FileDiagnosticsResult::default()
    };

    if settings.diagnostics_on_write && result.server.is_none() {
        // Full LSP diagnostics require a language-server client (future work).
        return None;
    }

    if result.formatter.is_none() && result.messages.is_empty() && result.summary.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::diagnostics::FormatResult;

    #[test]
    fn skips_when_disabled() {
        let settings = LspSettings {
            enabled: false,
            format_on_write: true,
            ..Default::default()
        };
        assert!(after_write(Path::new("/tmp/x.rs"), &settings).is_none());
    }

    #[test]
    fn skips_format_when_format_on_write_off() {
        let settings = LspSettings {
            enabled: true,
            format_on_write: false,
            diagnostics_on_write: false,
            ..Default::default()
        };
        assert!(after_write(Path::new("/tmp/x.rs"), &settings).is_none());
    }

    #[cfg_attr(miri, ignore)] // ponytail: Miri cannot emulate rustfmt subprocess
    #[test]
    fn formats_rust_file_when_rustfmt_available() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let path = temp.path().join("sample.rs");
        std::fs::write(&path, "fn main(){}\n").expect("write");
        if std::process::Command::new("rustfmt")
            .arg("--version")
            .status()
            .is_ok_and(|status| status.success())
        {
            // rustfmt available
        } else {
            return;
        }
        let settings = LspSettings {
            enabled: true,
            format_on_write: true,
            ..Default::default()
        };
        let result = after_write(&path, &settings).expect("writethrough");
        assert_eq!(result.formatter, Some(FormatResult::Formatted));
    }
}
