use crate::core::parser::markers::PositionMarker;

use super::base::Segment;

#[derive(Debug, Clone)]
pub struct BracketedSegment {
    pub segments: Vec<Box<dyn Segment>>,
    pub start_bracket: Box<dyn Segment>,
    pub end_bracket: Box<dyn Segment>,
    pub pos_marker: Option<PositionMarker>,
    pub uuid: Option<uuid::Uuid>,
}

impl BracketedSegment {
    pub fn new(
        segments: Vec<Box<dyn Segment>>,
        start_bracket: Box<dyn Segment>,
        end_bracket: Box<dyn Segment>,
    ) -> Self {
        BracketedSegment {
            segments,
            start_bracket,
            end_bracket,
            pos_marker: None,
            uuid: None,
        }
    }
}

impl Segment for BracketedSegment {
    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn is_code(&self) -> bool {
        todo!()
    }

    fn is_comment(&self) -> bool {
        todo!()
    }

    fn is_whitespace(&self) -> bool {
        todo!()
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        todo!()
    }

    fn set_position_marker(&mut self, position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<uuid::Uuid> {
        todo!()
    }

    fn edit(
        &self,
        raw: Option<String>,
        source_fixes: Option<Vec<super::fix::SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }
}
