pub mod errors;
pub mod normalize;
pub mod replace;

pub use errors::{ApplyPatchError, EditMatchError, FuzzyMatch, ParseError};
pub use normalize::adjust_indentation;
pub use replace::{find_match, FindMatchOptions, MatchOutcome, DEFAULT_FUZZY_THRESHOLD};

#[cfg(test)]
mod normalize_tests;

#[cfg(test)]
mod replace_tests;

#[cfg(test)]
mod patch_tests;
