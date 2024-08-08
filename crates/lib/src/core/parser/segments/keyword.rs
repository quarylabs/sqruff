use std::borrow::Cow;

use smol_str::SmolStr;

use super::base::{ErasedSegment, Segment};
use super::fix::SourceFix;
use crate::core::parser::markers::PositionMarker;
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::helpers::ToErasedSegment;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeywordSegment {
    raw: SmolStr,
    id: u32,
    position_marker: Option<PositionMarker>,
}

impl KeywordSegment {
    pub fn new(id: u32, raw: SmolStr, position_marker: Option<PositionMarker>) -> Self {
        Self { raw, id, position_marker }
    }
}

impl Segment for KeywordSegment {
    fn new(&self, _segments: Vec<ErasedSegment>) -> ErasedSegment {
        KeywordSegment::new(self.id(), self.raw.clone(), self.position_marker.clone())
            .to_erased_segment()
    }

    fn raw(&self) -> Cow<str> {
        self.raw.as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::Keyword
    }

    fn is_code(&self) -> bool {
        true
    }

    fn is_comment(&self) -> bool {
        false
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

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn edit(
        &self,
        id: u32,
        raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
        Self::new(id, raw.unwrap().into(), self.get_position_marker()).to_erased_segment()
    }

    fn class_types(&self) -> SyntaxSet {
        SyntaxSet::new(&[SyntaxKind::Keyword, SyntaxKind::Word])
    }
}
