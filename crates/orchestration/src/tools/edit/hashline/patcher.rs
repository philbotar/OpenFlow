//! High-level sync patch orchestrator.

use super::apply::apply_edits;
use super::block::{has_block_edit, resolve_block_edits, OnUnresolved, ResolveBlockEditsOptions};
use super::format::{compute_file_hash, format_hashline_header};
use super::fs::{is_not_found, HashlineFilesystem, WriteResult};
use super::input::{Patch, PatchSection};
use super::messages::{missing_snapshot_tag_message, HEADTAIL_DRIFT_WARNING};
use super::mismatch::{MismatchDetails, MismatchError};
use super::recovery::{recovery_to_apply_result, Recovery};
use super::snapshots::SnapshotStore;
use super::types::{ApplyResult, BlockResolver, Cursor, Edit};

use crate::tools::edit::normalize::{
    detect_line_ending, normalize_to_lf, restore_line_endings, strip_bom, LineEnding,
};

pub struct PatcherOptions<F: HashlineFilesystem, S: SnapshotStore> {
    pub fs: F,
    pub snapshots: S,
    pub block_resolver: Option<Box<BlockResolver>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchOp {
    Create,
    Update,
    Noop,
}

#[derive(Debug, Clone)]
pub struct PatchSectionResult {
    pub path: String,
    pub canonical_path: String,
    pub op: PatchOp,
    pub before: String,
    pub after: String,
    pub persisted: String,
    pub written: String,
    pub file_hash: String,
    pub header: String,
    pub first_changed_line: Option<u32>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PatcherApplyResult {
    pub sections: Vec<PatchSectionResult>,
}

#[derive(Debug, Clone)]
pub struct PreparedSection {
    pub section: PatchSection,
    pub canonical_path: String,
    pub exists: bool,
    pub raw_content: String,
    pub bom: String,
    pub line_ending: LineEnding,
    pub normalized: String,
    pub apply_result: ApplyResult,
    pub parse_warnings: Vec<String>,
}

impl PreparedSection {
    pub fn is_noop(&self) -> bool {
        self.apply_result.text == self.normalized
    }
}

pub struct Patcher<F: HashlineFilesystem, S: SnapshotStore> {
    pub fs: F,
    pub snapshots: S,
    pub block_resolver: Option<Box<BlockResolver>>,
}

impl<F: HashlineFilesystem, S: SnapshotStore> Patcher<F, S> {
    pub fn new(options: PatcherOptions<F, S>) -> Self {
        Self {
            fs: options.fs,
            snapshots: options.snapshots,
            block_resolver: options.block_resolver,
        }
    }

    pub fn apply(&self, patch: &Patch) -> Result<PatcherApplyResult, String> {
        if patch.sections.len() == 1 {
            let mut section = patch.sections[0].clone();
            let prepared = self.prepare(&mut section)?;
            return Ok(PatcherApplyResult {
                sections: vec![self.commit(prepared)?],
            });
        }
        let mut prepared = Vec::new();
        for section in &patch.sections {
            let mut s = section.clone();
            prepared.push(self.prepare(&mut s)?);
        }
        assert_unique_canonical_paths(&prepared)?;
        for entry in &prepared {
            if entry.is_noop() {
                return Err(format!(
                    "Edits to {} resulted in no changes being made.",
                    entry.section.path
                ));
            }
        }
        let mut results = Vec::new();
        for entry in prepared {
            results.push(self.commit(entry)?);
        }
        Ok(PatcherApplyResult { sections: results })
    }

    pub fn preflight(&self, patch: &Patch) -> Result<(), String> {
        let mut prepared = Vec::new();
        for section in &patch.sections {
            let mut s = section.clone();
            prepared.push(self.prepare(&mut s)?);
        }
        assert_unique_canonical_paths(&prepared)?;
        for entry in &prepared {
            if entry.is_noop() {
                return Err(format!(
                    "Edits to {} resulted in no changes being made.",
                    entry.section.path
                ));
            }
        }
        Ok(())
    }

    pub fn prepare(&self, section: &mut PatchSection) -> Result<PreparedSection, String> {
        let parsed = section.parse()?;
        let parse_warnings = parsed.warnings.clone();
        let edits = parsed.edits.clone();
        assert_section_hash_present(&section.path, section.file_hash.as_deref())?;

        let canonical_path = self.fs.canonical_path(&section.path);
        self.fs
            .preflight_write(&section.path)
            .map_err(|e| e.to_string())?;
        let (exists, raw_content) = self.try_read(&section.path)?;
        if !exists {
            return Err(format!(
                "File not found: {}. Use the write tool to create new files.",
                section.path
            ));
        }
        let bom_result = strip_bom(&raw_content);
        let line_ending = detect_line_ending(&bom_result.text);
        let normalized = normalize_to_lf(&bom_result.text);
        let apply_result =
            self.apply_with_recovery(section, &canonical_path, exists, &normalized, &edits)?;
        Ok(PreparedSection {
            section: section.clone(),
            canonical_path,
            exists,
            raw_content,
            bom: bom_result.bom,
            line_ending,
            normalized,
            apply_result,
            parse_warnings,
        })
    }

    pub fn commit(&self, prepared: PreparedSection) -> Result<PatchSectionResult, String> {
        let PreparedSection {
            section,
            normalized,
            bom,
            line_ending,
            parse_warnings,
            exists,
            apply_result,
            canonical_path,
            raw_content,
        } = prepared;
        let after = apply_result.text.clone();
        let warnings = merge_warnings(&[&parse_warnings, &apply_result.warnings]);
        if after == normalized {
            let hash = self.record_full_snapshot(&canonical_path, &normalized);
            return Ok(PatchSectionResult {
                path: section.path.clone(),
                canonical_path,
                op: PatchOp::Noop,
                before: normalized,
                after: after.clone(),
                persisted: raw_content.clone(),
                written: raw_content,
                file_hash: hash.clone(),
                header: format_hashline_header(&section.path, &hash),
                first_changed_line: None,
                warnings,
            });
        }
        let persisted = format!("{}{}", bom, restore_line_endings(&after, line_ending));
        let write: WriteResult = self
            .fs
            .write_text(&section.path, &persisted)
            .map_err(|e| e.to_string())?;
        let file_hash = self.record_full_snapshot(&canonical_path, &write.normalized);
        let op = if exists {
            PatchOp::Update
        } else {
            PatchOp::Create
        };
        Ok(PatchSectionResult {
            path: section.path.clone(),
            canonical_path,
            op,
            before: normalized,
            after: write.normalized.clone(),
            persisted: persisted.clone(),
            written: write.text,
            file_hash: file_hash.clone(),
            header: format_hashline_header(&section.path, &file_hash),
            first_changed_line: apply_result.first_changed_line,
            warnings,
        })
    }

    fn try_read(&self, path: &str) -> Result<(bool, String), String> {
        match self.fs.read_text(path) {
            Ok(content) => Ok((true, content)),
            Err(error) if is_not_found(error.as_ref()) => Ok((false, String::new())),
            Err(error) => Err(error.to_string()),
        }
    }

    fn record_full_snapshot(&self, canonical_path: &str, normalized: &str) -> String {
        self.snapshots.record(canonical_path, normalized)
    }

    fn mismatch_error(
        &self,
        section: &PatchSection,
        canonical_path: &str,
        normalized: &str,
        expected: &str,
        hash_recognized: bool,
        anchor_lines: Vec<u32>,
    ) -> MismatchError {
        let actual_file_hash = self.record_full_snapshot(canonical_path, normalized);
        MismatchError::new(MismatchDetails {
            path: Some(section.path.clone()),
            expected_file_hash: expected.to_string(),
            actual_file_hash,
            file_lines: normalized.split('\n').map(String::from).collect(),
            anchor_lines,
            hash_recognized: Some(hash_recognized),
        })
    }

    fn apply_with_recovery(
        &self,
        section: &mut PatchSection,
        canonical_path: &str,
        exists: bool,
        normalized: &str,
        edits: &[Edit],
    ) -> Result<ApplyResult, String> {
        let expected = if exists {
            section.file_hash.clone()
        } else {
            None
        };
        let live_matches = expected
            .as_ref()
            .is_some_and(|tag| compute_file_hash(normalized) == *tag);
        let resolver = self.block_resolver.as_deref();
        let mut resolved = edits.to_vec();
        if has_block_edit(edits) {
            let base_text = match (&expected, live_matches) {
                (None, _) | (Some(_), true) => normalized.to_string(),
                (Some(tag), false) => match self.snapshots.by_hash(canonical_path, tag) {
                    Some(snapshot) => snapshot.text,
                    None => {
                        let anchors = section.collect_anchor_lines()?;
                        return Err(self
                            .mismatch_error(
                                section,
                                canonical_path,
                                normalized,
                                tag,
                                false,
                                anchors,
                            )
                            .to_string());
                    }
                },
            };
            resolved = resolve_block_edits(
                edits,
                &base_text,
                &section.path,
                resolver,
                ResolveBlockEditsOptions {
                    on_unresolved: OnUnresolved::Throw,
                },
            )?;
        }
        if expected.is_none() {
            return apply_edits(normalized, &resolved);
        }
        if live_matches {
            return apply_edits(normalized, &resolved);
        }
        if !has_anchor_scoped_edit(&resolved) {
            let mut result = apply_edits(normalized, &resolved)?;
            result
                .warnings
                .insert(0, HEADTAIL_DRIFT_WARNING.to_string());
            return Ok(result);
        }
        let recovery = Recovery::new(&self.snapshots);
        if let Some(recovered) = recovery.try_recover(super::recovery::RecoveryArgs {
            path: canonical_path,
            current_text: normalized,
            file_hash: expected.as_ref().expect("tag"),
            edits: &resolved,
        }) {
            return Ok(recovery_to_apply_result(recovered));
        }
        let tag = expected.as_ref().expect("tag");
        let hash_recognized = self.snapshots.by_hash(canonical_path, tag).is_some();
        let anchors = section.collect_anchor_lines()?;
        Err(self
            .mismatch_error(
                section,
                canonical_path,
                normalized,
                tag,
                hash_recognized,
                anchors,
            )
            .to_string())
    }
}

fn has_anchor_scoped_edit(edits: &[Edit]) -> bool {
    edits.iter().any(|edit| match edit {
        Edit::Delete { .. } | Edit::Block { .. } => true,
        Edit::Insert { cursor, .. } => matches!(
            cursor,
            Cursor::BeforeAnchor { .. } | Cursor::AfterAnchor { .. }
        ),
    })
}

fn assert_section_hash_present(section_path: &str, file_hash: Option<&str>) -> Result<(), String> {
    if file_hash.is_some() {
        return Ok(());
    }
    Err(missing_snapshot_tag_message(section_path))
}

fn merge_warnings(sources: &[&[String]]) -> Vec<String> {
    let mut out = Vec::new();
    for source in sources {
        out.extend((*source).iter().cloned());
    }
    out
}

fn assert_unique_canonical_paths(prepared: &[PreparedSection]) -> Result<(), String> {
    let mut seen = std::collections::HashMap::new();
    for entry in prepared {
        if let Some(previous) =
            seen.insert(entry.canonical_path.clone(), entry.section.path.clone())
        {
            return Err(format!(
                "Multiple hashline sections resolve to the same file ({previous} and {}). Merge their ops under one header before applying.",
                entry.section.path
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::edit::hashline::format::compute_file_hash;
    use crate::tools::edit::hashline::fs::InMemoryFilesystem;
    use crate::tools::edit::hashline::snapshots::InMemorySnapshotStore;
    use crate::tools::edit::hashline::SplitOptions;

    const PATH: &str = "a.ts";

    fn make_patcher(
        fs: InMemoryFilesystem,
        snapshots: InMemorySnapshotStore,
    ) -> Patcher<InMemoryFilesystem, InMemorySnapshotStore> {
        Patcher::new(PatcherOptions {
            fs,
            snapshots,
            block_resolver: None,
        })
    }

    #[test]
    fn requires_snapshot_store_at_construction() {
        let fs = InMemoryFilesystem::new();
        let snapshots = InMemorySnapshotStore::new();
        let patcher = make_patcher(fs, snapshots);
        assert!(patcher.snapshots.head(PATH).is_none());
    }

    #[test]
    fn applies_when_section_tag_matches_live_hash() {
        let fs = InMemoryFilesystem::with_files([(PATH, "before\n".to_string())]);
        let snapshots = InMemorySnapshotStore::new();
        let tag = snapshots.record(PATH, "before\n");
        let patcher = make_patcher(fs.clone(), snapshots);
        let patch = Patch::parse(
            &format!("¶{PATH}#{tag}\nreplace 1..1:\n+after"),
            SplitOptions::default(),
        )
        .expect("parse");
        let result = patcher.apply(&patch).expect("apply");
        let section = &result.sections[0];
        assert_eq!(section.op, PatchOp::Update);
        assert!(section.file_hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(section.file_hash.len(), 4);
        assert_ne!(section.file_hash, tag);
        assert_eq!(fs.get(PATH), Some("after\n".to_string()));
    }

    #[test]
    fn validates_anchor_from_content_hash_without_recorded_snapshot() {
        let content = "l1\nl2\nl3\nl4\nl5\n";
        let fs = InMemoryFilesystem::with_files([(PATH, content.to_string())]);
        let snapshots = InMemorySnapshotStore::new();
        let tag = compute_file_hash(content);
        assert!(snapshots.by_hash(PATH, &tag).is_none());
        let patcher = make_patcher(fs.clone(), snapshots);
        let patch = Patch::parse(
            &format!("¶{PATH}#{tag}\nreplace 3..3:\n+L3"),
            SplitOptions::default(),
        )
        .expect("parse");
        let result = patcher.apply(&patch).expect("apply");
        assert_eq!(result.sections[0].op, PatchOp::Update);
        assert_eq!(fs.get(PATH), Some("l1\nl2\nL3\nl4\nl5\n".to_string()));
    }

    #[test]
    fn normalizes_lowercase_section_tags_while_parsing() {
        let section = Patch::parse_single(
            &format!("¶{PATH}#1a2b\nreplace 1..1:\n+after"),
            SplitOptions::default(),
        )
        .expect("parse");
        assert_eq!(section.file_hash.as_deref(), Some("1A2B"));
    }

    #[test]
    fn refuses_mismatch_when_recorded_version_drifted() {
        let fs = InMemoryFilesystem::with_files([(PATH, "drifted\n".to_string())]);
        let snapshots = InMemorySnapshotStore::new();
        let tag = snapshots.record(PATH, "before\n");
        let patcher = make_patcher(fs.clone(), snapshots);
        let patch = Patch::parse(
            &format!("¶{PATH}#{tag}\nreplace 1..1:\n+after"),
            SplitOptions::default(),
        )
        .expect("parse");
        let error = patcher.apply(&patch).expect_err("mismatch");
        assert!(
            error.contains("file changed between read and edit"),
            "{error}"
        );
        assert!(error.contains("Section is bound to #"), "{error}");
        assert_eq!(fs.get(PATH), Some("drifted\n".to_string()));
    }

    #[test]
    fn refuses_unrecorded_tag_with_session_diagnostic() {
        let fs = InMemoryFilesystem::with_files([(PATH, "current\n".to_string())]);
        let snapshots = InMemorySnapshotStore::new();
        let patcher = make_patcher(fs.clone(), snapshots);
        let live = compute_file_hash("current\n");
        let bogus = if live == "FFFF" { "0000" } else { "FFFF" };
        let patch = Patch::parse(
            &format!("¶{PATH}#{bogus}\nreplace 1..1:\n+after"),
            SplitOptions::default(),
        )
        .expect("parse");
        let error = patcher.apply(&patch).expect_err("mismatch");
        assert!(
            error.contains(&format!("hash #{bogus} is not from this session")),
            "{error}"
        );
        assert!(error.contains("never invent the tag"), "{error}");
        assert!(error.contains("current file hashes to #"), "{error}");
        assert_eq!(fs.get(PATH), Some("current\n".to_string()));
    }
}
