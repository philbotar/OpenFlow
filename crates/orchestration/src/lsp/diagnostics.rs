//! Diagnostic and formatter result types for writethrough output.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatResult {
    Unchanged,
    Formatted,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDiagnosticsResult {
    pub server: Option<String>,
    pub messages: Vec<String>,
    pub summary: String,
    pub errored: bool,
    pub formatter: Option<FormatResult>,
}

impl FileDiagnosticsResult {
    #[must_use]
    pub fn ok(formatter: FormatResult, server: impl Into<String>) -> Self {
        Self {
            server: Some(server.into()),
            messages: Vec::new(),
            summary: "OK".to_string(),
            errored: false,
            formatter: Some(formatter),
        }
    }

    #[must_use]
    pub fn formatter_failed(server: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            server: Some(server.into()),
            messages: vec![message.into()],
            summary: "formatter failed".to_string(),
            errored: true,
            formatter: Some(FormatResult::Failed),
        }
    }
}

#[must_use]
pub fn append_writethrough_to_output(base: &str, results: &[FileDiagnosticsResult]) -> String {
    if results.is_empty() {
        return base.to_string();
    }
    let mut out = base.to_string();
    for result in results {
        let block = format_writethrough_block(result);
        if block.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&block);
    }
    out
}

fn format_writethrough_block(result: &FileDiagnosticsResult) -> String {
    let mut lines = Vec::new();
    if let Some(formatter) = result.formatter {
        let label = match formatter {
            FormatResult::Formatted => "Formatted",
            FormatResult::Unchanged => "Format unchanged",
            FormatResult::Failed => "Format failed",
            FormatResult::Skipped => "Format skipped",
        };
        if let Some(server) = &result.server {
            lines.push(format!("{label} ({server})"));
        } else {
            lines.push(label.to_string());
        }
    }
    if !result.messages.is_empty() {
        if !result.summary.is_empty() && result.summary != "OK" {
            lines.push(result.summary.clone());
        }
        lines.extend(result.messages.iter().cloned());
    }
    lines.join("\n")
}
