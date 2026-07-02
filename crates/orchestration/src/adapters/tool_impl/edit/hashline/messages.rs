//! Centralized error and warning text for the hashline parser, applier, and patcher.

use super::format::{HL_FILE_HASH_SEP, HL_FILE_PREFIX};

/// Lines of context shown either side of a hash mismatch.
pub const MISMATCH_CONTEXT: u32 = 2;

/// Optional patch envelope start marker; silently consumed when present.
pub const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";

/// Optional patch envelope end marker; terminates parsing when encountered.
pub const END_PATCH_MARKER: &str = "*** End Patch";

/// Recovery sentinel for truncated tool-call streams.
pub const ABORT_MARKER: &str = "*** Abort";

pub const BARE_BODY_AUTO_PIPED_WARNING: &str = "Auto-prefixed bare body row(s) with `+`. Body rows must be `+TEXT` literal lines; pasting raw code as payload is not a portable shape.";

pub const MINUS_ROW_REJECTED: &str = "`-` rows are not valid; hashline ranges already name the lines being changed. To insert a literal line starting with `-`, write `+-…`.";

pub const EMPTY_REPLACE: &str =
    "`replace N..M:` needs at least one `+TEXT` body row. To delete lines, use `delete N..M`.";

pub const EMPTY_BLOCK: &str = "`replace block N:` needs at least one `+TEXT` body row. To delete a block, use `delete N..M` with the block's line range.";

pub fn block_unresolved_message(line: u32) -> String {
    format!(
        "`replace block {line}:` could not resolve a syntactic block beginning on line {line}. \
         The language may be unsupported, the line may be blank or a closing delimiter, or the block may not parse. \
         Use `replace {line}..M:` with the block's explicit end line instead."
    )
}

pub const BLOCK_RESOLVER_UNAVAILABLE: &str =
    "`replace block N:` is not available here (no tree-sitter block resolver is configured). Use `replace N..M:` with an explicit range.";

pub const UNRESOLVED_BLOCK_INTERNAL: &str =
    "internal error: unresolved `replace block` edit reached the applier (resolveBlockEdits was not run).";

pub const DELETE_TAKES_NO_BODY: &str =
    "`delete N..M` does not take body rows. Remove the body, or use `replace N..M:`.";

pub const DELETE_BLOCK_TAKES_NO_BODY: &str =
    "`delete block N` does not take body rows. Remove the body, or use `replace block N:` to replace the block.";

pub const EMPTY_INSERT: &str = "`insert` needs at least one `+TEXT` body row.";

pub const RECOVERY_EXTERNAL_WARNING: &str =
    "Recovered from a stale file hash using a previous read snapshot (file changed externally between read and edit).";

pub const RECOVERY_SESSION_CHAIN_WARNING: &str =
    "Recovered from a stale file hash using an earlier in-session snapshot (the file hash advanced after a prior edit in this session).";

pub const RECOVERY_SESSION_REPLAY_WARNING: &str =
    "Recovered by replaying your edits onto the current file content — your previous edit in this session changed line(s) you re-targeted with a stale hash. Verify the diff matches your intent before continuing.";

pub const HEADTAIL_DRIFT_WARNING: &str =
    "Applied an `insert head:`/`insert tail:` edit onto the current file content even though the snapshot tag was stale (the file changed since your read). Head/tail position is content-independent, so the insert was not rejected — but re-read if the drift was unexpected.";

pub fn missing_snapshot_tag_message(section_path: &str) -> String {
    format!(
        "Missing hashline snapshot tag for edit to {section_path}; use `{HL_FILE_PREFIX}{section_path}{HL_FILE_HASH_SEP}tag` from your latest read/search output. To create a new file, use the write tool."
    )
}
