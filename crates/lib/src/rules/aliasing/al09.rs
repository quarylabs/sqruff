use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

#[derive(Default, Clone, Debug)]
pub struct RuleAL09;

impl Rule for RuleAL09 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
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
Column aliases should not alias to themselves.

Renaming a column to itself is redundant. This rule only removes aliases that
are exact copies of the column reference, including their quoting and casing.
Aliases that change the effective casing of an identifier are allowed.

**Anti-pattern**

Aliasing a column to itself where the alias is not needed to change its case.

```sql
SELECT
    col AS col,
    "Col" AS "Col",
    COL AS col
FROM table;
```

**Best practice**

Remove aliases that exactly repeat their column reference. Case-changing
aliases remain valid.

```sql
SELECT
    col,
    "Col",
    COL
FROM table;

SELECT
    col AS "Col",
    "col" AS "COL"
FROM table;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut violations = Vec::new();

        let children = FunctionalContext::new(context).segment().children_all();

        for clause_element in
            children.filter(|sp: &ErasedSegment| sp.is_type(SyntaxKind::SelectClauseElement))
        {
            let clause_element_raw_segment = clause_element.get_raw_segments();

            let column =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::ColumnReference]) });
            let alias_expression =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::AliasExpression]) });

            let (Some(column), Some(alias_expression)) = (column, alias_expression) else {
                continue;
            };

            let identifier_types = const {
                &SyntaxSet::new(&[
                    SyntaxKind::Identifier,
                    SyntaxKind::NakedIdentifier,
                    SyntaxKind::QuotedIdentifier,
                ])
            };

            // A qualified column reference may contain multiple identifiers. The
            // final one is the column being aliased.
            let Some(column_identifier) = column.children(identifier_types).last().cloned() else {
                continue;
            };

            let whitespace =
                clause_element.child(const { &SyntaxSet::new(&[SyntaxKind::Whitespace]) });
            let alias_identifier = alias_expression.child(identifier_types);

            let syntax_parts_found = (whitespace.is_some(), alias_identifier.is_some());
            let (Some(whitespace), Some(alias_identifier)) = (whitespace, alias_identifier) else {
                log::warn!(
                    "AL09 found unexpected syntax in an alias expression. Unable to determine if \
                     this is a self-alias. Please report this as a bug on GitHub.\n\nDebug \
                     details: dialect: {:?}, whitespace: {}, alias_identifier: {}, \
                     alias_expression: {:?}.",
                    context.dialect.name,
                    syntax_parts_found.0,
                    syntax_parts_found.1,
                    clause_element.raw(),
                );
                continue;
            };

            // Only exact matches, including quoting and case, are safe to fix.
            if column_identifier.raw() == alias_identifier.raw() {
                violations.push(LintResult::new(
                    Some(clause_element_raw_segment[0].clone()),
                    vec![
                        LintFix::delete(whitespace),
                        LintFix::delete(alias_expression),
                    ],
                    Some("Column should not be self-aliased.".into()),
                    None,
                ));
            } else if context.dialect.name != DialectKind::Clickhouse
                && (column_identifier.is_type(SyntaxKind::Identifier)
                    || column_identifier.is_type(SyntaxKind::NakedIdentifier))
                && (alias_identifier.is_type(SyntaxKind::Identifier)
                    || alias_identifier.is_type(SyntaxKind::NakedIdentifier))
                && column_identifier
                    .raw()
                    .eq_ignore_ascii_case(alias_identifier.raw())
            {
                // For case-insensitive unquoted identifiers the user's intent is
                // ambiguous: remove the alias, or quote it to make the case change
                // explicit. Report the issue without proposing a fix.
                violations.push(LintResult::new(
                    Some(clause_element_raw_segment[0].clone()),
                    Vec::new(),
                    Some(
                        "Ambiguous self alias. Either remove unnecessary alias, or quote \
                         alias/reference to make case change explicit."
                            .into(),
                    ),
                    None,
                ));
            }
        }

        violations
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    const INPUT_QUERY: &str = r#"
select
    a as A,
    B as b,
    "C" as C,
    "d" as d,
    "E" as e,
    "f" as F,
    g as "G",
    h as h,
    I as I
from foo
"#;

    #[derive(Debug)]
    struct Case {
        rules: &'static str,
        dialect: &'static str,
        fixed_sql: &'static str,
        post_fix_errors: &'static [(&'static str, usize, usize)],
    }

    #[test]
    fn al09_cp02_rf06_interactions_match_sqlfluff() {
        let cases = [
            Case {
                rules: "AL09",
                dialect: "ansi",
                fixed_sql: r#"
select
    a as A,
    B as b,
    "C" as C,
    "d" as d,
    "E" as e,
    "f" as F,
    g as "G",
    h,
    I
from foo
"#,
                post_fix_errors: &[("AL09", 3, 5), ("AL09", 4, 5)],
            },
            Case {
                rules: "CP02",
                dialect: "ansi",
                fixed_sql: r#"
select
    a as a,
    b as b,
    "C" as c,
    "d" as d,
    "E" as e,
    "f" as f,
    g as "G",
    h as h,
    i as i
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "RF06",
                dialect: "ansi",
                fixed_sql: r#"
select
    a as A,
    B as b,
    C as C,
    "d" as d,
    E as e,
    "f" as F,
    g as G,
    h as h,
    I as I
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "AL09, CP02",
                dialect: "ansi",
                fixed_sql: r#"
select
    a,
    b,
    "C" as c,
    "d" as d,
    "E" as e,
    "f" as f,
    g as "G",
    h,
    i
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "AL09, RF06",
                dialect: "ansi",
                fixed_sql: r#"
select
    a as A,
    B as b,
    C,
    "d" as d,
    E as e,
    "f" as F,
    g as G,
    h,
    I
from foo
"#,
                post_fix_errors: &[
                    ("AL09", 3, 5),
                    ("AL09", 4, 5),
                    ("AL09", 7, 5),
                    ("AL09", 9, 5),
                ],
            },
            Case {
                rules: "CP02, RF06",
                dialect: "ansi",
                fixed_sql: r#"
select
    a as a,
    b as b,
    c as c,
    "d" as d,
    e as e,
    "f" as f,
    g as g,
    h as h,
    i as i
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "AL09, CP02, RF06",
                dialect: "ansi",
                fixed_sql: r#"
select
    a,
    b,
    c,
    "d" as d,
    e,
    "f" as f,
    g,
    h,
    i
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "AL09, CP02, RF06",
                dialect: "postgres",
                fixed_sql: r#"
select
    a,
    b,
    "C" as c,
    d,
    "E" as e,
    f,
    g as "G",
    h,
    i
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "AL09, CP02, RF06",
                dialect: "duckdb",
                fixed_sql: r#"
select
    a,
    b,
    c,
    d,
    e,
    f,
    g,
    h,
    i
from foo
"#,
                post_fix_errors: &[],
            },
            Case {
                rules: "AL09, RF06",
                dialect: "clickhouse",
                fixed_sql: r#"
select
    a as A,
    B as b,
    C,
    d,
    E as e,
    f as F,
    g as G,
    h,
    I
from foo
"#,
                post_fix_errors: &[],
            },
        ];

        for case in cases {
            let config = FluffConfig::from_source(
                &format!(
                    "[sqruff]\nrules = {}\ndialect = {}\n",
                    case.rules, case.dialect
                ),
                None,
            );
            let mut linter = Linter::new(config, None, None, true).unwrap();
            let linted = linter.lint_string_wrapped(INPUT_QUERY, true).unwrap();
            let fixed = linted.fix_string();
            assert_eq!(fixed, case.fixed_sql, "{case:?}");

            let post_fix = linter.lint_string_wrapped(&fixed, false).unwrap();
            let actual = post_fix
                .violations()
                .iter()
                .map(|violation| (violation.rule_code(), violation.line_no, violation.line_pos))
                .collect::<Vec<_>>();
            assert_eq!(actual, case.post_fix_errors, "{case:?}");
        }
    }
}
