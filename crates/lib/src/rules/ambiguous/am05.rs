use hashbrown::HashMap;
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::config::{RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug)]
pub struct RuleAM05 {
    fully_qualify_join_types: JoinType,
}

crate::rule_config_enum! {
    /// Which kinds of `JOIN` clause have to be fully qualified.
    #[derive(Default)]
    pub enum JoinType {
        /// Only `INNER JOIN` has to be spelled out.
        #[default]
        Inner => "inner",
        /// Only `[LEFT/RIGHT/FULL] OUTER JOIN` has to be spelled out.
        Outer => "outer",
        /// Both inner and outer joins have to be spelled out.
        Both => "both",
    }
}

crate::rule_config! {
    /// Configuration for `ambiguous.join` (AM05).
    RuleAM05Config {
        /// Which types of `JOIN` clause should be fully qualified.
        fully_qualify_join_types: JoinType = JoinType::Inner,
    }
}

impl Default for RuleAM05 {
    fn default() -> Self {
        Self {
            fully_qualify_join_types: JoinType::Inner,
        }
    }
}

impl Rule for RuleAM05 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleAM05Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleAM05Config::from_config(config)?;

        Ok(RuleAM05 {
            fully_qualify_join_types: config.fully_qualify_join_types,
        }
        .erased())
    }

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
        assert!(context.segment.is_type(SyntaxKind::JoinClause));

        let join_clause_keywords = context
            .segment
            .segments()
            .iter()
            .filter(|segment| segment.is_type(SyntaxKind::Keyword))
            .collect::<Vec<_>>();

        // Identify LEFT/RIGHT/OUTER JOIN and if the next keyword is JOIN.
        if (self.fully_qualify_join_types == JoinType::Outer
            || self.fully_qualify_join_types == JoinType::Both)
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
        if (self.fully_qualify_join_types == JoinType::Inner
            || self.fully_qualify_join_types == JoinType::Both)
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
