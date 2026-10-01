use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RulePG01;

fn has_keyword(segment: &ErasedSegment, keyword: &str) -> bool {
    segment.segments().iter().any(|child| {
        child.is_type(SyntaxKind::Keyword) && child.raw().eq_ignore_ascii_case(keyword)
    })
}

impl Rule for RulePG01 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RulePG01.erased())
    }

    fn name(&self) -> &'static str {
        "postgres.excessive_locks"
    }

    fn description(&self) -> &'static str {
        "Avoid excessive locks in PostgreSQL DDL statements."
    }

    fn long_description(&self) -> &'static str {
        r#"
PostgreSQL DDL operations can block reads or writes for the duration of an operation.

**Anti-pattern**

```sql
CREATE INDEX idx_foo ON bar (tenant_id);
DROP INDEX idx_foo;
REINDEX INDEX idx_foo;
REFRESH MATERIALIZED VIEW my_view;
ALTER TABLE foo ADD CONSTRAINT fk_bar FOREIGN KEY (bar_id) REFERENCES bar (id);
```

**Best practice**

```sql
CREATE INDEX CONCURRENTLY idx_foo ON bar (tenant_id);
DROP INDEX CONCURRENTLY idx_foo;
REINDEX INDEX CONCURRENTLY idx_foo;
REFRESH MATERIALIZED VIEW CONCURRENTLY my_view;
ALTER TABLE foo ADD CONSTRAINT fk_bar FOREIGN KEY (bar_id) REFERENCES bar (id) NOT VALID;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Postgres]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.dialect.name != DialectKind::Postgres {
            return Vec::new();
        }

        let segment = &context.segment;
        let description = match segment.get_type() {
            SyntaxKind::CreateIndexStatement => {
                if has_keyword(segment, "CONCURRENTLY") || has_keyword(segment, "UNIQUE") {
                    return Vec::new();
                }
                "CREATE INDEX should use CONCURRENTLY to avoid locking the table during the build."
                    .to_owned()
            }
            SyntaxKind::DropIndexStatement
            | SyntaxKind::ReindexStatementSegment
            | SyntaxKind::RefreshMaterializedViewStatement => {
                if has_keyword(segment, "CONCURRENTLY") {
                    return Vec::new();
                }
                let statement = segment
                    .segments()
                    .first()
                    .map(|child| child.raw().to_uppercase())
                    .unwrap_or_else(|| "DDL".to_owned());
                format!("{statement} statement should use CONCURRENTLY to avoid locking the table.")
            }
            SyntaxKind::AlterTableStatement => {
                let Some(action) = segment
                    .child(const { &SyntaxSet::single(SyntaxKind::AlterTableActionSegment) })
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
                "ADD CONSTRAINT ... FOREIGN KEY should use NOT VALID to avoid locking the table while validating existing rows."
                    .to_owned()
            }
            _ => return Vec::new(),
        };

        vec![LintResult::new(
            Some(segment.clone()),
            Vec::new(),
            Some(description),
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::CreateIndexStatement,
                    SyntaxKind::DropIndexStatement,
                    SyntaxKind::ReindexStatementSegment,
                    SyntaxKind::RefreshMaterializedViewStatement,
                    SyntaxKind::AlterTableStatement,
                ])
            },
        )
        .into()
    }
}
