use std::borrow::Cow;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

use ahash::AHashSet;
use uuid::Uuid;

use super::base::{CloneSegment, ErasedSegment};
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;
use crate::helpers::ToErasedSegment;

pub type Indent = MetaSegment<IndentChange>;

pub trait MetaSegmentKind: Debug + Hash + Clone + PartialEq + 'static {
    fn kind(&self) -> &'static str {
        "meta"
    }

    fn indent_val(&self) -> i8 {
        0
    }

    fn is_implicit(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct MetaSegment<M> {
    uuid: Uuid,
    position_marker: Option<PositionMarker>,
    kind: M,
}

impl MetaSegment<TemplateSegment> {
    pub fn template(pos_marker: PositionMarker, source_str: &str, block_type: &str) -> Self {
        MetaSegment {
            uuid: Uuid::new_v4(),
            position_marker: pos_marker.into(),
            kind: TemplateSegment::new(source_str.into(), block_type.into(), None, None),
        }
    }
}

impl Indent {
    fn from_kind(kind: IndentChange) -> Self {
        Self { kind, position_marker: None, uuid: Uuid::new_v4() }
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
    fn get_type(&self) -> &'static str {
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

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }
}

impl<M: MetaSegmentKind + Send + Sync> Matchable for MetaSegment<M> {
    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        None
    }

    fn match_segments(
        &self,
        _segments: &[ErasedSegment],
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
    fn kind(&self) -> &'static str {
        match self {
            IndentChange::Indent => "indent",
            IndentChange::Implicit => "indent",
            IndentChange::Dedent => "dedent",
        }
    }

    fn indent_val(&self) -> i8 {
        match self {
            IndentChange::Indent | IndentChange::Implicit => 1,
            IndentChange::Dedent => -1,
        }
    }
}

pub struct IndentNewArgs {}

#[derive(Hash, Clone, Debug, PartialEq)]
pub struct EndOfFile {
    uuid: Uuid,
    position_maker: PositionMarker,
}

impl EndOfFile {
    pub fn create(position_maker: PositionMarker) -> ErasedSegment {
        EndOfFile { position_maker, uuid: Uuid::new_v4() }.to_erased_segment()
    }
}

impl Segment for EndOfFile {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self { uuid: self.uuid, position_maker: self.position_maker.clone() }.to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        "".into()
    }

    fn get_type(&self) -> &'static str {
        "end_of_file"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        todo!()
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn is_meta(&self) -> bool {
        true
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker.unwrap();
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone_box()]
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    fn edit(&self, _raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        todo!()
    }

    fn class_types(&self) -> AHashSet<&'static str> {
        ["end_of_file"].into()
    }
}

#[derive(PartialEq, Clone, Hash, Debug)]
pub struct TemplateSegment {
    source_str: String,
    block_type: String,
    source_fixes: Option<Vec<SourceFix>>,
    block_uuid: Option<Uuid>,
}

impl TemplateSegment {
    pub fn new(
        source_str: String,
        block_type: String,
        source_fixes: Option<Vec<SourceFix>>,
        block_uuid: Option<Uuid>,
    ) -> Self {
        if source_str.is_empty() {
            panic!("Cannot instantiate TemplateSegment without a source_str.");
        }

        TemplateSegment { source_str, block_type, source_fixes, block_uuid }
    }
}

impl MetaSegmentKind for TemplateSegment {
    fn kind(&self) -> &'static str {
        "placeholder"
    }
}
