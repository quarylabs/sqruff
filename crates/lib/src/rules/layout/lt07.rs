use hashbrown::{HashMap, HashSet};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT07;

impl Rule for RuleLT07 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT07.erased())
    }
    fn name(&self) -> &'static str {
        "layout.cte_bracket"
    }

    fn description(&self) -> &'static str {
        "'WITH' clause closing bracket should be on a new line."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the closing bracket is on the same line as CTE.

```sql
 WITH zoo AS (
     SELECT a FROM foo)

 SELECT * FROM zoo
```

**Best practice**

Move the closing bracket on a new line.

```sql
WITH zoo AS (
    SELECT a FROM foo
)

SELECT * FROM zoo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let segments = FunctionalContext::new(context)
            .segment()
            .children_where(|seg| seg.is_type(SyntaxKind::CommonTableExpression));

        let mut cte_end_brackets = HashSet::new();
        for cte in segments.iterate_segments() {
            let cte_bracketed = cte
                .children_all()
                .find_last_where(|seg| seg.is_type(SyntaxKind::Bracketed));
            let cte_start_bracket = cte_bracketed
                .children_all()
                .find_first_where(|seg: &ErasedSegment| seg.is_type(SyntaxKind::StartBracket));

            let cte_end_bracket = cte_bracketed
                .children_all()
                .find_first_where(|seg: &ErasedSegment| seg.is_type(SyntaxKind::EndBracket));

            if !cte_start_bracket.is_empty() && !cte_end_bracket.is_empty() {
                // Other rules can insert newlines before position markers are
                // recomputed. Inspect the current CTE tree instead of comparing
                // the original bracket line numbers.
                let spans_multiple_lines = cte_bracketed[0].get_raw_segments().iter().any(|seg| {
                    seg.is_type(SyntaxKind::Newline)
                        || (seg.is_type(SyntaxKind::Placeholder) && seg.source_str() == "\n")
                });
                if !spans_multiple_lines {
                    continue;
                }
                cte_end_brackets.insert(cte_end_bracket[0].clone());
            }
        }

        for seg in cte_end_brackets {
            let mut contains_non_whitespace = false;
            let idx = context
                .segment
                .get_raw_segments()
                .iter()
                .position(|it| it == &seg)
                .unwrap();
            if idx > 0 {
                for elem in context.segment.get_raw_segments()[..idx].iter().rev() {
                    if elem.is_type(SyntaxKind::Newline)
                        || (elem.is_type(SyntaxKind::Placeholder) && elem.source_str() == "\n")
                    {
                        break;
                    } else if !(matches!(
                        elem.get_type(),
                        SyntaxKind::Indent | SyntaxKind::Implicit
                    ) || elem.is_type(SyntaxKind::Dedent)
                        || elem.is_type(SyntaxKind::Whitespace))
                    {
                        contains_non_whitespace = true;
                        break;
                    }
                }
            }

            if contains_non_whitespace {
                return vec![LintResult::new(
                    seg.clone().into(),
                    vec![LintFix::create_before(
                        seg,
                        vec![SegmentBuilder::newline(context.tables.next_id(), "\n")],
                    )],
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::WithCompoundStatement]) })
            .into()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    #[test]
    fn cte_bracket_fixes_are_idempotent_with_all_rules() {
        let config = FluffConfig::from_source("[sqruff]\ndialect = ansi\nrules = all\n", None);
        let linter = Linter::new(config, None, None, false).unwrap();
        let cases = [
            (
                "single_cte_becomes_multiline",
                "WITH blah AS (SELECT x, y FROM foo)\n\nSELECT z FROM blah\n",
                "WITH blah AS (\n    SELECT\n        x,\n        y\n    FROM foo\n)\n\nSELECT z FROM blah\n",
            ),
            (
                "multiple_ctes_become_multiline",
                "WITH foo AS (SELECT a, b FROM t1), bar AS (SELECT c, d FROM t2)\n\nSELECT foo.a, bar.c FROM foo CROSS JOIN bar\n",
                "WITH foo AS (\n    SELECT\n        a,\n        b\n    FROM t1\n),\n\nbar AS (\n    SELECT\n        c,\n        d\n    FROM t2\n)\n\nSELECT\n    foo.a,\n    bar.c\nFROM foo CROSS JOIN bar\n",
            ),
            (
                "multiline_cte_unchanged",
                "WITH cte AS (\n    SELECT\n        a,\n        b\n    FROM foo\n)\n\nSELECT\n    a,\n    b\nFROM cte\n",
                "WITH cte AS (\n    SELECT\n        a,\n        b\n    FROM foo\n)\n\nSELECT\n    a,\n    b\nFROM cte\n",
            ),
        ];

        for (name, before, expected) in cases {
            let fixed = linter
                .lint_string(before, Some(format!("{name}.sql")), true)
                .unwrap()
                .fix_string();
            assert_eq!(fixed, expected, "first fix for {name}");

            let fixed_again = linter
                .lint_string(&fixed, Some(format!("{name}.sql")), true)
                .unwrap()
                .fix_string();
            assert_eq!(fixed_again, fixed, "second fix for {name}");
        }
    }
}
