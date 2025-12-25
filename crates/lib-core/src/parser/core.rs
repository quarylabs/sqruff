use std::ops::Range;

use smol_str::SmolStr;

use crate::dialects::syntax::SyntaxKind;

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
}

impl Token {
    pub fn new(kind: SyntaxKind, raw: impl Into<SmolStr>, span: TokenSpan) -> Self {
        Self {
            kind,
            raw: raw.into(),
            span,
        }
    }
}

pub trait EventSink {
    fn enter_node(&mut self, kind: SyntaxKind);
    fn exit_node(&mut self, kind: SyntaxKind);
    fn token(&mut self, token: Token);
}
