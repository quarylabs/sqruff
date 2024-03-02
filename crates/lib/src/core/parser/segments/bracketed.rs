use uuid::Uuid;

use super::base::{pos_marker, Segment};
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
    fn eq(&self, other: &Self) -> bool {
        self.segments.iter().zip(&other.segments).all(|(lhs, rhs)| lhs.dyn_eq(rhs.as_ref()))
            && self.start_bracket == other.start_bracket
            && self.end_bracket == other.end_bracket
    }
}

impl BracketedSegment {
    pub fn new(
        segments: Vec<Box<dyn Segment>>,
        start_bracket: Vec<Box<dyn Segment>>,
        end_bracket: Vec<Box<dyn Segment>>,
    ) -> Self {
        let mut this = BracketedSegment {
            segments,
            start_bracket,
            end_bracket,
            pos_marker: None,
            uuid: Uuid::new_v4(),
        };
        this.pos_marker = pos_marker(&this).into();
        this
    }
}

impl Segment for BracketedSegment {
    fn new(&self, segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        let mut this = self.clone();
        this.segments = segments;
        this.boxed()
    }

    fn segments(&self) -> &[Box<dyn Segment>] {
        &self.segments
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_type(&self) -> &'static str {
        "bracketed"
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.pos_marker.clone()
    }
}
