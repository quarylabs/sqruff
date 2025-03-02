use ahash::{AHashMap, AHashSet};
use itertools::{Itertools, chain};
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Default, Debug, Clone)]
pub struct RuleST02;

impl Rule for RuleST02 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST02.erased())
    }

    fn name(&self) -> &'static str {
        "structure.simple_case"
    }

    fn description(&self) -> &'static str {
        "Unnecessary 'CASE' statement."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

CASE statement returns booleans.

```sql
select
    case
        when fab > 0 then true
        else false
    end as is_fab
from fancy_table

-- This rule can also simplify CASE statements
-- that aim to fill NULL values.

select
    case
        when fab is null then 0
        else fab
    end as fab_clean
from fancy_table

-- This also covers where the case statement
-- replaces NULL values with NULL values.

select
    case
        when fab is null then null
        else fab
    end as fab_clean
from fancy_table
```

**Best practice**

Reduce to WHEN condition within COALESCE function.

```sql
select
    coalesce(fab > 0, false) as is_fab
from fancy_table

-- To fill NULL values.

select
    coalesce(fab, 0) as fab_clean
from fancy_table

-- NULL filling NULL.

select fab as fab_clean
from fancy_table
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.segment.segments()[0]
            .raw()
            .eq_ignore_ascii_case("CASE")
        {
            let children = FunctionalContext::new(context).segment().children(None);

            let when_clauses = children.select(
                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::WhenClause)),
                None,
                None,
                None,
            );
            let else_clauses = children.select(
                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::ElseClause)),
                None,
                None,
                None,
            );

            if when_clauses.len() > 1 {
                return Vec::new();
            }

            let condition_expression =
                when_clauses.children(Some(|it| it.is_type(SyntaxKind::Expression)))[0].clone();
            let then_expression =
                when_clauses.children(Some(|it| it.is_type(SyntaxKind::Expression)))[1].clone();

            if !else_clauses.is_empty() {
                if let Some(else_expression) = else_clauses
                    .children(Some(|it| it.is_type(SyntaxKind::Expression)))
                    .first()
                {
                    let upper_bools = ["TRUE", "FALSE"];

                    let then_expression_upper = then_expression.raw().to_uppercase_smolstr();
                    let else_expression_upper = else_expression.raw().to_uppercase_smolstr();

                    if upper_bools.contains(&then_expression_upper.as_str())
                        && upper_bools.contains(&else_expression_upper.as_str())
                        && then_expression_upper != else_expression_upper
                    {
                        let coalesce_arg_1 = condition_expression.clone();
                        let coalesce_arg_2 =
                            SegmentBuilder::keyword(context.tables.next_id(), "false");
                        let preceding_not = then_expression_upper == "FALSE";

                        let fixes = Self::coalesce_fix_list(
                            context,
                            coalesce_arg_1,
                            coalesce_arg_2,
                            preceding_not,
                        );

                        return vec![LintResult::new(
                            condition_expression.into(),
                            fixes,
                            "Unnecessary CASE statement. Use COALESCE function instead."
                                .to_owned()
                                .into(),
                            None,
                        )];
                    }
                }
            }

            let condition_expression_segments_raw: AHashSet<_> = AHashSet::from_iter(
                condition_expression
                    .segments()
                    .iter()
                    .map(|segment| segment.raw().to_uppercase_smolstr()),
            );

            if condition_expression_segments_raw.contains("IS")
                && condition_expression_segments_raw.contains("NULL")
                && condition_expression_segments_raw
                    .intersection(&AHashSet::from_iter(["AND".into(), "OR".into()]))
                    .next()
                    .is_none()
            {
                let is_not_prefix = condition_expression_segments_raw.contains("NOT");

                let tmp = Segments::new(condition_expression.clone(), None)
                    .children(Some(|it| it.is_type(SyntaxKind::ColumnReference)));

                let Some(column_reference_segment) = tmp.first() else {
                    return Vec::new();
                };

                let array_accessor_segment = Segments::new(condition_expression.clone(), None)
                    .children(Some(|it: &ErasedSegment| {
                        it.is_type(SyntaxKind::ArrayAccessor)
                    }))
                    .first()
                    .cloned();

                let column_reference_segment_raw_upper = match array_accessor_segment {
                    Some(array_accessor_segment) => {
                        column_reference_segment.raw().to_lowercase()
                            + &array_accessor_segment.raw().to_uppercase()
                    }
                    None => column_reference_segment.raw().to_uppercase(),
                };

                if !else_clauses.is_empty() {
                    let else_expression = else_clauses
                        .children(Some(|it| it.is_type(SyntaxKind::Expression)))[0]
                        .clone();

                    let (coalesce_arg_1, coalesce_arg_2) = if !is_not_prefix
                        && column_reference_segment_raw_upper
                            == else_expression.raw().to_uppercase_smolstr()
                    {
                        (else_expression, then_expression)
                    } else if is_not_prefix
                        && column_reference_segment_raw_upper
                            == then_expression.raw().to_uppercase_smolstr()
                    {
                        (then_expression, else_expression)
                    } else {
                        return Vec::new();
                    };

                    if coalesce_arg_2.raw().eq_ignore_ascii_case("NULL") {
                        let fixes =
                            Self::column_only_fix_list(context, column_reference_segment.clone());
                        return vec![LintResult::new(
                            condition_expression.into(),
                            fixes,
                            Some(String::new()),
                            None,
                        )];
                    }

                    let fixes =
                        Self::coalesce_fix_list(context, coalesce_arg_1, coalesce_arg_2, false);

                    return vec![LintResult::new(
                        condition_expression.into(),
                        fixes,
                        "Unnecessary CASE statement. Use COALESCE function instead."
                            .to_owned()
                            .into(),
                        None,
                    )];
                } else if column_reference_segment
                    .raw()
                    .eq_ignore_ascii_case(then_expression.raw())
                {
                    let fixes =
                        Self::column_only_fix_list(context, column_reference_segment.clone());

                    return vec![LintResult::new(
                        condition_expression.into(),
                        fixes,
                        format!(
                            "Unnecessary CASE statement. Just use column '{}'.",
                            column_reference_segment.raw()
                        )
                        .into(),
                        None,
                    )];
                }
            }

            Vec::new()
        } else {
            Vec::new()
        }
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::CaseExpression]) }).into()
    }
}

impl RuleST02 {
    fn coalesce_fix_list(
        context: &RuleContext,
        coalesce_arg_1: ErasedSegment,
        coalesce_arg_2: ErasedSegment,
        preceding_not: bool,
    ) -> Vec<LintFix> {
        let mut edits = vec![
            SegmentBuilder::token(
                context.tables.next_id(),
                "coalesce",
                SyntaxKind::FunctionNameIdentifier,
            )
            .finish(),
            SegmentBuilder::symbol(context.tables.next_id(), "("),
            coalesce_arg_1,
            SegmentBuilder::symbol(context.tables.next_id(), ","),
            SegmentBuilder::whitespace(context.tables.next_id(), " "),
            coalesce_arg_2,
            SegmentBuilder::symbol(context.tables.next_id(), ")"),
        ];

        if preceding_not {
            edits = chain(
                [
                    SegmentBuilder::keyword(context.tables.next_id(), "not"),
                    SegmentBuilder::whitespace(context.tables.next_id(), " "),
                ],
                edits,
            )
            .collect_vec();
        }

        vec![LintFix::replace(context.segment.clone(), edits, None)]
    }

    fn column_only_fix_list(
        context: &RuleContext,
        column_reference_segment: ErasedSegment,
    ) -> Vec<LintFix> {
        vec![LintFix::replace(
            context.segment.clone(),
            vec![column_reference_segment],
            None,
        )]
    }
}
