use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};

use super::cp01::handle_segment;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{LintResult, Rule, RuleGroups};

#[derive(Debug, Default, Clone)]
pub struct RuleCP05;

impl Rule for RuleCP05 {
    fn name(&self) -> &'static str {
        "capitalisation.types"
    }

    fn description(&self) -> &'static str {
        "Inconsistent capitalisation of datatypes."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, `int` and `unsigned` are in lower-case whereas `VARCHAR` is in upper-case.

```sql
CREATE TABLE t (
    a int unsigned,
    b VARCHAR(15)
);
```

**Best practice**

Ensure all datatypes are consistently upper or lower case

```sql
CREATE TABLE t (
    a INT UNSIGNED,
    b VARCHAR(15)
);
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[
            RuleGroups::All,
            RuleGroups::Core,
            RuleGroups::Capitalisation,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let extended_capitalisation_policy = context
            .config
            .rules
            .capitalisation_types
            .extended_capitalisation_policy
            .as_str();
        let mut results = Vec::new();

        if context.segment.is_type(SyntaxKind::PrimitiveType)
            || context.segment.is_type(SyntaxKind::DatetimeTypeIdentifier)
            || context.segment.is_type(SyntaxKind::DataType)
        {
            for seg in context.segment.segments() {
                if seg.is_type(SyntaxKind::Symbol)
                    || seg.is_type(SyntaxKind::Identifier)
                    || seg.is_type(SyntaxKind::QuotedLiteral)
                    || !seg.segments().is_empty()
                {
                    continue;
                }

                results.push(handle_segment(
                    "Datatypes",
                    extended_capitalisation_policy,
                    "extended_capitalisation_policy",
                    seg.clone(),
                    context,
                ));
            }
        }

        results
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const {
                SyntaxSet::new(&[
                    SyntaxKind::DataTypeIdentifier,
                    SyntaxKind::PrimitiveType,
                    SyntaxKind::DatetimeTypeIdentifier,
                    SyntaxKind::DataType,
                ])
            },
        )
        .into()
    }
}
