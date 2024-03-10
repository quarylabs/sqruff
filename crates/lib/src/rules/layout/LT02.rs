use crate::core::rules::base::{LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Default, Debug, Clone)]
pub struct RuleLT02 {}

impl Rule for RuleLT02 {
    fn name(&self) -> &'static str {
        "layout.indent"
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler::default().into()
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_root(context.segment, context.config.clone().unwrap())
            .reindent()
            .results()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::{fix, lint};
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::layout::LT02::RuleLT02;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleLT02::default().erased()]
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
        let fixed = fix("  select 1 from tbl;".into(), rules());
        assert_eq!(fixed, "select 1 from tbl;");
    }

    #[test]
    #[ignore]
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
    #[ignore]
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

    // LT02-tab-space.yml

    #[test]
    fn spaces_pass_default() {
        let violations = lint("SELECT\n    1".into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn tabs_fail_default() {
        let fixed = fix("SELECT\n\t\t1\n".into(), rules());
        assert_eq!(fixed, "SELECT\n    1\n");
    }
}
