use hashbrown::HashSet;

use crate::dialects::syntax::SyntaxSet;
use crate::errors::SQLParseError;
use crate::parser::context::ParseContext;
use crate::parser::match_result::MatchResult;
use crate::parser::matchable::{
    Matchable, MatchableCacheKey, MatchableTrait, next_matchable_cache_key,
};
use crate::parser::segments::ErasedSegment;

/// Matches at the current position when the preceding code segments match one
/// of the configured keyword sequences.
#[derive(Clone, Debug, PartialEq)]
pub struct PrecededBy {
    preceding_sequences: Vec<Vec<&'static str>>,
    cache_key: MatchableCacheKey,
}

impl PrecededBy {
    pub fn new(preceding_sequences: Vec<Vec<&'static str>>) -> Self {
        Self {
            preceding_sequences,
            cache_key: next_matchable_cache_key(),
        }
    }

    fn preceded_by(&self, segments: &[ErasedSegment], idx: u32, sequence: &[&str]) -> bool {
        let mut preceding_idx = idx as usize;

        for keyword in sequence.iter().rev() {
            loop {
                let Some(candidate_idx) = preceding_idx.checked_sub(1) else {
                    return false;
                };
                preceding_idx = candidate_idx;

                if segments[candidate_idx].is_code() {
                    break;
                }
            }

            if !segments[preceding_idx].raw().eq_ignore_ascii_case(keyword) {
                return false;
            }
        }

        true
    }
}

impl MatchableTrait for PrecededBy {
    fn elements(&self) -> &[Matchable] {
        &[]
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, SyntaxSet)> {
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        idx: u32,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        if idx < segments.len() as u32
            && self
                .preceding_sequences
                .iter()
                .any(|sequence| self.preceded_by(segments, idx, sequence))
        {
            return Ok(MatchResult::from_span(idx, idx + 1));
        }

        Ok(MatchResult::empty_at(idx))
    }

    fn cache_key(&self) -> MatchableCacheKey {
        self.cache_key
    }
}

#[cfg(test)]
mod tests {
    use super::PrecededBy;
    use crate::dialects::Dialect;
    use crate::dialects::syntax::SyntaxKind;
    use crate::parser::IndentationConfig;
    use crate::parser::context::ParseContext;
    use crate::parser::matchable::MatchableTrait;
    use crate::parser::segments::{ErasedSegment, SegmentBuilder};

    fn matches(segments: Vec<ErasedSegment>) -> bool {
        let dialect = Dialect::new();
        let mut context = ParseContext::new(&dialect, IndentationConfig::default());
        let matcher = PrecededBy::new(vec![vec!["IS", "DISTINCT"], vec!["IS", "NOT", "DISTINCT"]]);
        let idx = segments.len() as u32 - 1;

        matcher
            .match_segments(&segments, idx, &mut context)
            .unwrap()
            .has_match()
    }

    fn keyword(raw: &str) -> ErasedSegment {
        SegmentBuilder::keyword(0, raw)
    }

    #[test]
    fn matches_preceding_keyword_sequences_across_non_code() {
        assert!(matches(vec![
            keyword("IS"),
            SegmentBuilder::whitespace(0, " "),
            keyword("DISTINCT"),
            SegmentBuilder::whitespace(0, " "),
            keyword("FROM"),
        ]));
        assert!(matches(vec![
            keyword("IS"),
            SegmentBuilder::token(0, "/* comment */", SyntaxKind::BlockComment).finish(),
            SegmentBuilder::token(0, "", SyntaxKind::Indent).finish(),
            keyword("NOT"),
            SegmentBuilder::newline(0, "\n"),
            keyword("DISTINCT"),
            keyword("FROM"),
        ]));
    }

    #[test]
    fn rejects_non_matching_preceding_keyword_sequences() {
        assert!(!matches(vec![
            keyword("IS"),
            keyword("REALLY"),
            keyword("DISTINCT"),
            keyword("FROM"),
        ]));
        assert!(!matches(vec![
            keyword("NOT"),
            keyword("DISTINCT"),
            keyword("FROM"),
        ]));
    }
}
