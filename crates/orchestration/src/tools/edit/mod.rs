pub mod apply_patch;
pub mod diff;
pub mod errors;
pub mod io;
pub mod normalize;
pub mod patch;
pub mod path;
pub mod replace;
pub mod replace_sequence;

pub use diff::{
    generate_diff_string, normalize_create_content, normalize_diff, parse_diff_hunks, replace_text,
    DiffHunk, DiffResult, ReplaceOptions, ReplaceResult,
};
pub use errors::{ApplyPatchError, EditMatchError, FuzzyMatch, ParseError};
pub use normalize::{
    adjust_indentation, detect_line_ending, min_indent, normalize_to_lf, normalize_unicode,
    restore_line_endings, strip_bom, BomResult, LineEnding,
};
pub use apply_patch::{
    expand_apply_patch_to_inputs, parse_apply_patch, parse_apply_patch_streaming,
};
pub use io::{EditIo, EditIoError};
pub use path::{resolve_writable, PathEscapeError};
pub use patch::{
    apply_patch_entry, PatchApplyResult, PatchError, PatchFileSystem, PatchInput, PatchOp,
    PatchOptions, PatchVerifyError, StdPatchFileSystem,
};
pub use replace::{find_match, FindMatchOptions, MatchOutcome, DEFAULT_FUZZY_THRESHOLD};

#[cfg(test)]
mod diff_tests;

#[cfg(test)]
mod normalize_tests;

#[cfg(test)]
mod replace_tests;

#[cfg(test)]
mod apply_patch_tests;

#[cfg(test)]
mod patch_tests;

#[cfg(test)]
mod replace_sequence_tests;
