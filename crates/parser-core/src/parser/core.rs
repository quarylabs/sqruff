use std::ops::Range;

use smol_str::SmolStr;

use crate::dialects::syntax::{SyntaxKind, SyntaxSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenSpan {
    pub source_start: usize,
    pub source_end: usize,
    pub templated_start: usize,
    pub templated_end: usize,
}

impl TokenSpan {
    pub fn new(
        source_start: usize,
        source_end: usize,
        templated_start: usize,
        templated_end: usize,
    ) -> Self {
        Self {
            source_start,
            source_end,
            templated_start,
            templated_end,
        }
    }

    pub fn source_range(self) -> Range<usize> {
        self.source_start..self.source_end
    }

    pub fn templated_range(self) -> Range<usize> {
        self.templated_start..self.templated_end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub raw: SmolStr,
    pub span: TokenSpan,
    class_types: SyntaxSet,
}

impl Token {
    pub fn new(kind: SyntaxKind, raw: impl Into<SmolStr>, span: TokenSpan) -> Self {
        Self {
            kind,
            raw: raw.into(),
            span,
            class_types: class_types(kind),
        }
    }

    pub fn raw(&self) -> &str {
        self.raw.as_ref()
    }

    pub fn class_types(&self) -> &SyntaxSet {
        &self.class_types
    }

    pub fn is_type(&self, kind: SyntaxKind) -> bool {
        self.kind == kind
    }

    pub fn is_meta(&self) -> bool {
        matches!(
            self.kind,
            SyntaxKind::Indent | SyntaxKind::Implicit | SyntaxKind::Dedent | SyntaxKind::EndOfFile
        )
    }

    pub fn is_comment(&self) -> bool {
        matches!(
            self.kind,
            SyntaxKind::Comment | SyntaxKind::InlineComment | SyntaxKind::BlockComment
        )
    }

    pub fn is_whitespace(&self) -> bool {
        matches!(self.kind, SyntaxKind::Whitespace | SyntaxKind::Newline)
    }

    pub fn is_code(&self) -> bool {
        !self.is_comment() && !self.is_whitespace() && !self.is_meta()
    }

    pub fn first_non_whitespace_segment_raw_upper(&self) -> Option<String> {
        if self.raw.is_empty() {
            None
        } else {
            Some(self.raw.to_uppercase())
        }
    }
}

pub trait EventSink {
    fn enter_node(&mut self, kind: SyntaxKind);
    fn exit_node(&mut self, kind: SyntaxKind);
    fn token(&mut self, token: Token);
}

fn class_types(syntax_kind: SyntaxKind) -> SyntaxSet {
    match syntax_kind {
        SyntaxKind::ColumnReference => SyntaxSet::new(&[SyntaxKind::ObjectReference, syntax_kind]),
        SyntaxKind::WildcardIdentifier => {
            SyntaxSet::new(&[SyntaxKind::WildcardIdentifier, SyntaxKind::ObjectReference])
        }
        SyntaxKind::TableReference => SyntaxSet::new(&[SyntaxKind::ObjectReference, syntax_kind]),
        _ => SyntaxSet::single(syntax_kind),
    }
}
