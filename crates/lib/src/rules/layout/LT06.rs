use ahash::AHashMap;
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT06 {}

impl Rule for RuleLT06 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT06::default().erased())
    }

    fn name(&self) -> &'static str {
        "layout.functions"
    }

    fn description(&self) -> &'static str {
        "Function name not immediately followed by parenthesis."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, there is a space between the function and the parenthesis.

```sql
SELECT
    sum (a)
FROM foo
```

**Best practice**

Remove the space between the function and the parenthesis.

```sql
SELECT
    sum(a)
FROM foo
```
"#
    }
    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context).segment();
        let children = segment.children(None);

        let function_name = children
            .find_first(Some(|segment: &ErasedSegment| segment.is_type("function_name")))
            .pop();
        let start_bracket =
            children.find_first(Some(|segment: &ErasedSegment| segment.is_type("bracketed"))).pop();

        let mut intermediate_segments =
            children.select(None, None, Some(&function_name), Some(&start_bracket));

        if !intermediate_segments.is_empty() {
            return if intermediate_segments
                .all(Some(|seg| matches!(seg.get_type(), "whitespace" | "newline")))
            {
                vec![LintResult::new(
                    intermediate_segments.first().cloned(),
                    intermediate_segments.into_iter().map(LintFix::delete).collect_vec(),
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

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["function"].into()).into()
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
        let result = fix(sql, vec![RuleLT06::default().erased()]);
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
