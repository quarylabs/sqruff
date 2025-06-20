use std::cell::RefCell;

use ahash::AHashMap;
use smol_str::StrExt;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::helpers::IndexMap;
use sqruff_lib_core::utils::analysis::query::Query;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Default, Clone)]
pub struct RuleST03;

impl Rule for RuleST03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST03.erased())
    }

    fn name(&self) -> &'static str {
        "structure.unused_cte"
    }

    fn description(&self) -> &'static str {
        "Query defines a CTE (common-table expression) but does not use it."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Defining a CTE that is not used by the query is harmless, but it means the code is unnecessary and could be removed.

```sql
WITH cte1 AS (
  SELECT a
  FROM t
),
cte2 AS (
  SELECT b
  FROM u
)

SELECT *
FROM cte1
```

**Best practice**

Remove unused CTEs.

```sql
WITH cte1 AS (
  SELECT a
  FROM t
)

SELECT *
FROM cte1
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut result = Vec::new();
        let query: Query<'_, ()> = Query::from_root(&context.segment, context.dialect).unwrap();

        let mut remaining_ctes: IndexMap<_, _> = RefCell::borrow(&query.inner)
            .ctes
            .keys()
            .map(|it| (it.to_uppercase_smolstr(), it.clone()))
            .collect();
        

        // First, collect all CTE definition segments to check if references are within them
        let cte_segments = if context.dialect.name == DialectKind::Tsql {
            context.segment.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::CommonTableExpression]) },
                false,
                const { &SyntaxSet::single(SyntaxKind::WithCompoundStatement) },
                true,
            )
        } else {
            Vec::new()
        };

        for reference in context.segment.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::TableReference, SyntaxKind::ObjectReference]) },
            true,
            const { &SyntaxSet::single(SyntaxKind::WithCompoundStatement) },
            true,
        ) {
            // TSQL Fix: Skip references that are within CTE definitions to avoid counting
            // table aliases within CTE definitions as references to the CTE itself.
            // In T-SQL, it's common to have table aliases that match CTE names (case-insensitively).
            if context.dialect.name == DialectKind::Tsql {
                // Check if this reference is within any CTE definition
                let is_within_cte = cte_segments.iter().any(|cte| {
                    // Check if the reference segment is a descendant of the CTE segment
                    !cte.path_to(&reference).is_empty()
                });
                
                if is_within_cte {
                    // Skip this reference as it's within a CTE definition
                    continue;
                }
            }
            
            remaining_ctes.shift_remove(&reference.raw().to_uppercase_smolstr());
        }

        for name in remaining_ctes.values() {
            let tmp = RefCell::borrow(&query.inner);
            let cte = RefCell::borrow(&tmp.ctes[name].inner);
            result.push(LintResult::new(
                cte.cte_name_segment.clone(),
                Vec::new(),
                Some(format!(
                    "Query defines CTE \"{}\" but does not use it.",
                    cte.cte_name_segment.as_ref().unwrap().raw()
                )),
                None,
            ));
        }

        result
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::WithCompoundStatement]) })
            .into()
    }
}
