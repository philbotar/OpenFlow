//! Unit tests for line-sequence matching used by patch mode.

use super::replace_sequence::{
    find_context_line, seek_sequence, ContextMatchStrategy, SequenceMatchStrategy,
};

fn lines(values: &[&str]) -> Vec<String> {
    values.iter().map(|line| (*line).to_string()).collect()
}

#[test]
fn seek_sequence_finds_exact_match() {
    let file = lines(&["alpha", "beta", "gamma"]);
    let pattern = lines(&["beta", "gamma"]);
    let result = seek_sequence(&file, &pattern, 0, false, false);
    assert_eq!(result.index, Some(1));
    assert_eq!(result.strategy, Some(SequenceMatchStrategy::Exact));
}

#[test]
fn seek_sequence_reports_multiple_exact_matches() {
    let file = lines(&["dup", "x", "dup", "y"]);
    let pattern = lines(&["dup"]);
    let result = seek_sequence(&file, &pattern, 0, false, false);
    assert_eq!(result.index, Some(0));
    assert_eq!(result.match_count, Some(2));
}

#[test]
fn seek_sequence_eof_starts_from_file_end() {
    let file = lines(&["keep", "tail", "end"]);
    let pattern = lines(&["tail", "end"]);
    let result = seek_sequence(&file, &pattern, 0, true, false);
    assert_eq!(result.index, Some(1));
}

#[test]
fn find_context_line_matches_trimmed_line() {
    let file = lines(&["  fn main() {", "    run();", "  }"]);
    let result = find_context_line(&file, "fn main() {", 0, false, false);
    assert_eq!(result.index, Some(0));
    assert_eq!(result.strategy, Some(ContextMatchStrategy::Trim));
}
