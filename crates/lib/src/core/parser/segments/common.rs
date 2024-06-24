use std::borrow::Cow;

use ahash::AHashSet;
use smol_str::SmolStr;

use super::base::{ErasedSegment, Segment};
use super::fix::SourceFix;
use crate::core::parser::markers::PositionMarker;
use crate::helpers::{next_cache_key, ToErasedSegment};

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct LiteralSegment {
    pub raw: SmolStr,
    pub position_maker: PositionMarker,
    pub uuid: u64,
}

impl LiteralSegment {
    pub fn create(raw: &str, position_maker: &PositionMarker) -> ErasedSegment {
        Self { raw: raw.into(), position_maker: position_maker.clone(), uuid: next_cache_key() }
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
            raw: raw.map(Into::into).unwrap_or_else(|| self.raw.clone()),
            position_maker: self.position_maker.clone(),
            uuid: self.uuid,
        }
        .to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
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

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        self.position_maker = position_marker.unwrap();
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn get_uuid(&self) -> u64 {
        self.uuid
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn class_types(&self) -> AHashSet<&'static str> {
        ["numeric_literal"].into()
    }
}

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct ComparisonOperatorSegment {
    pub raw: SmolStr,
    pub position_maker: PositionMarker,
    pub uuid: u64,
}

impl ComparisonOperatorSegment {
    pub fn create(raw: &str, position_maker: &PositionMarker) -> ErasedSegment {
        Self { raw: raw.into(), position_maker: position_maker.clone(), uuid: next_cache_key() }
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
        self.position_maker = _position_marker.unwrap();
    }

    fn segments(&self) -> &[ErasedSegment] {
        &[]
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone().to_erased_segment()]
    }

    fn get_uuid(&self) -> u64 {
        self.uuid
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn class_types(&self) -> AHashSet<&'static str> {
        ["numeric_literal"].into()
    }
}
