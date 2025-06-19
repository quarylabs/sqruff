use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use super::al01::{Aliasing, RuleAL01};
use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

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
        // For T-SQL, check if this is the equals-style alias syntax
        if context.dialect.name == sqruff_lib_core::dialects::init::DialectKind::Tsql {
            // First check: Look for = in the parent SelectClauseElement
            if let Some(parent) = context.parent_stack.last() {
                if parent.get_type() == SyntaxKind::SelectClauseElement {
                    // Look for equals sign in the parent element
                    let parent_segments = parent.segments();
                    for segment in parent_segments {
                        if segment.raw() == "=" {
                            // This is T-SQL equals-style alias syntax
                            return Vec::new();
                        }
                    }
                    
                    // Also check all descendants of the parent for "="
                    fn has_equals_in_tree(segment: &ErasedSegment) -> bool {
                        if segment.raw() == "=" {
                            return true;
                        }
                        for child in segment.segments() {
                            if has_equals_in_tree(child) {
                                return true;
                            }
                        }
                        false
                    }
                    
                    if has_equals_in_tree(parent) {
                        return Vec::new();
                    }
                }
            }
            
            // Second check: For cases where the parser created an AliasExpression
            // but it's actually part of a T-SQL equals syntax
            // Check if the segment immediately after this alias in the parent is "="
            if let Some(parent) = context.parent_stack.last() {
                let parent_segments = parent.segments();
                let mut found_current = false;
                
                for segment in parent_segments {
                    if found_current && segment.raw() == "=" {
                        // The next segment after our alias is "=", this is T-SQL syntax
                        return Vec::new();
                    }
                    if segment == &context.segment {
                        found_current = true;
                    }
                }
            }
        }
        
        // Original check - if the last child of the alias expression is "="
        if let Some(last_child) = FunctionalContext::new(context)
            .segment()
            .children(None)
            .last()
        {
            if last_child.raw() == "=" {
                return Vec::new();
            }
        }

        self.base.eval(context)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}
