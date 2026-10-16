use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleCV13 {
    force_enable: bool,
}

impl Rule for RuleCV13 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV13 {
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "convention.last_select_star"
    }

    fn description(&self) -> &'static str {
        "The final SELECT of a CTE model should be `SELECT * FROM ...`."
    }

    fn long_description(&self) -> &'static str {
        r#"
This is a convention popularised by the dbt
[style guide](https://docs.getdbt.com/best-practices/how-we-style/0-how-we-style-our-dbt-projects),
where the final statement of a model is a "passthrough" select from the last
CTE. Keeping the final select as `SELECT * FROM final` makes a model easier to
debug, because the source of the final result set can be swapped by changing a
single line (for example replacing `final` with an earlier CTE).

To avoid flagging ad-hoc queries, this rule only applies when the *last*
top-level statement of a file is a `WITH ... SELECT` (i.e. it uses CTEs). Plain
`SELECT` statements, and files ending in DML or DDL, are not in scope. The final
select must also be a pure passthrough: a lone `SELECT * FROM <cte>` with no
further transformation (`WHERE`, `GROUP BY`, `DISTINCT`, `ORDER BY`, `LIMIT`,
etc.).

This convention is opinionated and not universally agreed upon, so this rule is
**disabled by default**. It can be enabled with the `force_enable = True` flag:

```cfg
[sqruff:rules:convention.last_select_star]
force_enable = True
```

**Anti-pattern**

The final select of a CTE model lists explicit columns rather than selecting
everything from the final CTE.

```sql
WITH final AS (
    SELECT
        a,
        b
    FROM foo
)

SELECT
    a,
    b
FROM final
```

**Best practice**

Select everything from the final CTE.

```sql
WITH final AS (
    SELECT
        a,
        b
    FROM foo
)

SELECT * FROM final
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // This convention is opinionated, so the rule is disabled by default
        // and only runs when explicitly enabled via `force_enable = true`.
        if !self.force_enable {
            return Vec::new();
        }

        // We only operate on the file root.
        debug_assert!(context.segment.is_type(SyntaxKind::File));

        // Find the last top-level statement in the file.
        let Some(last_statement) = context
            .segment
            .children(const { &SyntaxSet::single(SyntaxKind::Statement) })
            .last()
        else {
            // Nothing to check (e.g. a file of only comments).
            return Vec::new();
        };

        // A statement wraps a single inner segment (the actual query/DDL/DML).
        let Some(inner) = last_statement.segments().iter().find(|seg| !seg.is_meta()) else {
            return Vec::new();
        };

        // The dbt convention applies to CTE-based models. To avoid false
        // positives on ad-hoc queries, only flag when the final statement is a
        // `WITH ... SELECT`.
        if !inner.is_type(SyntaxKind::WithCompoundStatement) {
            return Vec::new();
        }

        // The passthrough is the trailing query element of the WITH statement.
        // A `WITH ... INSERT/UPDATE/DELETE` has no trailing SELECT or set
        // expression here (some dialects, e.g. Postgres, still parse it as a
        // `with_compound_statement`); such DML is out of scope, like any file
        // ending in DML or DDL.
        let Some(final_select) = trailing_query(inner) else {
            return Vec::new();
        };

        if !final_select.is_type(SyntaxKind::SelectStatement) {
            // The WITH ends in a set expression (e.g. a UNION), which is not a
            // single `SELECT * FROM ...`. Anchor on the whole statement.
            return vec![LintResult::new(Some(inner.clone()), Vec::new(), None, None)];
        }

        if is_select_star_from(&final_select) {
            return Vec::new();
        }

        // Anchor the violation on the select clause so the offending columns
        // are highlighted.
        let anchor = final_select
            .child(const { &SyntaxSet::single(SyntaxKind::SelectClause) })
            .unwrap_or(final_select);
        vec![LintResult::new(Some(anchor), Vec::new(), None, None)]
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

/// Return the trailing query element (a `select_statement` or `set_expression`)
/// of a WITH statement, or `None` when there is none.
///
/// A `WITH ... INSERT/UPDATE/DELETE` has no trailing query element here (its
/// final child is the DML statement), so the rule can treat it as out of scope,
/// like any file ending in DML or DDL.
fn trailing_query(with_statement: &ErasedSegment) -> Option<ErasedSegment> {
    with_statement
        .children(
            const { &SyntaxSet::new(&[SyntaxKind::SelectStatement, SyntaxKind::SetExpression]) },
        )
        .last()
        .cloned()
}

/// Return whether a select is exactly `SELECT * FROM <source>`.
///
/// This must be a *pure* passthrough: a lone wildcard selected from a single
/// source, with no further transformation clauses (e.g. `WHERE`, `GROUP BY`,
/// `DISTINCT`, `ORDER BY`, `LIMIT`).
fn is_select_star_from(select_statement: &ErasedSegment) -> bool {
    // The convention is `select * from`, so a source must be present.
    if select_statement
        .child(const { &SyntaxSet::single(SyntaxKind::FromClause) })
        .is_none()
    {
        return false;
    }

    // Reject any transformation clause beyond `select`/`from`. A passthrough
    // must not filter, aggregate, sort or limit the final result set.
    for clause in select_statement.segments() {
        if clause.is_type(SyntaxKind::Keyword) || clause.is_meta() {
            continue;
        }
        let kind = clause.get_type();
        if kind.as_str().ends_with("_clause")
            && kind != SyntaxKind::SelectClause
            && kind != SyntaxKind::FromClause
        {
            return false;
        }
    }

    let Some(select_clause) =
        select_statement.child(const { &SyntaxSet::single(SyntaxKind::SelectClause) })
    else {
        return false;
    };

    // A select modifier such as `DISTINCT` reshapes the result set, so it is
    // not a passthrough.
    if select_clause
        .child(const { &SyntaxSet::single(SyntaxKind::SelectClauseModifier) })
        .is_some()
    {
        return false;
    }

    let elements: Vec<_> = select_clause
        .children(const { &SyntaxSet::single(SyntaxKind::SelectClauseElement) })
        .collect();
    // There must be exactly one selected element, and it must be a wildcard.
    if elements.len() != 1 {
        return false;
    }

    elements[0]
        .segments()
        .iter()
        .find(|seg| !seg.is_meta())
        .is_some_and(|first| first.is_type(SyntaxKind::WildcardExpression))
}
