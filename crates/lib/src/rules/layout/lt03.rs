use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::{ReflowSequence, TargetSide};

#[derive(Debug, Default, Clone)]
pub struct RuleLT03;

impl Rule for RuleLT03 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT03.erased())
    }
    fn name(&self) -> &'static str {
        "layout.operators"
    }

    fn description(&self) -> &'static str {
        "Operators should follow a standard for being before/after newlines."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, if line_position = leading (or unspecified, as is the default), then the operator + should not be at the end of the second line.

```sql
SELECT
    a +
    b
FROM foo
```

**Best practice**

If line_position = leading (or unspecified, as this is the default), place the operator after the newline.

```sql
SELECT
    a
    + b
FROM foo
```

If line_position = trailing, place the operator before the newline.

```sql
SELECT
    a +
    b
FROM foo
```
"#
    }
    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.segment.is_type(SyntaxKind::ComparisonOperator) {
            let comparison_positioning =
                context.config.raw["layout"]["type"]["comparison_operator"]["line_position"]
                    .as_string()
                    .unwrap();

            if self.check_trail_lead_shortcut(
                &context.segment,
                context.parent_stack.last().unwrap(),
                comparison_positioning,
            ) {
                return vec![LintResult::new(None, Vec::new(), None, None)];
            }
        } else if context.segment.is_type(SyntaxKind::BinaryOperator) {
            let binary_positioning =
                context.config.raw["layout"]["type"]["binary_operator"]["line_position"]
                    .as_string()
                    .unwrap();

            if self.check_trail_lead_shortcut(
                &context.segment,
                context.parent_stack.last().unwrap(),
                binary_positioning,
            ) {
                return vec![LintResult::new(None, Vec::new(), None, None)];
            }
        }

        ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack.first().unwrap().clone(),
            TargetSide::Both,
            context.config,
        )
        .rebreak(context.tables)
        .results()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::BinaryOperator, SyntaxKind::ComparisonOperator]) },
        )
        .into()
    }
}

impl RuleLT03 {
    pub(crate) fn check_trail_lead_shortcut(
        &self,
        segment: &ErasedSegment,
        parent: &ErasedSegment,
        line_position: &str,
    ) -> bool {
        let idx = parent
            .segments()
            .iter()
            .position(|it| it == segment)
            .unwrap();

        // Shortcut #1: Leading.
        if line_position == "leading" {
            if self.seek_newline(parent.segments(), idx, Direction::Backward) {
                return true;
            }
            // If we didn't find a newline before, if there's _also_ not a newline
            // after, then we can also shortcut. i.e., it's a comma "mid line".
            if !self.seek_newline(parent.segments(), idx, Direction::Forward) {
                return true;
            }
        }
        // Shortcut #2: Trailing.
        else if line_position == "trailing" {
            if self.seek_newline(parent.segments(), idx, Direction::Forward) {
                return true;
            }
            // If we didn't find a newline after, if there's _also_ not a newline
            // before, then we can also shortcut. i.e., it's a comma "mid line".
            if !self.seek_newline(parent.segments(), idx, Direction::Backward) {
                return true;
            }
        }

        false
    }

    fn seek_newline(&self, segments: &[ErasedSegment], idx: usize, direction: Direction) -> bool {
        let segments: &mut dyn Iterator<Item = _> = match direction {
            Direction::Forward => &mut segments[idx + 1..].iter(),
            Direction::Backward => &mut segments.iter().take(idx).rev(),
        };

        for segment in segments {
            if segment.is_type(SyntaxKind::Newline) {
                return true;
            } else if !segment.is_type(SyntaxKind::Whitespace)
                && !segment.is_type(SyntaxKind::Indent)
                && !segment.is_type(SyntaxKind::Implicit)
                && !segment.is_type(SyntaxKind::Comment)
                && !segment.is_type(SyntaxKind::InlineComment)
                && !segment.is_type(SyntaxKind::BlockComment)
            {
                break;
            }
        }

        false
    }
}

#[derive(Debug)]
enum Direction {
    Forward,
    Backward,
}
