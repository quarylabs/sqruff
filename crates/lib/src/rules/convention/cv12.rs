use hashbrown::{HashMap, HashSet};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};

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

        let select_statement = &context.segment;
        let Some(where_clause) =
            select_statement.child(const { &SyntaxSet::single(SyntaxKind::WhereClause) })
        else {
            return Vec::new();
        };

        let where_clause_simplifiable = is_where_clause_simplifiable(&where_clause);
        let subexpressions = if where_clause_simplifiable {
            where_clause
                .child(const { &SyntaxSet::single(SyntaxKind::Expression) })
                .map(|expression| get_subexpression_chunks(&expression))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut consumed_subexpressions = HashSet::new();

        let mut encountered_references: HashSet<String> = select_statement
            .recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) },
                true,
                const { &SyntaxSet::new(&[SyntaxKind::JoinClause, SyntaxKind::SelectStatement]) },
                false,
            )
            .into_iter()
            .map(|table_reference| from_expression_element_alias(&table_reference))
            .collect();

        let join_clauses = select_statement.recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::JoinClause) },
            true,
            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
            false,
        );

        let mut results = Vec::new();

        for join in join_clauses {
            let join_table_references = join.recursive_crawl(
                const { &SyntaxSet::single(SyntaxKind::FromExpressionElement) },
                true,
                const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                false,
            );
            let Some(join_table_reference) = join_table_references.first() else {
                continue;
            };
            encountered_references.insert(from_expression_element_alias(join_table_reference));

            let join_children = join.segments();
            let join_keywords: Vec<_> = join_children
                .iter()
                .filter(|segment| segment.is_type(SyntaxKind::Keyword))
                .collect();

            if join_keywords.iter().any(|keyword| {
                ["CROSS", "NATURAL", "POSITIONAL", "USING"]
                    .iter()
                    .any(|expected| keyword.raw().eq_ignore_ascii_case(expected))
            }) || join_children
                .iter()
                .any(|segment| segment.is_type(SyntaxKind::JoinOnCondition))
            {
                continue;
            }

            if !where_clause_simplifiable {
                results.push(LintResult::new(Some(join), Vec::new(), None, None));
                continue;
            }

            let mut this_join_subexpressions = HashSet::new();
            for (subexpression_idx, subexpression) in subexpressions.iter().enumerate() {
                if consumed_subexpressions.contains(&subexpression_idx) {
                    continue;
                }

                let qualified_column_references: Vec<_> = subexpression
                    .iter()
                    .flat_map(|segment| {
                        segment.recursive_crawl(
                            const { &SyntaxSet::single(SyntaxKind::ColumnReference) },
                            true,
                            const { &SyntaxSet::single(SyntaxKind::SelectStatement) },
                            true,
                        )
                    })
                    .filter(|column_reference| {
                        column_reference
                            .descendant_type_set()
                            .contains(SyntaxKind::Dot)
                    })
                    .collect();

                if qualified_column_references.len() > 1
                    && qualified_column_references.iter().all(|column_reference| {
                        let raw_upper = column_reference.raw().to_uppercase();
                        encountered_references.iter().any(|table_reference| {
                            raw_upper.starts_with(&format!("{table_reference}."))
                        })
                    })
                {
                    this_join_subexpressions.insert(subexpression_idx);
                    consumed_subexpressions.insert(subexpression_idx);
                }
            }

            if this_join_subexpressions.is_empty() {
                results.push(LintResult::new(Some(join), Vec::new(), None, None));
                continue;
            }

            let mut join_condition_segments = Vec::new();
            append_selected_subexpressions(
                &mut join_condition_segments,
                &subexpressions,
                &this_join_subexpressions,
                context,
            );
            trim_whitespace_and_operators(&mut join_condition_segments);

            let mut join_edit_segments = join.segments().to_vec();
            join_edit_segments.extend([
                SegmentBuilder::whitespace(context.tables.next_id(), " "),
                SegmentBuilder::keyword(context.tables.next_id(), "ON"),
                SegmentBuilder::whitespace(context.tables.next_id(), " "),
            ]);
            join_edit_segments.extend(join_condition_segments);
            let replacement_join = SegmentBuilder::node(
                context.tables.next_id(),
                SyntaxKind::JoinClause,
                context.dialect.name,
                join_edit_segments,
            )
            .finish();

            results.push(LintResult::new(
                Some(join.clone()),
                vec![LintFix::replace(join, vec![replacement_join], None)],
                None,
                None,
            ));
        }

        if !where_clause_simplifiable || consumed_subexpressions.is_empty() {
            return results;
        }

        let remaining_subexpressions: HashSet<_> = (0..subexpressions.len())
            .filter(|idx| !consumed_subexpressions.contains(idx))
            .collect();
        let mut where_edit_segments = Vec::new();
        append_selected_subexpressions(
            &mut where_edit_segments,
            &subexpressions,
            &remaining_subexpressions,
            context,
        );
        trim_whitespace_and_operators(&mut where_edit_segments);

        if !where_edit_segments.is_empty() {
            if let Some(where_expression) =
                where_clause.child(const { &SyntaxSet::single(SyntaxKind::Expression) })
            {
                let replacement_expression = SegmentBuilder::node(
                    context.tables.next_id(),
                    SyntaxKind::Expression,
                    context.dialect.name,
                    where_edit_segments,
                )
                .finish();
                results.push(LintResult::new(
                    Some(where_expression.clone()),
                    vec![LintFix::replace(
                        where_expression,
                        vec![replacement_expression],
                        None,
                    )],
                    None,
                    None,
                ));
            }
        } else if let Some(where_idx) = select_statement
            .segments()
            .iter()
            .position(|segment| segment.is(&where_clause))
        {
            let mut fixes = Vec::new();
            if let Some(preceding_segment) = where_idx
                .checked_sub(1)
                .and_then(|idx| select_statement.segments().get(idx))
                .filter(|segment| {
                    matches!(
                        segment.get_type(),
                        SyntaxKind::Whitespace | SyntaxKind::Newline
                    )
                })
            {
                fixes.push(LintFix::delete(preceding_segment.clone()));
            }
            fixes.push(LintFix::delete(where_clause.clone()));
            results.push(LintResult::new(Some(where_clause), fixes, None, None));
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

fn from_expression_element_alias(from_expression_element: &ErasedSegment) -> String {
    if let Some(alias_expression) =
        from_expression_element.child(const { &SyntaxSet::single(SyntaxKind::AliasExpression) })
        && let Some(identifier) = alias_expression.child(
            const {
                &SyntaxSet::new(&[
                    SyntaxKind::Identifier,
                    SyntaxKind::NakedIdentifier,
                    SyntaxKind::QuotedIdentifier,
                    SyntaxKind::Literal,
                ])
            },
        )
    {
        return identifier.raw().to_uppercase();
    }

    from_expression_element.raw().to_uppercase()
}

fn is_where_clause_simplifiable(where_clause: &ErasedSegment) -> bool {
    let Some(expression) = where_clause.child(const { &SyntaxSet::single(SyntaxKind::Expression) })
    else {
        return false;
    };

    expression
        .recursive_crawl(
            const { &SyntaxSet::single(SyntaxKind::BinaryOperator) },
            true,
            &SyntaxSet::EMPTY,
            true,
        )
        .iter()
        .all(|operator| operator.raw().eq_ignore_ascii_case("AND"))
}

fn get_subexpression_chunks(expression: &ErasedSegment) -> Vec<Vec<ErasedSegment>> {
    let expression_segments = expression.segments();
    let mut chunks = Vec::new();
    let mut start_idx = 0;

    for (idx, segment) in expression_segments.iter().enumerate() {
        if segment.is_type(SyntaxKind::BinaryOperator) {
            chunks.push(expression_segments[start_idx..idx].to_vec());
            start_idx = idx + 1;
        }
    }
    chunks.push(expression_segments[start_idx..].to_vec());
    chunks
}

fn append_selected_subexpressions(
    target: &mut Vec<ErasedSegment>,
    subexpressions: &[Vec<ErasedSegment>],
    selected: &HashSet<usize>,
    context: &RuleContext,
) {
    for (idx, subexpression) in subexpressions.iter().enumerate() {
        if selected.contains(&idx) {
            target.extend(subexpression.iter().cloned());
            target.push(SegmentBuilder::keyword(context.tables.next_id(), "AND"));
        }
    }
}

fn trim_whitespace_and_operators(segments: &mut Vec<ErasedSegment>) {
    while segments
        .first()
        .is_some_and(is_whitespace_or_binary_operator)
    {
        segments.remove(0);
    }
    while segments
        .last()
        .is_some_and(is_whitespace_or_binary_operator)
    {
        segments.pop();
    }
}

fn is_whitespace_or_binary_operator(segment: &ErasedSegment) -> bool {
    matches!(
        segment.get_type(),
        SyntaxKind::Whitespace | SyntaxKind::BinaryOperator
    ) || segment.is_keyword("AND")
}
