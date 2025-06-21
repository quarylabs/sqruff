use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintPhase, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};

fn get_trailing_newlines(segment: &ErasedSegment) -> Vec<ErasedSegment> {
    let mut result = Vec::new();
    let mut found_non_whitespace = false;

    for seg in segment.recursive_crawl_all(true) {
        // Skip meta segments
        if seg.is_type(SyntaxKind::Dedent) 
            || seg.is_type(SyntaxKind::EndOfFile)
            || seg.is_type(SyntaxKind::Indent) {
            continue;
        }
        
        if !found_non_whitespace {
            if seg.is_type(SyntaxKind::Newline) {
                result.push(seg.clone());
            } else if !seg.is_whitespace() {
                found_non_whitespace = true;
            }
        }
    }

    result
}


fn get_last_non_whitespace_segment(segment: &ErasedSegment) -> Option<ErasedSegment> {
    let mut last_non_whitespace = None;
    
    for seg in segment.recursive_crawl_all(true) {
        if !seg.is_whitespace() 
            && !seg.is_type(SyntaxKind::Newline) 
            && !seg.is_type(SyntaxKind::EndOfFile) 
            && !seg.is_type(SyntaxKind::Dedent)
            && !seg.is_type(SyntaxKind::Indent) {
            last_non_whitespace = Some(seg.clone());
        }
    }
    
    last_non_whitespace
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
        let trailing_newlines = Segments::from_vec(get_trailing_newlines(&context.segment), None);
        
        if trailing_newlines.is_empty() {
            // Find the last non-whitespace segment in the file
            let last_segment = get_last_non_whitespace_segment(&context.segment);
            
            if let Some(anchor) = last_segment {
                vec![LintResult::new(
                    anchor.clone().into(),
                    vec![LintFix::create_after(
                        anchor,
                        vec![SegmentBuilder::newline(context.tables.next_id(), "\n")],
                        None,
                    )],
                    None,
                    None,
                )]
            } else {
                // If we can't find a non-whitespace segment, just add newline at the end
                vec![LintResult::new(
                    context.segment.clone().into(),
                    vec![LintFix::create_after(
                        context.segment.clone(),
                        vec![SegmentBuilder::newline(context.tables.next_id(), "\n")],
                        None,
                    )],
                    None,
                    None,
                )]
            }
        } else if trailing_newlines.len() > 1 {
            // Find the first trailing newline to report the error on
            let first_newline = trailing_newlines.first().unwrap();
            vec![LintResult::new(
                first_newline.clone().into(),
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
