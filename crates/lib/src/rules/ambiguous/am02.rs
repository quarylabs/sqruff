use ahash::AHashMap;

use crate::core::config::Value;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::segments::base::{CodeSegment, CodeSegmentNewArgs};
use crate::core::rules::base::{CloneRule, ErasedRule, LintFix, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::dialects::{SyntaxKind, SyntaxSet};

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
        // TODO: add ansi, hive, mysql, redshift
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

    fn eval(&self, rule_cx: RuleContext) -> Vec<LintResult> {
        let raw = rule_cx.segment.raw();
        let raw_upper = raw.to_uppercase();

        if rule_cx.segment.raw().contains("union")
            && !(raw_upper.contains("ALL") || raw_upper.contains("DISTINCT"))
        {
            let edits = vec![
                CodeSegment::keyword(rule_cx.tables.next_id(), "union"),
                CodeSegment::create(
                    rule_cx.tables.next_id(),
                    " ",
                    None,
                    CodeSegmentNewArgs { code_type: SyntaxKind::Whitespace },
                ),
                CodeSegment::keyword(rule_cx.tables.next_id(), "distinct"),
            ];

            let segments = rule_cx.segment.clone();
            let fixes = vec![LintFix::replace(rule_cx.segment.segments()[0].clone(), edits, None)];

            return vec![LintResult::new(Some(segments), fixes, None, None, None)];
        } else if raw_upper.contains("UNION")
            && !(raw_upper.contains("ALL") || raw_upper.contains("DISTINCT"))
        {
            let edits = vec![
                CodeSegment::keyword(rule_cx.tables.next_id(), "UNION"),
                CodeSegment::create(
                    rule_cx.tables.next_id(),
                    " ",
                    None,
                    CodeSegmentNewArgs { code_type: SyntaxKind::Newline },
                ),
                CodeSegment::keyword(rule_cx.tables.next_id(), "DISTINCT"),
            ];

            let segments = rule_cx.segment.clone();
            let fixes = vec![LintFix::replace(rule_cx.segment.segments()[0].clone(), edits, None)];

            return vec![LintResult::new(Some(segments), fixes, None, None, None)];
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
