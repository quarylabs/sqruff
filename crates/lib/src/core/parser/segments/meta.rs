use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

use ahash::AHashSet;

use super::base::ErasedSegment;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::helpers::ToErasedSegment;

pub type Indent = MetaSegment<IndentChange>;

pub trait MetaSegmentKind: Debug + Clone + PartialEq + 'static {
    fn kind(&self) -> SyntaxKind {
        SyntaxKind::Meta
    }

    fn indent_val(&self) -> i8 {
        0
    }

    fn is_implicit(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetaSegment<M> {
    id: u32,
    position_marker: Option<PositionMarker>,
    pub(crate) kind: M,
}

impl MetaSegment<TemplateSegment> {
    pub fn template(pos_marker: PositionMarker, source_str: &str, block_type: &str) -> Self {
        MetaSegment {
            id: 0,
            position_marker: pos_marker.into(),
            kind: TemplateSegment::new(source_str.into(), block_type.into(), None, None),
        }
    }
}

impl Indent {
    pub fn from_kind(kind: IndentChange) -> Self {
        Self { kind, position_marker: None, id: 0 }
    }

    pub fn indent() -> Self {
        Self::from_kind(IndentChange::Indent)
    }

    pub fn dedent() -> Self {
        Self::from_kind(IndentChange::Dedent)
    }

    pub fn implicit_indent() -> Self {
        Self::from_kind(IndentChange::Implicit)
    }
}

impl<M: MetaSegmentKind> Deref for MetaSegment<M> {
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

impl<M: MetaSegmentKind + Send + Sync> Segment for MetaSegment<M> {
    fn get_type(&self) -> SyntaxKind {
        self.kind.kind()
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_meta(&self) -> bool {
        true
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }
}

impl<M: MetaSegmentKind + Send + Sync> Matchable for MetaSegment<M> {
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

/// A segment which is empty but indicates where an indent should be.
///
///     This segment is always empty, i.e. its raw format is '', but it
/// indicates     the position of a theoretical indent which will be used in
/// linting     and reconstruction. Even if there is an *actual indent* that
/// occurs     in the same place this intentionally *won't* capture it, they
/// will just     be compared later.
#[derive(Hash, Debug, Clone, Copy, PartialEq)]
pub enum IndentChange {
    Indent,
    Implicit,
    Dedent,
}

impl MetaSegmentKind for IndentChange {
    fn kind(&self) -> SyntaxKind {
        match self {
            IndentChange::Indent => SyntaxKind::Indent,
            IndentChange::Implicit => SyntaxKind::Indent,
            IndentChange::Dedent => SyntaxKind::Dedent,
        }
    }

    fn indent_val(&self) -> i8 {
        match self {
            IndentChange::Indent | IndentChange::Implicit => 1,
            IndentChange::Dedent => -1,
        }
    }

    fn is_implicit(&self) -> bool {
        matches!(self, IndentChange::Implicit)
    }
}

pub struct IndentNewArgs {}

#[derive(PartialEq, Clone, Hash, Debug)]
pub struct TemplateSegment {
    source_str: String,
    block_type: String,
    source_fixes: Option<Vec<SourceFix>>,
    block_uuid: Option<u32>,
}

impl TemplateSegment {
    pub fn new(
        source_str: String,
        block_type: String,
        source_fixes: Option<Vec<SourceFix>>,
        block_uuid: Option<u32>,
    ) -> Self {
        if source_str.is_empty() {
            panic!("Cannot instantiate TemplateSegment without a source_str.");
        }

        TemplateSegment { source_str, block_type, source_fixes, block_uuid }
    }
}

impl MetaSegmentKind for TemplateSegment {
    fn kind(&self) -> SyntaxKind {
        SyntaxKind::Placeholder
    }
}
