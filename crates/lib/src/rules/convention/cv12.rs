use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleCV12;

impl Rule for RuleCV12 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV12.erased())
    }

    fn name(&self) -> &'static str {
        "convention.join_condition"
    }

    fn description(&self) -> &'static str {
        "Join conditions should use the JOIN ... ON syntax."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Placing join conditions in the `WHERE` clause instead of using `JOIN ... ON` mixes join logic with filtering logic, making queries harder to read.

```sql
SELECT
    foo
FROM bar
JOIN baz
WHERE bar.id = baz.id;
```

**Best practice**

Use `JOIN ... ON` to specify join conditions.

```sql
SELECT
    foo
FROM bar
JOIN baz ON bar.id = baz.id;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        assert!(context.segment.is_type(SyntaxKind::SelectStatement));

        let children = context.segment.segments();

        // Find the FROM clause.
        let from_clause = children.iter().find(|s| s.is_type(SyntaxKind::FromClause));

        let Some(from_clause) = from_clause else {
            return Vec::new();
        };

        // Only flag when there's a WHERE clause — that's the signal that
        // join conditions may have been placed there instead of ON.
        let has_where = children.iter().any(|s| s.is_type(SyntaxKind::WhereClause));

        if !has_where {
            return Vec::new();
        }

        // Find all JoinClause descendants within the FROM clause.
        let join_clauses = from_clause.recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::JoinClause) },
            true,
            &SyntaxSet::EMPTY,
            true,
        );

        let mut results = Vec::new();

        for join in join_clauses {
            let join_children = join.segments();

            // Check if this join has an ON condition or USING clause.
            let has_condition = join_children.iter().any(|s| {
                s.is_type(SyntaxKind::JoinOnCondition)
                    || s.is_type(SyntaxKind::UsingClause)
                    || s.is_keyword("USING")
            });

            if has_condition {
                continue;
            }

            // Skip CROSS JOIN and NATURAL JOIN — they don't need ON.
            let is_cross_or_natural = join_children.iter().any(|s| {
                s.is_type(SyntaxKind::Keyword)
                    && (s.raw().eq_ignore_ascii_case("CROSS")
                        || s.raw().eq_ignore_ascii_case("NATURAL"))
            });

            if is_cross_or_natural {
                continue;
            }

            results.push(LintResult::new(
                Some(join.clone()),
                Vec::new(),
                None,
                Some(
                    "Join conditions should use the JOIN ... ON syntax rather than the WHERE clause."
                        .into(),
                ),
            ));
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        false
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}
