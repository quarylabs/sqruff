use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Default, Clone, Debug)]
pub struct RuleCV07;

impl Rule for RuleCV07 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV07.erased())
    }

    fn name(&self) -> &'static str {
        "convention.statement_brackets"
    }

    fn description(&self) -> &'static str {
        "Top-level statements should not be wrapped in brackets."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

A top-level statement is wrapped in brackets.

```sql
 (SELECT
     foo
 FROM bar)

 -- This also applies to statements containing a sub-query.

 (SELECT
     foo
 FROM (SELECT * FROM bar))
```

**Best practice**

Don’t wrap top-level statements in brackets.

```sql
 SELECT
     foo
 FROM bar

 -- Likewise for statements containing a sub-query.

 SELECT
     foo
 FROM (SELECT * FROM bar)
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        Self::iter_bracketed_statements(context.segment.clone())
            .into_iter()
            .map(|(parent, bracketed_segment)| {
                let bracket_set = [SyntaxKind::StartBracket, SyntaxKind::EndBracket];

                let filtered_children: Vec<ErasedSegment> = bracketed_segment
                    .segments()
                    .iter()
                    .filter(|&segment| {
                        !bracket_set.contains(&segment.get_type()) && !segment.is_meta()
                    })
                    .cloned()
                    .collect();

                // Lift leading/trailing whitespace and inline comments to the
                // segment above. This avoids introducing a parse error (ANSI and other
                // dialects generally don't allow this at lower levels of the parse
                // tree).
                let to_lift_predicate = |segment: &ErasedSegment| {
                    segment.is_type(SyntaxKind::Whitespace)
                        || segment.is_type(SyntaxKind::InlineComment)
                };
                let leading: Vec<_> = filtered_children
                    .iter()
                    .take_while(|s| to_lift_predicate(s))
                    .cloned()
                    .collect();
                let trailing: Vec<_> = filtered_children
                    .iter()
                    .rev()
                    .take_while(|s| to_lift_predicate(s))
                    .cloned()
                    .collect();

                let lift_nodes: Vec<_> = leading.iter().chain(trailing.iter()).cloned().collect();

                let (fixes, final_children) = if !lift_nodes.is_empty() {
                    let mut fixes = vec![
                        LintFix::create_before(parent.clone(), leading.clone()),
                        LintFix::create_after(parent, trailing.clone(), None),
                    ];
                    fixes.extend(lift_nodes.into_iter().map(LintFix::delete));
                    let trimmed = filtered_children
                        [leading.len()..filtered_children.len() - trailing.len()]
                        .to_vec();
                    (fixes, trimmed)
                } else {
                    (vec![], filtered_children)
                };

                let mut all_fixes = fixes;
                all_fixes.push(LintFix::replace(
                    bracketed_segment.clone(),
                    final_children,
                    None,
                ));

                LintResult::new(Some(bracketed_segment), all_fixes, None, None)
            })
            .collect()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}

impl RuleCV07 {
    fn iter_statements(file_segment: ErasedSegment) -> Vec<ErasedSegment> {
        file_segment
            .segments()
            .iter()
            .filter_map(|seg| {
                if seg.is_type(SyntaxKind::Batch) {
                    Some(
                        seg.segments()
                            .iter()
                            .filter(|seg| seg.is_type(SyntaxKind::Statement))
                            .cloned()
                            .collect::<Vec<_>>(),
                    )
                } else if seg.is_type(SyntaxKind::Statement) {
                    Some(vec![seg.clone()])
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }

    fn iter_bracketed_statements(
        file_segment: ErasedSegment,
    ) -> Vec<(ErasedSegment, ErasedSegment)> {
        Self::iter_statements(file_segment)
            .into_iter()
            .flat_map(|stmt| {
                stmt.segments()
                    .iter()
                    .filter_map(|seg| {
                        if seg.is_type(SyntaxKind::Bracketed) {
                            Some((stmt.clone(), seg.clone()))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
