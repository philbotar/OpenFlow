use super::normalize::normalize_for_fuzzy;
use thiserror::Error;

/// Closest fuzzy match candidate when search text is not found.
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyMatch {
    pub actual_text: String,
    pub start_index: usize,
    pub start_line: usize,
    pub confidence: f64,
}

#[derive(Debug, Error, Clone, PartialEq)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    pub line_number: Option<usize>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, line_number: Option<usize>) -> Self {
        let message = message.into();
        let display = match line_number {
            Some(n) => format!("Line {n}: {message}"),
            None => message.clone(),
        };
        Self {
            message: display,
            line_number,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct ApplyPatchError(pub String);

#[derive(Debug, Error, Clone, PartialEq)]
pub struct EditMatchError {
    pub path: String,
    pub search_text: String,
    pub closest: Option<FuzzyMatch>,
    pub allow_fuzzy: bool,
    pub threshold: f64,
    pub fuzzy_matches: Option<usize>,
}

impl EditMatchError {
    pub fn new(
        path: impl Into<String>,
        search_text: impl Into<String>,
        closest: Option<FuzzyMatch>,
        allow_fuzzy: bool,
        threshold: f64,
        fuzzy_matches: Option<usize>,
    ) -> Self {
        Self {
            path: path.into(),
            search_text: search_text.into(),
            closest,
            allow_fuzzy,
            threshold,
            fuzzy_matches,
        }
    }

    pub fn format_message(&self) -> String {
        Self::format_message_with(
            &self.path,
            &self.search_text,
            self.closest.as_ref(),
            self.allow_fuzzy,
            self.threshold,
            self.fuzzy_matches,
        )
    }

    pub fn format_message_with(
        path: &str,
        search_text: &str,
        closest: Option<&FuzzyMatch>,
        allow_fuzzy: bool,
        threshold: f64,
        fuzzy_matches: Option<usize>,
    ) -> String {
        let Some(closest) = closest else {
            return if allow_fuzzy {
                format!("Could not find a close enough match in {path}.")
            } else {
                format!(
                    "Could not find the exact text in {path}. The old text must match exactly including all whitespace and newlines."
                )
            };
        };

        let similarity = (closest.confidence * 100.0).round() as i64;
        let search_lines: Vec<&str> = search_text.split('\n').collect();
        let actual_lines: Vec<&str> = closest.actual_text.split('\n').collect();
        let (old_line, new_line) = find_first_different_line(&search_lines, &actual_lines);
        let threshold_percent = (threshold * 100.0).round() as i64;

        let hint = if allow_fuzzy {
            if let Some(n) = fuzzy_matches.filter(|&n| n > 1) {
                format!(
                    "Found {n} high-confidence matches. Provide more context to make it unique."
                )
            } else {
                format!("Closest match was below the {threshold_percent}% similarity threshold.")
            }
        } else {
            "Fuzzy matching is disabled. Enable 'Edit fuzzy match' in settings to accept high-confidence matches.".to_string()
        };

        let header = if allow_fuzzy {
            format!("Could not find a close enough match in {path}.")
        } else {
            format!("Could not find the exact text in {path}.")
        };

        format!(
            "{header}\n\nClosest match ({similarity}% similar) at line {}:\n  - {old_line}\n  + {new_line}\n{hint}",
            closest.start_line
        )
    }
}

impl std::fmt::Display for EditMatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_message())
    }
}

fn find_first_different_line(old_lines: &[&str], new_lines: &[&str]) -> (String, String) {
    let max = old_lines.len().max(new_lines.len());
    for i in 0..max {
        let old_line = old_lines.get(i).copied().unwrap_or("");
        let new_line = new_lines.get(i).copied().unwrap_or("");
        if old_line != new_line {
            return (old_line.to_string(), new_line.to_string());
        }
    }

    for i in 0..max {
        let old_line = old_lines.get(i).copied().unwrap_or("");
        let new_line = new_lines.get(i).copied().unwrap_or("");
        if normalize_for_fuzzy(old_line) != normalize_for_fuzzy(new_line) {
            return (old_line.to_string(), new_line.to_string());
        }
    }

    (
        "(matched region differs only in whitespace or indentation)".to_string(),
        String::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_includes_line_number() {
        let err = ParseError::new("bad hunk", Some(3));
        assert_eq!(err.to_string(), "Line 3: bad hunk");
    }

    #[test]
    fn edit_match_error_no_closest_exact_mode() {
        let msg = EditMatchError::format_message_with("foo.rs", "old", None, false, 0.95, None);
        assert!(msg.contains("exact text"));
        assert!(msg.contains("foo.rs"));
    }

    #[test]
    fn edit_match_error_with_closest_includes_similarity() {
        let closest = FuzzyMatch {
            actual_text: "bar".into(),
            start_index: 0,
            start_line: 2,
            confidence: 0.87,
        };
        let msg =
            EditMatchError::format_message_with("foo.rs", "baz", Some(&closest), true, 0.95, None);
        assert!(msg.contains("87% similar"));
        assert!(msg.contains("line 2"));
    }

    #[test]
    fn edit_match_error_display_delegates_to_format_message() {
        let err = EditMatchError::new("foo.rs", "old", None, false, 0.95, None);
        assert_eq!(err.to_string(), err.format_message());
    }

    #[test]
    fn edit_match_error_fuzzy_disabled_with_closest() {
        let closest = FuzzyMatch {
            actual_text: "bar".into(),
            start_index: 0,
            start_line: 1,
            confidence: 0.9,
        };
        let msg =
            EditMatchError::format_message_with("foo.rs", "baz", Some(&closest), false, 0.95, None);
        assert!(msg.contains("Fuzzy matching is disabled"));
        assert!(msg.contains("exact text"));
    }

    #[test]
    fn edit_match_error_multiple_fuzzy_matches_hint() {
        let closest = FuzzyMatch {
            actual_text: "item1".into(),
            start_index: 0,
            start_line: 1,
            confidence: 0.75,
        };
        let msg = EditMatchError::format_message_with(
            "foo.rs",
            "itemX",
            Some(&closest),
            true,
            0.7,
            Some(3),
        );
        assert!(msg.contains("Found 3 high-confidence matches"));
    }

    #[test]
    fn edit_match_error_identical_lines_shows_whitespace_hint() {
        let closest = FuzzyMatch {
            actual_text: "foo\nbar".into(),
            start_index: 0,
            start_line: 1,
            confidence: 0.8,
        };
        let msg = EditMatchError::format_message_with(
            "foo.rs",
            "foo\nbar",
            Some(&closest),
            true,
            0.95,
            None,
        );
        assert!(msg.contains("whitespace or indentation"));
    }
}
