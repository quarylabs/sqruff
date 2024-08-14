use super::context::ParseContext;
use super::helpers::check_still_complete;
use super::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use crate::core::config::FluffConfig;
use crate::core::dialects::init::DialectKind;
use crate::core::errors::SQLParseError;
use crate::dialects::SyntaxKind;

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
        let start_idx = segments.iter().position(|segment| segment.is_code()).unwrap_or(0) as u32;

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
            let idx = unmatched.iter().position(|it| it.is_code()).unwrap_or(unmatched.len());
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
            [&segments[..start_idx as usize], content, &segments[end_idx as usize..]].concat(),
        ))
    }
}

/// Instantiates parsed queries from a sequence of lexed raw segments.
pub struct Parser<'a> {
    config: &'a FluffConfig,
    root_segment: FileSegment,
}

impl<'a> Parser<'a> {
    pub fn new(config: &'a FluffConfig, _dialect: Option<String>) -> Self {
        Self { config, root_segment: FileSegment }
    }

    pub fn config(&self) -> &FluffConfig {
        self.config
    }

    pub fn parse(
        &self,
        tables: &Tables,
        segments: &[ErasedSegment],
        f_name: Option<String>,
        parse_statistics: bool,
    ) -> Result<Option<ErasedSegment>, SQLParseError> {
        if segments.is_empty() {
            // This should normally never happen because there will usually
            // be an end_of_file segment. It would probably only happen in
            // api use cases.
            return Ok(None);
        }

        // NOTE: This is the only time we use the parse context not in the
        // context of a context manager. That's because it's the initial
        // instantiation.
        let mut parse_cx = ParseContext::from_config(self.config);
        // Kick off parsing with the root segment. The BaseFileSegment has
        // a unique entry point to facilitate exaclty this. All other segments
        // will use the standard .match()/.parse() route.
        let root = self.root_segment.root_parse(
            tables,
            parse_cx.dialect().name,
            segments,
            &mut parse_cx,
            f_name,
        )?;

        // Basic Validation, that we haven't dropped anything.
        check_still_complete(segments, &[root.clone()], &[]);

        if parse_statistics {
            unimplemented!();
        }

        Ok(root.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::linter::Linter;
    use crate::core::parser::segments::base::Tables;

    #[test]
    #[ignore]
    fn test_parser_parse_error() {
        let in_str = "SELECT ;".to_string();
        let config = FluffConfig::new(<_>::default(), None, None);
        let linter = Linter::new(config, None, None);
        let tables = Tables::default();
        let _ = linter.parse_string(&tables, &in_str, None, None, None);
    }
}
