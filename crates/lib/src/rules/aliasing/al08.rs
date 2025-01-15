use std::collections::hash_map::Entry;

use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default, Clone)]
pub struct RuleAL08;

impl Rule for RuleAL08 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL08.erased())
    }

    fn name(&self) -> &'static str {
        "layout.cte_newline"
    }

    fn description(&self) -> &'static str {
        "Column aliases should be unique within each clause."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, alias o is used for the orders table, and c is used for customers table.

```sql
SELECT
    COUNT(o.customer_id) as order_amount,
    c.name
FROM orders as o
JOIN customers as c on o.id = c.user_id
```

**Best practice**

Avoid aliases.

```sql
SELECT
    COUNT(orders.customer_id) as order_amount,
    customers.name
FROM orders
JOIN customers on orders.id = customers.user_id

-- Self-join will not raise issue

SELECT
    table1.a,
    table_alias.b,
FROM
    table1
    LEFT JOIN table1 AS table_alias ON
        table1.foreign_key = table_alias.foreign_key
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut used_aliases = AHashMap::new();
        let mut violations = Vec::new();

        for clause_element in context
            .segment
            .children(const { &SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) })
        {
            let mut column_alias = None;

            if let Some(alias_expression) =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) })
            {
                for it in alias_expression.segments() {
                    if !it.is_code() || it.raw().eq_ignore_ascii_case("AS") {
                        continue;
                    }

                    column_alias = it.clone().into();
                    break;
                }
            } else if let Some(column_reference) =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) })
            {
                column_alias = column_reference.segments().last().cloned();
            }

            let Some(column_alias) = column_alias else {
                continue;
            };

            let key = column_alias
                .raw()
                .to_uppercase()
                .replace(['\"', '\'', '`'], "");

            match used_aliases.entry(key) {
                Entry::Occupied(entry) => {
                    let previous: &ErasedSegment = entry.get();

                    let alias = column_alias.raw();
                    let line_no = previous.get_position_marker().unwrap().source_position().0;

                    violations.push(LintResult::new(
                        column_alias.clone().into(),
                        vec![],
                        format!("Reuse of column alias '{alias}' from line {line_no}.").into(),
                        None,
                    ))
                }
                Entry::Vacant(entry) => _ = entry.insert(clause_element.clone()),
            };
        }

        violations
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}
