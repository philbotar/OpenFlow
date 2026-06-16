use std::path::Path;

use super::render::{render_text, DEFAULT_READ_LINE_LIMIT};
use super::selector::ReadSelector;

const SUMMARY_EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt"];

pub fn should_summarize_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SUMMARY_EXTENSIONS.contains(&ext))
}

pub fn structural_summary(label: &str, text: &str) -> Option<String> {
    let items = extract_outline_items(text);
    if items.is_empty() {
        return None;
    }
    let mut output = format!("¶{label} (structural summary)\n");
    for item in items {
        output.push_str(&item);
        output.push('\n');
    }
    output.push_str(
        "\n… use :{start}-{end} or :raw for full content; summary elisions are approximate …",
    );
    Some(output)
}

pub fn render_read(label: &str, text: &str, selector: ReadSelector) -> String {
    if matches!(selector, ReadSelector::None) && should_summarize_path(label) {
        if let Some(summary) = structural_summary(label, text) {
            return summary;
        }
    }
    render_text(label, text, selector)
}

fn extract_outline_items(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        depth = depth
            .saturating_add(trimmed.matches('{').count())
            .saturating_sub(trimmed.matches('}').count());
        if is_outline_line(trimmed) && depth <= 1 {
            items.push(format!("{}: {}", index + 1, trimmed));
        }
        if items.len() >= DEFAULT_READ_LINE_LIMIT {
            break;
        }
    }
    items
}

fn is_outline_line(line: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "pub fn ",
        "fn ",
        "async fn ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "pub trait ",
        "trait ",
        "impl ",
        "pub mod ",
        "mod ",
        "class ",
        "interface ",
        "type ",
        "export function ",
        "export class ",
        "def ",
        "func ",
    ];
    PREFIXES.iter().any(|prefix| line.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::{render_read, structural_summary};
    use crate::tool::read::selector::ReadSelector;

    #[test]
    fn structural_summary_lists_top_level_items() {
        let source = "fn alpha() {}\n\nstruct Beta;\n\nfn gamma() {}\n";
        let summary = structural_summary("lib.rs", source).expect("summary");
        assert!(summary.contains("fn alpha"));
        assert!(summary.contains("struct Beta"));
        assert!(summary.contains("structural summary"));
    }

    #[test]
    fn bare_code_read_prefers_summary() {
        let source = "pub fn hello() {\n    println!(\"hi\");\n}\n";
        let rendered = render_read("main.rs", source, ReadSelector::None);
        assert!(rendered.contains("pub fn hello"));
        assert!(!rendered.contains("println"));
    }

    #[test]
    fn plain_text_read_stays_numbered() {
        let rendered = render_read("notes.txt", "alpha\nbeta\n", ReadSelector::None);
        assert!(rendered.contains("1:alpha"));
        assert!(rendered.contains("2:beta"));
    }
}
