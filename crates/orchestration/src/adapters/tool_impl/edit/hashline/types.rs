//! Pure data types shared across the hashline parser, applier, and patcher.

/// A line-number anchor (1-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub line: u32,
}

/// Where an `insert` edit should land relative to existing content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    Bof,
    Eof,
    BeforeAnchor { anchor: Anchor },
    AfterAnchor { anchor: Anchor },
}

/// A single low-level edit produced by the parser and consumed by the applier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Insert {
        cursor: Cursor,
        text: String,
        line_num: u32,
        index: u32,
        mode: Option<InsertMode>,
    },
    Delete {
        anchor: Anchor,
        line_num: u32,
        index: u32,
        old_assertion: Option<String>,
    },
    /// Deferred block edit — expanded by [`super::block::resolve_block_edits`].
    Block {
        anchor: Anchor,
        payloads: Vec<String>,
        line_num: u32,
        index: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertMode {
    Replacement,
}

/// Result of applying a parsed set of edits to a text body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub text: String,
    pub first_changed_line: Option<u32>,
    pub warnings: Vec<String>,
}

/// A parsed `[A..B]` line range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedRange {
    pub start: Anchor,
    pub end: Anchor,
}

/// Optional hints for [`super::input::Patch::parse`].
#[derive(Debug, Clone, Default)]
pub struct SplitOptions {
    /// Resolves absolute paths inside hashline headers to cwd-relative form.
    pub cwd: Option<String>,
    /// Fallback path when the input lacks a `¶PATH` header.
    pub path: Option<String>,
}

/// Resolved 1-indexed inclusive line span of a `replace block N:` target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockSpan {
    pub start: u32,
    pub end: u32,
}

/// Request handed to a [`BlockResolver`] to resolve one `replace block N:` anchor.
#[derive(Debug, Clone)]
pub struct BlockResolverRequest<'a> {
    pub path: &'a str,
    pub text: &'a str,
    pub line: u32,
}

/// Resolves a `replace block N:` anchor to the line span of the syntactic block.
pub type BlockResolver = dyn Fn(&BlockResolverRequest<'_>) -> Option<BlockSpan>;
