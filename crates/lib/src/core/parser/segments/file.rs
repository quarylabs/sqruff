use uuid::Uuid;

use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::Segment;
use crate::core::parser::segments::fix::SourceFix;
/// A segment representing a whole file or script.
///
///     This is also the default "root" segment of the dialect,
///     and so is usually instantiated directly. It therefore
///     has no match_grammar.
#[derive(Hash, Debug, Clone, PartialEq)]
struct BaseFileSegment {
    pub f_name: Option<String>,
}

#[allow(unused_variables, dead_code)]
struct BaseFileSegmentNewArgs {
    f_name: Option<String>,
}

impl Segment for BaseFileSegment {
    fn get_raw(&self) -> Option<String> {
        todo!()
    }

    fn get_type(&self) -> &'static str {
        "file"
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
        todo!()
    }

    fn set_position_marker(&mut self, _position_marker: Option<PositionMarker>) {
        todo!()
    }

    fn get_can_start_end_non_code(&self) -> bool {
        false
    }

    fn get_allow_empty(&self) -> bool {
        true
    }

    fn get_file_path(&self) -> Option<String> {
        self.f_name.clone()
    }

    fn get_uuid(&self) -> Option<Uuid> {
        todo!()
    }

    fn edit(
        &self,
        _raw: Option<String>,
        _source_fixes: Option<Vec<SourceFix>>,
    ) -> Box<dyn Segment> {
        todo!()
    }
}

impl BaseFileSegment {
    #[allow(unused_variables, dead_code)]
    pub fn new(
        _segments: Vec<Box<dyn Segment>>,
        _position_maker: Option<PositionMarker>,
        f_name: Option<String>,
    ) -> Box<dyn Segment> {
        Box::new(BaseFileSegment { f_name })
    }
}

#[cfg(test)]
mod test {
    use crate::core::parser::segments::file::BaseFileSegment;
    use crate::core::parser::segments::test_functions::raw_segments;

    #[test]
    fn test__parser__base_segments_file() {
        let segments = raw_segments();
        let base_seg = BaseFileSegment::new(segments, None, Some("/some/dir/file.sql".to_string()));

        assert_eq!(base_seg.get_type(), "file");
        assert_eq!(base_seg.get_file_path(), Some("/some/dir/file.sql".to_string()));
        assert!(!base_seg.get_can_start_end_non_code());
        assert!(base_seg.get_allow_empty());
    }
}
