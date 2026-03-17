use hashbrown::HashMap;
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Default, Debug, Clone)]
pub struct RuleST10;

impl Rule for RuleST10 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST10.erased())
    }

    fn name(&self) -> &'static str {
        "structure.constant_expression"
    }

    fn description(&self) -> &'static str {
        "Redundant constant expression."
    }

    fn long_description(&self) -> &'static str {
        r#"
Including an expression that always evaluates to either `TRUE` or `FALSE`
regardless of the input columns is unnecessary and makes statements harder
to read and understand.

**Anti-pattern**

```sql
SELECT *
FROM my_table
-- This following WHERE clause is redundant.
WHERE my_table.col = my_table.col
```

**Best practice**

```sql
SELECT *
FROM my_table
-- Replace with a condition that includes meaningful logic,
-- or remove the condition entirely.
WHERE my_table.col > 3
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let subsegments = context.segment.segments();
        let count_subsegments = subsegments.len();

        let allowable_literal_expressions = ["1 = 1", "1 = 0"];

        let mut results = Vec::new();

        for (idx, seg) in subsegments.iter().enumerate() {
            if !seg.is_type(SyntaxKind::ComparisonOperator) {
                continue;
            }

            let raw_op = seg.raw();
            if raw_op.as_str() != "=" && raw_op.as_str() != "!=" && raw_op.as_str() != "<>" {
                continue;
            }

            // Check for other comparison/binary operators before this one
            // (precedence concerns). Following SQLFluff's approach: only check
            // segments before the current operator. This means the first
            // comparison operator in an expression is always checked, while
            // later ones are skipped due to precedence ambiguity.
            let has_other_operators_before = subsegments[..idx].iter().any(|s| {
                s.is_type(SyntaxKind::ComparisonOperator) || s.is_type(SyntaxKind::BinaryOperator)
            });

            if has_other_operators_before {
                continue;
            }

            // Find LHS: first non-whitespace segment before the operator
            let lhs = subsegments[..idx]
                .iter()
                .rev()
                .find(|s| !is_whitespace_or_newline(s));

            // Find RHS: first non-whitespace segment after the operator
            let rhs = subsegments[idx + 1..count_subsegments]
                .iter()
                .find(|s| !is_whitespace_or_newline(s));

            let (lhs, rhs) = match (lhs, rhs) {
                (Some(l), Some(r)) => (l, r),
                _ => continue,
            };

            // Skip templated segments
            if lhs.is_templated() || rhs.is_templated() {
                continue;
            }

            // Handle literal comparisons with allowlist
            if lhs.is_type(SyntaxKind::NumericLiteral) && rhs.is_type(SyntaxKind::NumericLiteral) {
                let expr = format!(
                    "{} {} {}",
                    lhs.raw().to_uppercase_smolstr(),
                    raw_op,
                    rhs.raw().to_uppercase_smolstr()
                );
                if allowable_literal_expressions.contains(&expr.as_str()) {
                    continue;
                }
            } else if is_literal(lhs) && is_literal(rhs) {
                // Non-numeric literals (e.g. quoted strings) - always flag
            } else {
                // Non-literal comparison: check type and value match
                if lhs.get_type() != rhs.get_type() {
                    continue;
                }
                if lhs.raw().to_uppercase_smolstr() != rhs.raw().to_uppercase_smolstr() {
                    continue;
                }
            }

            // Attach violation to the comparison operator
            results.push(LintResult::new(seg.clone().into(), Vec::new(), None, None));
        }

        results
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Expression]) }).into()
    }
}

fn is_whitespace_or_newline(seg: &ErasedSegment) -> bool {
    matches!(seg.get_type(), SyntaxKind::Whitespace | SyntaxKind::Newline)
}

fn is_literal(seg: &ErasedSegment) -> bool {
    matches!(
        seg.get_type(),
        SyntaxKind::Literal
            | SyntaxKind::NumericLiteral
            | SyntaxKind::QuotedLiteral
            | SyntaxKind::BooleanLiteral
            | SyntaxKind::NullLiteral
    )
}
