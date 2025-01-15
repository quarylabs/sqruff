use ahash::{AHashMap, AHashSet};
use smol_str::{SmolStr, StrExt};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Clone, Debug, Default)]
pub struct RuleAM03;

impl Rule for RuleAM03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM03.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.order_by"
    }

    fn description(&self) -> &'static str {
        "Ambiguous ordering directions for columns in order by clause."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the `ORDER BY` clause is ambiguous because some columns are explicitly ordered, while others are not.

```sql
SELECT
    a, b
FROM foo
ORDER BY a, b DESC
```

**Best practice**

If any columns in the `ORDER BY` clause specify `ASC` or `DESC`, they should all do so.

```sql
SELECT
    a, b
FROM foo
ORDER BY a ASC, b DESC
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // Only trigger on orderby_clause
        let order_by_spec = Self::get_order_by_info(context.segment.clone());
        let order_types = order_by_spec
            .iter()
            .map(|spec| spec.order.clone())
            .collect::<AHashSet<Option<_>>>();

        // If all or no columns are explicitly ordered, then it's not ambiguous
        if !order_types.contains(&None) || (order_types.len() == 1 && order_types.contains(&None)) {
            return vec![];
        }

        // If there is a mix of explicit and implicit ordering, then it's ambiguous
        let fixes = order_by_spec
            .into_iter()
            .filter(|spec| spec.order.is_none())
            .map(|spec| {
                LintFix::create_after(
                    spec.column_reference,
                    vec![
                        SegmentBuilder::whitespace(context.tables.next_id(), " "),
                        SegmentBuilder::keyword(context.tables.next_id(), "ASC"),
                    ],
                    None,
                )
            })
            .collect();

        vec![LintResult::new(
            Some(context.segment.clone()),
            fixes,
            None,
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::OrderbyClause]) }).into()
    }
}

/// For AM03, segment that ends an ORDER BY column and any order provided.
struct OrderByColumnInfo {
    column_reference: ErasedSegment,
    order: Option<SmolStr>,
}

impl RuleAM03 {
    fn get_order_by_info(segment: ErasedSegment) -> Vec<OrderByColumnInfo> {
        assert!(segment.is_type(SyntaxKind::OrderbyClause));

        let mut result = vec![];
        let mut column_reference = None;
        let mut ordering_reference = None;

        for child_segment in segment.segments() {
            if child_segment.is_type(SyntaxKind::ColumnReference) {
                column_reference = Some(child_segment.clone());
            } else if child_segment.is_type(SyntaxKind::Keyword)
                && (child_segment.raw().eq_ignore_ascii_case("ASC")
                    || child_segment.raw().eq_ignore_ascii_case("DESC"))
            {
                ordering_reference = Some(child_segment.raw().to_uppercase_smolstr());
            };

            if column_reference.is_some() && child_segment.raw() == "," {
                result.push(OrderByColumnInfo {
                    column_reference: column_reference.clone().unwrap(),
                    order: ordering_reference.clone(),
                });

                column_reference = None;
                ordering_reference = None;
            }
        }
        // Special handling for last column
        if column_reference.is_some() {
            result.push(OrderByColumnInfo {
                column_reference: column_reference.clone().unwrap(),
                order: ordering_reference.clone(),
            });
        }

        result
    }
}
