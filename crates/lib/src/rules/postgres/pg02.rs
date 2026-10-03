use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RulePG02 {
    force_enable: bool,
}

fn has_keyword(segment: &ErasedSegment, keyword: &str) -> bool {
    segment.segments().iter().any(|child| {
        child.is_type(SyntaxKind::Keyword) && child.raw().eq_ignore_ascii_case(keyword)
    })
}

impl Rule for RulePG02 {
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RulePG02 {
            force_enable: config["force_enable"].as_bool().unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "postgres.not_valid_foreign_key"
    }

    fn description(&self) -> &'static str {
        "Create PostgreSQL foreign keys as NOT VALID before validating separately."
    }

    fn long_description(&self) -> &'static str {
        r#"
Adding a foreign key constraint normally validates existing rows during `ALTER TABLE`.
On large tables this can hold locks longer than necessary. This PostgreSQL-only
rule is disabled by default; set `force_enable = true` to enable it.

**Anti-pattern**

```sql
ALTER TABLE foo ADD CONSTRAINT fk_bar FOREIGN KEY (bar_id) REFERENCES bar (id);
```

**Best practice**

```sql
ALTER TABLE foo ADD CONSTRAINT fk_bar FOREIGN KEY (bar_id) REFERENCES bar (id) NOT VALID;
ALTER TABLE foo VALIDATE CONSTRAINT fk_bar;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Postgres]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if !self.force_enable || context.dialect.name != DialectKind::Postgres {
            return Vec::new();
        }

        let segment = &context.segment;
        let Some(action) =
            segment.child(const { &SyntaxSet::single(SyntaxKind::AlterTableActionSegment) })
        else {
            return Vec::new();
        };
        let Some(constraint) =
            action.child(const { &SyntaxSet::single(SyntaxKind::TableConstraint) })
        else {
            return Vec::new();
        };
        if !has_keyword(&constraint, "FOREIGN") || has_keyword(&constraint, "VALID") {
            return Vec::new();
        }

        vec![LintResult::new(
            Some(segment.clone()),
            Vec::new(),
            Some("ADD CONSTRAINT ... FOREIGN KEY should use NOT VALID, then VALIDATE CONSTRAINT separately, to avoid locking the table while validating existing rows.".to_owned()),
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::single(SyntaxKind::AlterTableStatement) })
            .into()
    }
}
