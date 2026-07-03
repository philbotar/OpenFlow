//! Text normalization for the edit engine (OMP `normalize.ts` port).

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Line ending style detected in file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    /// Classic Mac `\r`-only line endings.
    Cr,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

/// Result of stripping a UTF-8 BOM prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BomResult {
    pub bom: String,
    pub text: String,
}

/// Detect the first line-ending style in `content`.
pub fn detect_line_ending(content: &str) -> LineEnding {
    let crlf_idx = content.find("\r\n");
    let lf_idx = content.find('\n');
    let cr_idx = content.find('\r');

    if let (Some(crlf), Some(lf)) = (crlf_idx, lf_idx) {
        return if crlf < lf {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };
    }
    if crlf_idx.is_some() {
        return LineEnding::CrLf;
    }
    if lf_idx.is_some() {
        return LineEnding::Lf;
    }
    if cr_idx.is_some() {
        return LineEnding::Cr;
    }
    LineEnding::Lf
}

/// Normalize every line ending to LF.
pub fn normalize_to_lf(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            result.push('\n');
        } else {
            result.push(ch);
        }
    }
    result
}

/// Re-encode LF text with the requested line ending.
///
/// Input is normalized to LF first so callers may pass CRLF text without doubling `\r`.
pub fn restore_line_endings(text: &str, ending: LineEnding) -> String {
    let lf = normalize_to_lf(text);
    match ending {
        LineEnding::Lf => lf,
        LineEnding::CrLf => lf.replace('\n', "\r\n"),
        LineEnding::Cr => lf.replace('\n', "\r"),
    }
}

/// Strip a UTF-8 BOM if present.
pub fn strip_bom(content: &str) -> BomResult {
    if let Some(text) = content.strip_prefix('\u{FEFF}') {
        BomResult {
            bom: "\u{FEFF}".to_string(),
            text: text.to_string(),
        }
    } else {
        BomResult {
            bom: String::new(),
            text: content.to_string(),
        }
    }
}

static UNICODE_REPLACEMENTS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

fn unicode_replacements() -> &'static [(Regex, &'static str)] {
    UNICODE_REPLACEMENTS
        .get_or_init(|| {
            let patterns: &[(&str, &str)] = &[
                ("[\u{2010}-\u{2015}\u{2212}]", "-"),
                ("[\u{2018}-\u{201B}]", "'"),
                ("[\u{201C}-\u{201F}]", "\""),
                ("[\u{00A0}\u{2002}-\u{200A}\u{202F}\u{205F}\u{3000}]", " "),
                ("\u{2260}", "!="),
                ("\u{00BD}", "1/2"),
                ("[\u{200B}-\u{200D}\u{FEFF}]", ""),
            ];
            patterns
                .iter()
                .map(|(pattern, replacement)| {
                    (
                        Regex::new(pattern).expect("valid unicode replacement pattern"),
                        *replacement,
                    )
                })
                .collect()
        })
        .as_slice()
}

/// Trim, apply Unicode replacements, and NFC-normalize (OMP `normalizeUnicode`).
pub fn normalize_unicode(s: &str) -> String {
    let mut result = s.trim().to_string();
    for (re, replacement) in unicode_replacements() {
        result = re.replace_all(&result, *replacement).into_owned();
    }
    result.nfc().collect::<String>()
}

/// Minimum indentation across non-empty lines.
pub fn min_indent(text: &str) -> usize {
    let mut min = usize::MAX;
    for line in text.split('\n') {
        if is_non_empty_line(line) {
            min = min.min(count_leading_whitespace(line));
        }
    }
    if min == usize::MAX {
        0
    } else {
        min
    }
}

/// Count leading whitespace characters in a line.
pub fn count_leading_whitespace(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Byte length of the leading whitespace prefix (safe for string slicing).
pub fn leading_whitespace_byte_len(line: &str) -> usize {
    line.char_indices()
        .take_while(|(_, c)| *c == ' ' || *c == '\t')
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0)
}

/// Leading whitespace prefix of a line.
pub fn get_leading_whitespace(line: &str) -> &str {
    &line[..leading_whitespace_byte_len(line)]
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

/// Convert leading tabs to spaces (OMP `convertLeadingTabsToSpaces`).
pub fn convert_leading_tabs_to_spaces(text: &str, spaces_per_tab: usize) -> String {
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
        .map(|line| apply_indent_delta_line(line, delta, indent_char))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Shift leading whitespace on one line by `delta` columns (patch + replace paths).
pub(crate) fn apply_indent_delta_line(line: &str, delta: isize, indent_char: char) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    if delta > 0 {
        return format!("{}{line}", indent_char.to_string().repeat(delta as usize));
    }
    let to_remove = (-delta as usize).min(leading_whitespace_byte_len(line));
    line[to_remove..].to_string()
}

pub(crate) fn detect_indent_char_from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> char {
    for line in lines {
        let ws = get_leading_whitespace(line);
        if !ws.is_empty() {
            return ws.chars().next().unwrap_or(' ');
        }
    }
    ' '
}

fn detect_indent_char(text: &str) -> char {
    detect_indent_char_from_lines(text.split('\n'))
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
            if let Some(converted) =
                maybe_convert_spaces_to_tab_indentation(&old_profile, &actual_profile, new_text)
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

fn maybe_convert_spaces_to_tab_indentation(
    old_profile: &IndentProfile,
    actual_profile: &IndentProfile,
    new_text: &str,
) -> Option<String> {
    if old_profile.mixed
        || actual_profile.mixed
        || !old_profile.space_only
        || !actual_profile.tab_only
    {
        return None;
    }

    let mut samples: HashMap<usize, usize> = HashMap::new();
    let line_count = old_profile.lines.len().min(actual_profile.lines.len());
    let mut consistent = true;
    for i in 0..line_count {
        let old_line = &old_profile.lines[i];
        let actual_line = &actual_profile.lines[i];
        if !is_non_empty_line(old_line) || !is_non_empty_line(actual_line) {
            continue;
        }
        let spaces = count_leading_whitespace(old_line);
        let tabs = count_leading_whitespace(actual_line);
        if tabs == 0 {
            continue;
        }
        if let Some(existing) = samples.get(&tabs) {
            if *existing != spaces {
                consistent = false;
                break;
            }
        }
        samples.insert(tabs, spaces);
    }

    if !consistent || samples.is_empty() {
        return None;
    }

    let tab_width = resolve_tab_width(&samples)?;
    let offset = samples
        .iter()
        .next()
        .map(|(tabs, spaces)| *spaces as isize - *tabs as isize * tab_width)
        .unwrap_or(0);
    Some(
        new_text
            .split('\n')
            .map(|line| convert_spaces_to_tabs_line(line, tab_width, offset))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn resolve_tab_width(samples: &HashMap<usize, usize>) -> Option<isize> {
    if samples.len() == 1 {
        let (&tabs, &spaces) = samples.iter().next()?;
        if spaces % tabs == 0 {
            return Some((spaces / tabs) as isize);
        }
        return None;
    }

    let entries: Vec<(usize, usize)> = samples.iter().map(|(k, v)| (*k, *v)).collect();
    let (t1, s1) = entries[0];
    let (t2, s2) = entries[1];
    if t1 == t2 {
        return None;
    }
    let w = (s2 as isize - s1 as isize) / (t2 as isize - t1 as isize);
    if w <= 0 {
        return None;
    }
    let b = s1 as isize - t1 as isize * w;
    for (&t, &s) in samples {
        if t as isize * w + b != s as isize {
            return None;
        }
    }
    Some(w)
}

fn convert_spaces_to_tabs_line(line: &str, tab_width: isize, offset: isize) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    let ws = count_leading_whitespace(line);
    let ws_bytes = leading_whitespace_byte_len(line);
    if ws == 0 {
        return line.to_string();
    }
    let adjusted = ws as isize - offset;
    if adjusted >= 0 && adjusted % tab_width == 0 {
        return format!(
            "{}{}",
            "\t".repeat((adjusted / tab_width) as usize),
            &line[ws_bytes..]
        );
    }
    let tab_count = adjusted.div_euclid(tab_width);
    let remainder = adjusted - tab_count * tab_width;
    if tab_count >= 0 {
        return format!(
            "{}{}{}",
            "\t".repeat(tab_count as usize),
            " ".repeat(remainder as usize),
            &line[ws_bytes..]
        );
    }
    line.to_string()
}

fn collect_line_indent_deltas(old_lines: &[String], actual_lines: &[String]) -> Vec<isize> {
    let line_count = old_lines.len().min(actual_lines.len());
    let mut deltas = Vec::new();
    for i in 0..line_count {
        let old_line = &old_lines[i];
        let actual_line = &actual_lines[i];
        if old_line.trim().is_empty() || actual_line.trim().is_empty() {
            continue;
        }
        deltas.push(
            count_leading_whitespace(actual_line) as isize
                - count_leading_whitespace(old_line) as isize,
        );
    }
    deltas
}

fn overlay_matching_actual_indents(
    pattern_lines: &[String],
    actual_lines: &[String],
    adjusted_text: String,
) -> Vec<String> {
    let new_lines: Vec<String> = adjusted_text.split('\n').map(str::to_string).collect();

    let mut content_to_actual_lines: HashMap<String, Vec<String>> = HashMap::new();
    for line in actual_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        content_to_actual_lines
            .entry(trimmed.to_string())
            .or_default()
            .push(line.clone());
    }

    let mut pattern_min = usize::MAX;
    for line in pattern_lines {
        if line.trim().is_empty() {
            continue;
        }
        pattern_min = pattern_min.min(count_leading_whitespace(line));
    }
    if pattern_min == usize::MAX {
        pattern_min = 0;
    }

    let deltas = collect_line_indent_deltas(pattern_lines, actual_lines);
    let delta = if !deltas.is_empty() && deltas.iter().all(|d| *d == deltas[0]) {
        Some(deltas[0])
    } else {
        None
    };
    let indent_char = detect_indent_char_from_lines(actual_lines.iter().map(String::as_str));

    let mut used_actual_lines: HashMap<String, usize> = HashMap::new();

    new_lines
        .into_iter()
        .map(|new_line| {
            if new_line.trim().is_empty() {
                return new_line;
            }

            let trimmed = new_line.trim();
            if let Some(matching_actual_lines) = content_to_actual_lines.get(trimmed) {
                if matching_actual_lines.len() == 1 {
                    return matching_actual_lines[0].clone();
                }
                if matching_actual_lines.contains(&new_line) {
                    return new_line;
                }
                let used_count = used_actual_lines.entry(trimmed.to_string()).or_insert(0);
                if *used_count < matching_actual_lines.len() {
                    let result = matching_actual_lines[*used_count].clone();
                    *used_count += 1;
                    return result;
                }
            }

            if let Some(delta) = delta.filter(|&d| d != 0) {
                let new_indent = count_leading_whitespace(&new_line);
                if new_indent == pattern_min {
                    return apply_indent_delta_line(&new_line, delta, indent_char);
                }
            }
            new_line
        })
        .collect()
}

/// Line-oriented indentation adjustment for patch hunks; delegates to [`adjust_indentation`].
pub(crate) fn adjust_lines_indentation(
    pattern_lines: &[String],
    actual_lines: &[String],
    new_lines: &[String],
) -> Vec<String> {
    if pattern_lines.is_empty() || actual_lines.is_empty() || new_lines.is_empty() {
        return new_lines.to_vec();
    }
    if pattern_lines == actual_lines {
        return new_lines.to_vec();
    }
    if pattern_lines.len() == new_lines.len()
        && pattern_lines
            .iter()
            .zip(new_lines.iter())
            .all(|(l, r)| l.trim() == r.trim())
    {
        return new_lines.to_vec();
    }

    let adjusted = adjust_indentation(
        &pattern_lines.join("\n"),
        &actual_lines.join("\n"),
        &new_lines.join("\n"),
    );
    overlay_matching_actual_indents(pattern_lines, actual_lines, adjusted)
}
