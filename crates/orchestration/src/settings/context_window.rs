//! Bundled context-window defaults and lookup for provider profiles.

use std::collections::BTreeMap;
use std::sync::LazyLock;

static DEFAULT_CONTEXT_WINDOW_SIZES: LazyLock<BTreeMap<String, u32>> =
    LazyLock::new(load_default_context_window_sizes);

fn load_default_context_window_sizes() -> BTreeMap<String, u32> {
    const JSON: &str = include_str!("../../resources/context_window_sizes.json");
    serde_json::from_str(JSON).unwrap_or_default()
}

/// Bundled default context-window sizes for well-known models.
#[must_use]
pub fn default_context_window_sizes() -> BTreeMap<String, u32> {
    DEFAULT_CONTEXT_WINDOW_SIZES.clone()
}

/// Look up context window size: profile overrides first, then bundled defaults.
#[must_use]
pub fn lookup_context_window_size(
    provider_overrides: &BTreeMap<String, u32>,
    model: &str,
) -> Option<u32> {
    if let Some(&size) = provider_overrides.get(model) {
        return Some(size);
    }
    let lower = model.to_lowercase();
    for (key, &size) in provider_overrides {
        if key.eq_ignore_ascii_case(&lower) {
            return Some(size);
        }
    }
    let defaults = &*DEFAULT_CONTEXT_WINDOW_SIZES;
    if let Some(&size) = defaults.get(model) {
        return Some(size);
    }
    for (key, &size) in defaults {
        if key.eq_ignore_ascii_case(&lower) {
            return Some(size);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_in_overrides() {
        let mut overrides = BTreeMap::new();
        overrides.insert("my-custom-model".into(), 42_000);
        assert_eq!(
            lookup_context_window_size(&overrides, "my-custom-model"),
            Some(42_000)
        );
    }

    #[test]
    fn bundled_defaults() {
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "gpt-4o"),
            Some(128_000)
        );
    }

    #[test]
    fn case_insensitive_lookup() {
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "GPT-4o"),
            Some(128_000)
        );
    }
}
