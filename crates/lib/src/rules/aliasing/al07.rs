use std::iter::once;

use ahash::{AHashMap, AHashSet};
use smol_str::ToSmolStr;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, Tables};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use sqruff_lib_core::utils::functional::segments::Segments;

#[derive(Debug)]
struct TableAliasInfo {
    table_ref: ErasedSegment,
    whitespace_ref: Option<ErasedSegment>,
    alias_exp_ref: ErasedSegment,
    alias_identifier_ref: Option<ErasedSegment>,
}

#[derive(Debug, Clone, Default)]
pub struct RuleAL07 {
    force_enable: bool,
}

impl RuleAL07 {
    fn lint_aliases_in_join(
        &self,
        tables: &Tables,
        base_table: Option<ErasedSegment>,
        from_expression_elements: Vec<ErasedSegment>,
        column_reference_segments: Vec<ErasedSegment>,
        segment: ErasedSegment,
    ) -> Vec<LintResult> {
        let mut violation_buff = Vec::new();
        let to_check = self.filter_table_expressions(base_table, from_expression_elements);

        let mut table_counts = AHashMap::new();
        for ai in &to_check {
            *table_counts
                .entry(ai.table_ref.raw().to_smolstr())
                .or_insert(0) += 1;
        }

        let mut table_aliases: AHashMap<_, AHashSet<_>> = AHashMap::new();
        for ai in &to_check {
            if let (table_ref, Some(alias_identifier_ref)) =
                (&ai.table_ref, &ai.alias_identifier_ref)
            {
                table_aliases
                    .entry(table_ref.raw().to_smolstr())
                    .or_default()
                    .insert(alias_identifier_ref.raw().to_smolstr());
            }
        }

        for alias_info in to_check {
            if let (table_ref, Some(alias_identifier_ref)) =
                (&alias_info.table_ref, &alias_info.alias_identifier_ref)
            {
                // Skip processing if table appears more than once with different aliases
                let raw_table = table_ref.raw().to_smolstr();
                if table_counts.get(&raw_table).unwrap_or(&0) > &1
                    && table_aliases
                        .get(&raw_table)
                        .is_some_and(|aliases| aliases.len() > 1)
                {
                    continue;
                }

                let select_clause = segment
                    .child(const { &SyntaxSet::new(&[SyntaxKind::SelectClause]) })
                    .unwrap();
                let mut ids_refs = Vec::new();

                let alias_name = alias_identifier_ref.raw();
                if !alias_name.is_empty() {
                    // Find all references to alias in select clause
                    for alias_with_column in select_clause.recursive_crawl(
                        const { &SyntaxSet::new(&[SyntaxKind::ObjectReference]) },
                        true,
                        &SyntaxSet::EMPTY,
                        true,
                    ) {
                        if let Some(used_alias_ref) = alias_with_column.child(
                            const {
                                &SyntaxSet::new(&[
                                    SyntaxKind::Identifier,
                                    SyntaxKind::NakedIdentifier,
                                ])
                            },
                        ) {
                            if used_alias_ref.raw() == alias_name {
                                ids_refs.push(used_alias_ref);
                            }
                        }
                    }

                    // Find all references to alias in column references
                    for exp_ref in column_reference_segments.clone() {
                        if let Some(used_alias_ref) = exp_ref.child(
                            const {
                                &SyntaxSet::new(&[
                                    SyntaxKind::Identifier,
                                    SyntaxKind::NakedIdentifier,
                                ])
                            },
                        ) {
                            if used_alias_ref.raw() == alias_name
                                && exp_ref
                                    .child(const { &SyntaxSet::new(&[SyntaxKind::Dot]) })
                                    .is_some()
                            {
                                ids_refs.push(used_alias_ref);
                            }
                        }
                    }
                }

                // Prepare fixes for deleting and editing references to aliased tables
                let mut fixes = Vec::new();

                fixes.push(LintFix::delete(alias_info.alias_exp_ref));

                if let Some(whitespace_ref) = &alias_info.whitespace_ref {
                    fixes.push(LintFix::delete(whitespace_ref.clone()));
                }

                for alias in ids_refs.iter().chain(once(alias_identifier_ref)) {
                    let tmp = table_ref.raw();
                    let identifier_parts: Vec<_> = tmp.split('.').collect();
                    let mut edits = Vec::new();
                    for (i, part) in identifier_parts.iter().enumerate() {
                        if i > 0 {
                            edits.push(SegmentBuilder::symbol(tables.next_id(), "."));
                        }
                        edits.push(
                            SegmentBuilder::token(tables.next_id(), part, SyntaxKind::Identifier)
                                .finish(),
                        );
                    }
                    fixes.push(LintFix::replace(
                        alias.clone(),
                        edits,
                        Some(vec![table_ref.clone()]),
                    ));
                }

                violation_buff.push(LintResult::new(
                    alias_info.alias_identifier_ref,
                    fixes,
                    "Avoid aliases in from clauses and join conditions."
                        .to_owned()
                        .into(),
                    None,
                ));
            }
        }

        violation_buff
    }

    fn filter_table_expressions(
        &self,
        base_table: Option<ErasedSegment>,
        from_expression_elements: Vec<ErasedSegment>,
    ) -> Vec<TableAliasInfo> {
        let mut acc = Vec::new();

        for from_expression in from_expression_elements {
            let table_expression =
                from_expression.child(const { &SyntaxSet::new(&[SyntaxKind::TableExpression]) });
            let Some(table_expression) = table_expression else {
                continue;
            };

            let table_ref =
                table_expression.child(const { &SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::TableReference]) });
            let Some(table_ref) = table_ref else {
                continue;
            };

            if let Some(ref base_table) = base_table {
                if base_table.raw() == table_ref.raw() && base_table != &table_ref {
                    continue;
                }
            }

            let whitespace_ref =
                from_expression.child(const { &SyntaxSet::new(&[SyntaxKind::Whitespace]) });

            let alias_exp_ref =
                from_expression.child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) });
            let Some(alias_exp_ref) = alias_exp_ref else {
                continue;
            };

            let alias_identifier_ref = alias_exp_ref.child(
                const { &SyntaxSet::new(&[SyntaxKind::Identifier, SyntaxKind::NakedIdentifier]) },
            );

            acc.push(TableAliasInfo {
                table_ref,
                whitespace_ref,
                alias_exp_ref,
                alias_identifier_ref,
            });
        }

        acc
    }
}

impl Rule for RuleAL07 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL07 {
            force_enable: _config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.forbid"
    }
    fn description(&self) -> &'static str {
        "Avoid table aliases in from clauses and join conditions."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, alias o is used for the orders table, and c is used for customers table.

```sql
SELECT
    COUNT(o.customer_id) as order_amount,
    c.name
FROM orders as o
JOIN customers as c on o.id = c.user_id
```

**Best practice**

Avoid aliases.

```sql
SELECT
    COUNT(orders.customer_id) as order_amount,
    customers.name
FROM orders
JOIN customers on orders.id = customers.user_id

-- Self-join will not raise issue

SELECT
    table1.a,
    table_alias.b,
FROM
    table1
    LEFT JOIN table1 AS table_alias ON
        table1.foreign_key = table_alias.foreign_key
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if !self.force_enable {
            return Vec::new();
        }

        // For CREATE TABLE ... AS (SELECT ...), find the embedded SELECT statement
        let segment_to_analyze = if context.segment.is_type(SyntaxKind::CreateTableStatement) {
            // Find SELECT statement within CREATE TABLE
            if let Some(select_segment) = context
                .segment
                .recursive_crawl(
                    const { &SyntaxSet::new(&[SyntaxKind::SelectStatement]) },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                )
                .into_iter()
                .next()
            {
                select_segment
            } else {
                return Vec::new();
            }
        } else {
            context.segment.clone()
        };

        // Get FROM clause from the segment we want to analyze
        // Use recursive_crawl instead of children for finding FROM clause
        let from_clause_opt = segment_to_analyze
            .recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::FromClause]) },
                true,
                &SyntaxSet::EMPTY,
                true,
            )
            .into_iter()
            .next();

        if from_clause_opt.is_none() {
            return Vec::new();
        }

        let from_clause = from_clause_opt.unwrap();
        let from_clause_segment = Segments::from_vec(vec![from_clause.clone()], None);

        let base_table = from_clause_segment
            .children(Some(|it| it.is_type(SyntaxKind::FromExpression)))
            .find_first::<fn(&_) -> _>(None)
            .children(Some(|it| it.is_type(SyntaxKind::FromExpressionElement)))
            .find_first::<fn(&_) -> _>(None)
            .children(Some(|it| it.is_type(SyntaxKind::TableExpression)))
            .find_first::<fn(&_) -> _>(None)
            .children(Some(|it| {
                it.is_type(SyntaxKind::ObjectReference) || it.is_type(SyntaxKind::TableReference)
            }));

        if base_table.is_empty() {
            return Vec::new();
        }

        let mut from_expression_elements = Vec::new();
        let mut column_reference_segments = Vec::new();

        // Get FROM clause and all segments after it from the segment we're analyzing
        // First, get all FROM expression elements from the FROM clause
        for from_expression_element in from_clause.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) },
            true,
            &SyntaxSet::EMPTY,
            true,
        ) {
            from_expression_elements.push(from_expression_element);
        }

        // Also get FROM expression elements from JOIN clauses and other parts after FROM
        for from_expression_element in segment_to_analyze.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) },
            true,
            &SyntaxSet::EMPTY,
            true,
        ) {
            // Skip if already added from FROM clause
            if !from_expression_elements
                .iter()
                .any(|e| e == &from_expression_element)
            {
                from_expression_elements.push(from_expression_element);
            }
        }

        // Get all column references in the entire statement
        for column_ref in segment_to_analyze.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) },
            true,
            &SyntaxSet::EMPTY,
            true,
        ) {
            column_reference_segments.push(column_ref);
        }

        self.lint_aliases_in_join(
            context.tables,
            base_table.first().cloned(),
            from_expression_elements,
            column_reference_segments,
            segment_to_analyze,
        )
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::SelectStatement,
                    SyntaxKind::CreateTableStatement,
                ])
            },
        )
        .into()
    }
}
