use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::{MatchResult, Span};
use crate::parser::matchable::{Matchable, MatchableTrait};
use crate::parser::segments::base::ErasedSegment;
use crate::parser::segments::meta::Indent;

#[derive(Clone, Debug, PartialEq)]
pub struct Conditional {
    meta: Indent,
    indented_joins: bool,
    indented_using_on: bool,
    indented_on_contents: bool,
    indented_then: bool,
    indented_then_contents: bool,
    indented_joins_on: bool,
    indented_ctes: bool,
}

impl Conditional {
    pub fn new(meta: Indent) -> Self {
        Self {
            meta,
            indented_joins: false,
            indented_using_on: false,
            indented_on_contents: false,
            indented_then: false,
            indented_then_contents: false,
            indented_joins_on: false,
            indented_ctes: false,
        }
    }

    pub fn indented_ctes(mut self) -> Self {
        self.indented_ctes = true;
        self
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
        self.indented_then_contents = true;
        self
    }

    pub fn indented_joins_on(mut self) -> Self {
        self.indented_joins_on = true;
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
        check_config_match!(self, parse_context, indented_then_contents);
        check_config_match!(self, parse_context, indented_joins_on);
        check_config_match!(self, parse_context, indented_ctes);

        true
    }
}

impl MatchableTrait for Conditional {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn match_segments(
        &self,
        _segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if !self.is_enabled(parse_context) {
            return Ok(MatchResult::empty_at(idx));
        }

        Ok(MatchResult {
            span: Span {
                start: idx,
                end: idx,
            },
            insert_segments: vec![(idx, self.meta.kind)],
            ..Default::default()
        })
    }
}
