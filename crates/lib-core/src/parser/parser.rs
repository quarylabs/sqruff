use ahash::AHashMap;

use super::context::ParseContext;
use super::helpers::check_still_complete;
use super::segments::base::{ErasedSegment, Tables};
use crate::dialects::base::Dialect;
use crate::errors::SQLParseError;
use crate::parser::segments::file::FileSegment;

#[derive(Clone)]
pub struct Parser<'a> {
    dialect: &'a Dialect,
    pub(crate) indentation_config: AHashMap<String, bool>,
}

impl<'a> Parser<'a> {
    pub fn new(dialect: &'a Dialect, indentation_config: AHashMap<String, bool>) -> Self {
        Self { dialect, indentation_config }
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
        let mut parse_cx: ParseContext = self.into();
        // Kick off parsing with the root segment. The BaseFileSegment has
        // a unique entry point to facilitate exaclty this. All other segments
        // will use the standard .match()/.parse() route.
        let root = FileSegment.root_parse(
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
