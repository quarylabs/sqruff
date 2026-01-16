use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::JoinType;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleAM05;

#[derive(Clone, Copy)]
struct CachedJoinType(JoinType);

impl Rule for RuleAM05 {
    fn name(&self) -> &'static str {
        "ambiguous.join"
    }

    fn description(&self) -> &'static str {
        "Join clauses should be fully qualified."
    }

    fn long_description(&self) -> &'static str {
        r#"
By default this rule is configured to enforce fully qualified `INNER JOIN` clauses, but not `[LEFT/RIGHT/FULL] OUTER JOIN`. If you prefer a stricter lint then this is configurable.

* `fully_qualify_join_types`: Which types of JOIN clauses should be fully qualified? Must be one of `['inner', 'outer', 'both']`.

**Anti-pattern**

A join is used without specifying the kind of join.

```sql
SELECT
    foo
FROM bar
JOIN baz;
```

**Best practice**

Use `INNER JOIN` rather than `JOIN`.

```sql
SELECT
    foo
FROM bar
INNER JOIN baz;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let join_type = context
            .try_get::<CachedJoinType>()
            .map(|cached| cached.0)
            .unwrap_or_else(|| {
                let parsed = context.config.rules.ambiguous_join.fully_qualify_join_types;
                context.set(CachedJoinType(parsed));
                parsed
            });

        assert!(context.segment.is_type(SyntaxKind::JoinClause));

        let join_clause_keywords = context
            .segment
            .segments()
            .iter()
            .filter(|segment| segment.is_type(SyntaxKind::Keyword))
            .collect::<Vec<_>>();

        // Identify LEFT/RIGHT/OUTER JOIN and if the next keyword is JOIN.
        if (join_type == JoinType::Outer || join_type == JoinType::Both)
            && ["RIGHT", "LEFT", "FULL"].contains(
                &join_clause_keywords[0]
                    .raw()
                    .to_uppercase_smolstr()
                    .as_str(),
            )
            && join_clause_keywords[1].raw().eq_ignore_ascii_case("JOIN")
        {
            let outer_keyword = if join_clause_keywords[1].raw() == "JOIN" {
                "OUTER"
            } else {
                "outer"
            };
            return vec![LintResult::new(
                context.segment.segments()[0].clone().into(),
                vec![LintFix::create_after(
                    context.segment.segments()[0].clone(),
                    vec![
                        SegmentBuilder::whitespace(context.tables.next_id(), " "),
                        SegmentBuilder::keyword(context.tables.next_id(), outer_keyword),
                    ],
                    None,
                )],
                None,
                None,
            )];
        };

        // Fully qualifying inner joins
        if (join_type == JoinType::Inner || join_type == JoinType::Both)
            && join_clause_keywords[0].raw().eq_ignore_ascii_case("JOIN")
        {
            let inner_keyword = if join_clause_keywords[0].raw() == "JOIN" {
                "INNER"
            } else {
                "inner"
            };
            return vec![LintResult::new(
                context.segment.segments()[0].clone().into(),
                vec![LintFix::create_before(
                    context.segment.segments()[0].clone(),
                    vec![
                        SegmentBuilder::keyword(context.tables.next_id(), inner_keyword),
                        SegmentBuilder::whitespace(context.tables.next_id(), " "),
                    ],
                )],
                None,
                None,
            )];
        }
        vec![]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::JoinClause]) }).into()
    }
}
