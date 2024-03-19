use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

use uuid::Uuid;

use super::base::CloneSegment;
use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;
use crate::helpers::Boxed;

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
}

impl<M: MetaSegmentKind> Deref for MetaSegment<M> {
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

impl<M: MetaSegmentKind> Segment for MetaSegment<M> {
    fn get_type(&self) -> &'static str {
        self.kind.kind()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_meta(&self) -> bool {
        true
    }

    fn segments(&self) -> &[Box<dyn Segment>] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone().boxed()]
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_marker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_marker = position_marker;
    }
}

impl<M: MetaSegmentKind> Matchable for MetaSegment<M> {
    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        None
    }

    fn match_segments(
        &self,
        _segments: Vec<Box<dyn Segment>>,
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
    Indent = 1,
    Dedent = -1,
}

impl MetaSegmentKind for IndentChange {
    fn indent_val(&self) -> i8 {
        *self as i8
    }

    fn kind(&self) -> &'static str {
        match self {
            IndentChange::Indent => "indent",
            IndentChange::Dedent => "dedent",
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
    pub fn new(position_maker: PositionMarker) -> Box<dyn Segment> {
        Box::new(EndOfFile { position_maker, uuid: Uuid::new_v4() })
    }
}

impl Segment for EndOfFile {
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { uuid: self.uuid, position_maker: self.position_maker.clone() }.boxed()
    }

    fn get_raw(&self) -> Option<String> {
        Some(String::new())
    }

    fn segments(&self) -> &[Box<dyn Segment>] {
        &[]
    }

    fn class_types(&self) -> HashSet<String> {
        HashSet::from(["end_of_file".into()])
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone_box()]
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

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
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
