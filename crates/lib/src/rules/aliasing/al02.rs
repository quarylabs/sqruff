use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::al01::{Aliasing, RuleAL01};
use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
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
        // For T-SQL, check if this AliasExpression is part of "alias = expression" syntax
        if let Some(parent) = context.parent_stack.last() {
            if parent.get_type() == SyntaxKind::SelectClauseElement {
                let parent_segments = parent.segments();
                
                // Find this AliasExpression and check for following "="
                if let Some(pos) = parent_segments.iter().position(|s| s == &context.segment) {
                    // Check if the next non-whitespace segment is "="
                    for i in (pos + 1)..parent_segments.len() {
                        let seg = &parent_segments[i];
                        if !seg.is_whitespace() && !seg.is_meta() {
                            if seg.raw() == "=" {
                                // This is T-SQL "alias = expression" syntax, not implicit aliasing
                                return Vec::new();
                            }
                            break;
                        }
                    }
                }
                
                // Additional check for T-SQL: If this is an implicit alias (no AS keyword),
                // check if the aliased identifier is followed by "=" in the SelectClauseElement
                // 
                // Known limitation: This doesn't work when the T-SQL alias syntax spans multiple lines
                // with TOP, e.g.:
                //   SELECT TOP 20
                //       JiraIssueID = expression
                // 
                // In this case, the parser has already committed to parsing JiraIssueID as a column
                // reference with implicit aliasing before it sees the "=" on the same line.
                if context.segment.segments().iter().all(|s| s.raw() != "AS" && s.raw() != "as") {
                    // Find the identifier being used as alias
                    if let Some(identifier) = context.segment.segments().iter().find(|s| {
                        matches!(s.get_type(), SyntaxKind::NakedIdentifier | SyntaxKind::QuotedIdentifier)
                    }) {
                        // Check if this identifier is followed by "=" in the parent
                        if let Some(id_pos) = parent_segments.iter().position(|s| s.raw() == identifier.raw()) {
                            // Look for "=" after this identifier
                            for i in (id_pos + 1)..parent_segments.len() {
                                let seg = &parent_segments[i];
                                if !seg.is_whitespace() && !seg.is_meta() {
                                    if seg.raw() == "=" {
                                        // This is T-SQL "identifier = expression" syntax
                                        return Vec::new();
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Original check for other cases
        if FunctionalContext::new(context)
            .segment()
            .children(None)
            .last()
            .unwrap()
            .raw()
            == "="
        {
            return Vec::new();
        }

        self.base.eval(context)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}
