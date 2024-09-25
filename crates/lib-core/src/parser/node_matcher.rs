use super::matchable::MatchableTrait;
use crate::dialects::syntax::SyntaxKind;
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::{MatchResult, Matched};
use crate::parser::matchable::Matchable;
use crate::parser::segments::base::ErasedSegment;

#[macro_export]
macro_rules! vec_of_erased {
    ($($elem:expr),* $(,)?) => {{
        vec![$(ToMatchable::to_matchable($elem)),*]
    }};
}

#[derive(Debug, Clone)]
pub struct NodeMatcher {
    node_kind: SyntaxKind,
    pub(crate) match_grammar: Matchable,
}

impl NodeMatcher {
    pub fn new(node_kind: SyntaxKind, match_grammar: Matchable) -> Self {
        Self {
            node_kind,
            match_grammar,
        }
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

    fn match_grammar(&self) -> Option<Matchable> {
        self.match_grammar.clone().into()
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

        let grammar = self.match_grammar().unwrap();
        let match_result = parse_context
            .deeper_match(false, &[], |ctx| grammar.match_segments(segments, idx, ctx))?;

        Ok(match_result.wrap(Matched::SyntaxKind(self.node_kind)))
    }
}
