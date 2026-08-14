use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;

use crate::core::config::Value;
use crate::core::rules::config::{RuleConfig, RuleConfigOption};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

crate::rule_config! {
    /// Configuration for `layout.newlines` (LT15).
    RuleLT15Config {
        /// How many blank lines are allowed between two statements.
        maximum_empty_lines_between_statements: usize = 2,
        /// How many blank lines are allowed within a statement.
        maximum_empty_lines_inside_statements: usize = 1,
    }
}

#[derive(Debug, Clone)]
pub struct RuleLT15 {
    maximum_empty_lines_between_statements: usize,
    maximum_empty_lines_inside_statements: usize,
}

impl Default for RuleLT15 {
    fn default() -> Self {
        let config = RuleLT15Config::default();
        Self {
            maximum_empty_lines_between_statements: config.maximum_empty_lines_between_statements,
            maximum_empty_lines_inside_statements: config.maximum_empty_lines_inside_statements,
        }
    }
}

impl Rule for RuleLT15 {
    fn config_options(&self) -> Vec<RuleConfigOption> {
        RuleLT15Config::config_options()
    }

    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        let config = RuleLT15Config::from_config(config)?;

        Ok(RuleLT15 {
            maximum_empty_lines_between_statements: config.maximum_empty_lines_between_statements,
            maximum_empty_lines_inside_statements: config.maximum_empty_lines_inside_statements,
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
