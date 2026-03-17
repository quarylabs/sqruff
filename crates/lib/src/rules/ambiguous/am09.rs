use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleAM09;

impl Rule for RuleAM09 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM09.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.order_by_limit"
    }

    fn description(&self) -> &'static str {
        "LIMIT/OFFSET without ORDER BY."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Using `LIMIT` or `OFFSET` without `ORDER BY` leads to non-deterministic results, as the database may return different rows on successive executions.

```sql
SELECT *
FROM foo
LIMIT 10;
```

**Best practice**

Always use `ORDER BY` when using `LIMIT` or `OFFSET`.

```sql
SELECT *
FROM foo
ORDER BY id
LIMIT 10;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        assert!(context.segment.is_type(SyntaxKind::SelectStatement));

        let children = context.segment.segments();

        // Check if this select has a LIMIT or OFFSET clause.
        let limit_or_offset = children
            .iter()
            .find(|s| s.is_type(SyntaxKind::LimitClause) || s.is_type(SyntaxKind::OffsetClause));

        let Some(anchor) = limit_or_offset else {
            return Vec::new();
        };

        // Check if there's an ORDER BY clause.
        let has_order_by = children
            .iter()
            .any(|s| s.is_type(SyntaxKind::OrderbyClause));

        if has_order_by {
            return Vec::new();
        }

        vec![LintResult::new(
            Some(anchor.clone()),
            Vec::new(),
            None,
            Some("LIMIT/OFFSET without ORDER BY may lead to non-deterministic results.".into()),
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}
