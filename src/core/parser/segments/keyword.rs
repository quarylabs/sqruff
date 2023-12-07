use crate::core::parser::markers::PositionMarker;

use super::base::Segment;

#[derive(Debug, Clone, Default)]
pub struct KeywordSegment {
    raw: String,
    uuid: uuid::Uuid,
    position_marker: PositionMarker,
}

impl KeywordSegment {
    pub fn new(raw: String, position_marker: PositionMarker) -> Self {
        Self {
            raw,
            uuid: uuid::Uuid::new_v4(),
            position_marker,
        }
    }
}

impl Segment for KeywordSegment {
    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn get_type(&self) -> &'static str {
        "kw"
    }

    fn is_code(&self) -> bool {
        true
    }

    fn is_comment(&self) -> bool {
        todo!()
    }

    fn is_whitespace(&self) -> bool {
        todo!()
    }

    fn get_position_marker(&self) -> Option<PositionMarker> {
        self.position_marker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<uuid::Uuid> {
        self.uuid.into()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<super::fix::SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }
}
