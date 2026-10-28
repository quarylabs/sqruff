use hashbrown::HashMap;
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::utils::analysis::query::{Query, Source};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleAM04;

const START_TYPES: [SyntaxKind; 3] = [
    SyntaxKind::SelectStatement,
    SyntaxKind::SetExpression,
    SyntaxKind::WithCompoundStatement,
];

impl Rule for RuleAM04 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM04.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.column_count"
    }

    fn description(&self) -> &'static str {
        "Outermost query should produce known number of columns."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Querying all columns using `*` produces a query result where the number or ordering of columns changes if the upstream table's schema changes. This should generally be avoided because it can cause slow performance, cause important schema changes to go undetected, or break production code. For example:

* If a query does `SELECT t.*` and is expected to return columns `a`, `b`, and `c`, the actual columns returned will be wrong/different if columns are added to or deleted from the input table.
* `UNION` and `DIFFERENCE` clauses require the inputs have the same number of columns (and compatible types).
* `JOIN` queries may break due to new column name conflicts, e.g. the query references a column `c` which initially existed in only one input table but a column of the same name is added to another table.
* `CREATE TABLE (<<column schema>>) AS SELECT *`

```sql
WITH cte AS (
    SELECT * FROM foo
)

SELECT * FROM cte
UNION
SELECT a, b FROM t
```

**Best practice**

Somewhere along the "path" to the source data, specify columns explicitly.

```sql
WITH cte AS (
    SELECT * FROM foo
)

SELECT a, b FROM cte
UNION
SELECT a, b FROM t
```"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, rule_cx: &RuleContext) -> Vec<LintResult> {
        let query = Query::from_segment(&rule_cx.segment, rule_cx.dialect, None);

        let result = self.analyze_result_columns(query, None);
        match result {
            Ok(_) => {
                vec![]
            }
            Err(anchor) => {
                vec![LintResult::new(Some(anchor), vec![], None, None)]
            }
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&START_TYPES) })
            .disallow_recurse()
            .into()
    }
}

impl RuleAM04 {
    /// returns an anchor to the rule
    ///
    /// `anchor` is the wildcard which started the analysis, and is what any
    /// resulting violation is reported against. It is threaded through the
    /// recursion so that a wildcard resolving to an ambiguous CTE is still
    /// reported against the wildcard the user can act on, rather than against
    /// whichever nested query the walk happened to terminate in.
    fn analyze_result_columns(
        &self,
        query: Query<'_>,
        anchor: Option<ErasedSegment>,
    ) -> Result<(), ErasedSegment> {
        if query.inner.borrow().selectables.is_empty() {
            return Ok(());
        }

        let selectables = query.inner.borrow().selectables.clone();
        for selectable in selectables {
            for wildcard in selectable.wildcard_info() {
                // Anchor on the star itself. It is the reason for the
                // violation, so it is also the right thing to report against -
                // and, for templated files, the right thing to judge the
                // violation's actionability by.
                let wildcard_anchor = anchor
                    .clone()
                    .unwrap_or_else(|| wildcard_anchor(&wildcard.segment));
                if !wildcard.tables.is_empty() {
                    for wildcard_table in wildcard.tables {
                        if let Some(alias_info) = selectable.find_alias(&wildcard_table) {
                            self.handle_alias(alias_info, &query, wildcard_anchor.clone())?;
                        } else {
                            let Some(cte) = query.lookup_cte(&wildcard_table, true) else {
                                return Err(wildcard_anchor);
                            };

                            self.analyze_result_columns(cte, Some(wildcard_anchor.clone()))?;
                        }
                    }
                } else {
                    let branch_selectable = selectable.selectable.clone();
                    for source in query.crawl_sources(branch_selectable.clone(), false, true) {
                        if let Source::Query(query) = source {
                            self.analyze_result_columns(query, Some(wildcard_anchor.clone()))?;
                            return Ok(());
                        }
                    }

                    return Err(wildcard_anchor);
                }
            }
        }

        Ok(())
    }

    fn handle_alias(
        &self,
        alias_info: AliasInfo,
        query: &Query<'_>,
        anchor: ErasedSegment,
    ) -> Result<(), ErasedSegment> {
        let select_info_target = query
            .crawl_sources(alias_info.from_expression_element, false, true)
            .into_iter()
            .next()
            .unwrap();
        match select_info_target {
            Source::TableReference(_) => Err(anchor),
            Source::Query(query) => self.analyze_result_columns(query, Some(anchor)),
        }
    }
}

/// The `*` within a wildcard select target, or the target if it has none.
///
/// Only the star is guaranteed to be part of the SQL the user wrote. In
/// `{{ alias }}.*` the qualifier is templated while the star is not, so
/// anchoring on the whole target would hand the templated-section filter a
/// span which is not literal and the violation would be discarded.
///
/// Dialects which structure wildcards without a `star` child fall back to the
/// target itself.
fn wildcard_anchor(select_target: &ErasedSegment) -> ErasedSegment {
    select_target
        .recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::Star) },
            true,
            &SyntaxSet::EMPTY,
            true,
        )
        .into_iter()
        .next()
        .unwrap_or_else(|| select_target.clone())
}
