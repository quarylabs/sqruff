use std::collections::HashMap;

use uuid::Uuid;

use super::base::{apply_fixes, Segment};
use super::fix::{AnchorEditInfo, SourceFix};
use crate::core::dialects::base::Dialect;
use crate::core::parser::markers::PositionMarker;
use crate::helpers::Boxed;

#[derive(Hash, Debug, Clone, PartialEq)]
pub struct LiteralSegment {
    pub raw: String,
    pub position_maker: PositionMarker,
    pub uuid: Uuid,
}

impl Segment for LiteralSegment {
    fn new(&self, _segments: Vec<Box<dyn Segment>>) -> Box<dyn Segment> {
        Self { raw: self.raw.clone(), position_maker: self.position_maker.clone(), uuid: self.uuid }
            .boxed()
    }

    fn get_raw(&self) -> Option<String> {
        self.raw.clone().into()
    }

    fn get_segments(&self) -> Vec<Box<dyn Segment>> {
        Vec::new()
    }

    fn apply_fixes(
        &self,
        dialect: Dialect,
        fixes: HashMap<Uuid, AnchorEditInfo>,
    ) -> (Box<dyn Segment>, Vec<Box<dyn Segment>>, Vec<Box<dyn Segment>>, bool) {
        apply_fixes(self, dialect, fixes)
    }

    fn get_raw_segments(&self) -> Vec<Box<dyn Segment>> {
        vec![self.clone().boxed()]
    }

    fn get_type(&self) -> &'static str {
        "numeric_literal"
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
        self.position_maker.clone().into()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.into()
    }

    fn get_source_fixes(&self) -> Vec<SourceFix> {
        Vec::new()
    }

    fn class_types(&self) -> std::collections::HashSet<String> {
        ["numeric_literal"].map(ToOwned::to_owned).into_iter().collect()
    }
}

#[cfg(test)]
mod test {
    use crate::core::parser::segments::base::Segment;
    use crate::core::parser::segments::test_functions::generate_test_segments_func;

    // NOTE: For legacy reasons we override this fixture for this module
    fn raw_segments() -> Vec<Box<dyn Segment>> {
        generate_test_segments_func(["bar", "foo", "bar"].to_vec())
    }
}
