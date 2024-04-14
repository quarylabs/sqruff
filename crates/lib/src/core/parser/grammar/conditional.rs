use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{ErasedSegment, Segment};
use crate::core::parser::segments::meta::Indent;
use crate::helpers::ToErasedSegment;

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct Conditional {
    meta: Indent,
    indented_joins: bool,
    indented_using_on: bool,
    indented_on_contents: bool,
    indented_then: bool,
}

impl Conditional {
    pub fn new(meta: Indent) -> Self {
        Self {
            meta,
            indented_joins: false,
            indented_using_on: false,
            indented_on_contents: false,
            indented_then: false,
        }
    }

    pub fn indented_joins(mut self) -> Self {
        self.indented_joins = true;
        self
    }

    pub fn indented_using_on(mut self) -> Self {
        self.indented_using_on = true;
        self
    }

    pub fn indented_on_contents(mut self) -> Self {
        self.indented_on_contents = true;
        self
    }

    pub fn indented_then(mut self) -> Self {
        self.indented_then = true;
        self
    }

    pub fn indented_then_contents(mut self) -> Self {
        self
    }

    fn is_enabled(&self, parse_context: &mut ParseContext) -> bool {
        macro_rules! check_config_match {
            ($self:expr, $parse_context:expr, $field:ident) => {{
                let config_value = $parse_context
                    .indentation_config
                    .get(stringify!($field))
                    .copied()
                    .unwrap_or_default();

                if $self.$field && $self.$field != config_value {
                    return false;
                }
            }};
        }

        check_config_match!(self, parse_context, indented_joins);
        check_config_match!(self, parse_context, indented_using_on);
        check_config_match!(self, parse_context, indented_on_contents);
        check_config_match!(self, parse_context, indented_then);

        true
    }
}

impl Segment for Conditional {}

impl Matchable for Conditional {
    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if !self.is_enabled(parse_context) {
            return Ok(MatchResult::from_unmatched(segments.to_vec()));
        }

        Ok(MatchResult {
            matched_segments: vec![self.meta.clone().to_erased_segment()],
            unmatched_segments: segments.to_vec(),
        })
    }
}
