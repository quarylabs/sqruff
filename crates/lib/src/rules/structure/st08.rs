use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;
use crate::utils::reflow::sequence::{Filter, ReflowSequence, TargetSide};

#[derive(Debug, Default, Clone)]
pub struct RuleST08;

impl RuleST08 {
    pub fn remove_unneeded_brackets<'a>(
        &self,
        context: &RuleContext<'a>,
        bracketed: Segments,
    ) -> (ErasedSegment, ReflowSequence<'a>) {
        let anchor = &bracketed.get(0, None).unwrap();
        let seq = ReflowSequence::from_around_target(
            anchor,
            context.parent_stack[0].clone(),
            TargetSide::Before,
            context.config,
        )
        .replace(
            anchor.clone(),
            &Self::filter_meta(&anchor.segments()[1..anchor.segments().len() - 1], false),
        );

        (anchor.clone(), seq)
    }

    pub fn filter_meta(segments: &[ErasedSegment], keep_meta: bool) -> Vec<ErasedSegment> {
        segments
            .iter()
            .filter(|&elem| elem.is_meta() == keep_meta)
            .cloned()
            .collect()
    }
}

impl Rule for RuleST08 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST08.erased())
    }

    fn name(&self) -> &'static str {
        "structure.distinct"
    }

    fn description(&self) -> &'static str {
        "Looking for DISTINCT before a bracket"
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, parentheses are not needed and confuse DISTINCT with a function. The parentheses can also be misleading about which columns are affected by the DISTINCT (all the columns!).

```sql
SELECT DISTINCT(a), b FROM foo
```

**Best practice**

Remove parentheses to be clear that the DISTINCT applies to both columns.

```sql
SELECT DISTINCT a, b FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut seq: Option<ReflowSequence> = None;
        let mut anchor: Option<ErasedSegment> = None;
        let children = FunctionalContext::new(context).segment().children(None);

        if context.segment.is_type(SyntaxKind::SelectClause) {
            let modifier = children.select(
                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::SelectClauseModifier)),
                None,
                None,
                None,
            );
            let selected_elements = children.select(
                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::SelectClauseElement)),
                None,
                None,
                None,
            );
            let first_element = selected_elements.find_first::<fn(&_) -> _>(None);
            let expression = first_element
                .children(Some(|it| it.is_type(SyntaxKind::Expression)))
                .find_first::<fn(&ErasedSegment) -> bool>(None);
            let expression = if expression.is_empty() {
                first_element
            } else {
                expression
            };
            let bracketed = expression
                .children(Some(|it| it.get_type() == SyntaxKind::Bracketed))
                .find_first::<fn(&_) -> _>(None);

            if !modifier.is_empty() && !bracketed.is_empty() {
                if expression[0].segments().len() == 1 {
                    let ret = self.remove_unneeded_brackets(context, bracketed);
                    anchor = ret.0.into();
                    seq = ret.1.into();
                } else {
                    anchor = Some(modifier[0].clone());
                    seq = Some(ReflowSequence::from_around_target(
                        &modifier[0],
                        context.parent_stack[0].clone(),
                        TargetSide::After,
                        context.config,
                    ));
                }
            }
        } else if context.segment.is_type(SyntaxKind::Function) {
            let anchor = context.parent_stack.last().unwrap();

            if !anchor.is_type(SyntaxKind::Expression) || anchor.segments().len() != 1 {
                return Vec::new();
            }

            let selected_functions = children.select(
                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::FunctionName)),
                None,
                None,
                None,
            );
            let function_name = selected_functions.first();
            let bracketed =
                children.find_first(Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)));

            if function_name.is_none()
                || !function_name
                    .unwrap()
                    .raw()
                    .eq_ignore_ascii_case("DISTINCT")
                || bracketed.is_empty()
            {
                return Vec::new();
            }

            let bracketed = &bracketed[0];
            let mut edits = vec![
                SegmentBuilder::token(
                    context.tables.next_id(),
                    "DISTINCT",
                    SyntaxKind::FunctionNameIdentifier,
                )
                .finish(),
                SegmentBuilder::whitespace(context.tables.next_id(), " "),
            ];
            edits.extend(Self::filter_meta(
                &bracketed.segments()[1..bracketed.segments().len() - 1],
                false,
            ));

            return vec![LintResult::new(
                anchor.clone().into(),
                vec![LintFix::replace(anchor.clone(), edits, None)],
                None,
                None,
            )];
        }

        if let Some(seq) = seq {
            if let Some(anchor) = anchor {
                let fixes = seq.respace(context.tables, false, Filter::All).fixes();

                if !fixes.is_empty() {
                    return vec![LintResult::new(Some(anchor), fixes, None, None)];
                }
            }
        }

        Vec::new()
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::SelectClause, SyntaxKind::Function]) },
        )
        .into()
    }
}
