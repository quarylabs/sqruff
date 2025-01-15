use ahash::{AHashMap, AHashSet};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Default, Clone)]
pub struct RuleLT07;

impl Rule for RuleLT07 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT07.erased())
    }
    fn name(&self) -> &'static str {
        "layout.cte_bracket"
    }

    fn description(&self) -> &'static str {
        "'WITH' clause closing bracket should be on a new line."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the closing bracket is on the same line as CTE.

```sql
 WITH zoo AS (
     SELECT a FROM foo)

 SELECT * FROM zoo
```

**Best practice**

Move the closing bracket on a new line.

```sql
WITH zoo AS (
    SELECT a FROM foo
)

SELECT * FROM zoo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }
    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let segments = FunctionalContext::new(context)
            .segment()
            .children(Some(|seg| seg.is_type(SyntaxKind::CommonTableExpression)));

        let mut cte_end_brackets = AHashSet::new();
        for cte in segments.iterate_segments() {
            let cte_start_bracket = cte
                .children(None)
                .find_last(Some(|seg| seg.is_type(SyntaxKind::Bracketed)))
                .children(None)
                .find_first(Some(|seg: &ErasedSegment| {
                    seg.is_type(SyntaxKind::StartBracket)
                }));

            let cte_end_bracket = cte
                .children(None)
                .find_last(Some(|seg| seg.is_type(SyntaxKind::Bracketed)))
                .children(None)
                .find_first(Some(|seg: &ErasedSegment| {
                    seg.is_type(SyntaxKind::EndBracket)
                }));

            if !cte_start_bracket.is_empty() && !cte_end_bracket.is_empty() {
                if cte_start_bracket[0]
                    .get_position_marker()
                    .unwrap()
                    .line_no()
                    == cte_end_bracket[0].get_position_marker().unwrap().line_no()
                {
                    continue;
                }
                cte_end_brackets.insert(cte_end_bracket[0].clone());
            }
        }

        for seg in cte_end_brackets {
            let mut contains_non_whitespace = false;
            let idx = context
                .segment
                .get_raw_segments()
                .iter()
                .position(|it| it == &seg)
                .unwrap();
            if idx > 0 {
                for elem in context.segment.get_raw_segments()[..idx].iter().rev() {
                    if elem.is_type(SyntaxKind::Newline) {
                        break;
                    } else if !(matches!(
                        elem.get_type(),
                        SyntaxKind::Indent | SyntaxKind::Implicit
                    ) || elem.is_type(SyntaxKind::Dedent)
                        || elem.is_type(SyntaxKind::Whitespace))
                    {
                        contains_non_whitespace = true;
                        break;
                    }
                }
            }

            if contains_non_whitespace {
                return vec![LintResult::new(
                    seg.clone().into(),
                    vec![LintFix::create_before(
                        seg,
                        vec![SegmentBuilder::newline(context.tables.next_id(), "\n")],
                    )],
                    None,
                    None,
                )];
            }
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        false
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::WithCompoundStatement]) })
            .into()
    }
}
