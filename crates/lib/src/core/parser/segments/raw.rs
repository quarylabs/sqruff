use smol_str::SmolStr;

use super::base::ErasedSegment;
use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;
use crate::dialects::SyntaxKind;

#[derive(Debug, Clone, PartialEq)]
pub struct RawSegment {
    raw: Option<SmolStr>,
    position_marker: Option<PositionMarker>,

    // From BaseSegment
    id: u32,
}

pub struct RawSegmentArgs {
    pub _type: Option<String>,
    pub _instance_types: Option<Vec<String>>,
    pub _trim_start: Option<Vec<String>>,
    pub _trim_cars: Option<Vec<String>>,
    pub _source_fixes: Option<Vec<SourceFix>>,
    pub _uuid: Option<u32>,
}

impl RawSegment {
    pub fn create(
        id: u32,
        raw: Option<String>,
        position_marker: Option<PositionMarker>,
        // For legacy and syntactic sugar we allow the simple
        // `type` argument here, but for more precise inheritance
        // we suggest using the `instance_types` option.
        _args: RawSegmentArgs,
    ) -> Self {
        Self { position_marker, raw: raw.map(Into::into), id }
    }
}

impl Segment for RawSegment {
    fn raw(&self) -> std::borrow::Cow<str> {
        self.raw.as_ref().unwrap().as_str().into()
    }

    fn get_type(&self) -> SyntaxKind {
        SyntaxKind::Raw
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

    fn id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    fn edit(
        &self,
        _id: u32,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> ErasedSegment {
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
