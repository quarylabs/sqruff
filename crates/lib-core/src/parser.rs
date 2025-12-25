pub mod adapters;
pub mod context;
pub mod core;
pub mod events;
pub mod grammar;
pub mod lexer;
pub mod lookahead;
pub mod markers;
pub mod match_algorithms;
pub mod match_result;
pub mod matchable;
pub mod node_matcher;
pub mod parsers;
pub mod segments;
pub mod types;

use ahash::AHashMap;

use crate::dialects::Dialect;
use crate::errors::SQLParseError;
use crate::parser::core::EventSink;
use crate::parser::events::{EventCollector, ParseEvent, ParseEventHandler, ParseEventHandlerSink};
use crate::parser::segments::builder::SegmentTreeBuilder;
use crate::parser::segments::file::FileSegment;
use crate::templaters::TemplatedFile;
use context::ParseContext;
use segments::{ErasedSegment, Tables};

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
            // This should normally never happen because there will usually
            // be an end_of_file segment. It would probably only happen in
            // api use cases.
            return Ok(None);
        }

        let templated_file = templated_file_for_segments(segments);
        let mut builder =
            SegmentTreeBuilder::new(self.dialect().name, tables, templated_file);
        self.parse_with_sink(segments, &mut builder)?;
        let root = builder.finish();

        #[cfg(debug_assertions)]
        {
            // Basic Validation, that we haven't dropped anything.
            let join_segments_raw = |segments: &[ErasedSegment]| {
                smol_str::SmolStr::from_iter(segments.iter().map(|s| s.raw().as_str()))
            };

            if let Some(root) = &root {
                pretty_assertions::assert_eq!(&join_segments_raw(segments), root.raw());
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

        // NOTE: This is the only time we use the parse context not in the
        // context of a context manager. That's because it's the initial
        // instantiation.
        let mut parse_cx: ParseContext = self.into();
        FileSegment.root_parse_events(segments, &mut parse_cx, sink)
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
