use super::base::Segment;

#[derive(Debug, Clone, Default)]
pub struct KeywordSegment {
    raw: String,
}

impl KeywordSegment {
    pub fn new(raw: String) -> Self {
        Self { raw }
    }
}

impl Segment for KeywordSegment {
    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn get_type(&self) -> &'static str {
        todo!()
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

    fn get_position_marker(&self) -> Option<crate::core::parser::markers::PositionMarker> {
        todo!()
    }

    fn set_position_marker(
        &mut self,
        position_marker: Option<crate::core::parser::markers::PositionMarker>,
    ) {
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
