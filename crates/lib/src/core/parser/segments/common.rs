use ahash::AHashSet;
use uuid::Uuid;

use super::base::{ErasedSegment, Segment};
use super::fix::SourceFix;
use crate::core::parser::markers::PositionMarker;
use crate::helpers::ToErasedSegment;

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct LiteralSegment {
    pub raw: String,
    pub position_maker: PositionMarker,
    pub uuid: Uuid,
}

impl LiteralSegment {
    pub fn create(raw: &str, position_maker: &PositionMarker) -> ErasedSegment {
        Self { raw: raw.to_string(), position_maker: position_maker.clone(), uuid: Uuid::new_v4() }
            .to_erased_segment()
    }
}

impl Segment for LiteralSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self { raw: self.raw.clone(), position_maker: self.position_maker.clone(), uuid: self.uuid }
            .to_erased_segment()
    }

    fn edit(&self, raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        Self {
            raw: raw.unwrap_or_else(|| self.raw.clone()),
            position_maker: self.position_maker.clone(),
            uuid: self.uuid,
        }
        .to_erased_segment()
    }

    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn get_type(&self) -> &'static str {
        "numeric_literal"
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
        todo!()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn class_types(&self) -> AHashSet<String> {
        ["numeric_literal"].map(ToOwned::to_owned).into_iter().collect()
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ComparisonOperatorSegment {
    pub raw: String,
    pub position_maker: PositionMarker,
    pub uuid: Uuid,
}

impl ComparisonOperatorSegment {
    pub fn create(raw: &str, position_maker: &PositionMarker) -> ErasedSegment {
        Self { raw: raw.to_string(), position_maker: position_maker.clone(), uuid: Uuid::new_v4() }
            .to_erased_segment()
    }
}

impl Segment for ComparisonOperatorSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        Self { raw: self.raw.clone(), position_maker: self.position_maker.clone(), uuid: self.uuid }
            .to_erased_segment()
    }

    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn get_type(&self) -> &'static str {
        "numeric_literal"
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
        todo!()
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn class_types(&self) -> AHashSet<String> {
        ["numeric_literal"].map(ToOwned::to_owned).into_iter().collect()
    }
}
