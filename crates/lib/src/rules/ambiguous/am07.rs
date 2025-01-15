use ahash::{AHashMap, HashSet, HashSetExt};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::utils::analysis::query::{Query, Selectable, Source, WildcardInfo};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Debug, Clone)]
pub struct RuleAM07;

impl Rule for RuleAM07 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM07.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.set_columns"
    }

    fn description(&self) -> &'static str {
        "All queries in set expression should return the same number of columns."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

When writing set expressions, all queries must return the same number of columns.

```sql
WITH cte AS (
    SELECT
        a,
        b
    FROM foo
)
SELECT * FROM cte
UNION
SELECT
    c,
    d,
    e
 FROM t
```

**Best practice**

Always specify columns when writing set queries and ensure that they all seleect same number of columns.

```sql
WITH cte AS (
    SELECT a, b FROM foo
)
SELECT
    a,
    b
FROM cte
UNION
SELECT
    c,
    d
FROM t
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        debug_assert!(context.segment.is_type(SyntaxKind::SetExpression));

        let mut root = &context.segment;

        // Is the parent of the set expression a WITH expression?
        // NOTE: Backward slice to work outward.
        for parent in context.parent_stack.iter().rev() {
            if parent.is_type(SyntaxKind::WithCompoundStatement) {
                root = parent;
                break;
            }
        }

        let query: Query<()> = Query::from_segment(root, context.dialect, None);
        let (set_segment_select_sizes, resolve_wildcard) = self.get_select_target_counts(query);

        // if queries had different select target counts and all wildcards had been
        // resolved; fail
        if set_segment_select_sizes.len() > 1 && resolve_wildcard {
            vec![LintResult::new(
                Some(context.segment.clone()),
                vec![],
                None,
                None,
            )]
        } else {
            vec![]
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SetExpression]) })
            .provide_raw_stack()
            .into()
    }
}

impl RuleAM07 {
    /// Given a set expression, get the number of select targets in each query.
    ///
    /// We keep track of the number of columns in each selectable using a
    /// ``set``. Ideally at the end there is only one item in the set,
    /// showing that all selectables have the same size. Importantly we
    /// can't guarantee that we can always resolve any wildcards (*), so
    /// we also return a flag to indicate whether any present have been
    /// fully resolved.
    fn get_select_target_counts(&self, query: Query<()>) -> (HashSet<usize>, bool) {
        let mut select_target_counts = HashSet::new();
        let mut resolved_wildcard = true;

        let selectables = query.inner.borrow().selectables.clone();
        for selectable in selectables {
            let (cnt, res) = self.resolve_selectable(selectable.clone(), query.clone());
            if !res {
                resolved_wildcard = false;
            }
            select_target_counts.insert(cnt);
        }

        (select_target_counts, resolved_wildcard)
    }

    /// Resolve the number of columns in a single Selectable.
    ///
    /// The selectable may opr may not have (*) wildcard expressions. If it
    /// does, we attempt to resolve them.
    fn resolve_selectable(&self, selectable: Selectable, root_query: Query<()>) -> (usize, bool) {
        debug_assert!(selectable.select_info().is_some());

        let wildcard_info = selectable.wildcard_info();

        // Start with the number of non-wildcard columns.
        let mut num_cols =
            selectable.select_info().unwrap().select_targets.len() - wildcard_info.len();

        // If there are no wildcards, we're done.
        if wildcard_info.is_empty() {
            return (num_cols, true);
        }

        let mut resolved = true;
        // If the set query contains one or more wildcards, attempt to resolve it to a
        // list of select targets that can be counted.
        for wildcard in wildcard_info {
            let (_cols, _resolved) =
                self.resolve_selectable_wildcard(wildcard, selectable.clone(), root_query.clone());
            resolved = resolved && _resolved;
            // Add on the number of columns which the wildcard resolves to.
            num_cols += _cols;
        }

        (num_cols, resolved)
    }

    /// Attempt to resolve a full query which may contain wildcards.
    ///
    /// NOTE: This requires a ``Query`` as input rather than just a
    /// ``Selectable`` and will delegate to ``__resolve_selectable``
    /// once any Selectables have been identified.
    ///
    /// This method is *not* called on the initial set expression as
    /// that is evaluated as a series of Selectables. This method is
    /// only called on any subqueries (which may themselves be SELECT,
    /// WITH or set expressions) found during the resolution of any
    /// wildcards.
    fn resolve_wild_query(&self, query: Query<()>) -> (usize, bool) {
        // if one of the source queries for a query within the set is a
        // set expression, just use the first query. If that first query isn't
        // reflective of the others, that will be caught when that segment
        // is processed. We'll know if we're in a set based on whether there
        // is more than one selectable. i.e. Just take the first selectable.
        let selectable = query.inner.borrow().selectables[0].clone();
        self.resolve_selectable(selectable, query.clone())
    }

    /// Attempt to resolve a single wildcard (*) within a Selectable.
    ///
    /// Note: This means resolving the number of columns implied by
    /// a single *. This method would be run multiple times if there
    /// are multiple wildcards in a single selectable.
    fn resolve_selectable_wildcard(
        &self,
        wildcard: WildcardInfo,
        selectable: Selectable,
        root_query: Query<()>,
    ) -> (usize, bool) {
        let mut resolved = true;

        // If there is no table specified, it is likely a subquery so handle that first.
        if wildcard.tables.is_empty() {
            // Crawl the query looking for the subquery, problem in the FROM.
            for source in root_query.crawl_sources(selectable.selectable, false, true) {
                if let Source::Query(query) = source {
                    return self.resolve_wild_query(query);
                }
            }
            return (0, false);
        }

        // There might be multiple tables references in some wildcard cases.
        let mut num_columns = 0;
        for wildcard_table in wildcard.tables {
            let mut cte_name = wildcard_table.clone();

            // Get the AliasInfo for the table referenced in the wildcard expression.
            let alias_info = selectable.find_alias(&wildcard_table);
            if let Some(alias_info) = alias_info {
                let select_info_target = root_query
                    .crawl_sources(alias_info.from_expression_element, false, true)
                    .into_iter()
                    .next()
                    .unwrap();

                match select_info_target {
                    Source::TableReference(name) => {
                        cte_name = name;
                    }
                    Source::Query(query) => {
                        let (_cols, _resolved) = self.resolve_wild_query(query);
                        num_columns += _cols;
                        resolved = resolved && _resolved;
                        continue;
                    }
                }
            }

            let cte = root_query.lookup_cte(&cte_name, true);
            if let Some(cte) = cte {
                let (cols, _resolved) = self.resolve_wild_query(cte);
                num_columns += cols;
                resolved = resolved && _resolved;
            } else {
                resolved = false;
            }
        }
        (num_columns, resolved)
    }
}
