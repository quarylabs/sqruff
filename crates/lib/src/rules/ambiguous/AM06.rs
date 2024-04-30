use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

struct PriorGroupByOrderByConvention(String);

#[derive(Debug, Clone)]
pub struct RuleAM06 {
    group_by_and_order_by_style: String,
}

impl Default for RuleAM06 {
    fn default() -> Self {
        Self { group_by_and_order_by_style: "consistent".into() }
    }
}

impl Rule for RuleAM06 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> ErasedRule {
        RuleAM06 { group_by_and_order_by_style: "consistent".into() }.erased()
    }

    fn name(&self) -> &'static str {
        "ambiguous.column_references"
    }

    fn description(&self) -> &'static str {
        "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let skip = FunctionalContext::new(context.clone()).parent_stack().any(Some(|it| {
            let ignore_types = ["withingroup_clause", "window_specification", "aggregate_order_by"];
            ignore_types.iter().any(|ty| it.is_type(ty))
        }));

        if skip {
            return Vec::new();
        }

        // Initialize the map
        let mut column_reference_category_map = AHashMap::new();
        column_reference_category_map.insert("column_reference", "explicit");
        column_reference_category_map.insert("expression", "explicit");
        column_reference_category_map.insert("numeric_literal", "implicit");

        let mut column_reference_category_set: Vec<_> = context
            .segment
            .segments()
            .iter()
            .filter_map(|segment| column_reference_category_map.get(segment.get_type()))
            .collect();
        column_reference_category_set.dedup();

        if column_reference_category_set.is_empty() {
            return Vec::new();
        }

        if self.group_by_and_order_by_style == "consistent" {
            if column_reference_category_set.len() > 1 {
                return vec![LintResult::new(context.segment.into(), Vec::new(), None, None, None)];
            } else {
                let current_group_by_order_by_convention =
                    column_reference_category_set.pop().unwrap();

                if let Some(PriorGroupByOrderByConvention(prior_group_by_order_by_convention)) =
                    context.memory.borrow().get::<PriorGroupByOrderByConvention>()
                {
                    if prior_group_by_order_by_convention != current_group_by_order_by_convention {
                        return vec![LintResult::new(
                            context.segment.into(),
                            Vec::new(),
                            None,
                            None,
                            None,
                        )];
                    }
                }

                context.memory.borrow_mut().insert(PriorGroupByOrderByConvention(
                    current_group_by_order_by_convention.to_string(),
                ));
            }
        } else if column_reference_category_set
            .iter()
            .any(|&&category| category != self.group_by_and_order_by_style)
        {
            return vec![LintResult::new(context.segment.into(), Vec::new(), None, None, None)];
        }

        vec![]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            ["groupby_clause", "orderby_clause", "grouping_expression_list"].into(),
        )
        .into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::RuleAM06;
    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};

    fn rules(group_by_and_order_by_style: Option<&str>) -> Vec<ErasedRule> {
        let mut rule = RuleAM06::default();
        if let Some(group_by_and_order_by_style) = group_by_and_order_by_style {
            rule.group_by_and_order_by_style = group_by_and_order_by_style.into();
        }
        vec![rule.erased()]
    }

    #[test]
    fn test_pass_explicit_group_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2".into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_implicit_group_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY foo, bar".into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_explicit_order_by_default() {
        let violations = lint(
            "SELECT foo, bar FROM fake_table ORDER BY 1, 2".into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_order_by_default() {
        let violations = lint(
            "SELECT foo, bar FROM fake_table ORDER BY foo, bar".into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_mix_group_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, bar".into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_implicit_group_by_and_order_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2 ORDER BY 1, 2"
                .into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_pass_explicit_group_by_and_order_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY foo, bar ORDER BY \
             foo, bar"
                .into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_within_line_mix_group_by_and_order_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, bar ORDER BY foo, \
             2"
            .into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);

        assert_eq!(
            violations[1].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[1].line_no, 1);
        assert_eq!(violations[1].line_pos, 72);

        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_fail_across_line_mix_group_by_and_order_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2 ORDER BY foo, \
             bar"
            .into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 70);

        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_explicit_expression_order_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table ORDER BY foo, power(bar, 2)"
                .into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_expression_order_by_default() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table ORDER BY 1, power(bar, 2)"
                .into(),
            "ansi".into(),
            rules(None),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);

        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_explicit_group_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY foo, bar".into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_group_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2".into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);

        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_mix_group_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, bar".into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);

        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_explicit_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar FROM fake_table ORDER BY foo, bar".into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar FROM fake_table ORDER BY 1, 2".into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 33);

        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_implicit_group_by_and_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2 ORDER BY 1, 2"
                .into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);

        assert_eq!(
            violations[1].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[1].line_no, 1);
        assert_eq!(violations[1].line_pos, 70);

        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_fail_within_line_mix_group_by_and_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, bar ORDER BY foo, \
             2"
            .into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);

        assert_eq!(
            violations[1].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[1].line_no, 1);
        assert_eq!(violations[1].line_pos, 72);

        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_fail_across_line_mix_group_by_and_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2 ORDER BY foo, \
             bar"
            .into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_explicit_expression_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table ORDER BY foo, power(bar, 2)"
                .into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_expression_order_by_custom_explicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table ORDER BY 1, power(bar, 2)"
                .into(),
            "ansi".into(),
            rules("explicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_explicit_group_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2".into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_group_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY foo, bar".into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_explicit_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar FROM fake_table ORDER BY 1, 2".into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_implicit_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar FROM fake_table ORDER BY foo, bar".into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 33);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_mix_group_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, bar".into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_implicit_group_by_and_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2 ORDER BY 1, 2"
                .into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_explicit_group_by_and_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY foo, bar ORDER BY \
             foo, bar"
                .into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(
            violations[1].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[1].line_no, 1);
        assert_eq!(violations[1].line_pos, 74);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_fail_within_line_mix_group_by_and_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, bar ORDER BY foo, \
             2"
            .into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(
            violations[1].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[1].line_no, 1);
        assert_eq!(violations[1].line_pos, 72);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_fail_across_line_mix_group_by_and_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table GROUP BY 1, 2 ORDER BY foo, \
             bar"
            .into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 70);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_explicit_expression_order_by_custom_implicit() {
        let violations = lint(
            "SELECT foo, bar, sum(baz) AS sum_value FROM fake_table ORDER BY foo, power(bar, 2)"
                .into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            violations[0].desc(),
            "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
        );
        assert_eq!(violations[0].line_no, 1);
        assert_eq!(violations[0].line_pos, 56);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_pass_window() {
        let violations = lint(
            "SELECT field_1, field_2, SUM(field_3) as field_3_total, SUM(field_3) OVER (ORDER BY \
             field_1) AS field_3_window_sum FROM table1 GROUP BY 1, 2 ORDER BY 1, 2"
                .into(),
            "ansi".into(),
            rules("implicit".into()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(violations, []);
    }
}
