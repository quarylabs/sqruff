use std::borrow::Cow;

use ahash::AHashMap;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::reflow::sequence::{Filter, ReflowSequence, TargetSide};

#[derive(Debug)]
enum CorrectionListItem {
    WhitespaceSegment,
    KeywordSegment(String),
}

type CorrectionList = Vec<CorrectionListItem>;

#[derive(Default, Clone, Debug)]
pub struct RuleCV05;

fn create_base_is_null_sequence(is_upper: bool, operator_raw: Cow<str>) -> CorrectionList {
    let is_seg = CorrectionListItem::KeywordSegment(if is_upper { "IS" } else { "is" }.to_string());
    let not_seg =
        CorrectionListItem::KeywordSegment(if is_upper { "NOT" } else { "not" }.to_string());

    if operator_raw == "=" {
        vec![is_seg]
    } else {
        vec![is_seg, CorrectionListItem::WhitespaceSegment, not_seg]
    }
}

impl Rule for RuleCV05 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV05.erased())
    }

    fn name(&self) -> &'static str {
        "convention.is_null"
    }

    fn description(&self) -> &'static str {
        "Relational operators should not be used to check for NULL values."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

In this example, the `=` operator is used to check for `NULL` values.

```sql
SELECT
    a
FROM foo
WHERE a = NULL
```

**Best practice**

Use `IS` or `IS NOT` to check for `NULL` values.

```sql
SELECT
    a
FROM foo
WHERE a IS NULL
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.parent_stack.len() >= 2 {
            for type_str in [
                SyntaxKind::SetClauseList,
                SyntaxKind::ExecuteScriptStatement,
                SyntaxKind::OptionsSegment,
            ] {
                if context.parent_stack[context.parent_stack.len() - 2].is_type(type_str) {
                    return Vec::new();
                }
            }
        }

        if !context.parent_stack.is_empty() {
            for type_str in [
                SyntaxKind::SetClauseList,
                SyntaxKind::ExecuteScriptStatement,
                SyntaxKind::AssignmentOperator,
            ] {
                if context.parent_stack[context.parent_stack.len() - 1].is_type(type_str) {
                    return Vec::new();
                }
            }
        }

        if !context.parent_stack.is_empty()
            && context.parent_stack[context.parent_stack.len() - 1]
                .is_type(SyntaxKind::ExclusionConstraintElement)
        {
            return Vec::new();
        }

        let raw_consist = context.segment.raw();
        if !["=", "!=", "<>"].contains(&raw_consist.as_str()) {
            return Vec::new();
        }

        let segment = context.parent_stack.last().unwrap().segments().to_vec();

        let siblings = Segments::from_vec(segment, None);
        let after_op_list =
            siblings.select::<fn(&ErasedSegment) -> bool>(None, None, Some(&context.segment), None);

        let next_code = after_op_list.find_first(Some(|sp: &ErasedSegment| sp.is_code()));

        if !next_code.all(Some(|it| it.is_type(SyntaxKind::NullLiteral))) {
            return Vec::new();
        }

        let sub_seg = next_code.get(0, None);
        let edit = create_base_is_null_sequence(
            sub_seg.as_ref().unwrap().raw().starts_with('N'),
            context.segment.raw().as_str().into(),
        );

        let mut seg = Vec::with_capacity(edit.len());

        for item in edit {
            match item {
                CorrectionListItem::KeywordSegment(keyword) => {
                    seg.push(SegmentBuilder::keyword(context.tables.next_id(), &keyword));
                }
                CorrectionListItem::WhitespaceSegment => {
                    seg.push(SegmentBuilder::whitespace(context.tables.next_id(), " "));
                }
            };
        }

        let fixes = ReflowSequence::from_around_target(
            &context.segment,
            context.parent_stack[0].clone(),
            TargetSide::Both,
            context.config,
        )
        .replace(context.segment.clone(), &seg)
        .respace(context.tables, false, Filter::All)
        .fixes();

        vec![LintResult::new(
            Some(context.segment.clone()),
            fixes,
            None,
            None,
        )]
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::ComparisonOperator]) })
            .into()
    }
}
