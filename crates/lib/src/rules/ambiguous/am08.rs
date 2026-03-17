use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleAM08;

impl Rule for RuleAM08 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM08.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.join_condition"
    }

    fn description(&self) -> &'static str {
        "Implicit cross join detected."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Cross joins are valid, but rare in the wild - and more often created by mistake than on purpose. This rule catches situations where a cross join has been specified, but not explicitly and so the risk of a mistaken cross join is highly likely.

```sql
SELECT
    foo
FROM bar
JOIN baz;
```

**Best practice**

Use `CROSS JOIN`.

```sql
SELECT
    foo
FROM bar
CROSS JOIN baz;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        assert!(context.segment.is_type(SyntaxKind::JoinClause));

        let children = context.segment.segments();

        // Check if this join has an ON condition or USING clause.
        // In the ANSI dialect, USING is parsed as a keyword within the join
        // clause rather than a separate UsingClause node.
        let has_condition = children.iter().any(|s| {
            s.is_type(SyntaxKind::JoinOnCondition)
                || s.is_type(SyntaxKind::UsingClause)
                || s.is_keyword("USING")
        });

        if has_condition {
            return Vec::new();
        }

        // Collect the keywords in the join clause.
        let keywords: Vec<_> = children
            .iter()
            .filter(|s| s.is_type(SyntaxKind::Keyword))
            .collect();

        // If it's already an explicit CROSS JOIN or a NATURAL JOIN, that's fine.
        if keywords.iter().any(|k| {
            k.raw().eq_ignore_ascii_case("CROSS") || k.raw().eq_ignore_ascii_case("NATURAL")
        }) {
            return Vec::new();
        }

        // Find the JOIN keyword to anchor the fix.
        let join_keyword = keywords
            .iter()
            .find(|k| k.raw().eq_ignore_ascii_case("JOIN"));

        let Some(join_kw) = join_keyword else {
            return Vec::new();
        };

        // Determine case to match existing style.
        let cross_keyword = if join_kw.raw() == "JOIN" || join_kw.raw() == "Join" {
            "CROSS"
        } else {
            "cross"
        };

        vec![LintResult::new(
            Some((*join_kw).clone()),
            vec![LintFix::create_before(
                (*join_kw).clone(),
                vec![
                    SegmentBuilder::keyword(context.tables.next_id(), cross_keyword),
                    SegmentBuilder::whitespace(context.tables.next_id(), " "),
                ],
            )],
            None,
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::JoinClause]) }).into()
    }
}
