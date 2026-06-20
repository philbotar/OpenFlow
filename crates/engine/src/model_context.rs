//! Context window size lookup — config-driven with bundled defaults.
//!
//! Actual model-to-size mappings live in the user's provider settings
//! (`ProviderProfile::context_window_sizes`). This module supplies only the
//! bundled default map and a pure lookup helper so that no model name is
//! hardcoded into business logic.

use std::collections::BTreeMap;

/// Bundled default context-window sizes for well-known models.
///
/// These are seeded into each provider profile at creation time and can be
/// overridden or extended by the user through settings.
#[must_use]
pub fn default_context_window_sizes() -> BTreeMap<String, u32> {
    let mut m = BTreeMap::new();

    // ── OpenAI ──────────────────────────────────────────────────────────
    m.insert("gpt-4o".into(), 128_000);
    m.insert("gpt-4o-2024-05-13".into(), 128_000);
    m.insert("gpt-4o-2024-08-06".into(), 128_000);
    m.insert("gpt-4o-2024-11-20".into(), 128_000);
    m.insert("gpt-4o-mini".into(), 128_000);
    m.insert("gpt-4o-mini-2024-07-18".into(), 128_000);
    m.insert("gpt-4-turbo".into(), 128_000);
    m.insert("gpt-4-turbo-2024-04-09".into(), 128_000);
    m.insert("gpt-4-turbo-preview".into(), 128_000);
    m.insert("gpt-4".into(), 8_192);
    m.insert("gpt-4-0613".into(), 8_192);
    m.insert("gpt-4-0314".into(), 8_192);
    m.insert("gpt-3.5-turbo".into(), 16_385);
    m.insert("gpt-3.5-turbo-0125".into(), 16_385);
    m.insert("gpt-3.5-turbo-1106".into(), 16_385);
    m.insert("o1".into(), 200_000);
    m.insert("o1-2024-12-17".into(), 200_000);
    m.insert("o1-mini".into(), 128_000);
    m.insert("o1-mini-2024-09-12".into(), 128_000);
    m.insert("o3".into(), 200_000);
    m.insert("o3-2025-04-16".into(), 200_000);
    m.insert("o3-mini".into(), 200_000);
    m.insert("o3-mini-2025-01-31".into(), 200_000);
    m.insert("o4-mini".into(), 200_000);
    m.insert("o4-mini-2025-04-16".into(), 200_000);

    // ── Anthropic ───────────────────────────────────────────────────────
    m.insert("claude-sonnet-4-20250514".into(), 200_000);
    m.insert("claude-3-5-sonnet-20241022".into(), 200_000);
    m.insert("claude-3-5-sonnet-latest".into(), 200_000);
    m.insert("claude-3-5-haiku-20241022".into(), 200_000);
    m.insert("claude-3-haiku-20240307".into(), 200_000);
    m.insert("claude-3-opus-20240229".into(), 200_000);
    m.insert("claude-3-sonnet-20240229".into(), 200_000);

    // ── Google Gemini ───────────────────────────────────────────────────
    m.insert("gemini-2.5-pro-preview-05-06".into(), 1_000_000);
    m.insert("gemini-2.5-pro-preview-03-25".into(), 1_000_000);
    m.insert("gemini-2.5-flash-preview-04-17".into(), 1_000_000);
    m.insert("gemini-2.0-flash".into(), 1_000_000);
    m.insert("gemini-2.0-flash-lite".into(), 1_000_000);
    m.insert("gemini-1.5-pro".into(), 2_000_000);
    m.insert("gemini-1.5-flash".into(), 1_000_000);

    m
}

/// Look up the context window size for `model` using the per-provider override
/// map first, then falling back to the bundled defaults.
///
/// Returns `None` when no size is known (caller should show an indeterminate
/// / unknown state in the UI).
#[must_use]
pub fn lookup_context_window_size(
    provider_overrides: &BTreeMap<String, u32>,
    model: &str,
) -> Option<u32> {
    // 1. Exact match in user-provided overrides
    if let Some(&size) = provider_overrides.get(model) {
        return Some(size);
    }
    // 2. Case-insensitive match in overrides
    let lower = model.to_lowercase();
    for (key, &size) in provider_overrides {
        if key.to_lowercase() == lower {
            return Some(size);
        }
    }
    // 3. Fallback to bundled defaults (exact)
    let defaults = default_context_window_sizes();
    if let Some(&size) = defaults.get(model) {
        return Some(size);
    }
    // 4. Case-insensitive fallback
    for (key, &size) in &defaults {
        if key.to_lowercase() == lower {
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
    fn overrides_take_precedence() {
        let mut overrides = BTreeMap::new();
        overrides.insert("gpt-4o".into(), 999_999);
        assert_eq!(
            lookup_context_window_size(&overrides, "gpt-4o"),
            Some(999_999)
        );
    }

    #[test]
    fn bundled_defaults() {
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "gpt-4o"),
            Some(128_000)
        );
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "claude-3-5-sonnet-latest"),
            Some(200_000)
        );
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "unknown-model"),
            None
        );
    }

    #[test]
    fn case_insensitive_lookup() {
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "GPT-4o"),
            Some(128_000)
        );
        assert_eq!(
            lookup_context_window_size(&BTreeMap::new(), "O3-mini"),
            Some(200_000)
        );
    }

    #[test]
    fn defaults_map_has_entries() {
        let defaults = default_context_window_sizes();
        assert!(!defaults.is_empty());
        assert!(defaults.contains_key("gpt-4o"));
        assert!(defaults.contains_key("claude-sonnet-4-20250514"));
    }
}
