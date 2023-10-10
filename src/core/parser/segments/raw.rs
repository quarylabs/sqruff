use crate::core::parser::segments::base::Segment;

#[derive(Debug, Clone)]
pub struct RawSegment {}

impl RawSegment {
    pub fn new() -> Self {
        Self {}
    }
}

impl Segment for RawSegment {
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
}
