use hashbrown::HashMap;
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;
use sqruff_lib_core::parser::segments::from::FromExpressionElementSegment;
use sqruff_lib_core::parser::segments::object_reference::{
    ObjectReferenceKind, ObjectReferenceSegment,
};
use sqruff_lib_core::utils::analysis::query::Query;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Default, Clone)]
pub struct RuleST11;

impl Rule for RuleST11 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST11.erased())
    }

    fn name(&self) -> &'static str {
        "structure.unused_join"
    }

    fn description(&self) -> &'static str {
        "Joined table not referenced in query."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the table ``bar`` is included in the ``JOIN`` clause
but no columns from it are referenced elsewhere in the query.

```sql
SELECT
    foo.a,
    foo.b
FROM foo
LEFT JOIN bar ON foo.a = bar.a
```

**Best practice**

Remove the join, or use the table.

```sql
SELECT
    foo.a,
    foo.b,
    bar.c
FROM foo
LEFT JOIN bar ON foo.a = bar.a
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let segment = &context.segment;

        // Extract joined tables and which are outer joins.
        let joined_tables = match self.extract_references_from_select(segment) {
            Some(tables) => tables,
            None => return Vec::new(),
        };

        if joined_tables.is_empty() {
            return Vec::new();
        }

        // Now scan all the other clauses for table references.
        let mut table_references = hashbrown::HashSet::new();
        let reference_clause_types: &[SyntaxKind] = &[
            SyntaxKind::SelectClause,
            SyntaxKind::WhereClause,
            SyntaxKind::GroupbyClause,
            SyntaxKind::OrderbyClause,
            SyntaxKind::HavingClause,
            SyntaxKind::QualifyClause,
        ];

        for clause_type in reference_clause_types {
            let clause = segment.child(&SyntaxSet::new(&[*clause_type]));
            if let Some(clause) = clause {
                // Extract all column references from this clause.
                for col_ref in clause.recursive_crawl(
                    const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) },
                    true,
                    &SyntaxSet::EMPTY,
                    true,
                ) {
                    let obj_ref = ObjectReferenceSegment(col_ref, ObjectReferenceKind::Object);
                    let parts = obj_ref.iter_raw_references();
                    if parts.len() < 2 {
                        // Unqualified reference found - abort for this SELECT
                        // because we can't resolve which table it belongs to.
                        return Vec::new();
                    }
                    // The table qualifier is the second-to-last part.
                    let table_part = &parts[parts.len() - 2].part;
                    table_references.insert(
                        table_part
                            .to_uppercase()
                            .trim_matches(|c| {
                                c == '"' || c == '\'' || c == '`' || c == '[' || c == ']'
                            })
                            .to_string(),
                    );
                }
            }
        }

        // Also check for wildcards (e.g., table.* or *)
        let query = Query::from_segment(segment, context.dialect, None);
        let inner = query.inner.borrow();
        for selectable in &inner.selectables {
            for wcinfo in selectable.wildcard_info() {
                for table in &wcinfo.tables {
                    table_references.insert(
                        table
                            .to_uppercase()
                            .trim_matches(|c: char| {
                                c == '"' || c == '\'' || c == '`' || c == '[' || c == ']'
                            })
                            .to_string(),
                    );
                }
            }
        }

        // Now check which joined tables are not referenced.
        let mut results = Vec::new();
        for (tbl_ref, segment) in &joined_tables {
            if !table_references.contains(tbl_ref.as_str()) {
                results.push(LintResult::new(
                    Some(segment.clone()),
                    Vec::new(),
                    Some(format!(
                        "Joined table '{}' not referenced elsewhere in query.",
                        segment.raw()
                    )),
                    None,
                ));
            }
        }

        results
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectStatement]) }).into()
    }
}

impl RuleST11 {
    /// Extract the alias/name from a from_expression_element.
    fn extract_reference_from_expression(&self, segment: &ErasedSegment) -> String {
        let alias = FromExpressionElementSegment(segment.clone()).eventual_alias();
        let ref_str = alias.ref_str.as_str();
        if ref_str.is_empty() {
            return String::new();
        }
        ref_str
            .to_uppercase()
            .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '[' || c == ']')
            .to_string()
    }

    /// Extract tables from column references within a segment, yielding
    /// the uppercase table qualifier. If allow_unqualified is false and an
    /// unqualified reference is found, returns None.
    fn extract_referenced_tables(
        &self,
        segment: &ErasedSegment,
        allow_unqualified: bool,
    ) -> Option<Vec<String>> {
        let mut tables = Vec::new();
        for col_ref in segment.recursive_crawl(
            const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) },
            true,
            &SyntaxSet::EMPTY,
            true,
        ) {
            let obj_ref = ObjectReferenceSegment(col_ref, ObjectReferenceKind::Object);
            let parts = obj_ref.iter_raw_references();
            if parts.len() < 2 {
                if allow_unqualified {
                    continue;
                } else {
                    return None;
                }
            }
            let table_part = &parts[parts.len() - 2].part;
            tables.push(
                table_part
                    .to_uppercase()
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '[' || c == ']')
                    .to_string(),
            );
        }
        Some(tables)
    }

    /// Extract the list of (uppercase_table_ref, segment) for tables brought in
    /// via FROM/JOIN that are candidates for being unused. Only explicit OUTER
    /// joins (LEFT, RIGHT, FULL) are flagged. Returns None if there are fewer
    /// than 2 tables overall (single table queries are not checked).
    fn extract_references_from_select(
        &self,
        segment: &ErasedSegment,
    ) -> Option<Vec<(String, ErasedSegment)>> {
        let from_clause = segment.child(const { &SyntaxSet::new(&[SyntaxKind::FromClause]) })?;

        let mut joined_tables: Vec<(String, ErasedSegment)> = Vec::new();
        let mut referenced_tables: Vec<String> = Vec::new();
        let mut total_table_count = 0;

        for from_expression in
            from_clause.children(const { &SyntaxSet::new(&[SyntaxKind::FromExpression]) })
        {
            // Handle the main FROM expression elements (implicit cross joins).
            let from_elements: Vec<_> = from_expression
                .children(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
                .cloned()
                .collect();

            if from_elements.len() > 1 {
                // Implicit cross join - don't add FROM tables as candidates.
                total_table_count += from_elements.len();
            } else {
                for elem in &from_elements {
                    let ref_str = self.extract_reference_from_expression(elem);
                    if !ref_str.is_empty() {
                        // FROM tables are not candidates for being unused (they
                        // aren't joins), but we still track them for the count.
                        total_table_count += 1;
                    }
                }
            }

            // Handle JOIN clauses.
            for join_clause in
                from_expression.children(const { &SyntaxSet::new(&[SyntaxKind::JoinClause]) })
            {
                // Check if this is an outer join (LEFT, RIGHT, FULL).
                let join_keywords: hashbrown::HashSet<String> = join_clause
                    .children(const { &SyntaxSet::new(&[SyntaxKind::Keyword]) })
                    .map(|kw| kw.raw().to_uppercase_smolstr().to_string())
                    .collect();

                let is_outer = join_keywords.contains("LEFT")
                    || join_keywords.contains("RIGHT")
                    || join_keywords.contains("FULL");

                let mut this_clause_refs = Vec::new();

                for from_elem in join_clause
                    .children(const { &SyntaxSet::new(&[SyntaxKind::FromExpressionElement]) })
                {
                    let ref_str = self.extract_reference_from_expression(from_elem);
                    total_table_count += 1;

                    if !ref_str.is_empty() && is_outer {
                        joined_tables.push((ref_str.clone(), from_elem.clone()));
                        this_clause_refs.push(ref_str);
                    }

                    // Check for table references within the from_expression_element
                    // (e.g., UNNEST(ft.generic_array) references ft).
                    if let Some(refs) = self.extract_referenced_tables(from_elem, true) {
                        for tbl_ref in refs {
                            if !this_clause_refs.contains(&tbl_ref) {
                                referenced_tables.push(tbl_ref);
                            }
                        }
                    }
                }

                // Check ON condition for references to other tables.
                for join_on_condition in
                    join_clause.children(const { &SyntaxSet::new(&[SyntaxKind::JoinOnCondition]) })
                {
                    if let Some(refs) = self.extract_referenced_tables(join_on_condition, true) {
                        for tbl_ref in refs {
                            if !this_clause_refs.contains(&tbl_ref) {
                                referenced_tables.push(tbl_ref);
                            }
                        }
                    }
                }
            }
        }

        // If there's only one table total, don't flag anything.
        if total_table_count <= 1 {
            return None;
        }

        // Remove tables that are referenced in other join clauses.
        let result: Vec<_> = joined_tables
            .into_iter()
            .filter(|(ref_str, _)| !referenced_tables.contains(ref_str))
            .collect();

        Some(result)
    }
}
