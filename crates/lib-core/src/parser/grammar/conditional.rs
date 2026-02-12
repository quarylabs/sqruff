use crate::errors::SQLParseError;
use crate::parser::IndentationConfig;
use crate::parser::context::ParseContext;
use crate::parser::match_result::{MatchResult, Span};
use crate::parser::matchable::{Matchable, MatchableTrait};
use crate::parser::segments::ErasedSegment;
use crate::parser::segments::meta::Indent;

#[derive(Clone, Debug, PartialEq)]
pub struct Conditional {
    meta: Indent,
    requirements: IndentationConfig,
}

impl Conditional {
    pub fn new(meta: Indent) -> Self {
        Self {
            meta,
            requirements: IndentationConfig::default(),
        }
    }

    fn require(mut self, flag: IndentationConfig) -> Self {
        self.requirements.insert(flag);
        self
    }

    pub fn indented_ctes(self) -> Self {
        self.require(IndentationConfig::INDENTED_CTES)
    }

    pub fn indented_joins(self) -> Self {
        self.require(IndentationConfig::INDENTED_JOINS)
    }

    pub fn indented_using_on(self) -> Self {
        self.require(IndentationConfig::INDENTED_USING_ON)
    }

    pub fn indented_on_contents(self) -> Self {
        self.require(IndentationConfig::INDENTED_ON_CONTENTS)
    }

    pub fn indented_then(self) -> Self {
        self.require(IndentationConfig::INDENTED_THEN)
    }

    pub fn indented_then_contents(self) -> Self {
        self.require(IndentationConfig::INDENTED_THEN_CONTENTS)
    }

    pub fn indented_joins_on(self) -> Self {
        self.require(IndentationConfig::INDENTED_JOINS_ON)
    }

    fn is_enabled(&self, parse_context: &ParseContext) -> bool {
        parse_context.indentation_config.contains(self.requirements)
    }

    pub(crate) fn meta_kind(&self) -> crate::dialects::syntax::SyntaxKind {
        self.meta.kind
    }

    pub(crate) fn requirements(&self) -> IndentationConfig {
        self.requirements
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
