//! Structural code rewrites through the installed `ast-grep` CLI.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use ignore::WalkBuilder;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use crate::lsp::{after_write, append_writethrough_to_output, FileDiagnosticsResult, LspSettings};
use crate::tool::errors::ToolError;
use crate::tools::edit::diff::generate_diff_string;
use crate::tools::edit::ledger::FileChangeLedger;
use crate::tools::edit::path::{resolve_writable, PathEscapeError};
use engine::FileChangeOp;

#[derive(Debug, Deserialize)]
struct AstEditArgs {
    ops: Vec<AstEditOp>,
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AstEditOp {
    pat: String,
    out: String,
}

pub(crate) struct AstEditRunOutcome {
    pub output: Result<String, ToolError>,
    pub file_changes: Vec<crate::tool::CapturedFileChange>,
}

#[derive(Default)]
struct RunStats {
    op_counts: Vec<usize>,
    file_counts: BTreeMap<PathBuf, usize>,
    snapshots: BTreeMap<PathBuf, String>,
}

struct ChangedFile {
    absolute: PathBuf,
    display_path: String,
    diff: String,
}

struct CollectedChanges {
    changed: Vec<ChangedFile>,
    file_changes: Vec<crate::tool::CapturedFileChange>,
    diagnostics: Vec<FileDiagnosticsResult>,
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

pub(crate) async fn execute_ast_edit(
    cwd: &Path,
    args: Value,
    cancel: &CancellationToken,
    lsp: LspSettings,
) -> AstEditRunOutcome {
    execute_ast_edit_with_binary(cwd, args, cancel, lsp, OsStr::new("ast-grep")).await
}

async fn execute_ast_edit_with_binary(
    cwd: &Path,
    args: Value,
    cancel: &CancellationToken,
    lsp: LspSettings,
    binary: &OsStr,
) -> AstEditRunOutcome {
    let (ops, targets) = match prepare(cwd, args) {
        Ok(prepared) => prepared,
        Err(error) => {
            return AstEditRunOutcome {
                output: Err(error),
                file_changes: Vec::new(),
            };
        }
    };
    let canonical_cwd = match cwd.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            return AstEditRunOutcome {
                output: Err(ToolError::failed(format!(
                    "ast_edit execution folder is invalid: {error}"
                ))),
                file_changes: Vec::new(),
            };
        }
    };
    let mut stats = RunStats::default();
    let run_result = run_ops(binary, &canonical_cwd, &ops, &targets, cancel, &mut stats).await;
    let change_result = collect_changes(
        &canonical_cwd,
        &stats.snapshots,
        &lsp.runtime(),
        run_result.is_ok(),
    );

    match (run_result, change_result) {
        (Ok(()), Ok(changes)) => {
            let output = render_output(&stats, &changes.changed, &changes.diagnostics);
            AstEditRunOutcome {
                output: Ok(output),
                file_changes: changes.file_changes,
            }
        }
        (Err(error), Ok(changes)) => AstEditRunOutcome {
            output: Err(error),
            file_changes: changes.file_changes,
        },
        (_, Err(error)) => AstEditRunOutcome {
            output: Err(error),
            file_changes: Vec::new(),
        },
    }
}

fn prepare(cwd: &Path, args: Value) -> Result<(Vec<AstEditOp>, Vec<PathBuf>), ToolError> {
    let args: AstEditArgs =
        serde_json::from_value(args).map_err(|error| ToolError::InvalidArgs {
            tool: "ast_edit".to_string(),
            problem: error.to_string(),
            hint: "required fields: ops (non-empty array of {pat, out}) and paths (non-empty array of files, directories, or globs)".to_string(),
        })?;
    if args.ops.is_empty() {
        return Err(invalid_args(
            "ops must contain at least one rewrite",
            "each op needs pat and out; use an empty out to delete matches",
        ));
    }
    for (index, op) in args.ops.iter().enumerate() {
        if op.pat.trim().is_empty() {
            return Err(invalid_args(
                format!("ops[{index}].pat must not be empty"),
                "provide a valid ast-grep pattern",
            ));
        }
    }
    if args.paths.is_empty() {
        return Err(invalid_args(
            "paths must contain at least one entry",
            "provide files, directories, or globs under the execution folder",
        ));
    }
    let targets = resolve_targets(cwd, &args.paths)?;
    Ok((args.ops, targets))
}

fn invalid_args(problem: impl Into<String>, hint: impl Into<String>) -> ToolError {
    ToolError::InvalidArgs {
        tool: "ast_edit".to_string(),
        problem: problem.into(),
        hint: hint.into(),
    }
}

fn resolve_targets(cwd: &Path, raw_paths: &[String]) -> Result<Vec<PathBuf>, ToolError> {
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|error| ToolError::failed(format!("invalid execution folder: {error}")))?;
    let mut resolved = BTreeSet::new();

    for raw_path in raw_paths {
        let jailed = resolve_writable(&canonical_cwd, raw_path).map_err(map_path_error)?;
        if jailed.exists() {
            resolved.insert(jailed);
            continue;
        }

        let relative_pattern = jailed
            .strip_prefix(&canonical_cwd)
            .map_err(|_| permission_denied(raw_path))?;
        let glob = glob::Pattern::new(&relative_pattern.to_string_lossy()).map_err(|error| {
            invalid_args(
                format!("invalid path glob {raw_path:?}: {error}"),
                "use glob syntax like src/**/*.ts",
            )
        })?;
        let mut builder = WalkBuilder::new(&canonical_cwd);
        builder.standard_filters(true).follow_links(false);
        for entry in builder.build().filter_map(Result::ok) {
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(&canonical_cwd) else {
                continue;
            };
            if glob.matches_path(relative) {
                let display = relative.to_string_lossy();
                resolved
                    .insert(resolve_writable(&canonical_cwd, &display).map_err(map_path_error)?);
            }
        }
        if !resolved
            .iter()
            .any(|path| glob_target_matches(path, &canonical_cwd, &glob))
        {
            return Err(ToolError::NotFound {
                what: format!("ast_edit path matched no files: {raw_path}"),
                hint: "use find to verify the path or glob under the execution folder".to_string(),
            });
        }
    }

    let mut by_depth: Vec<PathBuf> = resolved.into_iter().collect();
    by_depth.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    let mut targets: Vec<PathBuf> = Vec::new();
    for path in by_depth {
        if targets
            .iter()
            .any(|parent| parent.is_dir() && path.starts_with(parent))
        {
            continue;
        }
        targets.push(path);
    }
    if targets.is_empty() {
        return Err(ToolError::NotFound {
            what: "ast_edit paths matched no files or directories".to_string(),
            hint: "use find to verify paths under the execution folder".to_string(),
        });
    }
    Ok(targets)
}

fn glob_target_matches(path: &Path, cwd: &Path, glob: &glob::Pattern) -> bool {
    path.strip_prefix(cwd)
        .is_ok_and(|relative| glob.matches_path(relative))
}

fn map_path_error(error: PathEscapeError) -> ToolError {
    if error.0.contains("path escapes execution folder") {
        return ToolError::PermissionDenied {
            what: error.0,
            hint: "paths must stay under the execution folder; use relative paths".to_string(),
        };
    }
    invalid_args(
        error.0,
        "provide a file, directory, or glob under the execution folder",
    )
}

fn permission_denied(path: &str) -> ToolError {
    ToolError::PermissionDenied {
        what: format!("path escapes execution folder: {path}"),
        hint: "paths must stay under the execution folder; use relative paths".to_string(),
    }
}

async fn run_ops(
    binary: &OsStr,
    cwd: &Path,
    ops: &[AstEditOp],
    targets: &[PathBuf],
    cancel: &CancellationToken,
    stats: &mut RunStats,
) -> Result<(), ToolError> {
    for (index, op) in ops.iter().enumerate() {
        let preview = run_ast_grep_command(binary, cwd, op, targets, true, cancel).await?;
        ensure_success(
            preview.status,
            &preview.stdout,
            &preview.stderr,
            index,
            "preview",
        )?;
        let matches = parse_preview_matches(cwd, &preview.stdout, index)?;
        let op_count: usize = matches.values().sum();
        stats.op_counts.push(op_count);
        for (path, count) in matches {
            stats
                .snapshots
                .entry(path.clone())
                .or_insert(read_file(&path, "snapshot before rewrite")?);
            *stats.file_counts.entry(path).or_default() += count;
        }

        let apply = run_ast_grep_command(binary, cwd, op, targets, false, cancel).await?;
        ensure_success(apply.status, &apply.stdout, &apply.stderr, index, "rewrite")?;
    }
    Ok(())
}

async fn run_ast_grep_command(
    binary: &OsStr,
    cwd: &Path,
    op: &AstEditOp,
    targets: &[PathBuf],
    preview: bool,
    cancel: &CancellationToken,
) -> Result<ProcessOutput, ToolError> {
    let mut command = tokio::process::Command::new(binary);
    command.arg("-p").arg(&op.pat).arg("--rewrite").arg(&op.out);
    if preview {
        command.arg("--json=stream");
    } else {
        command.arg("--update-all");
    }
    command
        .args(targets)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ToolError::NotFound {
                what: format!(
                    "ast_edit requires {}, but it was not found on PATH",
                    binary.to_string_lossy()
                ),
                hint: "install ast-grep and ensure the ast-grep binary is available on PATH"
                    .to_string(),
            }
        } else {
            ToolError::failed(format!("ast_edit could not start ast-grep: {error}"))
        }
    })?;
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| ToolError::failed("ast_edit ast-grep stdout unavailable"))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| ToolError::failed("ast_edit ast-grep stderr unavailable"))?;

    tokio::select! {
        biased;
        () = cancel.cancelled() => {
            let _ = child.kill().await;
            Err(ToolError::Cancelled {
                tool: "ast_edit".to_string(),
            })
        }
        result = async {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let (stdout_result, stderr_result, status) = tokio::join!(
                stdout_pipe.read_to_end(&mut stdout),
                stderr_pipe.read_to_end(&mut stderr),
                child.wait(),
            );
            stdout_result.map_err(|error| {
                ToolError::failed(format!("ast_edit failed to read ast-grep stdout: {error}"))
            })?;
            stderr_result.map_err(|error| {
                ToolError::failed(format!("ast_edit failed to read ast-grep stderr: {error}"))
            })?;
            let status = status.map_err(|error| {
                ToolError::failed(format!("ast_edit failed while waiting for ast-grep: {error}"))
            })?;
            Ok(ProcessOutput {
                status,
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
            })
        } => result,
    }
}

fn ensure_success(
    status: ExitStatus,
    stdout: &str,
    stderr: &str,
    op_index: usize,
    phase: &str,
) -> Result<(), ToolError> {
    if status.success() {
        return Ok(());
    }
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    let detail = if detail.is_empty() {
        format!("ast-grep exited with {status}")
    } else {
        detail.to_string()
    };
    Err(ToolError::failed(format!(
        "ast_edit op {} {phase} failed: {detail}",
        op_index + 1
    )))
}

fn parse_preview_matches(
    cwd: &Path,
    stdout: &str,
    op_index: usize,
) -> Result<BTreeMap<PathBuf, usize>, ToolError> {
    let mut matches = BTreeMap::new();
    for (line_index, line) in stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let value: Value = serde_json::from_str(line).map_err(|error| {
            ToolError::failed(format!(
                "ast_edit op {} could not parse ast-grep JSON on line {}: {error}",
                op_index + 1,
                line_index + 1
            ))
        })?;
        let file = value.get("file").and_then(Value::as_str).ok_or_else(|| {
            ToolError::failed(format!(
                "ast_edit op {} received ast-grep JSON without a file path",
                op_index + 1
            ))
        })?;
        let path = resolve_writable(cwd, file).map_err(map_path_error)?;
        if !path.is_file() {
            return Err(ToolError::failed(format!(
                "ast_edit matched path is not a file: {file}"
            )));
        }
        *matches.entry(path).or_default() += 1;
    }
    Ok(matches)
}

fn collect_changes(
    cwd: &Path,
    snapshots: &BTreeMap<PathBuf, String>,
    lsp: &LspSettings,
    run_lsp: bool,
) -> Result<CollectedChanges, ToolError> {
    let mut changed_paths = Vec::new();
    for (path, before) in snapshots {
        if read_file(path, "read after rewrite")? != *before {
            changed_paths.push(path.clone());
        }
    }

    let diagnostics = if run_lsp {
        changed_paths
            .iter()
            .filter_map(|path| after_write(path, lsp))
            .collect()
    } else {
        Vec::new()
    };
    let ledger = FileChangeLedger::new();
    let mut changed = Vec::new();
    for (path, before) in snapshots {
        let after = read_file(path, "read after rewrite")?;
        if after == *before {
            continue;
        }
        let display_path = display_path(cwd, path)?;
        let diff = generate_diff_string(before, &after, 2).diff;
        ledger.record(
            display_path.clone(),
            FileChangeOp::Update,
            None,
            Some(diff.clone()),
        );
        changed.push(ChangedFile {
            absolute: path.clone(),
            display_path,
            diff,
        });
    }
    Ok(CollectedChanges {
        changed,
        file_changes: ledger.take(),
        diagnostics,
    })
}

fn read_file(path: &Path, operation: &str) -> Result<String, ToolError> {
    fs::read_to_string(path).map_err(|error| {
        ToolError::failed(format!(
            "{operation} failed for {}: {error}",
            path.display()
        ))
    })
}

fn display_path(cwd: &Path, path: &Path) -> Result<String, ToolError> {
    path.strip_prefix(cwd)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| permission_denied(&path.display().to_string()))
}

fn render_output(
    stats: &RunStats,
    changed: &[ChangedFile],
    diagnostics: &[FileDiagnosticsResult],
) -> String {
    let replacements: usize = stats.op_counts.iter().sum();
    let mut lines = vec![format!(
        "AST edit applied {} across {}.",
        count_label(replacements, "replacement"),
        count_label(changed.len(), "file")
    )];
    for (index, count) in stats.op_counts.iter().enumerate() {
        lines.push(format!(
            "Op {}: {}.",
            index + 1,
            count_label(*count, "replacement")
        ));
    }
    for (path, count) in &stats.file_counts {
        let display = changed
            .iter()
            .find(|change| change.absolute == *path)
            .map(|change| change.display_path.clone())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        lines.push(String::new());
        lines.push(format!("{display}: {}", count_label(*count, "replacement")));
        if let Some(change) = changed.iter().find(|change| change.absolute == *path) {
            lines.push(change.diff.clone());
        } else {
            lines.push("(no net change)".to_string());
        }
    }
    append_writethrough_to_output(&lines.join("\n"), diagnostics)
}

fn count_label(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ast_grep_available() -> bool {
        std::process::Command::new("ast-grep")
            .arg("--version")
            .status()
            .is_ok_and(|status| status.success())
    }

    fn disabled_lsp() -> LspSettings {
        LspSettings {
            enabled: false,
            ..LspSettings::default()
        }
    }

    #[test]
    fn rejects_empty_ops_and_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let empty_ops = prepare(
            dir.path(),
            serde_json::json!({"ops": [], "paths": ["src/**/*.ts"]}),
        )
        .unwrap_err();
        assert!(empty_ops
            .to_string()
            .contains("ops must contain at least one"));

        let empty_paths = prepare(
            dir.path(),
            serde_json::json!({"ops": [{"pat": "old()", "out": "new()"}], "paths": []}),
        )
        .unwrap_err();
        assert!(empty_paths
            .to_string()
            .contains("paths must contain at least one"));
    }

    #[test]
    fn rejects_missing_op_fields_and_empty_pattern() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing_out = prepare(
            dir.path(),
            serde_json::json!({"ops": [{"pat": "old()"}], "paths": ["src.ts"]}),
        )
        .unwrap_err();
        assert!(missing_out.to_string().contains("missing field `out`"));

        let empty_pattern = prepare(
            dir.path(),
            serde_json::json!({"ops": [{"pat": " ", "out": ""}], "paths": ["src.ts"]}),
        )
        .unwrap_err();
        assert!(empty_pattern.to_string().contains("pat must not be empty"));
    }

    #[test]
    fn rejects_path_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = prepare(
            dir.path(),
            serde_json::json!({
                "ops": [{"pat": "old()", "out": "new()"}],
                "paths": ["../outside.ts"]
            }),
        )
        .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied { .. }));
        assert!(error.to_string().contains("escapes execution folder"));
    }

    #[tokio::test]
    async fn missing_binary_is_actionable() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("source.ts"), "oldApi();\n").expect("seed");
        let outcome = execute_ast_edit_with_binary(
            dir.path(),
            serde_json::json!({
                "ops": [{"pat": "oldApi()", "out": "newApi()"}],
                "paths": ["source.ts"]
            }),
            &CancellationToken::new(),
            disabled_lsp(),
            OsStr::new("openflow-definitely-missing-ast-grep"),
        )
        .await;
        let error = outcome.output.unwrap_err();
        assert!(error.to_string().contains("not found on PATH"));
        assert!(error.to_string().contains("install ast-grep"));
    }

    #[tokio::test]
    async fn rewrites_one_match_when_ast_grep_is_available() {
        if !ast_grep_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("source.ts"), "const x = oldApi(1);\n").expect("seed");
        let outcome = execute_ast_edit(
            dir.path(),
            serde_json::json!({
                "ops": [{"pat": "oldApi($$$ARGS)", "out": "newApi($$$ARGS)"}],
                "paths": ["source.ts"]
            }),
            &CancellationToken::new(),
            disabled_lsp(),
        )
        .await;
        assert!(outcome.output.expect("rewrite").contains("1 replacement"));
        assert_eq!(
            fs::read_to_string(dir.path().join("source.ts")).expect("read"),
            "const x = newApi(1);\n"
        );
        assert_eq!(outcome.file_changes.len(), 1);
    }

    #[tokio::test]
    async fn empty_output_deletes_match_when_ast_grep_is_available() {
        if !ast_grep_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("source.ts"), "console.log('x');\n").expect("seed");
        let outcome = execute_ast_edit(
            dir.path(),
            serde_json::json!({
                "ops": [{"pat": "console.log($$$ARGS)", "out": ""}],
                "paths": ["source.ts"]
            }),
            &CancellationToken::new(),
            disabled_lsp(),
        )
        .await;
        outcome.output.expect("rewrite");
        let content = fs::read_to_string(dir.path().join("source.ts")).expect("read");
        assert!(!content.contains("console.log"));
    }

    #[tokio::test]
    async fn applies_ops_sequentially_when_ast_grep_is_available() {
        if !ast_grep_available() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("src")).expect("mkdir");
        fs::write(dir.path().join("src/source.ts"), "oldApi();\n").expect("seed");
        let outcome = execute_ast_edit(
            dir.path(),
            serde_json::json!({
                "ops": [
                    {"pat": "oldApi()", "out": "middleApi()"},
                    {"pat": "middleApi()", "out": "newApi()"}
                ],
                "paths": ["src/**/*.ts"]
            }),
            &CancellationToken::new(),
            disabled_lsp(),
        )
        .await;
        let output = outcome.output.expect("rewrite");
        assert!(output.contains("Op 1: 1 replacement."));
        assert!(output.contains("Op 2: 1 replacement."));
        assert_eq!(
            fs::read_to_string(dir.path().join("src/source.ts")).expect("read"),
            "newApi();\n"
        );
    }
}
