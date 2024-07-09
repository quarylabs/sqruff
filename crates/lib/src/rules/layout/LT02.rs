use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Default, Debug, Clone)]
pub struct RuleLT02;

impl Rule for RuleLT02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT02.erased())
    }
    fn name(&self) -> &'static str {
        "layout.indent"
    }

    fn description(&self) -> &'static str {
        "Incorrect Indentation."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

The ``•`` character represents a space and the ``→`` character represents a tab.
In this example, the third line contains five spaces instead of four and
the second line contains two spaces and one tab.

```sql
SELECT
••→a,
•••••b
FROM foo
```

**Best practice**

Change the indentation to use a multiple of four spaces. This example also assumes that the indent_unit config value is set to space. If it had instead been set to tab, then the indents would be tabs instead.

```sql
SELECT
••••a,
••••b
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_root(context.segment, context.config.unwrap()).reindent().results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT02::RuleLT02;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT02.erased()]
    }

    #[test]
    fn test_fail_reindent_first_line_1() {
        let fail_str = "     SELECT 1";
        let violations = lint(fail_str.into(), "ansi".into(), rules(), None, None).unwrap();

        assert_eq!(violations[0].desc(), "First line should not be indented.");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_reindent_first_line_2() {
        let fixed = fix("  select 1 from tbl;", rules());
        assert_eq!(fixed, "select 1 from tbl;");
    }

    #[test]
    fn test_pass_indentation_of_comments_1() {
        let sql = "
SELECT
    -- Compute the thing
    (a + b) AS c
FROM
    acceptable_buckets"
            .trim_start();

        let violations = lint(sql.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_indentation_of_comments_2() {
        let pass_str = "
SELECT
    user_id
FROM
    age_data
JOIN
    audience_size
    USING (user_id, list_id)
-- We LEFT JOIN because blah
LEFT JOIN
    verts
    USING
        (user_id)"
            .trim_start();

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    #[ignore = "impl config"]
    fn test_fail_tab_indentation() {}

    #[test]
    fn test_pass_indented_joins_default() {
        let pass_str = "
SELECT a, b, c
FROM my_tbl
LEFT JOIN another_tbl USING(a)"
            .trim_start();

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_trailing_comment_1() {
        let pass_str = "
select
    bar
    -- comment
from foo";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_trailing_comment_2() {
        let pass_str = "
select
    bar
    -- comment
from foo";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_issue_4582() {
        let pass_str = "
select
    bar
    -- comment
from foo";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    // LT02-tab-space.yml

    #[test]
    fn spaces_pass_default() {
        let violations = lint("SELECT\n    1".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn tabs_fail_default() {
        let fixed = fix("SELECT\n\t\t1\n", rules());
        assert_eq!(fixed, "SELECT\n    1\n");
    }

    #[test]
    fn indented_comments() {
        let pass_str = "
SELECT
    a,         -- Some comment
    longer_col -- A lined up comment
FROM spam";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn indented_comments_default_config() {
        let fail_str = "
SELECT
	a,			-- Some comment
	longer_col	-- A lined up comment
FROM spam";
        let fix_str = "\nSELECT\n    a,\t\t\t-- Some comment\n    longer_col\t-- A lined up \
                       comment\nFROM spam";

        assert_eq!(fix(fail_str, rules()), fix_str);
    }
}
