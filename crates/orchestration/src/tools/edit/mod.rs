pub mod diff;
pub mod errors;
pub mod normalize;
pub mod replace;

pub use diff::{
    generate_diff_string, normalize_create_content, normalize_diff, parse_diff_hunks, replace_text,
    DiffHunk, DiffResult, ReplaceOptions, ReplaceResult,
};
pub use errors::{ApplyPatchError, EditMatchError, FuzzyMatch, ParseError};
pub use normalize::{
    adjust_indentation, detect_line_ending, min_indent, normalize_to_lf, normalize_unicode,
    restore_line_endings, strip_bom, BomResult, LineEnding,
};
pub use replace::{find_match, FindMatchOptions, MatchOutcome, DEFAULT_FUZZY_THRESHOLD};

#[cfg(test)]
mod diff_tests;

#[cfg(test)]
mod normalize_tests;

#[cfg(test)]
mod replace_tests;

#[cfg(test)]
mod patch_tests;
