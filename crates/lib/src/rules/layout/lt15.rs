use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleLT15 {
    maximum_empty_lines_between_statements: usize,
    maximum_empty_lines_inside_statements: usize,
}

impl Default for RuleLT15 {
    fn default() -> Self {
        Self {
            maximum_empty_lines_between_statements: 2,
            maximum_empty_lines_inside_statements: 1,
        }
    }
}

impl Rule for RuleLT15 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT15 {
            maximum_empty_lines_between_statements: config
                .get("maximum_empty_lines_between_statements")
                .and_then(Value::as_int)
                .map(|v| v as usize)
                .unwrap_or(self.maximum_empty_lines_between_statements),
            maximum_empty_lines_inside_statements: config
                .get("maximum_empty_lines_inside_statements")
                .and_then(Value::as_int)
                .map(|v| v as usize)
                .unwrap_or(self.maximum_empty_lines_inside_statements),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "layout.newlines"
    }

    fn description(&self) -> &'static str {
        "Too many consecutive blank lines."
    }

    fn long_description(&self) -> &'static str {
        r#"**Anti-pattern**

In this example, the maximum number of empty lines inside a statement is set to 0.

```sql
SELECT 'a' AS col
FROM tab


WHERE x = 4
ORDER BY y


LIMIT 5
;
```

**Best practice**

```sql
SELECT 'a' AS col
FROM tab
WHERE x = 4
ORDER BY y
LIMIT 5
;
```"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if !context.segment.is_type(SyntaxKind::Newline) {
            return Vec::new();
        }

        let inside_statement = context
            .parent_stack
            .iter()
            .any(|seg| seg.is_type(SyntaxKind::Statement));

        let maximum_empty_lines = if inside_statement {
            self.maximum_empty_lines_inside_statements
        } else {
            self.maximum_empty_lines_between_statements
        };

        let Some(parent) = context.parent_stack.last() else {
            return Vec::new();
        };

        let siblings = parent.segments();
        let Some(current_idx) = siblings.iter().position(|s| s == &context.segment) else {
            return Vec::new();
        };

        // Count consecutive newlines including this one
        let mut consecutive_newlines = 1;

        // Count backwards from current position
        for i in (0..current_idx).rev() {
            if siblings[i].is_type(SyntaxKind::Newline) {
                consecutive_newlines += 1;
            } else {
                break;
            }
        }

        // Too many consecutive newlines means too many empty lines
        if consecutive_newlines > maximum_empty_lines + 1 {
            return vec![LintResult::new(
                context.segment.clone().into(),
                vec![LintFix::delete(context.segment.clone())],
                None,
                None,
            )];
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Newline]) }).into()
    }
}
