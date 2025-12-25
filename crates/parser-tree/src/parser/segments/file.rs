use crate::dialects::init::DialectKind;
use crate::dialects::syntax::SyntaxKind;
use crate::parser::segments::{ErasedSegment, SegmentBuilder, Tables};

#[derive(Debug, Clone, PartialEq)]
pub struct FileSegment;

impl FileSegment {
    pub fn of(
        tables: &Tables,
        dialect: DialectKind,
        segments: Vec<ErasedSegment>,
    ) -> ErasedSegment {
        SegmentBuilder::node(tables.next_id(), SyntaxKind::File, dialect, segments)
            .position_from_segments()
            .finish()
    }
}
