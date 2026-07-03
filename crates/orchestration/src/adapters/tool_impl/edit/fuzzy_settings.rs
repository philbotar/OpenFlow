//! Env-based fuzzy match toggles shared by edit / apply_patch / preview.

use super::replace::DEFAULT_FUZZY_THRESHOLD;

pub(crate) fn allow_fuzzy() -> bool {
    !matches!(
        std::env::var("PI_EDIT_FUZZY").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}

pub(crate) fn edit_fuzzy_threshold() -> Option<f64> {
    std::env::var("PI_EDIT_FUZZY_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
}

pub(crate) fn patch_fuzzy_threshold() -> f64 {
    std::env::var("PI_EDIT_FUZZY_THRESHOLD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_FUZZY_THRESHOLD)
}
