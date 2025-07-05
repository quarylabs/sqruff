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
                    
                    // Special handling for multiline T-SQL alias syntax with TOP
                    // When the parser sees:
                    //   SELECT TOP 20
                    //       JiraIssueID = expression
                    // It parses JiraIssueID as a column with implicit alias before seeing the =
                    // 
                    // To detect this, we check if:
                    // 1. We're in a SELECT with TOP
                    // 2. The entire SELECT clause contains "identifier ="
                    if context.parent_stack.len() >= 2 {
                        if let Some(select_clause) = context.parent_stack.iter().rev().find(|s| s.get_type() == SyntaxKind::SelectClause) {
                            // Check if this SELECT has TOP by looking at the raw text
                            let select_raw = select_clause.raw();
                            
                            // Look for TOP in the select clause text
                            let has_top = select_raw.split_whitespace()
                                .take(10) // Look at first few tokens
                                .any(|token| token.to_uppercase() == "TOP");
                            
                            if has_top {
                                // Get the identifier from the alias expression
                                if let Some(identifier) = context.segment.segments().iter().find(|s| {
                                    matches!(s.get_type(), SyntaxKind::NakedIdentifier | SyntaxKind::QuotedIdentifier)
                                }) {
                                    // Since the parser creates separate elements for identifier and = expression,
                                    // we need to look at the entire SELECT clause to detect the pattern
                                    let identifier_raw = identifier.raw();
                                    
                                    // Look in the entire SELECT clause for the pattern "identifier ="
                                    if let Some(id_pos) = select_raw.find(identifier_raw.as_str()) {
                                        let after_id = &select_raw[id_pos + identifier_raw.len()..];
                                        
                                        // Skip whitespace and check for =
                                        let trimmed = after_id.trim_start();
                                        if trimmed.starts_with('=') {
                                            // This is T-SQL alias syntax
                                            return Vec::new();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Original check for other cases
        // This was checking if the alias ends with "=" but that's too broad
        // It was incorrectly catching quoted identifiers like "example="

        self.base.eval(context)
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::AliasExpression]) }).into()
    }
}
