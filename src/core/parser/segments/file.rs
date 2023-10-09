use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::Segment;

/// A segment representing a whole file or script.
///
///     This is also the default "root" segment of the dialect,
///     and so is usually instantiated directly. It therefore
///     has no match_grammar.
#[derive(Debug, Clone)]
struct FileSegment {
    f_name: Option<String>,
}

struct FileSegmentNewArgs {
    f_name: Option<String>,
}

impl Segment for FileSegment {
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
}

impl FileSegment {
    pub fn new(
        raw: &str,
        position_maker: &PositionMarker,
        args: FileSegmentNewArgs,
    ) -> Box<dyn Segment> {
        Box::new(FileSegment {
            f_name: args.f_name,
        })
    }

    pub fn get_file_path(&self) -> Option<String> {
        self.f_name.clone()
    }
}
