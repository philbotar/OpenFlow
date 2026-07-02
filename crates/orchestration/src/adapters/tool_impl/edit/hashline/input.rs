//! Top-level patch parser: splits hashline input into sections.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use super::apply::apply_edits;
use super::block::{resolve_block_edits, OnUnresolved, ResolveBlockEditsOptions};
use super::format::{HL_FILE_HASH_LENGTH, HL_FILE_HASH_SEP, HL_FILE_PREFIX};
use super::parser::{parse_patch, ParseResult};
use super::tokenizer::Tokenizer;
use super::types::{ApplyResult, BlockResolver, Edit, SplitOptions};

static TOKENIZER: OnceLock<Tokenizer> = OnceLock::new();

fn tokenizer() -> &'static Tokenizer {
    TOKENIZER.get_or_init(Tokenizer::new)
}

fn unquote_hashline_path(path_text: &str) -> &str {
    if path_text.len() < 2 {
        return path_text;
    }
    let bytes = path_text.as_bytes();
    let first = bytes[0] as char;
    let last = bytes[bytes.len() - 1] as char;
    if (first == '"' || first == '\'') && first == last {
        &path_text[1..path_text.len() - 1]
    } else {
        path_text
    }
}

fn apply_patch_path_noise_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^\*{0,3}\s*(?:(?:update|add|delete|move)[^A-Za-z0-9]*(?:file|to)?[^A-Za-z0-9]*:)?\s*\*{0,3}\s*",
        )
        .expect("valid regex")
    })
}

fn strip_apply_patch_path_noise(path_text: &str) -> String {
    apply_patch_path_noise_re()
        .replace(path_text, "")
        .into_owned()
}

fn normalize_hashline_path(raw_path: &str, cwd: Option<&str>) -> String {
    let unquoted = strip_apply_patch_path_noise(unquote_hashline_path(raw_path.trim()));
    let Some(cwd) = cwd else {
        return unquoted;
    };
    let path = Path::new(&unquoted);
    if !path.is_absolute() {
        return unquoted;
    }
    let cwd_path = Path::new(cwd);
    let Ok(cwd_canon) = cwd_path.canonicalize() else {
        return unquoted;
    };
    let Ok(abs) = path.canonicalize() else {
        return unquoted;
    };
    if let Ok(relative) = abs.strip_prefix(&cwd_canon) {
        let rel = relative.components().collect::<PathBuf>();
        if rel.components().any(|c| matches!(c, Component::ParentDir)) {
            return unquoted;
        }
        if rel.as_os_str().is_empty() {
            return ".".to_string();
        }
        return rel.to_string_lossy().into_owned();
    }
    unquoted
}

#[derive(Debug, Clone)]
struct RawSection {
    path: String,
    file_hash: Option<String>,
    diff: String,
}

fn try_parse_recovery_header(line: &str, cwd: Option<&str>) -> Option<RawSection> {
    if !line.starts_with(HL_FILE_PREFIX) {
        return None;
    }
    let body = strip_apply_patch_path_noise(line[HL_FILE_PREFIX.len()..].trim());
    if body.is_empty() {
        return None;
    }
    let re = format!(r"^(\S+?)(?:#([0-9A-Fa-f]{{{HL_FILE_HASH_LENGTH}}}))?\s*$");
    let caps = Regex::new(&re).ok()?.captures(&body)?;
    let path = normalize_hashline_path(&caps[1], cwd);
    if path.is_empty() {
        return None;
    }
    Some(RawSection {
        path,
        file_hash: caps.get(2).map(|m| m.as_str().to_uppercase()),
        diff: String::new(),
    })
}

fn parse_hashline_header_line(line: &str, cwd: Option<&str>) -> Result<Option<RawSection>, String> {
    let trimmed = line.trim_end();
    if !trimmed.starts_with(HL_FILE_PREFIX) {
        return Ok(None);
    }
    let token = tokenizer().tokenize(trimmed, 0);
    match token {
        super::tokenizer::Token::Header {
            path, file_hash, ..
        } => {
            let parsed_path = normalize_hashline_path(&path, cwd);
            if parsed_path.is_empty() {
                return Err(format!(
                    "Input header \"{HL_FILE_PREFIX}\" is empty; provide a file path."
                ));
            }
            Ok(Some(RawSection {
                path: parsed_path,
                file_hash,
                diff: String::new(),
            }))
        }
        _ => {
            if let Some(recovered) = try_parse_recovery_header(trimmed, cwd) {
                return Ok(Some(recovered));
            }
            Err(format!(
                "Input header must be {HL_FILE_PREFIX}PATH or {HL_FILE_PREFIX}PATH{HL_FILE_HASH_SEP}TAG with a {HL_FILE_HASH_LENGTH}-hex content-hash tag; got {}.",
                serde_json::to_string(trimmed).unwrap_or_else(|_| format!("\"{trimmed}\""))
            ))
        }
    }
}

fn strip_leading_blank_lines(input: &str) -> String {
    let stripped = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let mut lines: Vec<&str> = stripped.split('\n').collect();
    while let Some(head) = lines.first() {
        let head_clean = head.trim_end_matches('\r');
        if head_clean.trim().is_empty()
            || matches!(
                tokenizer().tokenize(head_clean, 0),
                super::tokenizer::Token::EnvelopeBegin { .. }
            )
        {
            lines.remove(0);
        } else {
            break;
        }
    }
    lines.join("\n")
}

pub fn contains_recognizable_hashline_operations(input: &str) -> bool {
    input
        .split("\r\n")
        .flat_map(|chunk| chunk.split('\n'))
        .any(|line| tokenizer().is_op(line))
}

fn normalize_fallback_input(input: &str, options: &SplitOptions) -> String {
    let stripped = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let has_explicit_header = stripped
        .split("\r\n")
        .flat_map(|c| c.split('\n'))
        .any(|raw_line| {
            parse_hashline_header_line(raw_line, options.cwd.as_deref())
                .ok()
                .flatten()
                .is_some()
        });
    if has_explicit_header {
        return input.to_string();
    }
    let Some(ref path) = options.path else {
        return input.to_string();
    };
    if !contains_recognizable_hashline_operations(input) {
        return input.to_string();
    }
    let fallback_path = normalize_hashline_path(path, options.cwd.as_deref());
    if fallback_path.is_empty() {
        return input.to_string();
    }
    format!("{HL_FILE_PREFIX}{fallback_path}\n{input}")
}

fn split_raw_sections(input: &str, options: &SplitOptions) -> Result<Vec<RawSection>, String> {
    let stripped = strip_leading_blank_lines(&normalize_fallback_input(input, options));
    let lines: Vec<String> = stripped
        .split("\r\n")
        .flat_map(|c| c.split('\n'))
        .map(String::from)
        .collect();
    let first_line = lines.first().map(String::as_str).unwrap_or("");
    if parse_hashline_header_line(first_line, options.cwd.as_deref())?.is_none() {
        let first_trimmed = first_line.trim_end();
        if Regex::new(r"^@@\s+[-+]?\d+,\d+\s+[-+]?\d+,\d+\s+@@")
            .ok()
            .is_some_and(|re| re.is_match(first_trimmed))
        {
            return Err(
                "unified-diff hunk header (`@@ -N,M +N,M @@`) is not valid in hashline. File sections start with `¶path#HASH`; use `replace`, `delete`, or `insert` ops.".to_string(),
            );
        }
        let preview = serde_json::to_string(&first_line.chars().take(120).collect::<String>())
            .unwrap_or_else(|_| "\"…\"".to_string());
        return Err(format!(
            "input must begin with \"{HL_FILE_PREFIX}PATH{HL_FILE_HASH_SEP}HASH\" on the first non-blank line for anchored edits; got: {preview}. Example: \"{HL_FILE_PREFIX}src/foo.ts{HL_FILE_HASH_SEP}0A3\" then edit ops."
        ));
    }
    let mut sections = Vec::new();
    let mut current: Option<RawSection> = None;
    let mut current_lines: Vec<String> = Vec::new();
    let flush = |sections: &mut Vec<RawSection>,
                 current: &mut Option<RawSection>,
                 current_lines: &mut Vec<String>| {
        if let Some(section) = current.take() {
            let has_ops = current_lines.iter().any(|l| !l.trim().is_empty());
            if has_ops {
                sections.push(RawSection {
                    diff: current_lines.join("\n"),
                    ..section
                });
            }
            current_lines.clear();
        }
    };
    for line in lines {
        let trimmed = line.trim_end();
        let token = tokenizer().tokenize(&line, 0);
        if matches!(
            token,
            super::tokenizer::Token::EnvelopeEnd { .. } | super::tokenizer::Token::Abort { .. }
        ) {
            break;
        }
        if matches!(token, super::tokenizer::Token::EnvelopeBegin { .. }) {
            continue;
        }
        if trimmed.starts_with(HL_FILE_PREFIX) {
            if let Some(header) = parse_hashline_header_line(&line, options.cwd.as_deref())? {
                flush(&mut sections, &mut current, &mut current_lines);
                current = Some(header);
                current_lines.clear();
                continue;
            }
        }
        current_lines.push(line);
    }
    if let Some(section) = current {
        let has_ops = current_lines.iter().any(|l| !l.trim().is_empty());
        if has_ops {
            sections.push(RawSection {
                diff: current_lines.join("\n"),
                ..section
            });
        }
    }
    Ok(sections)
}

fn merge_same_path_sections(sections: Vec<RawSection>) -> Result<Vec<RawSection>, String> {
    let mut by_path: HashMap<String, (Option<String>, Vec<String>)> = HashMap::new();
    for section in sections {
        let entry = by_path
            .entry(section.path.clone())
            .or_insert((None, Vec::new()));
        if let (Some(existing), Some(new_hash)) = (&entry.0, &section.file_hash) {
            if existing != new_hash {
                return Err(format!(
                    "Conflicting hashline snapshot tags for {}: #{existing} and #{new_hash}. Re-read the file and retry with one current header.",
                    section.path
                ));
            }
        }
        if entry.0.is_none() {
            entry.0 = section.file_hash;
        }
        entry.1.push(section.diff);
    }
    Ok(by_path
        .into_iter()
        .map(|(path, (file_hash, diffs))| RawSection {
            path,
            file_hash,
            diff: diffs.join("\n"),
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct PatchSection {
    pub path: String,
    pub file_hash: Option<String>,
    pub diff: String,
    parsed: Option<ParseResult>,
}

impl PatchSection {
    fn from_raw(raw: RawSection) -> Self {
        Self {
            path: raw.path,
            file_hash: raw.file_hash,
            diff: raw.diff,
            parsed: None,
        }
    }

    pub fn parse(&mut self) -> Result<&ParseResult, String> {
        if self.parsed.is_none() {
            self.parsed = Some(parse_patch(&self.diff)?);
        }
        Ok(self.parsed.as_ref().expect("parsed"))
    }

    pub fn edits(&mut self) -> Result<Vec<Edit>, String> {
        Ok(self.parse()?.edits.clone())
    }

    pub fn warnings(&mut self) -> Result<Vec<String>, String> {
        Ok(self.parse()?.warnings.clone())
    }

    pub fn has_anchor_scoped_edit(&mut self) -> Result<bool, String> {
        Ok(self.parse()?.edits.iter().any(|edit| match edit {
            Edit::Delete { .. } | Edit::Block { .. } => true,
            Edit::Insert { cursor, .. } => matches!(
                cursor,
                super::types::Cursor::BeforeAnchor { .. }
                    | super::types::Cursor::AfterAnchor { .. }
            ),
        }))
    }

    pub fn collect_anchor_lines(&mut self) -> Result<Vec<u32>, String> {
        let mut lines = std::collections::BTreeSet::new();
        for edit in &self.parse()?.edits {
            match edit {
                Edit::Delete { anchor, .. } | Edit::Block { anchor, .. } => {
                    lines.insert(anchor.line);
                }
                Edit::Insert { cursor, .. } => {
                    if let super::types::Cursor::BeforeAnchor { anchor }
                    | super::types::Cursor::AfterAnchor { anchor } = cursor
                    {
                        lines.insert(anchor.line);
                    }
                }
            }
        }
        Ok(lines.into_iter().collect())
    }

    pub fn apply_to(
        &mut self,
        text: &str,
        block_resolver: Option<&BlockResolver>,
    ) -> Result<ApplyResult, String> {
        let path = self.path.clone();
        let parsed = self.parse()?;
        let resolved = resolve_block_edits(
            &parsed.edits,
            text,
            &path,
            block_resolver,
            ResolveBlockEditsOptions {
                on_unresolved: OnUnresolved::Throw,
            },
        )?;
        let mut result = apply_edits(text, &resolved)?;
        if !parsed.warnings.is_empty() {
            let mut merged = parsed.warnings.clone();
            merged.append(&mut result.warnings);
            result.warnings = merged;
        }
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct Patch {
    pub sections: Vec<PatchSection>,
}

impl Patch {
    pub fn parse(input: &str, options: SplitOptions) -> Result<Self, String> {
        let raw = merge_same_path_sections(split_raw_sections(input, &options)?)?;
        Ok(Self {
            sections: raw.into_iter().map(PatchSection::from_raw).collect(),
        })
    }

    pub fn parse_single(input: &str, options: SplitOptions) -> Result<PatchSection, String> {
        let patch = Self::parse(input, options)?;
        patch
            .sections
            .into_iter()
            .next()
            .ok_or_else(|| "Patch input did not produce any sections.".to_string())
    }
}
