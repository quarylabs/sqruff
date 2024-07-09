use std::str::FromStr;

use ahash::AHashMap;
use strum_macros::{AsRefStr, EnumString};

use crate::core::config::Value;
use crate::core::parser::segments::base::{WhitespaceSegment, WhitespaceSegmentNewArgs};
use crate::core::parser::segments::keyword::KeywordSegment;
use crate::core::rules::base::{Erased, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::helpers::ToErasedSegment;

#[derive(Clone, Debug)]
pub struct RuleAM05 {
    fully_qualify_join_types: JoinType,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString)]
#[strum(serialize_all = "lowercase")]
enum JoinType {
    Inner,
    Outer,
    Both,
}

impl Default for RuleAM05 {
    fn default() -> Self {
        Self { fully_qualify_join_types: JoinType::Inner }
    }
}

impl Rule for RuleAM05 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        let fully_qualify_join_types = config["fully_qualify_join_types"].as_string();
        // TODO We will need a more complete story for all the config parsing
        match fully_qualify_join_types {
            None => Err("Rule AM05 expects a `fully_qualify_join_types` array".to_string()),
            Some(join_type) => {
                let join_type = JoinType::from_str(join_type).map_err(|_| {
                    format!(
                        "Rule AM05 expects a `fully_qualify_join_types` array of valid join \
                         types. Got: {}",
                        join_type
                    )
                })?;
                Ok(RuleAM05 { fully_qualify_join_types: join_type }.erased())
            }
        }
    }

    fn name(&self) -> &'static str {
        "ambiguous.join"
    }

    fn description(&self) -> &'static str {
        "Join clauses should be fully qualified."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `UNION DISTINCT` should be preferred over `UNION`, because explicit is better than implicit.


```sql
SELECT a, b FROM table_1
UNION
SELECT a, b FROM table_2
```

**Best practice**

Specify `DISTINCT` or `ALL` after `UNION` (note that `DISTINCT` is the default behavior).

```sql
SELECT a, b FROM table_1
UNION DISTINCT
SELECT a, b FROM table_2
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Ambiguous]
    }

    fn eval(&self, context: RuleContext) -> Vec<LintResult> {
        assert!(context.segment.is_type("join_clause"));

        let join_clause_keywords = context
            .segment
            .segments()
            .iter()
            .filter(|segment| segment.is_type("keyword"))
            .collect::<Vec<_>>();

        // Identify LEFT/RIGHT/OUTER JOIN and if the next keyword is JOIN.
        if (self.fully_qualify_join_types == JoinType::Outer
            || self.fully_qualify_join_types == JoinType::Both)
            && ["RIGHT", "LEFT", "FULL"]
                .contains(&&*join_clause_keywords[0].get_raw_upper().unwrap())
            && join_clause_keywords[1].get_raw_upper().unwrap() == "JOIN"
        {
            let outer_keyword =
                if join_clause_keywords[1].raw() == "JOIN" { "OUTER" } else { "outer" };
            return vec![LintResult::new(
                context.segment.segments()[0].clone().into(),
                vec![LintFix::create_after(
                    context.segment.segments()[0].clone(),
                    vec![
                        WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
                        KeywordSegment::new(outer_keyword.into(), None).to_erased_segment(),
                    ],
                    None,
                )],
                None,
                None,
                None,
            )];
        };

        // Fully qualifying inner joins
        if (self.fully_qualify_join_types == JoinType::Inner
            || self.fully_qualify_join_types == JoinType::Both)
            && join_clause_keywords[0].get_raw_upper().unwrap() == "JOIN"
        {
            let inner_keyword =
                if join_clause_keywords[0].raw() == "JOIN" { "INNER" } else { "inner" };
            return vec![LintResult::new(
                context.segment.segments()[0].clone().into(),
                vec![LintFix::create_before(
                    context.segment.segments()[0].clone(),
                    vec![
                        KeywordSegment::new(inner_keyword.into(), None).to_erased_segment(),
                        WhitespaceSegment::create(" ", None, WhitespaceSegmentNewArgs),
                    ],
                )],
                None,
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
        SegmentSeekerCrawler::new(["join_clause"].into()).into()
    }
}
