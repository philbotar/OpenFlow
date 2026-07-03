//! Hunk normalization and fallback variants.

use std::collections::HashSet;

use super::super::diff::DiffHunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HunkVariantKind {
    TrimCommon,
    DedupeShared,
    CollapseRepeated,
    SingleLine,
}

pub(super) struct HunkVariant {
    pub(super) old_lines: Vec<String>,
    pub(super) new_lines: Vec<String>,
    pub(super) kind: HunkVariantKind,
}

pub(super) fn trim_common_context(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
    let mut start = 0;
    let mut end_old = old_lines.len();
    let mut end_new = new_lines.len();

    while start < end_old && start < end_new && old_lines[start] == new_lines[start] {
        start += 1;
    }

    while end_old > start && end_new > start && old_lines[end_old - 1] == new_lines[end_new - 1] {
        end_old -= 1;
        end_new -= 1;
    }

    if start == 0 && end_old == old_lines.len() && end_new == new_lines.len() {
        return None;
    }

    let trimmed_old = old_lines[start..end_old].to_vec();
    let trimmed_new = new_lines[start..end_new].to_vec();
    if trimmed_old.is_empty() && trimmed_new.is_empty() {
        return None;
    }
    Some(HunkVariant {
        old_lines: trimmed_old,
        new_lines: trimmed_new,
        kind: HunkVariantKind::TrimCommon,
    })
}

pub(super) fn collapse_consecutive_shared_lines(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
    let shared: HashSet<&str> = old_lines
        .iter()
        .filter(|line| new_lines.contains(line))
        .map(String::as_str)
        .collect();

    let collapse = |lines: &[String]| -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = &lines[i];
            out.push(line.clone());
            let mut j = i + 1;
            while j < lines.len() && lines[j] == *line && shared.contains(line.as_str()) {
                j += 1;
            }
            i = j;
        }
        out
    };

    let collapsed_old = collapse(old_lines);
    let collapsed_new = collapse(new_lines);
    if collapsed_old.len() == old_lines.len() && collapsed_new.len() == new_lines.len() {
        return None;
    }
    Some(HunkVariant {
        old_lines: collapsed_old,
        new_lines: collapsed_new,
        kind: HunkVariantKind::DedupeShared,
    })
}

pub(super) fn collapse_repeated_blocks(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
    let shared: HashSet<&str> = old_lines
        .iter()
        .filter(|line| new_lines.contains(line))
        .map(String::as_str)
        .collect();

    let collapse = |lines: &[String]| -> Vec<String> {
        let mut output = lines.to_vec();
        let mut changed = false;
        let mut i = 0;
        while i < output.len() {
            let mut collapsed = false;
            for size in (2..=(output.len() - i) / 2).rev() {
                let first = &output[i..i + size];
                let second = &output[i + size..i + size * 2];
                if first.len() != second.len() || first.is_empty() {
                    continue;
                }
                if !first.iter().all(|line| shared.contains(line.as_str())) {
                    continue;
                }
                if first != second {
                    continue;
                }
                output.drain(i + size..i + size * 2);
                changed = true;
                collapsed = true;
                break;
            }
            if !collapsed {
                i += 1;
            }
        }
        if changed {
            output
        } else {
            lines.to_vec()
        }
    };

    let collapsed_old = collapse(old_lines);
    let collapsed_new = collapse(new_lines);
    if collapsed_old.len() == old_lines.len() && collapsed_new.len() == new_lines.len() {
        return None;
    }
    Some(HunkVariant {
        old_lines: collapsed_old,
        new_lines: collapsed_new,
        kind: HunkVariantKind::CollapseRepeated,
    })
}

pub(super) fn reduce_to_single_line_change(
    old_lines: &[String],
    new_lines: &[String],
) -> Option<HunkVariant> {
    if old_lines.len() != new_lines.len() || old_lines.is_empty() {
        return None;
    }
    let mut changed_index = None;
    for (i, (old, new)) in old_lines.iter().zip(new_lines.iter()).enumerate() {
        if old != new {
            if changed_index.is_some() {
                return None;
            }
            changed_index = Some(i);
        }
    }
    let idx = changed_index?;
    Some(HunkVariant {
        old_lines: vec![old_lines[idx].clone()],
        new_lines: vec![new_lines[idx].clone()],
        kind: HunkVariantKind::SingleLine,
    })
}

pub(super) fn build_fallback_variants(hunk: &DiffHunk) -> Vec<HunkVariant> {
    let base = HunkVariant {
        old_lines: hunk.old_lines.clone(),
        new_lines: hunk.new_lines.clone(),
        kind: HunkVariantKind::TrimCommon,
    };

    let trimmed = trim_common_context(&base.old_lines, &base.new_lines);
    let deduped = collapse_consecutive_shared_lines(
        trimmed
            .as_ref()
            .map(|v| v.old_lines.as_slice())
            .unwrap_or(&base.old_lines),
        trimmed
            .as_ref()
            .map(|v| v.new_lines.as_slice())
            .unwrap_or(&base.new_lines),
    );
    let collapsed = collapse_repeated_blocks(
        deduped.as_ref().map(|v| v.old_lines.as_slice()).unwrap_or(
            trimmed
                .as_ref()
                .map(|v| v.old_lines.as_slice())
                .unwrap_or(&base.old_lines),
        ),
        deduped.as_ref().map(|v| v.new_lines.as_slice()).unwrap_or(
            trimmed
                .as_ref()
                .map(|v| v.new_lines.as_slice())
                .unwrap_or(&base.new_lines),
        ),
    );
    let single_line = reduce_to_single_line_change(
        trimmed
            .as_ref()
            .map(|v| v.old_lines.as_slice())
            .unwrap_or(&base.old_lines),
        trimmed
            .as_ref()
            .map(|v| v.new_lines.as_slice())
            .unwrap_or(&base.new_lines),
    );

    let mut variants = Vec::new();
    if let Some(v) = trimmed {
        variants.push(v);
    }
    if let Some(v) = deduped {
        variants.push(v);
    }
    if let Some(v) = collapsed {
        variants.push(v);
    }
    if let Some(v) = single_line {
        variants.push(v);
    }

    let mut seen = HashSet::new();
    variants
        .into_iter()
        .filter(|variant| {
            if variant.old_lines.is_empty() && variant.new_lines.is_empty() {
                return false;
            }
            let key = format!(
                "{}||{}",
                variant.old_lines.join("\n"),
                variant.new_lines.join("\n")
            );
            seen.insert(key)
        })
        .collect()
}

pub(super) fn filter_fallback_variants(
    variants: Vec<HunkVariant>,
    allow_aggressive: bool,
) -> Vec<HunkVariant> {
    if allow_aggressive {
        return variants;
    }
    variants
        .into_iter()
        .filter(|variant| {
            !matches!(
                variant.kind,
                HunkVariantKind::CollapseRepeated | HunkVariantKind::SingleLine
            )
        })
        .collect()
}
