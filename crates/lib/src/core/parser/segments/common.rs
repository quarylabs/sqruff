use std::borrow::Cow;

use smol_str::SmolStr;
use uuid::Uuid;

use super::base::{ErasedSegment, Segment};
use super::fix::SourceFix;
use crate::core::parser::markers::PositionMarker;
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::helpers::ToErasedSegment;

#[derive(Debug, Clone, PartialEq)]
pub struct LiteralSegment {
    pub raw: SmolStr,
    pub position_maker: Option<PositionMarker>,
    pub uuid: Uuid,
}

impl LiteralSegment {
    pub fn create(raw: &str, position_maker: &PositionMarker) -> ErasedSegment {
        Self {
            raw: raw.into(),
            position_maker: position_maker.clone().into(),
            uuid: Uuid::new_v4(),
        }
        .to_erased_segment()
    }
}

impl Segment for LiteralSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self { raw: self.raw.clone(), position_maker: self.position_maker.clone(), uuid: self.uuid }
            .to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::NumericLiteral
    }

    fn is_code(&self) -> bool {
        true
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker;
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

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn edit(&self, raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        Self {
            raw: raw.map(Into::into).unwrap_or_else(|| self.raw.clone()),
            position_maker: self.position_maker.clone(),
            uuid: self.uuid,
        }
        .to_erased_segment()
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::new(&[SyntaxKind::NumericLiteral])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonOperatorSegment {
    pub raw: SmolStr,
    pub position_maker: PositionMarker,
    pub uuid: Uuid,
}

impl ComparisonOperatorSegment {
    pub fn create(raw: &str, position_maker: &PositionMarker) -> ErasedSegment {
        Self { raw: raw.into(), position_maker: position_maker.clone(), uuid: Uuid::new_v4() }
            .to_erased_segment()
    }
}

impl Segment for ComparisonOperatorSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self { raw: self.raw.clone(), position_maker: self.position_maker.clone(), uuid: self.uuid }
            .to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::NumericLiteral
    }

    fn is_code(&self) -> bool {
        true
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        self.position_maker = _position_marker.unwrap();
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

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::new(&[SyntaxKind::NumericLiteral])
    }
}
