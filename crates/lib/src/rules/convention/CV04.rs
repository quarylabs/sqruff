use ahash::AHashMap;
use itertools::Itertools;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::parser::segments::common::LiteralSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleCV04 {
    pub prefer_count_1: bool,
    pub prefer_count_0: bool,
}

impl Rule for RuleCV04 {
    fn name(&self) -> &'static str {
        "convention.count_rows"
    }

    fn load_from_config(&self, config: &AHashMap<String, Value>) -> ErasedRule {
        RuleCV04 {
            prefer_count_1: config
                .get("prefer_count_1")
                .unwrap_or(&Value::Bool(false))
                .as_bool()
                .unwrap(),
            prefer_count_0: config
                .get("prefer_count_0")
                .unwrap_or(&Value::Bool(false))
                .as_bool()
                .unwrap(),
        }
        .erased()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(["function"].into()).into()
    }

    fn description(&self) -> &'static str {
        "Use consistent syntax to express \"count number of rows\"."
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let Some(function_name) = context.segment.child(&["function_name"]) else {
            return Vec::new();
        };

        if function_name.get_raw_upper().unwrap() == "COUNT" {
            let f_content = FunctionalContext::new(context.clone())
                .segment()
                .children(Some(|it: &ErasedSegment| it.is_type("bracketed")))
                .children(Some(|it: &ErasedSegment| {
                    !it.is_meta()
                        && !matches!(
                            it.get_type(),
                            "start_bracket" | "end_bracket" | "whitespace" | "newline"
                        )
                }));

            if f_content.len() != 1 {
                return Vec::new();
            }

            let preferred = if self.prefer_count_1 {
                "1"
            } else if self.prefer_count_0 {
                "0"
            } else {
                "*"
            };

            if f_content[0].is_type("star") && (self.prefer_count_0 || self.prefer_count_1) {
                let new_segment = LiteralSegment::create(preferred, &<_>::default());
                return vec![LintResult::new(
                    context.segment.into(),
                    vec![LintFix::replace(f_content[0].clone(), vec![new_segment], None)],
                    None,
                    None,
                    None,
                )];
            }

            if f_content[0].is_type("expression") {
                let expression_content =
                    f_content[0].segments().iter().filter(|it| !it.is_meta()).collect_vec();

                let raw = expression_content[0].get_raw().unwrap();
                if expression_content.len() == 1
                    && matches!(expression_content[0].get_type(), "numeric_literal" | "literal")
                    && (raw == "0" || raw == "1")
                    && raw != preferred
                {
                    let first_expression = expression_content[0].clone();
                    let first_expression_raw = first_expression.get_raw().unwrap();

                    return vec![LintResult::new(
                        context.segment.into(),
                        vec![LintFix::replace(
                            first_expression.clone(),
                            vec![
                                first_expression.edit(
                                    first_expression
                                        .get_raw()
                                        .unwrap()
                                        .replace(&first_expression_raw, preferred)
                                        .into(),
                                    None,
                                ),
                            ],
                            None,
                        )],
                        None,
                        None,
                        None,
                    )];
                }
            }
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::api::simple::{fix, lint};

    fn rules() -> Vec<ErasedRule> {
        rules_with_config(false, false)
    }

    fn rules_with_config(prefer_count_1: bool, prefer_count_0: bool) -> Vec<ErasedRule> {
        vec![RuleCV04 { prefer_count_1, prefer_count_0 }.erased()]
    }

    #[test]
    fn passes_on_count_star() {
        let pass_str = "select
            foo,
            count(*)
        from my_table
        group by
          foo";

        let violations = lint(pass_str.into(), "ansi".into(), rules(), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn passes_on_count_1() {
        let pass_str = "select
            foo,
            count(1)
        from my_table
        group by
          foo";

        let violations =
            lint(pass_str.into(), "ansi".into(), rules_with_config(true, false), None, None)
                .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_count_0_to_count_star() {
        let fail_str = r#"
            select
                foo,
                count(0)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(*)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules());
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn passes_on_count_0() {
        let pass_str = r#"
            select
                foo,
                count(0)
            from my_table
            group by
                foo
        "#;

        let violations =
            lint(pass_str.into(), "ansi".into(), rules_with_config(false, true), None, None)
                .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn passes_on_count_1_if_both_present() {
        let pass_str = r#"
            select
                foo,
                count(1)
            from my_table
            group by
                foo
        "#;

        let violations =
            lint(pass_str.into(), "ansi".into(), rules_with_config(true, true), None, None)
                .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn changes_to_count_1_if_both_present() {
        let fail_str = r#"
            select
                foo,
                count(*)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(1)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules_with_config(true, true));
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn changes_count_1_to_count_star() {
        let fail_str = r#"
            select
                foo,
                count(1)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(*)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules());
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn handles_whitespaces() {
        let fail_str = r#"
            select
                foo,
                count( 1 )
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count( * )
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules());
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn changes_count_star_to_count_0() {
        let fail_str = r#"
            select
                foo,
                count(*)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(0)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules_with_config(false, true));
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn changes_count_star_to_count_1() {
        let fail_str = r#"
            select
                foo,
                count(*)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(1)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules_with_config(true, false));
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn changes_count_1_to_count_0_with_config() {
        let fail_str = r#"
            select
                foo,
                count(1)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(0)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules_with_config(false, true));
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn changes_count_0_to_count_1_with_config() {
        let fail_str = r#"
            select
                foo,
                count(0)
            from my_table
            group by
                foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(1)
            from my_table
            group by
                foo
        "#;

        let actual = fix(fail_str.into(), rules_with_config(true, false));
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn changes_count_star_to_count_1_handle_new_line() {
        let fail_str = r#"
            select
                foo,
                count(

                  *

                )
            from my_table
            group by
              foo
        "#;
        let fix_str = r#"
            select
                foo,
                count(

                  1

                )
            from my_table
            group by
              foo
        "#;

        let actual = fix(fail_str.into(), rules_with_config(true, false));
        assert_eq!(fix_str, actual);
    }

    #[test]
    fn no_false_positive_on_count_col() {
        let pass_str = r#"
            select
                foo,
                count(bar)
            from my_table
        "#;

        let violations =
            lint(pass_str.into(), "ansi".into(), rules_with_config(true, false), None, None)
                .unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn no_false_positive_on_expression() {
        let pass_str = r#"
            select
                foo,
                count(1 + 10)
            from my_table
        "#;

        let violations =
            lint(pass_str.into(), "ansi".into(), rules_with_config(true, false), None, None)
                .unwrap();
        assert_eq!(violations, []);
    }
}
