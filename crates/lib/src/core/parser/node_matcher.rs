use std::sync::Arc;

use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::match_result::{MatchResult, Matched};
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::ErasedSegment;
use crate::dialects::SyntaxKind;

#[macro_export]
macro_rules! vec_of_erased {
    ($($elem:expr),* $(,)?) => {{
        vec![$(Arc::new($elem)),*]
    }};
}

#[derive(Debug, Clone)]
pub struct NodeMatcher {
    node_kind: SyntaxKind,
    pub(crate) match_grammar: Arc<dyn Matchable>,
}

impl NodeMatcher {
    pub fn new(node_kind: SyntaxKind, match_grammar: Arc<dyn Matchable>) -> Self {
        Self { node_kind, match_grammar }
    }
}

impl PartialEq for NodeMatcher {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Matchable for NodeMatcher {
    fn get_type(&self) -> SyntaxKind {
        self.node_kind
    }

    fn match_grammar(&self) -> Option<Arc<dyn Matchable>> {
        self.match_grammar.clone().into()
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
