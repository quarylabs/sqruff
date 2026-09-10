use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleAM08;

impl Rule for RuleAM08 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM08.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.join_condition"
    }

    fn description(&self) -> &'static str {
        "Implicit cross join detected."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Cross joins are valid, but rare in the wild - and more often created by mistake than on purpose. This rule catches situations where a cross join has been specified, but not explicitly and so the risk of a mistaken cross join is highly likely.

```sql
SELECT
    foo
FROM bar
JOIN baz;
```

**Best practice**

Use `CROSS JOIN`.

```sql
SELECT
    foo
FROM bar
CROSS JOIN baz;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        assert!(context.segment.is_type(SyntaxKind::JoinClause));

        let children = context.segment.segments();
        let keywords: Vec<_> = children
            .iter()
            .filter(|s| s.is_type(SyntaxKind::Keyword))
            .collect();

        // Explicit cross-like joins and JOIN ... USING do not need ON clauses.
        if keywords.iter().any(|k| {
            ["CROSS", "NATURAL", "POSITIONAL", "USING"]
                .iter()
                .any(|keyword| k.raw().eq_ignore_ascii_case(keyword))
        }) {
            return Vec::new();
        }

        if children
            .iter()
            .any(|segment| segment.is_type(SyntaxKind::JoinOnCondition))
        {
            return Vec::new();
        }

        // A join followed by WHERE is handled by CV12. Do not emit AM08 in
        // UPDATE or DELETE statements, where joins without ON are valid.
        let select_statement = context
            .parent_stack
            .iter()
            .rev()
            .take_while(|segment| {
                !segment.is_type(SyntaxKind::UpdateStatement)
                    && !segment.is_type(SyntaxKind::DeleteStatement)
            })
            .find(|segment| segment.is_type(SyntaxKind::SelectStatement));

        let Some(select_statement) = select_statement else {
            return Vec::new();
        };

        if select_statement
            .child(const { &SyntaxSet::single(SyntaxKind::WhereClause) })
            .is_some()
        {
            return Vec::new();
        }

        // T-SQL CROSS APPLY / OUTER APPLY do not contain exactly one JOIN
        // keyword and should not be treated as implicit cross joins.
        if keywords
            .iter()
            .filter(|keyword| keyword.raw().eq_ignore_ascii_case("JOIN"))
            .count()
            != 1
        {
            return Vec::new();
        }

        vec![LintResult::new(
            Some(context.segment.clone()),
            Vec::new(),
            None,
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::JoinClause]) }).into()
    }
}
