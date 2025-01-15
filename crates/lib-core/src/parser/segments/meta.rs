use std::fmt::Debug;

use ahash::AHashSet;

use super::base::ErasedSegment;
use crate::dialects::syntax::{SyntaxKind, SyntaxSet};
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::MatchResult;
use crate::parser::matchable::{Matchable, MatchableTrait};

pub type Indent = MetaSegment;

#[derive(Debug, Clone, PartialEq)]
pub struct MetaSegment {
    id: u32,
    pub(crate) kind: SyntaxKind,
}

impl MetaSegment {
    pub fn from_kind(kind: SyntaxKind) -> Self {
        Self { kind, id: 0 }
    }

    pub fn indent() -> Self {
        Self::from_kind(SyntaxKind::Indent)
    }

    pub fn dedent() -> Self {
        Self::from_kind(SyntaxKind::Dedent)
    }

    pub fn implicit_indent() -> Self {
        Self::from_kind(SyntaxKind::Implicit)
    }
}

impl MatchableTrait for MetaSegment {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, SyntaxSet)> {
        None
    }

    fn match_segments(
        &self,
        _segments: &[ErasedSegment],
        _idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        panic!(
            "{} has no match method, it should only be used in a Sequence!",
            std::any::type_name::<Self>()
        );
    }
}
