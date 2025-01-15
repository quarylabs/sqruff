use ahash::AHashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Clone, Debug, Default)]
pub struct RuleAM02;

impl Rule for RuleAM02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAM02.erased())
    }

    fn name(&self) -> &'static str {
        "ambiguous.union"
    }

    fn description(&self) -> &'static str {
        "Look for UNION keyword not immediately followed by DISTINCT or ALL"
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
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Ambiguous]
    }

    fn dialect_skip(&self) -> &'static [DialectKind] {
        // TODO: add ansi, hive, mysql
        // TODO This feels wrong and should bneed fixing
        &[
            DialectKind::Bigquery,
            DialectKind::Postgres,
            DialectKind::Snowflake,
            DialectKind::Clickhouse,
            DialectKind::Sparksql,
            DialectKind::Duckdb,
        ]
    }

    fn eval(&self, rule_cx: &RuleContext) -> Vec<LintResult> {
        let raw = rule_cx.segment.raw();
        let raw_upper = raw.to_uppercase();

        if rule_cx.segment.raw().contains("union")
            && !(raw_upper.contains("ALL") || raw_upper.contains("DISTINCT"))
        {
            let edits = vec![
                SegmentBuilder::keyword(rule_cx.tables.next_id(), "union"),
                SegmentBuilder::whitespace(rule_cx.tables.next_id(), " "),
                SegmentBuilder::keyword(rule_cx.tables.next_id(), "distinct"),
            ];

            let segments = rule_cx.segment.clone();
            let fixes = vec![LintFix::replace(
                rule_cx.segment.segments()[0].clone(),
                edits,
                None,
            )];

            return vec![LintResult::new(Some(segments), fixes, None, None)];
        } else if raw_upper.contains("UNION")
            && !(raw_upper.contains("ALL") || raw_upper.contains("DISTINCT"))
        {
            let edits = vec![
                SegmentBuilder::keyword(rule_cx.tables.next_id(), "UNION"),
                SegmentBuilder::whitespace(rule_cx.tables.next_id(), " "),
                SegmentBuilder::keyword(rule_cx.tables.next_id(), "DISTINCT"),
            ];

            let segments = rule_cx.segment.clone();
            let fixes = vec![LintFix::replace(
                rule_cx.segment.segments()[0].clone(),
                edits,
                None,
            )];

            return vec![LintResult::new(Some(segments), fixes, None, None)];
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SetOperator]) }).into()
    }
}
