use hashbrown::HashMap;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Debug, Default, Clone)]
pub struct RuleST12;

fn is_semicolon(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::StatementTerminator | SyntaxKind::Semicolon
    )
}

impl Rule for RuleST12 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST12.erased())
    }

    fn name(&self) -> &'static str {
        "structure.consecutive_semicolons"
    }

    fn description(&self) -> &'static str {
        "Remove consecutive semicolons."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Multiple semicolons in a row, with only whitespace between them.

```sql
SELECT 1;;
```

**Best practice**

Use only a single semicolon.

```sql
SELECT 1;
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let all_segments: Vec<_> = context
            .segment
            .recursive_crawl_all(false)
            .into_iter()
            .filter(|seg| seg.segments().is_empty())
            .collect();

        let mut results = Vec::new();
        let mut i = 0;

        while i < all_segments.len() {
            if !is_semicolon(all_segments[i].get_type()) {
                i += 1;
                continue;
            }

            let first_term = i;
            i += 1;

            let mut fixes = Vec::new();
            loop {
                let ws_start = i;
                while i < all_segments.len()
                    && matches!(
                        all_segments[i].get_type(),
                        SyntaxKind::Whitespace
                            | SyntaxKind::Newline
                            | SyntaxKind::Indent
                            | SyntaxKind::Dedent
                    )
                {
                    i += 1;
                }

                if i < all_segments.len() && is_semicolon(all_segments[i].get_type()) {
                    for seg in &all_segments[ws_start..i] {
                        if !seg.is_meta() {
                            fixes.push(LintFix::delete(seg.clone()));
                        }
                    }
                    fixes.push(LintFix::delete(all_segments[i].clone()));
                    i += 1;
                } else {
                    break;
                }
            }

            if !fixes.is_empty() {
                results.push(LintResult::new(
                    all_segments[first_term].clone().into(),
                    fixes,
                    None,
                    None,
                ));
            }
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}
