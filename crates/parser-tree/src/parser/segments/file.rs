use crate::dialects::DialectKind;
use crate::dialects::SyntaxKind;
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
