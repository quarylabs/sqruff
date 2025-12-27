use std::ops::Range;

use smol_str::SmolStr;

use crate::dialects::{SyntaxKind, SyntaxSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenSpan {
    pub source: Range<usize>,
    pub templated: Range<usize>,
}

impl TokenSpan {
    pub fn new(source: Range<usize>, templated: Range<usize>) -> Self {
        Self { source, templated }
    }

    pub fn source_range(&self) -> Range<usize> {
        self.source.clone()
    }

    pub fn templated_range(&self) -> Range<usize> {
        self.templated.clone()
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
