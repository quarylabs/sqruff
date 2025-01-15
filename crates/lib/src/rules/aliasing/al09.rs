use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Default, Clone, Debug)]
pub struct RuleAL09;

impl Rule for RuleAL09 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleAL09.erased())
    }

    fn name(&self) -> &'static str {
        "aliasing.self_alias.column"
    }

    fn description(&self) -> &'static str {
        "Find self-aliased columns and fix them"
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Aliasing the column to itself.

```sql
SELECT
    col AS col
FROM table;
```

**Best practice**

Not to use alias to rename the column to its original name. Self-aliasing leads to redundant code without changing any functionality.

```sql
SELECT
    col
FROM table;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut violations = Vec::new();

        let children = FunctionalContext::new(context).segment().children(None);

        for clause_element in children.select(
            Some(|sp: &ErasedSegment| sp.is_type(SyntaxKind::SelectClauseElement)),
            None,
            None,
            None,
        ) {
            let clause_element_raw_segment = clause_element.get_raw_segments();

            let column =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) });
            let alias_expression =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) });

            if let Some(column) = column {
                if let Some(alias_expression) = alias_expression {
                    if column
                        .child(
                            const {
                                &SyntaxSet::new(&[
                                    SyntaxKind::Identifier,
                                    SyntaxKind::NakedIdentifier,
                                ])
                            },
                        )
                        .is_some()
                        || column
                            .child(const { &SyntaxSet::new(&[SyntaxKind::QuotedIdentifier]) })
                            .is_some()
                    {
                        let Some(whitespace) = clause_element
                            .child(const { &SyntaxSet::new(&[SyntaxKind::Whitespace]) })
                        else {
                            return Vec::new();
                        };

                        let column_identifier = if let Some(quoted_identifier) =
                            column.child(const { &SyntaxSet::new(&[SyntaxKind::QuotedIdentifier]) })
                        {
                            quoted_identifier.clone()
                        } else {
                            column
                                .children(
                                    const {
                                        &SyntaxSet::new(&[
                                            SyntaxKind::Identifier,
                                            SyntaxKind::NakedIdentifier,
                                        ])
                                    },
                                )
                                .last()
                                .expect("No naked_identifier found")
                                .clone()
                        };

                        let alias_identifier = alias_expression
                            .child(const { &SyntaxSet::new(&[SyntaxKind::NakedIdentifier]) })
                            .or_else(|| {
                                alias_expression.child(
                                    const { &SyntaxSet::new(&[SyntaxKind::QuotedIdentifier]) },
                                )
                            })
                            .expect("identifier is none");

                        if column_identifier
                            .raw()
                            .eq_ignore_ascii_case(alias_identifier.raw())
                        {
                            let fixes = vec![
                                LintFix::delete(whitespace),
                                LintFix::delete(alias_expression),
                            ];

                            violations.push(LintResult::new(
                                Some(clause_element_raw_segment[0].clone()),
                                fixes,
                                Some("Column should not be self-aliased.".into()),
                                None,
                            ));
                        }
                    }
                }
            }
        }

        violations
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}
