pub mod context;
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
use crate::parser::segments::file::FileSegment;
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

        // NOTE: This is the only time we use the parse context not in the
        // context of a context manager. That's because it's the initial
        // instantiation.
        let mut parse_cx: ParseContext = self.into();

        // Kick off parsing with the root segment. The BaseFileSegment has
        // a unique entry point to facilitate exaclty this. All other segments
        // will use the standard .match()/.parse() route.
        let root =
            FileSegment.root_parse(tables, parse_cx.dialect().name, segments, &mut parse_cx)?;

        #[cfg(debug_assertions)]
        {
            // Basic Validation, that we haven't dropped anything.
            let join_segments_raw = |segments: &[ErasedSegment]| {
                smol_str::SmolStr::from_iter(segments.iter().map(|s| s.raw().as_str()))
            };

            pretty_assertions::assert_eq!(&join_segments_raw(segments), root.raw());
        }

        Ok(root.into())
    }
}
