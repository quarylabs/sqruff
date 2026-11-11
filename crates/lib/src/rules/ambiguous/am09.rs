use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

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

`OFFSET 0` without `LIMIT`, `FETCH` or `TOP` is allowed: it skips no rows, so the result does not depend on row order. Only a literal `0` written in the source is allowed; a templated value is still reported even when it renders as `0`. It is also still reported when the file has a parse error, because the unparsable part may hide a `LIMIT` or `FETCH`.
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

        // `OFFSET 0` skips no rows, so on its own it cannot make the result depend on row
        // order; PostgreSQL users write it to keep a subquery from being pulled up. Keep
        // the finding when a LIMIT, FETCH or TOP is present, or when the file has a parse
        // error: the unparsable part may hide one, and the parser can leave it outside this
        // statement. A templated value is not exempt even when it renders as 0, because
        // another render can give a different value.
        let has_row_limit = children.iter().any(|s| {
            s.is_type(SyntaxKind::LimitClause)
                || s.is_type(SyntaxKind::FetchClause)
                || (s.is_type(SyntaxKind::SelectClause) && has_top(s))
        });
        let file_has_parse_error = context
            .parent_stack
            .first()
            .is_some_and(|root| root.descendant_type_set().contains(SyntaxKind::Unparsable));
        if !has_row_limit && !file_has_parse_error && is_offset_zero(anchor) {
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

fn is_offset_zero(clause: &ErasedSegment) -> bool {
    clause.is_type(SyntaxKind::OffsetClause)
        && clause
            .segments()
            .iter()
            .find(|s| s.is_type(SyntaxKind::NumericLiteral))
            .is_some_and(|literal| !literal.is_templated() && literal.raw().parse::<u64>() == Ok(0))
}

// Teradata nests the TOP modifier one level deeper than other dialects, so search the raw
// segments of each modifier rather than its direct children.
fn has_top(select_clause: &ErasedSegment) -> bool {
    select_clause
        .segments()
        .iter()
        .filter(|s| s.is_type(SyntaxKind::SelectClauseModifier))
        .any(|modifier| {
            modifier
                .get_raw_segments()
                .iter()
                .any(|raw| raw.is_keyword("TOP"))
        })
}
