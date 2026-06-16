use super::selector::{LineRange, ReadSelector};

pub const DEFAULT_READ_LINE_LIMIT: usize = 3000;
const MAX_LINE_COLUMNS: usize = 2000;

pub fn render_text(label: &str, text: &str, selector: ReadSelector) -> String {
    match selector {
        ReadSelector::Raw => text.to_string(),
        ReadSelector::Conflicts => format!("¶{label}\n{text}"),
        ReadSelector::Lines { ranges, raw } => render_ranges(label, text, &ranges, raw),
        ReadSelector::None => render_default_numbered(label, text, DEFAULT_READ_LINE_LIMIT),
    }
}

fn render_default_numbered(label: &str, text: &str, line_limit: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();
    let shown = lines
        .iter()
        .take(line_limit)
        .enumerate()
        .map(|(index, line)| format_numbered_line(index + 1, line))
        .collect::<Vec<_>>();
    let mut output = format!("¶{label}\n{}", shown.join("\n"));
    if total_lines > line_limit {
        output.push_str(&format!(
            "\n… truncated at line {line_limit} of {total_lines}; use :{{start}}-{{end}} or :raw to read more …"
        ));
    }
    output
}

fn render_ranges(label: &str, text: &str, ranges: &[LineRange], raw: bool) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    for range in ranges {
        let start_index = range.start.saturating_sub(1);
        if start_index >= lines.len() {
            continue;
        }
        let end_index = range
            .end
            .map(|end| end.min(lines.len()))
            .unwrap_or(lines.len());
        if end_index <= start_index {
            continue;
        }
        let slice = &lines[start_index..end_index];
        if raw {
            chunks.push(slice.join("\n"));
        } else {
            let numbered = slice
                .iter()
                .enumerate()
                .map(|(offset, line)| format_numbered_line(start_index + offset + 1, line))
                .collect::<Vec<_>>()
                .join("\n");
            chunks.push(numbered);
        }
    }
    if chunks.is_empty() {
        return format!("¶{label}\n(no lines in requested range)");
    }
    if raw && chunks.len() == 1 {
        return chunks.remove(0);
    }
    format!("¶{label}\n{}", chunks.join("\n…\n"))
}

fn format_numbered_line(line_number: usize, line: &str) -> String {
    let clipped = clip_line_columns(line);
    format!("{line_number}:{clipped}")
}

fn clip_line_columns(line: &str) -> String {
    if line.chars().count() <= MAX_LINE_COLUMNS {
        return line.to_string();
    }
    let clipped: String = line.chars().take(MAX_LINE_COLUMNS).collect();
    format!("{clipped}…")
}

#[cfg(test)]
mod tests {
    use super::{render_text, DEFAULT_READ_LINE_LIMIT};
    use crate::tool::read::selector::{LineRange, ReadSelector};

    #[test]
    fn default_read_caps_at_configured_limit() {
        let text = (1..=DEFAULT_READ_LINE_LIMIT + 5)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render_text("big.txt", &text, ReadSelector::None);
        assert!(rendered.contains(&format!(
            "{DEFAULT_READ_LINE_LIMIT}:line-{DEFAULT_READ_LINE_LIMIT}"
        )));
        assert!(!rendered.contains(&format!(
            "{}:line-{}",
            DEFAULT_READ_LINE_LIMIT + 1,
            DEFAULT_READ_LINE_LIMIT + 1
        )));
    }

    #[test]
    fn multi_range_renders_numbered_slices() {
        let text = (1..=10)
            .map(|index| format!("v{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render_text(
            "sample.txt",
            &text,
            ReadSelector::Lines {
                ranges: vec![
                    LineRange {
                        start: 2,
                        end: Some(3),
                    },
                    LineRange {
                        start: 8,
                        end: Some(9),
                    },
                ],
                raw: false,
            },
        );
        assert!(rendered.contains("2:v2"));
        assert!(rendered.contains("9:v9"));
        assert!(!rendered.contains("5:v5"));
    }
}
