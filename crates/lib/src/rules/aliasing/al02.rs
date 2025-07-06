use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::ErasedSegment;

use super::al01::{Aliasing, RuleAL01};
use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Clone)]
pub struct RuleAL02 {
    base: RuleAL01,
}

impl Default for RuleAL02 {
    fn default() -> Self {
        Self {
            base: RuleAL01::default()
                .target_parent_types(const { SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) }),
        }
    }
}

impl RuleAL02 {
    pub fn aliasing(mut self, aliasing: Aliasing) -> Self {
        self.base = self.base.aliasing(aliasing);
        self
    }

    /// Check if this alias expression is part of T-SQL "alias = expression" syntax
    fn is_tsql_alias_equals_syntax(&self, context: &RuleContext, parent: &ErasedSegment) -> bool {
        let parent_segments = parent.segments();

        // Check 1: Is the alias expression followed by "="?
        if let Some(pos) = parent_segments.iter().position(|s| s == &context.segment) {
            // Check if the next non-whitespace segment is "="
            for seg in parent_segments.iter().skip(pos + 1) {
                if !seg.is_whitespace() && !seg.is_meta() {
                    if seg.raw() == "=" {
                        return true;
                    }
                    break;
                }
            }
        }

        // Only check further if this is an implicit alias (no AS keyword)
        if !context
            .segment
            .segments()
            .iter()
            .all(|s| s.raw() != "AS" && s.raw() != "as")
        {
            return false;
        }

        // Check 2: Is the identifier in the alias followed by "=" in the parent?
        if let Some(identifier) = context.segment.segments().iter().find(|s| {
            matches!(
                s.get_type(),
                SyntaxKind::NakedIdentifier | SyntaxKind::QuotedIdentifier
            )
        }) {
            if let Some(id_pos) = parent_segments
                .iter()
                .position(|s| s.raw() == identifier.raw())
            {
                for seg in parent_segments.iter().skip(id_pos + 1) {
                    if !seg.is_whitespace() && !seg.is_meta() {
                        if seg.raw() == "=" {
                            return true;
                        }
                        break;
                    }
                }
            }
        }

        // Check 3: Multiline T-SQL alias syntax with TOP
        // When parser sees "SELECT TOP 20\n    JiraIssueID = expression"
        // it parses JiraIssueID as implicit alias before seeing the "="
        if context.parent_stack.len() >= 2 {
            if let Some(select_clause) = context
                .parent_stack
                .iter()
                .rev()
                .find(|s| s.get_type() == SyntaxKind::SelectClause)
            {
                let select_raw = select_clause.raw();

                // Only check if SELECT has TOP
                let has_top = select_raw
                    .split_whitespace()
                    .take(10)
                    .any(|token| token.to_uppercase() == "TOP");

                if has_top {
                    if let Some(identifier) = context.segment.segments().iter().find(|s| {
                        matches!(
                            s.get_type(),
                            SyntaxKind::NakedIdentifier | SyntaxKind::QuotedIdentifier
                        )
                    }) {
                        let identifier_raw = identifier.raw();

                        // Look for "identifier =" pattern in the entire SELECT clause
                        if let Some(id_pos) = select_raw.find(identifier_raw.as_str()) {
                            let after_id = &select_raw[id_pos + identifier_raw.len()..];
                            if after_id.trim_start().starts_with('=') {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

impl Rule for RuleAL02 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        let aliasing = match config.get("aliasing").unwrap().as_string().unwrap() {
            "explicit" => Aliasing::Explicit,
            "implicit" => Aliasing::Implicit,
            _ => unreachable!(),
        };

        let mut rule = RuleAL02::default();
        rule.base = rule.base.aliasing(aliasing);

        Ok(rule.erased())
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "aliasing.column"
    }

    fn description(&self) -> &'static str {
        "Implicit/explicit aliasing of columns."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the alias for column `a` is implicit.

```sql
SELECT
  a alias_col
FROM foo
```

**Best practice**

Add the `AS` keyword to make the alias explicit.

```sql
SELECT
    a AS alias_col
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Aliasing]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // Check if this is within a SELECT clause element
        if let Some(parent) = context.parent_stack.last() {
            if parent.get_type() == SyntaxKind::SelectClauseElement {
                // Skip if this is T-SQL "alias = expression" syntax
                if self.is_tsql_alias_equals_syntax(context, parent) {
                    return Vec::new();
                }
            }
        }

        self.base.eval(context)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}
