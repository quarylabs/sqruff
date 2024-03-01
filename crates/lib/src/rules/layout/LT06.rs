use std::collections::HashSet;

use itertools::Itertools;

use crate::core::parser::segments::base::{CloneSegment, Segment};
use crate::core::rules::base::{LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT06 {}

impl Rule for RuleLT06 {
    fn description(&self) -> &'static str {
        "Function name not immediately followed by parenthesis."
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(HashSet::from(["function".into()])).into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context).segment();
        let children = segment.children(None);

        let function_name = children
            .find_first(Some(|segment: &dyn Segment| segment.is_type("function_name")))
            .pop();
        let start_bracket =
            children.find_first(Some(|segment: &dyn Segment| segment.is_type("bracketed"))).pop();

        let mut intermediate_segments = children.select(
            None,
            None,
            function_name.as_ref().into(),
            start_bracket.as_ref().into(),
        );

        if !intermediate_segments.is_empty() {
            return if intermediate_segments
                .all(Some(|seg| matches!(seg.get_type(), "whitespace" | "newline")))
            {
                vec![LintResult::new(
                    intermediate_segments.first().map(CloneSegment::clone_box),
                    intermediate_segments.into_iter().map(|seg| LintFix::delete(seg)).collect_vec(),
                    None,
                    None,
                    None,
                )]
            } else {
                vec![LintResult::new(intermediate_segments.pop().into(), vec![], None, None, None)]
            };
        }

        vec![]
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::Erased;
    use crate::rules::layout::LT06::RuleLT06;

    #[test]
    fn passing_example() {
        let sql = "SELECT SUM(1)";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleLT06::default().erased()], None, None)
                .unwrap();

        assert_eq!(result, &[]);
    }

    #[test]
    fn passing_example_window_function() {
        let sql = "SELECT AVG(c) OVER (PARTITION BY a)";
        let result =
            lint(sql.to_string(), "ansi".into(), vec![RuleLT06::default().erased()], None, None)
                .unwrap();
        assert_eq!(result, &[]);
    }

    #[test]
    fn simple_fail() {
        let sql = "SELECT SUM (1)";
        let result = fix(sql.to_string(), vec![RuleLT06::default().erased()]);
        assert_eq!(result, "SELECT SUM(1)");
    }

    #[test]
    fn complex_fail_1() {
        let sql = "SELECT SUM /* SOMETHING */ (1)";
        let violations =
            lint(sql.to_string(), "ansi".into(), vec![RuleLT06::default().erased()], None, None)
                .unwrap();

        assert_eq!(violations[0].desc(), "Function name not immediately followed by parenthesis.");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn complex_fail_2() {
        let sql = "
    SELECT
      SUM
      -- COMMENT
      (1)";

        let violations =
            lint(sql.to_string(), "ansi".into(), vec![RuleLT06::default().erased()], None, None)
                .unwrap();

        assert_eq!(violations[0].desc(), "Function name not immediately followed by parenthesis.");
        assert_eq!(violations.len(), 1);
    }
}
