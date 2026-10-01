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

            // Find LHS: first non-whitespace segment before the operator
            let lhs_idx = subsegments[..idx]
                .iter()
                .rposition(|s| !is_whitespace_or_newline(s));

            // Find RHS: first non-whitespace segment after the operator
            let rhs_idx = subsegments[idx + 1..]
                .iter()
                .position(|s| !is_whitespace_or_newline(s))
                .map(|offset| idx + 1 + offset);

            let (lhs_idx, rhs_idx) = match (lhs_idx, rhs_idx) {
                (Some(lhs_idx), Some(rhs_idx)) => (lhs_idx, rhs_idx),
                _ => continue,
            };
            let lhs = &subsegments[lhs_idx];
            let rhs = &subsegments[rhs_idx];

            // Skip templated segments
            if lhs.is_templated() || rhs.is_templated() {
                continue;
            }

            let previous_before_lhs = subsegments[..lhs_idx]
                .iter()
                .rev()
                .find(|s| !is_whitespace_or_newline(s));
            let next_after_rhs = subsegments[rhs_idx + 1..]
                .iter()
                .find(|s| !is_whitespace_or_newline(s));

            // The pieces immediately left and right of `=` are its operands after
            // AND or OR (e.g. `... OR 1 = 2`). After another binary operator they
            // are not (e.g. `num % 2 = 0` or `flags = flags & @mask`), so leave
            // those expressions alone.
            if previous_before_lhs.is_some_and(is_non_boolean_binary_operator)
                || next_after_rhs.is_some_and(is_non_boolean_binary_operator)
            {
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

fn is_non_boolean_binary_operator(seg: &ErasedSegment) -> bool {
    let raw = seg.raw();
    (seg.is_type(SyntaxKind::BinaryOperator)
        && !matches!(raw.to_uppercase_smolstr().as_str(), "AND" | "OR"))
        // Sqruff's bitwise operators are wrapped as comparison operators.
        || (seg.is_type(SyntaxKind::ComparisonOperator)
            && matches!(raw.as_str(), "&" | "|" | "<<" | ">>"))
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
