pub mod adapters;
pub mod markers;
pub mod segments;

use ahash::AHashMap;

use sqruff_parser_core::errors::SQLParseError as CoreParseError;
use sqruff_parser_core::parser::Parser as CoreParser;
use sqruff_parser_core::parser::event_sink::EventSink;
use sqruff_parser_core::parser::events::{
    EventCollector, ParseEvent, ParseEventHandler, ParseEventHandlerSink,
};

use crate::dialects::Dialect;
use crate::errors::SQLParseError;
use crate::parser::adapters::{token_span_from_marker, tokens_from_segments};
use crate::parser::segments::builder::SegmentTreeBuilder;
use crate::parser::segments::{ErasedSegment, Tables};
use crate::templaters::TemplatedFile;

#[derive(Clone)]
pub struct Parser<'a> {
    dialect: &'a Dialect,
    pub(crate) indentation_config: AHashMap<String, bool>,
}

impl<'a> From<&'a Dialect> for Parser<'a> {
    fn from(value: &'a Dialect) -> Self {
        Self {
            dialect: value,
            indentation_config: AHashMap::new(),
        }
    }
}

impl<'a> Parser<'a> {
    pub fn new(dialect: &'a Dialect, indentation_config: AHashMap<String, bool>) -> Self {
        Self {
            dialect,
            indentation_config,
        }
    }

    pub fn dialect(&self) -> &Dialect {
        self.dialect
    }

    pub fn indentation_config(&self) -> &AHashMap<String, bool> {
        &self.indentation_config
    }

    pub fn parse(
        &self,
        tables: &Tables,
        segments: &[ErasedSegment],
    ) -> Result<Option<ErasedSegment>, SQLParseError> {
        if segments.is_empty() {
            return Ok(None);
        }

        let templated_file = templated_file_for_segments(segments);
        let mut builder = SegmentTreeBuilder::new(self.dialect().name, tables, templated_file);
        self.parse_with_sink(segments, &mut builder)?;
        let root = builder.finish();

        #[cfg(debug_assertions)]
        {
            let join_segments_raw = |segments: &[ErasedSegment]| {
                smol_str::SmolStr::from_iter(segments.iter().map(|s| s.raw().as_str()))
            };

            if let Some(root) = &root {
                assert_eq!(&join_segments_raw(segments), root.raw());
            }
        }

        Ok(root)
    }

    pub fn parse_with_sink(
        &self,
        segments: &[ErasedSegment],
        sink: &mut impl EventSink,
    ) -> Result<(), SQLParseError> {
        if segments.is_empty() {
            return Ok(());
        }

        let tokens = tokens_from_segments(segments);
        let parser = CoreParser::new(self.dialect, self.indentation_config.clone());
        parser
            .parse_with_sink(&tokens, sink)
            .map_err(|err| map_parse_error(err, segments))
    }

    pub fn parse_with(
        &self,
        _tables: &Tables,
        segments: &[ErasedSegment],
        handler: &mut impl ParseEventHandler,
    ) -> Result<(), SQLParseError> {
        let mut sink = ParseEventHandlerSink::new(handler);
        self.parse_with_sink(segments, &mut sink)
    }

    pub fn parse_events(
        &self,
        _tables: &Tables,
        segments: &[ErasedSegment],
    ) -> Result<Vec<ParseEvent>, SQLParseError> {
        let mut collector = EventCollector::default();
        self.parse_with_sink(segments, &mut collector)?;
        Ok(collector.into_events())
    }
}

fn templated_file_for_segments(segments: &[ErasedSegment]) -> TemplatedFile {
    segments
        .iter()
        .find_map(|segment| segment.get_position_marker())
        .map(|marker| marker.templated_file.clone())
        .expect("parsed segments should have a position marker")
}

fn map_parse_error(err: CoreParseError, segments: &[ErasedSegment]) -> SQLParseError {
    let segment = err.span.and_then(|span| {
        segments
            .iter()
            .find(|segment| {
                segment
                    .get_position_marker()
                    .map(|marker| token_span_from_marker(marker) == span)
                    .unwrap_or(false)
            })
            .cloned()
    });

    SQLParseError {
        description: err.description,
        segment,
    }
}
