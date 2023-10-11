use crate::core::parser::markers::PositionMarker;
use crate::core::parser::segments::base::Segment;

/// A segment which is empty but indicates where an indent should be.
///
///     This segment is always empty, i.e. its raw format is '', but it indicates
///     the position of a theoretical indent which will be used in linting
///     and reconstruction. Even if there is an *actual indent* that occurs
///     in the same place this intentionally *won't* capture it, they will just
///     be compared later.
#[derive(Debug, Clone)]
pub struct Indent {}

pub struct IndentNewArgs {}

impl Segment for Indent {
    fn get_type(&self) -> &'static str {
        "indent"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }
}

impl Indent {
    pub fn new(_position_maker: PositionMarker) -> Box<dyn Segment> {
        Box::new(Indent {})
    }
}

/// A segment which is empty but indicates where an dedent should be.
///
///     This segment is always empty, i.e. its raw format is '', but it indicates
///     the position of a theoretical dedent which will be used in linting
///     and reconstruction. Even if there is an *actual dedent* that occurs
///     in the same place this intentionally *won't* capture it, they will just
///     be compared later.
#[derive(Debug, Clone)]
pub struct Dedent {}

pub struct DedentNewArgs {}

impl Segment for Dedent {
    fn get_type(&self) -> &'static str {
        "dedent"
    }

    fn is_code(&self) -> bool {
        false
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }
}

impl Dedent {
    pub fn new(_position_maker: PositionMarker) -> Box<dyn Segment> {
        Box::new(Dedent {})
    }
}
