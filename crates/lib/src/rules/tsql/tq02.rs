use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::SegmentBuilder;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleTQ02;

impl Rule for RuleTQ02 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleTQ02.erased())
    }

    fn name(&self) -> &'static str {
        "tsql.procedure_begin_end"
    }

    fn description(&self) -> &'static str {
        "Procedure body with multiple statements should be wrapped in BEGIN/END block."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Procedure bodies with multiple statements should be wrapped in `BEGIN`/`END`
for clarity and consistency.

```sql
CREATE PROCEDURE Reporting.MultipleStatements
AS
SELECT * FROM Table1;
SELECT * FROM Table2;
```

**Best practice**

Wrap procedure bodies with multiple statements in `BEGIN`/`END` blocks.

```sql
CREATE PROCEDURE Reporting.MultipleStatements
AS
BEGIN
    SELECT * FROM Table1;
    SELECT * FROM Table2;
END
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Tsql]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.dialect.name != DialectKind::Tsql {
            return Vec::new();
        }

        let Some(procedure_statement) = context
            .segment
            .segments()
            .iter()
            .find(|segment| segment.is_type(SyntaxKind::ProcedureStatement))
            .cloned()
        else {
            return Vec::new();
        };

        let statements = procedure_statement
            .segments()
            .iter()
            .filter(|segment| segment.is_type(SyntaxKind::Statement))
            .cloned()
            .collect::<Vec<_>>();

        if statements.len() < 2 {
            return Vec::new();
        }

        let first_statement_is_begin_end = statements[0]
            .segments()
            .first()
            .is_some_and(|segment| segment.is_type(SyntaxKind::BeginEndBlock));
        let body_is_begin_end = procedure_statement
            .segments()
            .iter()
            .find(|segment| !segment.is_meta() && !segment.is_whitespace())
            .is_some_and(|segment| segment.raw().eq_ignore_ascii_case("BEGIN"))
            && procedure_statement
                .segments()
                .iter()
                .rev()
                .find(|segment| !segment.is_meta() && !segment.is_whitespace())
                .is_some_and(|segment| segment.raw().eq_ignore_ascii_case("END"));
        if first_statement_is_begin_end || body_is_begin_end {
            return Vec::new();
        }

        let first_statement = statements.first().unwrap().clone();
        let last_statement = statements.last().unwrap().clone();
        let insert_after = procedure_statement
            .segments()
            .iter()
            .position(|segment| segment == &last_statement)
            .and_then(|last_statement_idx| {
                procedure_statement.segments()[last_statement_idx + 1..]
                    .iter()
                    .take_while(|segment| !segment.is_type(SyntaxKind::Statement))
                    .find(|segment| segment.is_type(SyntaxKind::StatementTerminator))
            })
            .cloned()
            .unwrap_or(last_statement);
        let fixes = vec![
            LintFix::create_before(
                first_statement,
                vec![
                    SegmentBuilder::keyword(context.tables.next_id(), "BEGIN"),
                    SegmentBuilder::newline(context.tables.next_id(), "\n"),
                ],
            ),
            LintFix::create_after(
                insert_after,
                vec![
                    SegmentBuilder::newline(context.tables.next_id(), "\n"),
                    SegmentBuilder::keyword(context.tables.next_id(), "END"),
                ],
                None,
            ),
        ];

        vec![LintResult::new(
            Some(procedure_statement),
            fixes,
            Some(self.description().into()),
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::CreateProcedureStatement]) })
            .into()
    }
}
