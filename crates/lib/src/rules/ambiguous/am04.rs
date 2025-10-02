use ahash::{AHashMap, AHashSet};
use smol_str::{SmolStr, StrExt};
use sqruff_lib_core::dialects::common::AliasInfo;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::helpers::IndexMap;
use sqruff_lib_core::utils::analysis::query::{Query, Selectable, Source};

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

// Types used to locate the inner query within a CTE definition, including VALUES.
const INNER_TYPES: [SyntaxKind; 4] = [
    SyntaxKind::WithCompoundStatement,
    SyntaxKind::SetExpression,
    SyntaxKind::SelectStatement,
    SyntaxKind::ValuesClause,
];

impl Rule for RuleAM04 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
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
        let query = Query::from_segment(&rule_cx.segment, rule_cx.dialect);
        let mut visited = AHashSet::new();
        let env = query.ctes.clone();
        let result = self.analyze_result_columns(query, &mut visited, &env);
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
    fn analyze_result_columns(
        &self,
        query: Query,
        visited: &mut AHashSet<SmolStr>,
        env: &IndexMap<SmolStr, ErasedSegment>,
    ) -> Result<(), ErasedSegment> {
        if query.selectables.is_empty() {
            return Ok(());
        }

        let selectables = query.selectables.clone();
        for selectable in selectables {
            for wildcard in selectable.wildcard_info() {
                if !wildcard.tables.is_empty() {
                    for wildcard_table in wildcard.tables {
                        if let Some(alias_info) = selectable.find_alias(&wildcard_table) {
                            self.handle_alias(&selectable, alias_info, &query, visited, env)?;
                        } else {
                            let key = wildcard_table.to_uppercase_smolstr();
                            if let Some(seg) = env.get(&key) {
                                if visited.contains(&key) {
                                    return Err(selectable.selectable);
                                }
                                // Build inner query from CTE definition
                                let inner = seg
                                    .recursive_crawl(
                                        const { &SyntaxSet::new(&INNER_TYPES) },
                                        true,
                                        &SyntaxSet::EMPTY,
                                        true,
                                    )
                                    .first()
                                    .cloned();
                                if let Some(inner) = inner {
                                    let child = Query::from_segment(&inner, query.dialect);
                                    // Merge env with child's own ctes (child overrides)
                                    let mut merged = env.clone();
                                    merged.extend(child.ctes.clone());
                                    visited.insert(key.clone());
                                    let res = self.analyze_result_columns(child, visited, &merged);
                                    visited.remove(&key);
                                    res?;
                                } else {
                                    return Err(selectable.selectable);
                                }
                            } else {
                                return Err(selectable.selectable);
                            }
                        }
                    }
                } else {
                    let selectable = query.selectables[0].selectable.clone();
                    for source in query.crawl_sources(selectable.clone(), false, true) {
                        match source {
                            Source::Query(q) => {
                                let mut merged = env.clone();
                                merged.extend(q.ctes.clone());
                                self.analyze_result_columns(q, visited, &merged)?;
                                return Ok(());
                            }
                            Source::TableReference(name) => {
                                let key = name.to_uppercase_smolstr();
                                if let Some(seg) = env.get(&key) {
                                    if visited.contains(&key) {
                                        return Err(selectable.clone());
                                    }
                                    let inner = seg
                                        .recursive_crawl(
                                            const { &SyntaxSet::new(&INNER_TYPES) },
                                            true,
                                            &SyntaxSet::EMPTY,
                                            true,
                                        )
                                        .first()
                                        .cloned();
                                    if let Some(inner) = inner {
                                        let child = Query::from_segment(&inner, query.dialect);
                                        let mut merged = env.clone();
                                        merged.extend(child.ctes.clone());
                                        visited.insert(key.clone());
                                        self.analyze_result_columns(child, visited, &merged)?;
                                        visited.remove(&key);
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }

                    return Err(selectable);
                }
            }
        }

        Ok(())
    }

    fn handle_alias(
        &self,
        selectable: &Selectable,
        alias_info: AliasInfo,
        query: &Query<'_>,
        visited: &mut AHashSet<SmolStr>,
        env: &IndexMap<SmolStr, ErasedSegment>,
    ) -> Result<(), ErasedSegment> {
        let select_info_target = query
            .crawl_sources(alias_info.from_expression_element, false, true)
            .into_iter()
            .next()
            .unwrap();
        match select_info_target {
            Source::TableReference(name) => {
                let key = name.to_uppercase_smolstr();
                if let Some(seg) = env.get(&key) {
                    if visited.contains(&key) {
                        return Err(selectable.selectable.clone());
                    }
                    let inner = seg
                        .recursive_crawl(
                            const { &SyntaxSet::new(&INNER_TYPES) },
                            true,
                            &SyntaxSet::EMPTY,
                            true,
                        )
                        .first()
                        .cloned();
                    if let Some(inner) = inner {
                        let child = Query::from_segment(&inner, query.dialect);
                        let mut merged = env.clone();
                        merged.extend(child.ctes.clone());
                        visited.insert(key.clone());
                        let res = self.analyze_result_columns(child, visited, &merged);
                        visited.remove(&key);
                        res
                    } else {
                        Err(selectable.selectable.clone())
                    }
                } else {
                    Err(selectable.selectable.clone())
                }
            }
            Source::Query(q) => {
                let mut merged = env.clone();
                merged.extend(q.ctes.clone());
                self.analyze_result_columns(q, visited, &merged)
            }
        }
    }
}
