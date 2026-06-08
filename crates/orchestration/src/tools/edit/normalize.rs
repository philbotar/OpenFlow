//! Text normalization for the edit engine (OMP `normalize.ts` port).

/// Count leading whitespace characters in a line.
pub fn count_leading_whitespace(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Leading whitespace prefix of a line.
pub fn get_leading_whitespace(line: &str) -> &str {
    let end = count_leading_whitespace(line);
    &line[..end]
}

fn is_non_empty_line(line: &str) -> bool {
    !line.trim().is_empty()
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

struct IndentProfile {
    lines: Vec<String>,
    indent_char: Option<char>,
    space_only: bool,
    tab_only: bool,
    mixed: bool,
    unit: usize,
    non_empty_count: usize,
}

fn build_indent_profile(text: &str) -> IndentProfile {
    let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut indent_counts = Vec::new();
    let mut indent_char = None;
    let mut space_only = true;
    let mut tab_only = true;
    let mut mixed = false;
    let mut non_empty_count = 0;
    let mut unit = 0;

    for line in &lines {
        if !is_non_empty_line(line) {
            continue;
        }
        non_empty_count += 1;
        let indent = get_leading_whitespace(line);
        indent_counts.push(indent.len());
        if indent.contains(' ') {
            tab_only = false;
        }
        if indent.contains('\t') {
            space_only = false;
        }
        if indent.contains(' ') && indent.contains('\t') {
            mixed = true;
        }
        if !indent.is_empty() {
            let current_char = indent.chars().next().unwrap();
            if let Some(existing) = indent_char {
                if existing != current_char {
                    mixed = true;
                }
            } else {
                indent_char = Some(current_char);
            }
        }
    }

    if space_only && non_empty_count > 0 {
        let mut current = 0;
        for count in &indent_counts {
            if *count == 0 {
                continue;
            }
            current = if current == 0 {
                *count
            } else {
                gcd(current, *count)
            };
        }
        unit = current;
    }

    if tab_only && non_empty_count > 0 {
        unit = 1;
    }

    IndentProfile {
        lines,
        indent_char,
        space_only,
        tab_only,
        mixed,
        unit,
        non_empty_count,
    }
}

fn convert_leading_tabs_to_spaces(text: &str, spaces_per_tab: usize) -> String {
    if spaces_per_tab == 0 {
        return text.to_string();
    }
    text.split('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                return line.to_string();
            }
            let leading = get_leading_whitespace(line);
            if !leading.contains('\t') || leading.contains(' ') {
                return line.to_string();
            }
            let converted = " ".repeat(leading.len() * spaces_per_tab);
            format!("{converted}{trimmed}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_indentation_only_rewrite(old_text: &str, new_text: &str) -> bool {
    let old_lines: Vec<&str> = old_text.split('\n').collect();
    let new_lines: Vec<&str> = new_text.split('\n').collect();
    if old_lines.len() != new_lines.len() {
        return false;
    }
    old_lines
        .iter()
        .zip(new_lines.iter())
        .all(|(old, new)| old.trim() == new.trim())
}

fn maybe_convert_tab_indentation(
    old_profile: &IndentProfile,
    actual_profile: &IndentProfile,
    new_profile: &IndentProfile,
    new_text: &str,
) -> Option<String> {
    if !actual_profile.space_only
        || !old_profile.tab_only
        || !new_profile.tab_only
        || actual_profile.unit == 0
    {
        return None;
    }

    let line_count = old_profile.lines.len().min(actual_profile.lines.len());
    for i in 0..line_count {
        let old_line = &old_profile.lines[i];
        let actual_line = &actual_profile.lines[i];
        if !is_non_empty_line(old_line) || !is_non_empty_line(actual_line) {
            continue;
        }
        let old_indent = get_leading_whitespace(old_line);
        if old_indent.is_empty() {
            continue;
        }
        let actual_indent = get_leading_whitespace(actual_line);
        if actual_indent.len() != old_indent.len() * actual_profile.unit {
            return None;
        }
    }

    Some(convert_leading_tabs_to_spaces(
        new_text,
        actual_profile.unit,
    ))
}

fn compute_uniform_indent_delta(
    old_profile: &IndentProfile,
    actual_profile: &IndentProfile,
) -> Option<isize> {
    let line_count = old_profile.lines.len().min(actual_profile.lines.len());
    let mut deltas = Vec::new();
    for i in 0..line_count {
        let old_line = &old_profile.lines[i];
        let actual_line = &actual_profile.lines[i];
        if !is_non_empty_line(old_line) || !is_non_empty_line(actual_line) {
            continue;
        }
        deltas.push(
            count_leading_whitespace(actual_line) as isize
                - count_leading_whitespace(old_line) as isize,
        );
    }

    if deltas.is_empty() {
        return None;
    }

    let delta = deltas[0];
    if deltas.iter().all(|value| *value == delta) {
        Some(delta)
    } else {
        None
    }
}

fn apply_indent_delta(text: &str, delta: isize, indent_char: char) -> String {
    text.split('\n')
        .map(|line| {
            if !is_non_empty_line(line) {
                return line.to_string();
            }
            if delta > 0 {
                let prefix = indent_char.to_string().repeat(delta as usize);
                return format!("{prefix}{line}");
            }
            let to_remove = (-delta as usize).min(count_leading_whitespace(line));
            line[to_remove..].to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn detect_indent_char(text: &str) -> char {
    for line in text.split('\n') {
        let ws = get_leading_whitespace(line);
        if let Some(ch) = ws.chars().next() {
            return ch;
        }
    }
    ' '
}

/// Normalize a line for fuzzy comparison.
pub fn normalize_for_fuzzy(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut result = trimmed.to_string();
    for (from, to) in [
        ('\u{201C}', "\""),
        ('\u{201D}', "\""),
        ('\u{201E}', "\""),
        ('\u{201F}', "\""),
        ('\u{00AB}', "\""),
        ('\u{00BB}', "\""),
        ('\u{2018}', "'"),
        ('\u{2019}', "'"),
        ('\u{201A}', "'"),
        ('\u{201B}', "'"),
        ('`', "'"),
        ('\u{00B4}', "'"),
        ('\u{2010}', "-"),
        ('\u{2011}', "-"),
        ('\u{2012}', "-"),
        ('\u{2013}', "-"),
        ('\u{2014}', "-"),
        ('\u{2212}', "-"),
    ] {
        result = result.replace(from, to);
    }

    let mut collapsed = String::new();
    let mut prev_space = false;
    for ch in result.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                collapsed.push(' ');
                prev_space = true;
            }
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }
    collapsed
}

/// Adjust `new_text` indentation to match the delta between `old_text` and `actual_text`.
pub fn adjust_indentation(old_text: &str, actual_text: &str, new_text: &str) -> String {
    if old_text == actual_text {
        return new_text.to_string();
    }

    if is_indentation_only_rewrite(old_text, new_text) {
        return new_text.to_string();
    }

    let old_profile = build_indent_profile(old_text);
    let actual_profile = build_indent_profile(actual_text);
    let new_profile = build_indent_profile(new_text);

    if old_profile.non_empty_count == 0
        || actual_profile.non_empty_count == 0
        || new_profile.non_empty_count == 0
    {
        return new_text.to_string();
    }

    if old_profile.mixed || actual_profile.mixed || new_profile.mixed {
        return new_text.to_string();
    }

    if let (Some(old_char), Some(actual_char)) =
        (old_profile.indent_char, actual_profile.indent_char)
    {
        if old_char != actual_char {
            if let Some(converted) =
                maybe_convert_tab_indentation(&old_profile, &actual_profile, &new_profile, new_text)
            {
                return converted;
            }
            return new_text.to_string();
        }
    }

    let Some(delta) = compute_uniform_indent_delta(&old_profile, &actual_profile) else {
        return new_text.to_string();
    };

    if delta == 0 {
        return new_text.to_string();
    }

    if let (Some(new_char), Some(actual_char)) =
        (new_profile.indent_char, actual_profile.indent_char)
    {
        if new_char != actual_char {
            return new_text.to_string();
        }
    }

    let indent_char = actual_profile
        .indent_char
        .or(old_profile.indent_char)
        .unwrap_or_else(|| detect_indent_char(actual_text));

    apply_indent_delta(new_text, delta, indent_char)
}
