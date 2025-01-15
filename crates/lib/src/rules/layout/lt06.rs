use ahash::AHashMap;
use itertools::Itertools;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT06;

impl Rule for RuleLT06 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT06.erased())
    }
    fn name(&self) -> &'static str {
        "layout.functions"
    }

    fn description(&self) -> &'static str {
        "Function name not immediately followed by parenthesis."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, there is a space between the function and the parenthesis.

```sql
SELECT
    sum (a)
FROM foo
```

**Best practice**

Remove the space between the function and the parenthesis.

```sql
SELECT
    sum(a)
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let segment = FunctionalContext::new(context).segment();
        let children = segment.children(None);

        let function_name = children
            .find_first(Some(|segment: &ErasedSegment| {
                segment.is_type(SyntaxKind::FunctionName)
            }))
            .pop();
        let start_bracket = children
            .find_first(Some(|segment: &ErasedSegment| {
                segment.is_type(SyntaxKind::Bracketed)
            }))
            .pop();

        let mut intermediate_segments = children.select::<fn(&ErasedSegment) -> bool>(
            None,
            None,
            Some(&function_name),
            Some(&start_bracket),
        );

        if !intermediate_segments.is_empty() {
            return if intermediate_segments.all(Some(|seg| {
                matches!(seg.get_type(), SyntaxKind::Whitespace | SyntaxKind::Newline)
            })) {
                vec![LintResult::new(
                    intermediate_segments.first().cloned(),
                    intermediate_segments
                        .into_iter()
                        .map(LintFix::delete)
                        .collect_vec(),
                    None,
                    None,
                )]
            } else {
                vec![LintResult::new(
                    intermediate_segments.pop().into(),
                    vec![],
                    None,
                    None,
                )]
            };
        }

        vec![]
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::Function]) }).into()
    }
}
