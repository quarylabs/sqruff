use ahash::{AHashMap, AHashSet};

use crate::core::config::Value;
use crate::core::parser::segments::base::{
    ErasedSegment, WhitespaceSegment, WhitespaceSegmentNewArgs,
};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{CloneRule, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};
use crate::helpers::ToErasedSegment;

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

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        // Only trigger on orderby_clause
        let order_by_spec = Self::get_order_by_info(context.segment.clone());
        let order_types = order_by_spec
            .iter()
            .map(|spec| spec.order.clone())
            .collect::<AHashSet<Option<String>>>();

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
                        WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
                        KeywordSegment::new("ASC".into(), None).to_erased_segment(),
                    ],
                    None,
                )
            })
            .collect();

        vec![LintResult::new(Some(context.segment.clone()), fixes, None, None, None)]
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
    order: Option<String>,
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
                && (child_segment.get_raw_upper() == Some("ASC".into())
                    || child_segment.get_raw_upper() == Some("DESC".into()))
            {
                ordering_reference = child_segment.get_raw_upper();
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

#[cfg(test)]
mod tests {
    use crate::api::simple::fix;
    use crate::core::rules::base::{Erased, ErasedRule};
    use crate::rules::ambiguous::AM03::RuleAM03;

    fn rules() -> Vec<ErasedRule> {
        vec![RuleAM03.erased()]
    }

    #[test]
    fn test_fail_bare_union() {
        let fail_str = "SELECT * FROM t ORDER BY a, b DESC";
        let fix_str = "SELECT * FROM t ORDER BY a ASC, b DESC";

        let actual = fix(fail_str, rules());
        assert_eq!(fix_str, actual);
    }
}
