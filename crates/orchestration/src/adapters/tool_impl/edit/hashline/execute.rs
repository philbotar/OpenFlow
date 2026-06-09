//! Hashline mode runner for the `edit` tool.

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use super::fs::{HashlineFilesystem, WriteResult};
use super::input::Patch;
use super::patcher::{PatchOp, Patcher, PatcherOptions};
use super::snapshots::InMemorySnapshotStore;
use super::types::SplitOptions;
use crate::lsp::{append_writethrough_to_output, LspSettings};
use crate::tools::edit::diff::generate_diff_string;
use crate::tools::edit::io::{EditIo, EditIoError, WriteTextOutcome};
use crate::tools::edit::ledger::FileChangeLedger;
use crate::tools::edit::normalize::{normalize_to_lf, strip_bom};
use crate::tools::errors::ToolError;

pub struct EditHashlineFs {
    io: EditIo,
    writethrough: Mutex<Vec<crate::lsp::FileDiagnosticsResult>>,
}

impl EditHashlineFs {
    pub fn new(io: EditIo) -> Self {
        Self {
            io,
            writethrough: Mutex::new(Vec::new()),
        }
    }

    pub fn take_writethrough(&self) -> Vec<crate::lsp::FileDiagnosticsResult> {
        self.writethrough.lock().drain(..).collect()
    }
}

impl HashlineFilesystem for EditHashlineFs {
    fn read_text(&self, path: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.io.read_raw(path).map_err(map_io_boxed)
    }

    fn write_text(
        &self,
        path: &str,
        content: &str,
    ) -> Result<WriteResult, Box<dyn std::error::Error + Send + Sync>> {
        let outcome: WriteTextOutcome = self
            .io
            .write_persisted(path, content)
            .map_err(map_io_boxed)?;
        if let Some(diagnostics) = outcome.diagnostics {
            self.writethrough.lock().push(diagnostics);
        }
        let stripped = strip_bom(content);
        let fallback_normalized = normalize_to_lf(&stripped.text);
        let normalized = outcome.disk_normalized.unwrap_or(fallback_normalized);
        let text = self
            .io
            .read_raw(path)
            .unwrap_or_else(|_| content.to_string());
        Ok(WriteResult { text, normalized })
    }

    fn canonical_path(&self, path: &str) -> String {
        self.io
            .resolve(path)
            .map(|absolute| absolute.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string())
    }

    fn preflight_write(&self, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.io.preflight_update(path).map_err(map_io_boxed)
    }
}

fn map_io_boxed(error: EditIoError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(error)
}

fn no_change_diagnostic(path: &str) -> String {
    format!(
        "Edits to {path} parsed and applied cleanly, but produced no change: \
         your body row(s) are byte-identical to the file at the targeted lines. \
         The bug is somewhere else — re-read the file before issuing another edit. \
         Do NOT widen the payload or add lines; verify the anchor first."
    )
}

pub fn execute_hashline(
    cwd: PathBuf,
    input: String,
    ledger: FileChangeLedger,
    snapshots: Arc<InMemorySnapshotStore>,
    lsp: LspSettings,
) -> Result<String, ToolError> {
    let cwd_display = cwd.to_string_lossy().into_owned();
    let patch = Patch::parse(
        &input,
        SplitOptions {
            cwd: Some(cwd_display),
            path: None,
        },
    )
    .map_err(ToolError::Failed)?;
    if patch.sections.is_empty() {
        return Err(ToolError::Failed(
            "No hashline sections found in input.".to_string(),
        ));
    }

    let io = EditIo::new(cwd).with_ledger(ledger).with_lsp_settings(lsp);
    let fs = EditHashlineFs::new(io);
    let patcher = Patcher::new(PatcherOptions {
        fs,
        snapshots,
        block_resolver: None,
    });

    let applied = patcher.apply(&patch).map_err(ToolError::Failed)?;
    let writethrough = patcher.fs.take_writethrough();
    let mut parts = Vec::new();
    for section in applied.sections {
        if section.op == PatchOp::Noop {
            parts.push(no_change_diagnostic(&section.path));
            continue;
        }
        let diff = generate_diff_string(&section.before, &section.after, 2);
        let mut block = section.header.clone();
        if !diff.diff.is_empty() {
            block.push('\n');
            block.push_str(&diff.diff);
        }
        if !section.warnings.is_empty() {
            block.push_str("\n\nWarnings:\n");
            block.push_str(&section.warnings.join("\n"));
        }
        parts.push(block);
    }
    let mut output = parts.join("\n\n");
    if !writethrough.is_empty() {
        output = append_writethrough_to_output(&output, &writethrough);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::edit::hashline::format::compute_file_hash;
    use std::fs;

    #[test]
    fn hashline_edit_applies_replace_hunk() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let path = "note.txt";
        fs::write(temp.path().join(path), "alpha\nbeta\n").expect("seed");
        let tag = compute_file_hash("alpha\nbeta\n");
        let input = format!("¶{path}#{tag}\nreplace 1..1:\n+gamma");
        let ledger = FileChangeLedger::new();
        let snapshots = Arc::new(InMemorySnapshotStore::new());
        let output = execute_hashline(
            temp.path().to_path_buf(),
            input,
            ledger,
            snapshots,
            LspSettings {
                enabled: false,
                ..Default::default()
            },
        )
        .expect("apply");
        assert!(output.contains(&format!("¶{path}#")));
        let text = fs::read_to_string(temp.path().join(path)).expect("read");
        assert_eq!(text, "gamma\nbeta\n");
    }
}
