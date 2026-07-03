//! Hashline patch format — mechanical port of `@oh-my-pi/hashline`.

pub mod apply;
pub mod block;
pub mod boundary_repair;
pub mod execute;
pub mod format;
pub mod fs;
pub mod input;
pub mod messages;
pub mod mismatch;
pub mod parser;
pub mod patcher;
pub mod prefixes;
pub mod recovery;
pub mod snapshots;
pub mod tokenizer;
pub mod types;

pub use apply::apply_edits;
pub use block::{has_block_edit, resolve_block_edits, OnUnresolved, ResolveBlockEditsOptions};
pub use execute::execute_hashline;
pub use format::{
    compute_file_hash, format_delete_header, format_hashline_header, format_insert_header,
    format_numbered_line, format_numbered_lines, format_replace_header, HL_FILE_HASH_EXAMPLES,
    HL_FILE_HASH_LENGTH, HL_FILE_HASH_SEP, HL_FILE_PREFIX,
};
pub use fs::{HashlineFilesystem, InMemoryFilesystem, NotFoundError, WriteResult};
pub use input::{contains_recognizable_hashline_operations, Patch, PatchSection};
pub use messages::{missing_snapshot_tag_message, HEADTAIL_DRIFT_WARNING, MISMATCH_CONTEXT};
pub use mismatch::{MismatchDetails, MismatchError};
pub use parser::{parse_patch, parse_patch_streaming, Executor, ParseResult};
pub use patcher::{
    PatchOp, PatchSectionResult, Patcher, PatcherApplyResult, PatcherOptions, PreparedSection,
};
pub use prefixes::{hashline_parse_text, strip_hashline_prefixes, strip_new_line_prefixes};
pub use recovery::{recovery_to_apply_result, Recovery, RecoveryArgs, RecoveryResult};
pub use snapshots::{InMemorySnapshotStore, InMemorySnapshotStoreOptions, Snapshot, SnapshotStore};
pub use tokenizer::{parse_lid, split_hashline_lines, BlockTarget, Token, Tokenizer};
pub use types::{
    Anchor, ApplyResult, BlockResolver, BlockResolverRequest, BlockSpan, Cursor, Edit, InsertMode,
    ParsedRange, SplitOptions,
};
