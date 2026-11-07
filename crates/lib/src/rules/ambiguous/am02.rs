use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleAM02;

impl Rule for RuleAM02 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
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
            DialectKind::Duckdb,
            DialectKind::Exasol,
            DialectKind::Postgres,
            DialectKind::Sparksql,
        ]
    }

    fn eval(&self, rule_cx: &RuleContext) -> Vec<LintResult> {
        let raw = rule_cx.segment.raw();
        let raw_upper = raw.to_uppercase();

        if !raw_upper.contains("UNION")
            || raw_upper.contains("ALL")
            || raw_upper.contains("DISTINCT")
        {
            return Vec::new();
        }

        // Preserve the existing UNION token; capitalisation belongs to CP01.
        let union_keyword = &rule_cx.segment.segments()[0];
        let distinct_keyword = if union_keyword.raw().chars().all(char::is_uppercase) {
            "DISTINCT"
        } else {
            "distinct"
        };
        let edits = vec![
            SegmentBuilder::whitespace(rule_cx.tables.next_id(), " "),
            SegmentBuilder::keyword(rule_cx.tables.next_id(), distinct_keyword),
        ];
        let fixes = vec![LintFix::create_after(union_keyword.clone(), edits, None)];
        vec![LintResult::new(
            Some(rule_cx.segment.clone()),
            fixes,
            None,
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SetOperator]) }).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_for_upstream_dialects() {
        let skipped = RuleAM02.dialect_skip();

        for dialect in [
            DialectKind::Bigquery,
            DialectKind::Clickhouse,
            DialectKind::Databricks,
            DialectKind::Db2,
            DialectKind::Snowflake,
            DialectKind::Trino,
        ] {
            assert!(!skipped.contains(&dialect));
        }
    }
}
