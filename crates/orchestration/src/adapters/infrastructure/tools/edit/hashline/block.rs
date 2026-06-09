//! Expand deferred `replace block N:` edits into concrete inserts + deletes.

use super::messages::{block_unresolved_message, BLOCK_RESOLVER_UNAVAILABLE};
use super::types::{Anchor, BlockResolver, Cursor, Edit, InsertMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnUnresolved {
    Throw,
    Drop,
}

#[derive(Debug, Clone)]
pub struct ResolveBlockEditsOptions {
    pub on_unresolved: OnUnresolved,
}

impl Default for ResolveBlockEditsOptions {
    fn default() -> Self {
        Self {
            on_unresolved: OnUnresolved::Throw,
        }
    }
}

pub fn has_block_edit(edits: &[Edit]) -> bool {
    edits.iter().any(|edit| matches!(edit, Edit::Block { .. }))
}

pub fn resolve_block_edits(
    edits: &[Edit],
    text: &str,
    path: &str,
    resolver: Option<&BlockResolver>,
    options: ResolveBlockEditsOptions,
) -> Result<Vec<Edit>, String> {
    if !has_block_edit(edits) {
        return Ok(edits.to_vec());
    }
    let on_unresolved = options.on_unresolved;
    let mut resolved = Vec::new();
    let mut synth_index = 0u32;
    for edit in edits {
        match edit {
            Edit::Block {
                anchor,
                payloads,
                line_num,
                ..
            } => {
                let span = resolver.map(|r| {
                    r(&super::types::BlockResolverRequest {
                        path,
                        text,
                        line: anchor.line,
                    })
                });
                let span = match span {
                    Some(Some(s)) => s,
                    _ => {
                        if on_unresolved == OnUnresolved::Drop {
                            continue;
                        }
                        let msg = if resolver.is_some() {
                            block_unresolved_message(anchor.line)
                        } else {
                            BLOCK_RESOLVER_UNAVAILABLE.to_string()
                        };
                        return Err(format!("line {line_num}: {msg}"));
                    }
                };
                for payload in payloads {
                    resolved.push(Edit::Insert {
                        cursor: Cursor::BeforeAnchor {
                            anchor: Anchor { line: span.start },
                        },
                        text: payload.clone(),
                        line_num: *line_num,
                        index: synth_index,
                        mode: Some(InsertMode::Replacement),
                    });
                    synth_index += 1;
                }
                for line in span.start..=span.end {
                    resolved.push(Edit::Delete {
                        anchor: Anchor { line },
                        line_num: *line_num,
                        index: synth_index,
                        old_assertion: None,
                    });
                    synth_index += 1;
                }
            }
            other => resolved.push(other.clone()),
        }
    }
    Ok(resolved)
}
