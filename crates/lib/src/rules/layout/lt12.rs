use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintPhase, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::utils::functional::context::FunctionalContext;

fn get_trailing_newlines(segment: &ErasedSegment) -> Vec<ErasedSegment> {
    let mut result = Vec::new();

    for seg in segment.recursive_crawl_all(true) {
        if seg.is_type(SyntaxKind::Newline) {
            result.push(seg.clone());
        } else if !seg.is_whitespace()
            && !seg.is_type(SyntaxKind::Dedent)
            && !seg.is_type(SyntaxKind::EndOfFile)
        {
            break;
        }
    }

    result
}

fn get_last_segment(mut segment: Segments) -> (Vec<ErasedSegment>, Segments) {
    let mut parent_stack = Vec::new();

    loop {
        let children = segment.children(None);

        if !children.is_empty() {
            parent_stack.push(segment.first().unwrap().clone());
            segment = children.find_last(Some(|s| !s.is_type(SyntaxKind::EndOfFile)));
        } else {
            return (parent_stack, segment);
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RuleLT12;

impl Rule for RuleLT12 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT12.erased())
    }
    fn lint_phase(&self) -> LintPhase {
        LintPhase::Post
    }

    fn name(&self) -> &'static str {
        "layout.end_of_file"
    }

    fn description(&self) -> &'static str {
        "Files must end with a single trailing newline."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

The content in file does not end with a single trailing newline. The $ represents end of file.

```sql
 SELECT
     a
 FROM foo$

 -- Ending on an indented line means there is no newline
 -- at the end of the file, the • represents space.

 SELECT
 ••••a
 FROM
 ••••foo
 ••••$

 -- Ending on a semi-colon means the last line is not a
 -- newline.

 SELECT
     a
 FROM foo
 ;$

 -- Ending with multiple newlines.

 SELECT
     a
 FROM foo

 $
```

**Best practice**

Add trailing newline to the end. The $ character represents end of file.

```sql
 SELECT
     a
 FROM foo
 $

 -- Ensuring the last line is not indented so is just a
 -- newline.

 SELECT
 ••••a
 FROM
 ••••foo
 $

 -- Even when ending on a semi-colon, ensure there is a
 -- newline after.

 SELECT
     a
 FROM foo
 ;
 $
```
"#
    }
    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let (parent_stack, segment) = get_last_segment(FunctionalContext::new(context).segment());

        if segment.is_empty() {
            return Vec::new();
        }

        let trailing_newlines = Segments::from_vec(get_trailing_newlines(&context.segment), None);
        if trailing_newlines.is_empty() {
            let fix_anchor_segment = if parent_stack.len() == 1 {
                segment.first().unwrap().clone()
            } else {
                parent_stack[1].clone()
            };

            vec![LintResult::new(
                segment.first().unwrap().clone().into(),
                vec![LintFix::create_after(
                    fix_anchor_segment,
                    vec![SegmentBuilder::newline(context.tables.next_id(), "\n")],
                    None,
                )],
                None,
                None,
            )]
        } else if trailing_newlines.len() > 1 {
            vec![LintResult::new(
                segment.first().unwrap().clone().into(),
                trailing_newlines
                    .into_iter()
                    .skip(1)
                    .map(|d| LintFix::delete(d.clone()))
                    .collect(),
                None,
                None,
            )]
        } else {
            vec![]
        }
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}
