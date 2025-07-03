use std::sync::OnceLock;

use super::matchable::MatchableTrait;
use crate::dialects::Dialect;
use crate::dialects::syntax::SyntaxKind;
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::{MatchResult, Matched};
use crate::parser::matchable::Matchable;
use crate::parser::segments::ErasedSegment;

#[macro_export]
macro_rules! vec_of_erased {
    ($($elem:expr),* $(,)?) => {{
        vec![$(ToMatchable::to_matchable($elem)),*]
    }};
}

#[derive(Clone)]
pub struct NodeMatcher {
    node_kind: SyntaxKind,
    match_grammar: OnceLock<Matchable>,
    factory: fn(&Dialect) -> Matchable,
}

impl NodeMatcher {
    pub fn new(node_kind: SyntaxKind, build_grammar: fn(&Dialect) -> Matchable) -> Self {
        Self {
            node_kind,
            match_grammar: OnceLock::new(),
            factory: build_grammar,
        }
    }

    pub fn match_grammar(&self, dialect: &Dialect) -> Matchable {
        self.match_grammar
            .get_or_init(|| (self.factory)(dialect))
            .clone()
    }

    pub fn replace(&mut self, match_grammar: Matchable) {
        self.match_grammar = OnceLock::new();
        let _ = self.match_grammar.set(match_grammar);
    }
}

impl std::fmt::Debug for NodeMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeMatcher")
            .field("node_kind", &self.node_kind)
            .field("match_grammar", &"...")
            .field("factory", &"...")
            .finish()
    }
}

impl PartialEq for NodeMatcher {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl MatchableTrait for NodeMatcher {
    fn get_type(&self) -> SyntaxKind {
        self.node_kind
    }

    fn match_grammar(&self, dialect: &Dialect) -> Option<Matchable> {
        self.match_grammar(dialect).into()
    }

    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if idx >= segments.len() as u32 {
            return Ok(MatchResult::empty_at(idx));
        }

        if segments[idx as usize].get_type() == self.get_type() {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        let grammar = self.match_grammar(parse_context.dialect());
        let match_result = parse_context
            .deeper_match(false, &[], |ctx| grammar.match_segments(segments, idx, ctx))?;

        Ok(match_result.wrap(Matched::SyntaxKind(self.node_kind)))
    }
}
