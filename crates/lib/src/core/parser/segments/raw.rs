use smol_str::SmolStr;
use uuid::Uuid;

use super::base::ErasedSegment;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::{CloneSegment, Segment};
use crate::core::parser::segments::fix::SourceFix;

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct RawSegment {
    raw: Option<SmolStr>,
    position_marker: Option<PositionMarker>,

    // From BaseSegment
    uuid: Uuid,
}

pub struct RawSegmentArgs {
    pub _type: Option<String>,
    pub _instance_types: Option<Vec<String>>,
    pub _trim_start: Option<Vec<String>>,
    pub _trim_cars: Option<Vec<String>>,
    pub _source_fixes: Option<Vec<SourceFix>>,
    pub _uuid: Option<Uuid>,
}

impl RawSegment {
    pub fn create(
        raw: Option<String>,
        position_marker: Option<PositionMarker>,
        // For legacy and syntactic sugar we allow the simple
        // `type` argument here, but for more precise inheritance
        // we suggest using the `instance_types` option.
        _args: RawSegmentArgs,
    ) -> Self {
        Self { position_marker, raw: raw.map(Into::into), uuid: Uuid::new_v4() }
    }
}

impl Segment for RawSegment {
    fn raw(&self) -> std::borrow::Cow<str> {
        self.raw.as_ref().unwrap().as_str().into()
    }

    fn get_type(&self) -> &'static str {
        "raw"
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
        self.position_marker.clone()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_raw_segments(&self) -> Vec<ErasedSegment> {
        vec![self.clone_box()]
    }

    fn get_uuid(&self) -> Option<Uuid> {
        Some(self.uuid)
    }

    fn edit(&self, _raw: Option<String>, _source_fixes: Option<Vec<SourceFix>>) -> ErasedSegment {
        todo!()
    }
}

#[cfg(test)]
mod test {
    // Test niche case of calling get_raw_segments on a raw segment.
    // TODO Implement
    // #[test]
    // fn test__parser__raw_get_raw_segments() {
    //     let segs = raw_segments();
    //
    //     for seg in segs {
    //         assert_eq!(seg.get_raw_segments(), Some(vec![seg.clone()]));
    //     }
    // }
}
