use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug, Default)]
pub struct RuleTQ03;

impl Rule for RuleTQ03 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleTQ03.erased())
    }

    fn name(&self) -> &'static str {
        "tsql.empty_batch"
    }

    fn description(&self) -> &'static str {
        "Remove empty batches."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

Empty batches (containing only `GO` statements) should be removed.

```sql
CREATE TABLE dbo.test (
    testcol1 INT NOT NULL,
    testcol2 INT NOT NULL
);

GO

GO
```

**Best practice**

Remove empty batches.

```sql
CREATE TABLE dbo.test (
    testcol1 INT NOT NULL,
    testcol2 INT NOT NULL
);

GO
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Tsql]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.dialect.name != DialectKind::Tsql || !context.segment.is_type(SyntaxKind::Batch)
        {
            return Vec::new();
        }

        let mut has_go = false;
        for segment in context
            .segment
            .segments()
            .iter()
            .filter(|segment| !segment.is_whitespace() && !segment.is_meta())
        {
            if segment.is_type(SyntaxKind::GoStatement) {
                has_go = true;
            } else {
                return Vec::new();
            }
        }

        if !has_go {
            return Vec::new();
        }

        let mut fixes = vec![LintFix::delete(context.segment.clone())];
        if let Some(parent) = context.parent_stack.last()
            && let Some(next_segment) = parent.segments().get(context.segment_idx + 1)
            && next_segment.is_type(SyntaxKind::Newline)
        {
            fixes.push(LintFix::delete(next_segment.clone()));
        }

        vec![LintResult::new(
            Some(context.segment.clone()),
            fixes,
            Some("Empty batch with only GO statement should be removed.".into()),
            None,
        )]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Batch]) }).into()
    }
}
