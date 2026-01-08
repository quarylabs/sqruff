use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Clone, Default)]
pub struct RuleAL06;

impl RuleAL06 {
    fn lint_aliases(
        &self,
        from_expression_elements: Vec<ErasedSegment>,
        min_alias_length: Option<usize>,
        max_alias_length: Option<usize>,
    ) -> Vec<LintResult> {
        let mut violation_buff = Vec::new();

        for from_expression_element in from_expression_elements {
            let table_ref = from_expression_element
                .child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) })
                .and_then(|table_expression| {
                    table_expression.child(
                        const {
                            &SyntaxSet::new(&[
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
                .child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) })
            else {
                return Vec::new();
            };

            if let Some(min_alias_length) = min_alias_length
                && let Some(alias_identifier_ref) =
                    alias_exp_ref.child(const { &SyntaxSet::new(&[SyntaxKind::Identifier, SyntaxKind::NakedIdentifier]) })
                {
                    let alias_identifier = alias_identifier_ref.raw();
                    if alias_identifier.len() < min_alias_length {
                        violation_buff.push(LintResult::new(
                            Some(alias_identifier_ref),
                            Vec::new(),
                            format!(
                                "Aliases should be at least '{:?}' character(s) long",
                                min_alias_length
                            )
                            .into(),
                            None,
                        ))
                    }
                }

            if let Some(max_alias_length) = max_alias_length
                && let Some(alias_identifier_ref) =
                    alias_exp_ref.child(const { &SyntaxSet::new(&[SyntaxKind::Identifier, SyntaxKind::NakedIdentifier]) })
                {
                    let alias_identifier = alias_identifier_ref.raw();

                    if alias_identifier.len() > max_alias_length {
                        violation_buff.push(LintResult::new(
                            Some(alias_identifier_ref),
                            Vec::new(),
                            format!(
                                "Aliases should be no more than '{:?}' character(s) long.",
                                max_alias_length
                            )
                            .into(),
                            None,
                        ))
                    }
                }
        }

        violation_buff
    }
}

impl Rule for RuleAL06 {
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

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let rules = &context.config.rules.aliasing_length;
        let children = FunctionalContext::new(context).segment().children_all();
        let from_expression_elements = children.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) },
            true,
        );
        self.lint_aliases(
            from_expression_elements.base,
            rules.min_alias_length,
            rules.max_alias_length,
        )
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}
