use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::parser::segments::base::ErasedSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone, Default)]
pub struct RuleAL06 {
    min_alias_length: Option<usize>,
    max_alias_length: Option<usize>,
}

impl RuleAL06 {
    fn lint_aliases(&self, from_expression_elements: Vec<ErasedSegment>) -> Vec<LintResult> {
        let mut violation_buff = Vec::new();

        for from_expression_element in from_expression_elements {
            let table_ref = from_expression_element
                .child(const { SyntaxSet::new(&[SyntaxKind::TableExpression]) })
                .and_then(|table_expression| {
                    table_expression.child(
                        const {
                            SyntaxSet::new(&[
                                SyntaxKind::ObjectReference,
                                SyntaxKind::TableReference,
                            ])
                        },
                    )
                });

            let Some(_table_ref) = table_ref else {
                return Vec::new();
            };

            let Some(alias_exp_ref) = from_expression_element
                .child(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) })
            else {
                return Vec::new();
            };

            if let Some(min_alias_length) = self.min_alias_length {
                if let Some(alias_identifier_ref) =
                    alias_exp_ref.child(const { SyntaxSet::new(&[SyntaxKind::Identifier, SyntaxKind::NakedIdentifier]) })
                {
                    let alias_identifier = alias_identifier_ref.raw();
                    if alias_identifier.len() < min_alias_length {
                        violation_buff.push(LintResult::new(
                            Some(alias_identifier_ref),
                            Vec::new(),
                            None,
                            format!(
                                "Aliases should be at least '{:?}' character(s) long",
                                self.min_alias_length
                            )
                            .into(),
                            None,
                        ))
                    }
                }
            }

            if let Some(max_alias_length) = self.max_alias_length {
                if let Some(alias_identifier_ref) =
                    alias_exp_ref.child(const { SyntaxSet::new(&[SyntaxKind::Identifier, SyntaxKind::NakedIdentifier]) })
                {
                    let alias_identifier = alias_identifier_ref.raw();

                    if alias_identifier.len() > max_alias_length {
                        violation_buff.push(LintResult::new(
                            Some(alias_identifier_ref),
                            Vec::new(),
                            None,
                            format!(
                                "Aliases should be no more than '{:?}' character(s) long.",
                                self.max_alias_length
                            )
                            .into(),
                            None,
                        ))
                    }
                }
            }
        }

        violation_buff
    }
}

impl Rule for RuleAL06 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL06::default().erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.length"
    }

    fn description(&self) -> &'static str {
        "Identify aliases in from clause and join conditions"
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, alias `o` is used for the orders table.

```sql
SELECT
    SUM(o.amount) as order_amount,
FROM orders as o
```

**Best practice**

Avoid aliases. Avoid short aliases when aliases are necessary.

See also: Rule_AL07.

```sql
SELECT
    SUM(orders.amount) as order_amount,
FROM orders

SELECT
    replacement_orders.amount,
    previous_orders.amount
FROM
    orders AS replacement_orders
JOIN
    orders AS previous_orders
    ON replacement_orders.id = previous_orders.replacement_id
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        let children = FunctionalContext::new(context.clone()).segment().children(None);
        let from_expression_elements = children
            .recursive_crawl(const { SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) }, true);
        self.lint_aliases(from_expression_elements.base)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::api::simple::lint;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::aliasing::AL06::RuleAL06;

    fn rules(min_alias_length: Option<usize>, max_alias_length: Option<usize>) -> Vec<ErasedRule> {
        vec![RuleAL06 { min_alias_length, max_alias_length }.erased()]
    }

    #[test]
    fn test_pass_no_config() {
        let sql = r#"
        SELECT x.a, x_2.b
            FROM x 
            LEFT JOIN x AS x_2 ON x.foreign_key = x.foreign_key"#;

        let violations = lint(sql.into(), "ansi".into(), rules(None, None), None, None).unwrap();
        assert_eq!(violations, []);
    }

    #[test]
    fn test_fail_alias_too_short() {
        let fail_str = r#"
        SELECT u.id, c.first_name, c.last_name,
            COUNT(o.user_id)
                FROM users AS u
                    JOIN customers AS c ON u.id = c.user_id
                    JOIN orders AS o ON u.id = o.user_id"#;

        let violations =
            lint(fail_str.into(), "ansi".into(), rules(4.into(), None), None, None).unwrap();

        assert_eq!(violations.len(), 3);
    }

    #[test]
    fn test_fail_alias_too_long() {
        let fail_str = r#"
    SELECT u.id, customers_customers_customers.first_name, customers_customers_customers.last_name,
        COUNT(o.user_id)
        FROM users AS u
        JOIN customers AS customers_customers_customers ON u.id = customers_customers_customers.user_id
        JOIN orders AS o ON u.id = o.user_id;"#;

        let violations =
            lint(fail_str.into(), "ansi".into(), rules(None, 10.into()), None, None).unwrap();
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_fail_alias_min_and_max() {
        let fail_str = r#"
    SELECT u.id, customers_customers_customers.first_name, customers_customers_customers.last_name,
        COUNT(o.user_id)
        FROM users AS u
        JOIN customers AS customers_customers_customers ON u.id = customers_customers_customers.user_id
        JOIN orders AS o ON u.id = o.user_id;"#;

        let violations =
            lint(fail_str.into(), "ansi".into(), rules(4.into(), 10.into()), None, None).unwrap();
        assert_eq!(violations.len(), 3);
    }

    #[test]
    fn test_pass_with_config() {
        let pass_str = r#"
    SELECT users.id, customers_customers_customers.first_name, customers_customers_customers.last_name,
        COUNT(latest_orders.user_id)
        FROM users
        JOIN customers AS customers_customers_customers ON users.id = customers_customers_customers.user_id
        JOIN orders AS latest_orders ON users.id = latest_orders.user_id;"#;

        let violations =
            lint(pass_str.into(), "ansi".into(), rules(10.into(), 30.into()), None, None).unwrap();
        assert_eq!(violations, []);
    }
}
