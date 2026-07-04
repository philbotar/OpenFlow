#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadSelector {
    None,
    Raw,
    Lines { ranges: Vec<LineRange>, raw: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

// Allow line-range suffixes on partial paths (e.g. `src/lib:10-20`).
pub fn split_selector(path: &str) -> (String, ReadSelector) {
    let Some(index) = path.rfind(':') else {
        return (path.to_string(), ReadSelector::None);
    };
    let base = &path[..index];
    let suffix = &path[index + 1..];

    if suffix.eq_ignore_ascii_case("raw") {
        let (maybe_base, selector) = split_selector(base);
        if let ReadSelector::Lines { ranges, .. } = selector {
            return (maybe_base, ReadSelector::Lines { ranges, raw: true });
        }
        return (base.to_string(), ReadSelector::Raw);
    }

    if let Some(ranges) = parse_ranges(suffix) {
        let (maybe_base, selector) = split_selector(base);
        let raw = matches!(selector, ReadSelector::Raw);
        let base = if raw { maybe_base } else { base.to_string() };
        return (base, ReadSelector::Lines { ranges, raw });
    }

    (path.to_string(), ReadSelector::None)
}

fn parse_ranges(input: &str) -> Option<Vec<LineRange>> {
    let mut ranges = Vec::new();
    for part in input.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return None;
        }
        ranges.push(parse_range(trimmed)?);
    }
    ranges.sort_by_key(|range| range.start);
    ranges.dedup_by(|left, right| left.start == right.start && left.end == right.end);
    Some(merge_ranges(ranges))
}

fn parse_range(input: &str) -> Option<LineRange> {
    if let Some((start, count)) = input.split_once('+') {
        let start = parse_line(start)?;
        let count = parse_line(count)?;
        return Some(LineRange {
            start,
            end: Some(start.saturating_add(count).saturating_sub(1)),
        });
    }
    if input.ends_with('-') {
        let start = parse_line(input.trim_end_matches('-'))?;
        return Some(LineRange { start, end: None });
    }
    if let Some((start, end)) = input.split_once('-') {
        let start = parse_line(start)?;
        let end = parse_line(end)?;
        return Some(LineRange {
            start,
            end: Some(end.max(start)),
        });
    }
    let start = parse_line(input)?;
    Some(LineRange {
        start,
        end: Some(start),
    })
}

fn parse_line(input: &str) -> Option<usize> {
    let value = input.trim().parse::<usize>().ok()?;
    (value > 0).then_some(value)
}

fn merge_ranges(mut ranges: Vec<LineRange>) -> Vec<LineRange> {
    if ranges.is_empty() {
        return ranges;
    }
    let mut merged = Vec::new();
    let mut current = ranges.remove(0);
    for next in ranges {
        let current_end = current.end.unwrap_or(usize::MAX);
        let next_end = next.end.unwrap_or(usize::MAX);
        if next.start <= current_end.saturating_add(1) {
            current.end = Some(current_end.max(next_end));
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);
    merged
}

#[cfg(test)]
mod tests {
    use super::{split_selector, LineRange, ReadSelector};

    #[test]
    fn parses_raw_selector() {
        assert_eq!(
            split_selector("src/lib.rs:raw"),
            ("src/lib.rs".to_string(), ReadSelector::Raw)
        );
    }

    #[test]
    fn parses_closed_range() {
        assert_eq!(
            split_selector("src/lib.rs:50-80"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![LineRange {
                        start: 50,
                        end: Some(80)
                    }],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_plus_range() {
        assert_eq!(
            split_selector("src/lib.rs:50+150"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![LineRange {
                        start: 50,
                        end: Some(199)
                    }],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_multi_range() {
        assert_eq!(
            split_selector("src/lib.rs:5-16,960-973"),
            (
                "src/lib.rs".to_string(),
                ReadSelector::Lines {
                    ranges: vec![
                        LineRange {
                            start: 5,
                            end: Some(16)
                        },
                        LineRange {
                            start: 960,
                            end: Some(973)
                        },
                    ],
                    raw: false,
                },
            ),
        );
    }

    #[test]
    fn parses_range_and_raw_selector_in_either_order() {
        assert_eq!(
            split_selector("src/lib.rs:2-4:raw").1,
            ReadSelector::Lines {
                ranges: vec![LineRange {
                    start: 2,
                    end: Some(4)
                }],
                raw: true,
            },
        );
        assert_eq!(
            split_selector("src/lib.rs:raw:2-4").1,
            ReadSelector::Lines {
                ranges: vec![LineRange {
                    start: 2,
                    end: Some(4)
                }],
                raw: true,
            },
        );
    }

    #[test]
    fn windows_paths_without_selector_stay_intact() {
        assert_eq!(
            split_selector(r"C:\repo\src\main.rs"),
            (r"C:\repo\src\main.rs".to_string(), ReadSelector::None)
        );
    }
}
