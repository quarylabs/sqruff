use ahash::AHashMap;
use itertools::chain;

use crate::core::config::Value;
use crate::core::parser::segments::base::{
    ErasedSegment, NewlineSegment, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT10;

impl Rule for RuleLT10 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT10.erased())
    }

    fn name(&self) -> &'static str {
        "layout.select_modifiers"
    }

    fn description(&self) -> &'static str {
        "'SELECT' modifiers (e.g. 'DISTINCT') must be on the same line as 'SELECT'."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the `DISTINCT` modifier is on the next line after the `SELECT` keyword.

```sql
select
    distinct a,
    b
from x
```

**Best practice**

Move the `DISTINCT` modifier to the same line as the `SELECT` keyword.

```sql
select distinct
    a,
    b
from x
```
"#
    }
    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        // Get children of select_clause and the corresponding select keyword.
        let child_segments = FunctionalContext::new(context.clone()).segment().children(None);
        let select_keyword = child_segments.first().unwrap();

        // See if we have a select_clause_modifier.
        let select_clause_modifier_seg = child_segments
            .find_first(Some(|sp: &ErasedSegment| sp.is_type("select_clause_modifier")));

        // Rule doesn't apply if there's no select clause modifier.
        if select_clause_modifier_seg.is_empty() {
            return Vec::new();
        }

        // Are there any newlines between the select keyword and the select clause
        // modifier.
        let leading_newline_segments = child_segments.select(
            Some(|seg| seg.is_type("newline")),
            Some(|seg| seg.is_whitespace() || seg.is_meta()),
            select_keyword.into(),
            None,
        );

        // Rule doesn't apply if select clause modifier is already on the same line as
        // the select keyword.
        if leading_newline_segments.is_empty() {
            return Vec::new();
        }

        let select_clause_modifier = select_clause_modifier_seg.first().unwrap();

        // We should check if there is whitespace before the select clause modifier and
        // remove this during the lint fix.
        let leading_whitespace_segments = child_segments.select(
            Some(|seg| seg.is_type("whitespace")),
            Some(|seg| seg.is_whitespace() || seg.is_meta()),
            select_keyword.into(),
            None,
        );

        // We should also check if the following select clause element
        // is on the same line as the select clause modifier.
        let trailing_newline_segments = child_segments.select(
            Some(|seg| seg.is_type("newline")),
            Some(|seg| seg.is_whitespace() || seg.is_meta()),
            select_clause_modifier.into(),
            None,
        );

        // We will insert these segments directly after the select keyword.
        let mut edit_segments = vec![
            WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
            select_clause_modifier.clone(),
        ];

        if trailing_newline_segments.is_empty() {
            edit_segments.push(NewlineSegment::create("\n", None, <_>::default()));
        }

        let mut fixes = Vec::new();
        // Move select clause modifier after select keyword.
        fixes.push(LintFix::create_after(select_keyword.clone(), edit_segments, None));

        if trailing_newline_segments.is_empty() {
            fixes.extend(leading_newline_segments.into_iter().map(LintFix::delete));
        } else {
            let segments = chain(leading_newline_segments, leading_whitespace_segments);
            fixes.extend(segments.map(LintFix::delete));
        }

        let trailing_whitespace_segments = child_segments.select(
            Some(|segment| segment.is_whitespace()),
            Some(|seg| seg.is_whitespace() || seg.is_meta()),
            select_clause_modifier.into(),
            None,
        );

        if !trailing_whitespace_segments.is_empty() {
            fixes.extend(trailing_whitespace_segments.into_iter().map(LintFix::delete));
        }

        // Delete the original select clause modifier.
        fixes.push(LintFix::delete(select_clause_modifier.clone()));

        vec![LintResult::new(context.segment.into(), fixes, None, None, None)]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["select_clause"].into()).into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT10::RuleLT10;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT10::default().erased()]
    }

    #[test]
    fn test_fail_distinct_on_next_line_1() {
        let fail_str = "
SELECT
    DISTINCT user_id,
    list_id
FROM
    safe_user";

        let fix_str = fix(fail_str, rules());
        assert_eq!(
            fix_str,
            "
SELECT DISTINCT
    user_id,
    list_id
FROM
    safe_user"
        );
    }

    #[test]
    fn test_fail_distinct_on_next_line_2() {
        let fail_str = "
SELECT
    -- The table contains duplicates, so we use DISTINCT.
    DISTINCT user_id
FROM
    safe_user";

        let fix_str = fix(fail_str, rules());
        assert_eq!(
            fix_str,
            "
SELECT DISTINCT
    -- The table contains duplicates, so we use DISTINCT.
    user_id
FROM
    safe_user"
        );
    }

    #[test]
    fn test_fail_distinct_on_next_line_3() {
        let fail_str = "
select
distinct
    abc,
    def
from a;";

        let fix_str = fix(fail_str, rules());
        println!("{}", &fix_str);
    }
}
