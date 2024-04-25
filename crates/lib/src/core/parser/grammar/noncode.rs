use ahash::AHashSet;

use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::{ErasedSegment, Segment};

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct NonCodeMatcher;

impl Segment for NonCodeMatcher {}

impl Matchable for NonCodeMatcher {
    fn is_optional(&self) -> bool {
        // Not optional
        false
    }

    fn simple(
        &self,
        _parse_context: &ParseContext,
        _crumbs: Option<Vec<&str>>,
    ) -> Option<(AHashSet<String>, AHashSet<String>)> {
        None
    }

    fn match_segments(
        &self,
        segments: &[ErasedSegment],
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        // Match any starting non-code segments
        let mut idx = 0;

        while idx < segments.len() && !segments[idx].is_code() {
            idx += 1;
        }

        Ok(MatchResult::new(segments[0..idx].to_vec(), segments[idx..].to_vec()))
    }

    fn cache_key(&self) -> String {
        "non-code-matcher".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::parser::context::ParseContext;
    use crate::core::parser::grammar::noncode::NonCodeMatcher;
    use crate::core::parser::matchable::Matchable;
    use crate::core::parser::segments::test_functions::{fresh_ansi_dialect, test_segments};

    #[test]
    fn test__parser__grammar_noncode() {
        let dialect = fresh_ansi_dialect(); // Assuming this function exists and returns a Dialect
        let mut ctx = ParseContext::new(&dialect, <_>::default());

        let matcher = NonCodeMatcher;
        let test_segments = test_segments(); // Assuming this function exists and generates test segments
        let m = matcher.match_segments(&test_segments[1..], &mut ctx).unwrap();

        // NonCode Matcher doesn't work with simple
        assert!(matcher.simple(&ctx, None).is_none());

        // We should match one and only one segment
        assert_eq!(m.len(), 1);
    }
}
