use uuid::Uuid;

use super::base::Segment;
use crate::core::parser::markers::PositionMarker;
use crate::helpers::Boxed;

#[derive(Hash, Debug, Clone)]
pub struct BracketedSegment {
    pub segments: Vec<Box<dyn Segment>>,
    pub start_bracket: Vec<Box<dyn Segment>>,
    pub end_bracket: Vec<Box<dyn Segment>>,
    pub pos_marker: Option<PositionMarker>,
    pub uuid: Uuid,
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
        BracketedSegment {
            segments,
            start_bracket,
            end_bracket,
            pos_marker: None,
            uuid: Uuid::new_v4(),
        }
    }
}

impl Segment for BracketedSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        let mut this = self.clone();
        this.segments = segments;
        this.boxed()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        self.segments.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }
}
