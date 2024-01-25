use super::base::Segment;
use crate::core::parser::markers::PositionMarker;

#[derive(Hash, Debug, Clone)]
pub struct BracketedSegment {
    pub segments: Vec<Box<dyn Segment>>,
    pub start_bracket: Vec<Box<dyn Segment>>,
    pub end_bracket: Vec<Box<dyn Segment>>,
    pub pos_marker: Option<PositionMarker>,
    pub uuid: Option<uuid::Uuid>,
}

impl PartialEq for BracketedSegment {
    fn eq(&self, _other: &Self) -> bool {
        unimplemented!()
    }
}

impl BracketedSegment {
    pub fn new(
        segments: Vec<Box<dyn Segment>>,
        start_bracket: Vec<Box<dyn Segment>>,
        end_bracket: Vec<Box<dyn Segment>>,
    ) -> Self {
        BracketedSegment { segments, start_bracket, end_bracket, pos_marker: None, uuid: None }
    }
}

impl Segment for BracketedSegment {
    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }
}
