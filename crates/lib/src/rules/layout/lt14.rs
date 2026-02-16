use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::reflow::sequence::ReflowSequence;

const CLAUSE_TYPES: SyntaxSet = SyntaxSet::new(&[
    SyntaxKind::SelectClause,
    SyntaxKind::FromClause,
    SyntaxKind::WhereClause,
    SyntaxKind::JoinClause,
    SyntaxKind::GroupbyClause,
    SyntaxKind::OrderbyClause,
    SyntaxKind::HavingClause,
    SyntaxKind::LimitClause,
]);

#[derive(Debug, Default, Clone)]
pub struct RuleLT14;

impl Rule for RuleLT14 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT14.erased())
    }

    fn name(&self) -> &'static str {
        "layout.keyword_newline"
    }

    fn description(&self) -> &'static str {
        "Keyword clause newline enforcement."
    }

    fn long_description(&self) -> &'static str {
        r#"
This rule checks the following clause types:

- `SELECT`
- `FROM`
- `WHERE`
- `JOIN`
- `GROUP BY`
- `ORDER BY`
- `HAVING`
- `LIMIT`

**Anti-pattern**

In this example, some clauses share a line while others don't,
creating inconsistent formatting.

```sql
SELECT a
FROM foo WHERE a = 1
```

**Best practice**

Each clause should start on a new line.

```sql
SELECT a
FROM foo
WHERE a = 1
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        ReflowSequence::from_root(&context.segment, context.config)
            .rebreak(context.tables)
            .results()
            .into_iter()
            .filter(|r| {
                r.anchor
                    .as_ref()
                    .is_some_and(|seg| CLAUSE_TYPES.contains(seg.get_type()))
            })
            .collect()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}
