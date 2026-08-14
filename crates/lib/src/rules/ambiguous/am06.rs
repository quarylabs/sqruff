use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::config::{RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

#[derive(Clone, Copy)]
struct PriorGroupByOrderByConvention(GroupByAndOrderByConvention);

#[derive(Debug, Clone)]
pub struct RuleAM06 {
    group_by_and_order_by_style: GroupByAndOrderByConvention,
}

impl Default for RuleAM06 {
    fn default() -> Self {
        Self {
            group_by_and_order_by_style: GroupByAndOrderByConvention::Consistent,
        }
    }
}

crate::rule_config_enum! {
    /// How `GROUP BY`/`ORDER BY` clauses refer to their columns.
    #[derive(Default)]
    pub enum GroupByAndOrderByConvention {
        /// Either style is fine, as long as a file sticks to one of them.
        #[default]
        Consistent => "consistent",
        /// Always reference columns by name.
        Explicit => "explicit",
        /// Always reference columns by their position in the select clause.
        Implicit => "implicit",
    }
}

crate::rule_config! {
    /// Configuration for `ambiguous.column_references` (AM06).
    RuleAM06Config {
        /// How `GROUP BY`/`ORDER BY` clauses should reference their columns.
        group_by_and_order_by_style: GroupByAndOrderByConvention =
            GroupByAndOrderByConvention::Consistent,
    }
}

impl Rule for RuleAM06 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleAM06Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleAM06Config::from_config(config)?;

        Ok(RuleAM06 {
            group_by_and_order_by_style: config.group_by_and_order_by_style,
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.column_references"
    }

    fn description(&self) -> &'static str {
        "Inconsistent column references in 'GROUP BY/ORDER BY' clauses."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the ORRDER BY clause mixes explicit and implicit order by column references.

```sql
SELECT
    a, b
FROM foo
ORDER BY a, b DESC
```

**Best practice**

If any columns in the ORDER BY clause specify ASC or DESC, they should all do so.

```sql
SELECT
    a, b
FROM foo
ORDER BY a ASC, b DESC
```
"#
    }
    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let skip = FunctionalContext::new(context)
            .parent_stack()
            .any_match(|it| {
                let ignore_types = [
                    SyntaxKind::WithingroupClause,
                    SyntaxKind::WindowSpecification,
                    SyntaxKind::AggregateOrderByClause,
                ];
                ignore_types.iter().any(|&ty| it.is_type(ty))
            });

        if skip {
            return Vec::new();
        }

        // Initialize the map
        let mut column_reference_category_map = HashMap::new();
        column_reference_category_map.insert(
            SyntaxKind::ColumnReference,
            GroupByAndOrderByConvention::Explicit,
        );
        column_reference_category_map.insert(
            SyntaxKind::Expression,
            GroupByAndOrderByConvention::Explicit,
        );
        column_reference_category_map.insert(
            SyntaxKind::NumericLiteral,
            GroupByAndOrderByConvention::Implicit,
        );

        let mut column_reference_category_set: Vec<_> = context
            .segment
            .segments()
            .iter()
            .filter_map(|segment| column_reference_category_map.get(&segment.get_type()))
            .collect();
        column_reference_category_set.dedup();

        if column_reference_category_set.is_empty() {
            return Vec::new();
        }

        if self.group_by_and_order_by_style == GroupByAndOrderByConvention::Consistent {
            if column_reference_category_set.len() > 1 {
                return vec![LintResult::new(
                    context.segment.clone().into(),
                    Vec::new(),
                    None,
                    None,
                )];
            } else {
                let current_group_by_order_by_convention =
                    column_reference_category_set.pop().copied().unwrap();

                if let Some(PriorGroupByOrderByConvention(prior_group_by_order_by_convention)) =
                    context.try_get::<PriorGroupByOrderByConvention>()
                    && prior_group_by_order_by_convention != current_group_by_order_by_convention
                {
                    return vec![LintResult::new(
                        context.segment.clone().into(),
                        Vec::new(),
                        None,
                        None,
                    )];
                }

                context.set(PriorGroupByOrderByConvention(
                    current_group_by_order_by_convention,
                ));
            }
        } else if column_reference_category_set
            .into_iter()
            .any(|category| *category != self.group_by_and_order_by_style)
        {
            return vec![LintResult::new(
                context.segment.clone().into(),
                Vec::new(),
                None,
                None,
            )];
        }

        vec![]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::GroupbyClause,
                    SyntaxKind::OrderbyClause,
                    SyntaxKind::GroupingExpressionList,
                ])
            },
        )
        .into()
    }
}
