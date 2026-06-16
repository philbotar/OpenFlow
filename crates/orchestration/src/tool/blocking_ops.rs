//! Blocking filesystem tool operations (edit, read, search) for [`crate::tool::runner`].

use crate::lsp::LspSettings as RuntimeLspSettings;
use crate::settings::model::LspSettings;
use crate::tool::errors::ToolError;
use crate::tool::ports::ContentSearch;
use crate::tool::read::selector::ReadSelector;
use crate::tool::read::summary::render_read;
use crate::tool::registry::BuiltinToolKind;
use crate::tools::grep::RipgrepSearch;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use walkdir::WalkDir;

pub(crate) struct BlockingRunOutcome {
    pub output: Result<String, ToolError>,
    pub file_changes: Vec<engine::FileChangeRecord>,
    pub edit_batch: Option<engine::EditBatch>,
}

pub(crate) struct BlockingBatchContext {
    pub node_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
}

pub(crate) struct BlockingToolOps {
    cwd: PathBuf,
    ledger: crate::tools::edit::ledger::FileChangeLedger,
    snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
    lsp: RuntimeLspSettings,
}

impl BlockingToolOps {
    pub(crate) fn new(
        cwd: PathBuf,
        ledger: crate::tools::edit::ledger::FileChangeLedger,
        snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
        lsp: RuntimeLspSettings,
    ) -> Self {
        Self {
            cwd,
            ledger,
            snapshots,
            lsp,
        }
    }

    pub(crate) fn run_blocking(
        cwd: PathBuf,
        snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
        lsp: LspSettings,
        kind: BuiltinToolKind,
        args: Value,
        batch_ctx: Option<BlockingBatchContext>,
    ) -> BlockingRunOutcome {
        let lsp = RuntimeLspSettings::from_persisted(&lsp);
        let edit_batch = batch_ctx.and_then(|context| {
            crate::tools::edit::batch::capture_edit_batch(
                &cwd,
                &context.node_id,
                &context.tool_call_id,
                &context.tool_name,
                kind,
                &args,
            )
        });
        let ledger = crate::tools::edit::ledger::FileChangeLedger::new();
        let ops = Self::new(cwd, ledger.clone(), snapshots, lsp);
        let output = match kind {
            BuiltinToolKind::Search => ops.search(args),
            BuiltinToolKind::Find => ops.find(args),
            BuiltinToolKind::Write => ops.write(args),
            BuiltinToolKind::Edit => ops.edit(args),
            BuiltinToolKind::ApplyPatch => ops.apply_patch(args),
            BuiltinToolKind::AstGrep => Err(ToolError::failed("ast_grep must use async runner")),
            _ => Err(ToolError::failed(
                "blocking runner received a non-blocking tool",
            )),
        };
        let mut file_changes = ledger.take();
        if let Some(ref batch) = edit_batch {
            for change in &mut file_changes {
                change.batch_id = Some(batch.batch_id.clone());
            }
        }
        BlockingRunOutcome {
            output,
            file_changes,
            edit_batch,
        }
    }

    pub(crate) fn read_local_at(
        cwd: PathBuf,
        snapshots: Arc<crate::tools::edit::hashline::snapshots::InMemorySnapshotStore>,
        path: &str,
    ) -> Result<String, ToolError> {
        Self::new(
            cwd,
            crate::tools::edit::ledger::FileChangeLedger::new(),
            snapshots,
            RuntimeLspSettings::default(),
        )
        .read_local(path)
    }

    pub(crate) fn read_local(&self, path: &str) -> Result<String, ToolError> {
        let (path, selector) = split_selector(path);
        let absolute = self.resolve_local(&path);
        let metadata = fs::metadata(&absolute).map_err(|error| map_read_io_error(&path, &error))?;
        if metadata.is_dir() {
            return crate::tool::read::directory::render_directory_listing(&absolute, &path);
        }
        let text =
            fs::read_to_string(&absolute).map_err(|error| map_read_io_error(&path, &error))?;
        if let Ok(canonical) =
            crate::tools::edit::file_snapshot_store::canonical_snapshot_path(&self.cwd, &path)
        {
            let _ = crate::tools::edit::file_snapshot_store::record_file_snapshot(
                self.snapshots.as_ref(),
                &canonical,
                &text,
            );
        }
        Ok(render_read(&path, &text, selector))
    }

    fn search(&self, args: Value) -> Result<String, ToolError> {
        RipgrepSearch::new(self.cwd.clone()).search(args)
    }

    fn write(&self, args: Value) -> Result<String, ToolError> {
        crate::tools::edit::write::execute_write(
            self.cwd.clone(),
            args,
            self.ledger.clone(),
            self.lsp.clone(),
        )
    }

    fn edit(&self, args: Value) -> Result<String, ToolError> {
        crate::tools::edit::edit_tool::execute_edit(
            self.cwd.clone(),
            args,
            self.ledger.clone(),
            self.snapshots.clone(),
            self.lsp.clone(),
        )
    }

    fn apply_patch(&self, args: Value) -> Result<String, ToolError> {
        crate::tools::edit::apply_patch_tool::execute_apply_patch(
            self.cwd.clone(),
            args,
            self.ledger.clone(),
            self.lsp.clone(),
        )
    }

    fn find(&self, args: Value) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        struct FindArgs {
            paths: StringOrMany,
        }
        let args: FindArgs =
            serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
                tool: "find".to_string(),
                problem: error.to_string(),
                hint: "required field: paths (string or array of glob patterns, e.g. **/*.rs)"
                    .to_string(),
            })?;
        let mut matches = Vec::new();
        for pattern in args.paths.into_vec() {
            for entry in self.expand_paths(&pattern)? {
                matches.push(entry.display().to_string());
            }
        }
        matches.sort();
        matches.dedup();
        Ok(matches.into_iter().take(200).collect::<Vec<_>>().join("\n"))
    }

    fn resolve_local(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }

    fn expand_paths(&self, pattern: &str) -> Result<Vec<PathBuf>, ToolError> {
        let absolute = self.resolve_local(pattern);
        if absolute.exists() {
            if absolute.is_dir() {
                return Ok(WalkDir::new(&absolute)
                    .into_iter()
                    .filter_map(Result::ok)
                    .map(|entry| entry.path().to_path_buf())
                    .collect());
            }
            return Ok(vec![absolute]);
        }

        let pattern = self.cwd.join(pattern).display().to_string();
        let mut matches = Vec::new();
        for entry in glob::glob(&pattern).map_err(|error| ToolError::InvalidArgs {
            tool: "find".to_string(),
            problem: format!("invalid glob pattern: {error}"),
            hint: "use glob syntax like **/*.rs or src/**/*.ts".to_string(),
        })? {
            let path = entry.map_err(|error| ToolError::failed(format!("glob failed: {error}")))?;
            if path.is_dir() {
                matches.extend(
                    WalkDir::new(&path)
                        .into_iter()
                        .filter_map(Result::ok)
                        .map(|entry| entry.path().to_path_buf()),
                );
            } else {
                matches.push(path);
            }
        }
        Ok(matches)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrMany {
    One(String),
    Many(Vec<String>),
}

impl StringOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

pub(crate) use crate::tool::read::selector::split_selector;

pub(crate) fn apply_read_selector(label: &str, text: &str, selector: ReadSelector) -> String {
    render_read(label, text, selector)
}

fn map_read_io_error(path: &str, error: &std::io::Error) -> ToolError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ToolError::NotFound {
            what: format!("read failed for {path}: {error}"),
            hint: "use find to locate the file".to_string(),
        }
    } else {
        ToolError::failed(format!("read failed for {path}: {error}"))
    }
}
