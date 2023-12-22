use std::collections::HashSet;

use crate::core::errors::SQLParseError;
use crate::core::parser::context::ParseContext;
use crate::core::parser::match_result::MatchResult;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::segments::base::Segment;

#[derive(Debug, Clone, PartialEq)]
pub struct NonCodeMatcher;

impl Segment for NonCodeMatcher {}

impl Matchable for NonCodeMatcher {
    // Implement the simple method
    fn simple(
        &self,
        parse_context: &ParseContext,
        crumbs: Option<Vec<&str>>,
    ) -> Option<(HashSet<String>, HashSet<String>)> {
        None
    }

    fn is_optional(&self) -> bool {
        // Not optional
        false
    }

    fn cache_key(&self) -> String {
        "non-code-matcher".to_string()
    }

    fn match_segments(
        &self,
        segments: Vec<Box<dyn Segment>>,
        _parse_context: &mut ParseContext,
    ) -> Result<MatchResult, SQLParseError> {
        // Match any starting non-code segments
        let mut idx = 0;

        while idx < segments.len() && !segments[idx].is_code() {
            idx += 1;
        }

        Ok(MatchResult::new(segments[0..idx].to_vec(), segments[idx..].to_vec()))
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
        let mut ctx = ParseContext::new(dialect);

        let matcher = NonCodeMatcher;
        let test_segments = test_segments(); // Assuming this function exists and generates test segments
        let m = matcher.match_segments(test_segments[1..].to_vec(), &mut ctx).unwrap();

        // NonCode Matcher doesn't work with simple
        assert!(matcher.simple(&ctx, None).is_none());

        // We should match one and only one segment
        assert_eq!(m.len(), 1);
    }
}
