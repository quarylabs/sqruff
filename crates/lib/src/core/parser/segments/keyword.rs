use std::borrow::Cow;

use ahash::AHashSet;
use smol_str::SmolStr;
use uuid::Uuid;

use super::base::{ErasedSegment, Segment};
use super::fix::SourceFix;
use crate::core::parser::markers::PositionMarker;
use crate::helpers::ToErasedSegment;

#[derive(Hash, Debug, Clone, Default, PartialEq)]
pub struct KeywordSegment {
    raw: SmolStr,
    uuid: Uuid,
    position_marker: Option<PositionMarker>,
}

impl KeywordSegment {
    pub fn new(raw: SmolStr, position_marker: Option<PositionMarker>) -> Self {
        Self { raw, uuid: Uuid::new_v4(), position_marker }
    }
}

impl Segment for KeywordSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        KeywordSegment::new(self.raw.clone(), self.position_marker.clone()).to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> &'static str {
        "keyword"
    }

    fn is_code(&self) -> bool {
        true
    }

    fn is_comment(&self) -> bool {
        todo!()
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

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn edit(&self, raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        Self::new(raw.unwrap().into(), self.get_position_marker()).to_erased_segment()
    }

    fn class_types(&self) -> AHashSet<&'static str> {
        ["keyword", "word"].into()
    }
}
