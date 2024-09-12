use ahash::AHashMap;
use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::rules::LintFix;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleCV04 {
    pub prefer_count_1: bool,
    pub prefer_count_0: bool,
}

impl Rule for RuleCV04 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV04 {
            prefer_count_1: _config
                .get("prefer_count_1")
                .unwrap_or(&Value::Bool(false))
                .as_bool()
                .unwrap(),
            prefer_count_0: _config
                .get("prefer_count_0")
                .unwrap_or(&Value::Bool(false))
                .as_bool()
                .unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "convention.count_rows"
    }

    fn description(&self) -> &'static str {
        "Use consistent syntax to express \"count number of rows\"."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `count(1)` is used to count the number of rows in a table.

```sql
select
    count(1)
from table_a
```

**Best practice**

Use count(*) unless specified otherwise by config prefer_count_1, or prefer_count_0 as preferred.

```sql
select
    count(*)
from table_a
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Convention]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let Some(function_name) =
            context.segment.child(const { &SyntaxSet::new(&[SyntaxKind::FunctionName]) })
        else {
            return Vec::new();
        };

        if function_name.get_raw_upper().unwrap() == "COUNT" {
            let f_content = FunctionalContext::new(context.clone())
                .segment()
                .children(Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)))
                .children(Some(|it: &ErasedSegment| {
                    !it.is_meta()
                        && !matches!(
                            it.get_type(),
                            SyntaxKind::StartBracket
                                | SyntaxKind::EndBracket
                                | SyntaxKind::Whitespace
                                | SyntaxKind::Newline
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

            if f_content[0].is_type(SyntaxKind::Star)
                && (self.prefer_count_0 || self.prefer_count_1)
            {
                let new_segment =
                    SegmentBuilder::token(context.tables.next_id(), preferred, SyntaxKind::Literal)
                        .finish();
                return vec![LintResult::new(
                    context.segment.into(),
                    vec![LintFix::replace(f_content[0].clone(), vec![new_segment], None)],
                    None,
                    None,
                    None,
                )];
            }

            if f_content[0].is_type(SyntaxKind::Expression) {
                let expression_content =
                    f_content[0].segments().iter().filter(|it| !it.is_meta()).collect_vec();

                let raw = expression_content[0].raw();
                if expression_content.len() == 1
                    && matches!(
                        expression_content[0].get_type(),
                        SyntaxKind::NumericLiteral | SyntaxKind::Literal
                    )
                    && (raw == "0" || raw == "1")
                    && raw != preferred
                {
                    let first_expression = expression_content[0].clone();
                    let first_expression_raw = first_expression.raw();

                    return vec![LintResult::new(
                        context.segment.into(),
                        vec![LintFix::replace(
                            first_expression.clone(),
                            vec![
                                first_expression.edit(
                                    context.tables.next_id(),
                                    first_expression
                                        .raw()
                                        .replace(&*first_expression_raw, preferred)
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

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Function]) }).into()
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

        let actual = fix(fail_str, rules());
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

        let actual = fix(fail_str, rules_with_config(true, true));
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

        let actual = fix(fail_str, rules());
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

        let actual = fix(fail_str, rules());
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

        let actual = fix(fail_str, rules_with_config(false, true));
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

        let actual = fix(fail_str, rules_with_config(true, false));
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

        let actual = fix(fail_str, rules_with_config(false, true));
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

        let actual = fix(fail_str, rules_with_config(true, false));
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

        let actual = fix(fail_str, rules_with_config(true, false));
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
