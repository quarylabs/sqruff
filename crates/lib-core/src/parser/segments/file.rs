use crate::dialects::init::DialectKind;
use crate::dialects::syntax::SyntaxKind;
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::matchable::MatchableTrait;
use crate::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};

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

    pub fn root_parse(
        &self,
        tables: &Tables,
        dialect: DialectKind,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
        _f_name: Option<String>,
    ) -> Result<ErasedSegment, SQLParseError> {
        let start_idx = segments
            .iter()
            .position(|segment| segment.is_code())
            .unwrap_or(0) as u32;

        let end_idx = segments
            .iter()
            .rposition(|segment| segment.is_code())
            .map_or(start_idx, |idx| idx as u32 + 1);

        if start_idx == end_idx {
            return Ok(FileSegment::of(tables, dialect, segments.to_vec()));
        }

        let final_seg = segments.last().unwrap();
        assert!(final_seg.get_position_marker().is_some());

        let file_segment = parse_context.dialect().r#ref("FileSegment");

        let match_result = file_segment.match_grammar().unwrap().match_segments(
            &segments[..end_idx as usize],
            start_idx,
            parse_context,
        )?;

        let match_span = match_result.span;
        let has_match = match_result.has_match();
        let mut matched = match_result.apply(tables, dialect, segments);
        let unmatched = &segments[match_span.end as usize..end_idx as usize];

        let content: &[ErasedSegment] = if !has_match {
            &[SegmentBuilder::node(
                tables.next_id(),
                SyntaxKind::Unparsable,
                dialect,
                segments[start_idx as usize..end_idx as usize].to_vec(),
            )
            .position_from_segments()
            .finish()]
        } else if !unmatched.is_empty() {
            let idx = unmatched
                .iter()
                .position(|it| it.is_code())
                .unwrap_or(unmatched.len());
            let (head, tail) = unmatched.split_at(idx);

            matched.extend_from_slice(head);
            matched.push(
                SegmentBuilder::node(tables.next_id(), SyntaxKind::File, dialect, tail.to_vec())
                    .position_from_segments()
                    .finish(),
            );
            &matched
        } else {
            matched.extend_from_slice(unmatched);
            &matched
        };

        Ok(Self::of(
            tables,
            dialect,
            [
                &segments[..start_idx as usize],
                content,
                &segments[end_idx as usize..],
            ]
            .concat(),
        ))
    }
}
