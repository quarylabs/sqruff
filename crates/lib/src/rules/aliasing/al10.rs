use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleAL10;

impl RuleAL10 {
    fn contains_derived_table(from_expression_element: &ErasedSegment) -> bool {
        for segment in from_expression_element
            .iter_segments(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }, false)
        {
            if segment.is_type(SyntaxKind::TableExpression)
                && segment
                    .iter_segments(const { &SyntaxSet::new(&[SyntaxKind::Bracketed]) }, false)
                    .iter()
                    .any(|segment| {
                        const {
                            SyntaxSet::new(&[
                                SyntaxKind::SelectStatement,
                                SyntaxKind::SetExpression,
                                SyntaxKind::WithCompoundStatement,
                            ])
                        }
                        .contains(segment.get_type())
                    })
            {
                return true;
            }
        }

        false
    }
}

impl Rule for RuleAL10 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL10.erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.required"
    }

    fn description(&self) -> &'static str {
        "Derived tables must have an alias."
    }

    fn long_description(&self) -> &'static str {
        r#"
A derived table (subquery in a `FROM` clause) without an alias will cause a
syntax error in most SQL dialects including MySQL, PostgreSQL, and T-SQL.

**Anti-pattern**

A subquery in a `FROM` clause without an alias.

```sql
SELECT *
FROM (
    SELECT 1 AS a
)
```

**Best practice**

Add an alias to the derived table.

```sql
SELECT *
FROM (
    SELECT 1 AS a
) AS derived
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        debug_assert!(context.segment.is_type(SyntaxKind::FromExpressionElement));

        if !Self::contains_derived_table(&context.segment)
            || context
                .segment
                .child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) })
                .is_some()
        {
            return Vec::new();
        }

        vec![LintResult::new(
            Some(context.segment.clone()),
            Vec::new(),
            Some("Derived table must have an alias.".into()),
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
            .into()
    }
}
