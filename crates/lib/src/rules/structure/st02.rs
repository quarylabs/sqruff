use hashbrown::HashMap;
use itertools::{Itertools, chain};
use smol_str::StrExt;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::parsers::CaseFold;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder};
use sqruff_lib_core::utils::functional::segments::Segments;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::functional::context::FunctionalContext;

#[derive(PartialEq, Eq)]
enum SegmentIdentity {
    Node(SyntaxKind, Vec<SegmentIdentity>),
    Leaf(SyntaxKind, String),
}

#[derive(Default, Debug, Clone)]
pub struct RuleST02;

impl Rule for RuleST02 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
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

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        if context.segment.segments()[0]
            .raw()
            .eq_ignore_ascii_case("CASE")
        {
            let children = FunctionalContext::new(context).segment().children_all();

            let when_clauses =
                children.filter(|it: &ErasedSegment| it.is_type(SyntaxKind::WhenClause));
            let else_clauses =
                children.filter(|it: &ErasedSegment| it.is_type(SyntaxKind::ElseClause));

            // Skip simple CASE statements (`CASE x WHEN y THEN ...`). Only searched CASE
            // statements (`CASE WHEN x = y THEN ...`) can be simplified by this rule.
            for child in children.iter() {
                if child.is_type(SyntaxKind::WhenClause) {
                    break;
                }
                if child.is_type(SyntaxKind::Expression) {
                    return Vec::new();
                }
            }

            if when_clauses.len() > 1 {
                return Vec::new();
            }

            let condition_expression =
                when_clauses.children_where(|it| it.is_type(SyntaxKind::Expression))[0].clone();
            let then_expression =
                when_clauses.children_where(|it| it.is_type(SyntaxKind::Expression))[1].clone();

            if !else_clauses.is_empty()
                && let Some(else_expression) = else_clauses
                    .children_where(|it| it.is_type(SyntaxKind::Expression))
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
                    let coalesce_arg_2 = SegmentBuilder::keyword(context.tables.next_id(), "false");
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

            if let Some(is_segment_index) = condition_expression
                .segments()
                .iter()
                .position(|segment| segment.raw().eq_ignore_ascii_case("IS"))
            {
                let tmp = Segments::new(condition_expression.clone(), None)
                    .children_where(|it| it.is_type(SyntaxKind::ColumnReference));

                let Some(column_reference_segment) = tmp.first() else {
                    return Vec::new();
                };

                let condition_operand_segments = condition_expression.segments()
                    [..is_segment_index]
                    .iter()
                    .filter(|segment| !segment.is_whitespace() && !segment.is_meta())
                    .collect_vec();
                if condition_operand_segments.is_empty()
                    || condition_operand_segments.iter().any(|segment| {
                        !segment.is_type(SyntaxKind::ColumnReference)
                            && !segment.is_type(SyntaxKind::ArrayAccessor)
                    })
                {
                    return Vec::new();
                }

                let comparison_segments = condition_expression.segments()[is_segment_index + 1..]
                    .iter()
                    .filter(|segment| !segment.is_whitespace() && !segment.is_meta())
                    .collect_vec();
                let is_not_prefix = match comparison_segments.as_slice() {
                    [null] if null.raw().eq_ignore_ascii_case("NULL") => false,
                    [not, null]
                        if not.raw().eq_ignore_ascii_case("NOT")
                            && null.raw().eq_ignore_ascii_case("NULL") =>
                    {
                        true
                    }
                    _ => return Vec::new(),
                };

                if condition_operand_segments.iter().any(|segment| {
                    segment.is_type(SyntaxKind::ArrayAccessor)
                        && !Self::is_static_array_accessor(segment)
                }) {
                    return Vec::new();
                }

                let casefold = context
                    .dialect
                    .grammar("NakedIdentifierSegment")
                    .as_regex()
                    .and_then(|parser| parser.casefold);
                let condition_operand_identity = condition_operand_segments
                    .iter()
                    .map(|segment| Self::segment_identity(segment, casefold))
                    .collect_vec();

                if !else_clauses.is_empty() {
                    let else_expression = else_clauses
                        .children_where(|it| it.is_type(SyntaxKind::Expression))[0]
                        .clone();

                    let (coalesce_arg_1, coalesce_arg_2) = if !is_not_prefix
                        && condition_operand_identity
                            == Self::expression_identity(&else_expression, casefold)
                    {
                        (else_expression, then_expression)
                    } else if is_not_prefix
                        && condition_operand_identity
                            == Self::expression_identity(&then_expression, casefold)
                    {
                        (then_expression, else_expression)
                    } else {
                        return Vec::new();
                    };

                    if coalesce_arg_2.raw().eq_ignore_ascii_case("NULL") {
                        let fixes = Self::column_only_fix_list(context, coalesce_arg_1);
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
                } else if is_not_prefix
                    && condition_operand_identity
                        == Self::expression_identity(&then_expression, casefold)
                {
                    let fixes = Self::column_only_fix_list(context, then_expression);

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
    fn segment_identity(segment: &ErasedSegment, casefold: Option<CaseFold>) -> SegmentIdentity {
        if segment.segments().is_empty() {
            let normalized = segment.raw_normalized().to_string();
            let normalized = if matches!(
                segment.get_type(),
                SyntaxKind::NakedIdentifier
                    | SyntaxKind::NakedIdentifierAll
                    | SyntaxKind::Identifier
                    | SyntaxKind::PropertiesNakedIdentifier
            ) {
                casefold.map_or_else(|| normalized.clone(), |fold| fold.apply(&normalized))
            } else {
                normalized
            };
            SegmentIdentity::Leaf(segment.get_type(), normalized)
        } else {
            SegmentIdentity::Node(
                segment.get_type(),
                segment
                    .segments()
                    .iter()
                    .filter(|child| !child.is_whitespace() && !child.is_meta())
                    .map(|child| Self::segment_identity(child, casefold))
                    .collect(),
            )
        }
    }

    fn expression_identity(
        expression: &ErasedSegment,
        casefold: Option<CaseFold>,
    ) -> Vec<SegmentIdentity> {
        expression
            .segments()
            .iter()
            .filter(|segment| !segment.is_whitespace() && !segment.is_meta())
            .map(|segment| Self::segment_identity(segment, casefold))
            .collect()
    }

    fn is_static_array_accessor(accessor: &ErasedSegment) -> bool {
        accessor.recursive_crawl_all(false).iter().all(|child| {
            !child.is_code()
                || matches!(
                    child.get_type(),
                    SyntaxKind::ArrayAccessor
                        | SyntaxKind::StartSquareBracket
                        | SyntaxKind::EndSquareBracket
                        | SyntaxKind::NumericLiteral
                        | SyntaxKind::IntegerLiteral
                        | SyntaxKind::QuotedLiteral
                        | SyntaxKind::Literal
                )
        })
    }

    fn coalesce_fix_list(
        context: &RuleContext,
        coalesce_arg_1: ErasedSegment,
        coalesce_arg_2: ErasedSegment,
        preceding_not: bool,
    ) -> Vec<LintFix> {
        let function_name = SegmentBuilder::node(
            context.tables.next_id(),
            SyntaxKind::FunctionName,
            context.dialect.name,
            vec![
                SegmentBuilder::token(
                    context.tables.next_id(),
                    "coalesce",
                    SyntaxKind::FunctionNameIdentifier,
                )
                .finish(),
            ],
        )
        .finish();
        let function_contents = SegmentBuilder::node(
            context.tables.next_id(),
            SyntaxKind::FunctionContents,
            context.dialect.name,
            vec![
                SegmentBuilder::token(context.tables.next_id(), "(", SyntaxKind::StartBracket)
                    .finish(),
                coalesce_arg_1,
                SegmentBuilder::comma(context.tables.next_id()),
                SegmentBuilder::whitespace(context.tables.next_id(), " "),
                coalesce_arg_2,
                SegmentBuilder::token(context.tables.next_id(), ")", SyntaxKind::EndBracket)
                    .finish(),
            ],
        )
        .finish();
        let mut edits = vec![function_name, function_contents];

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
        replacement_segment: ErasedSegment,
    ) -> Vec<LintFix> {
        vec![LintFix::replace(
            context.segment.clone(),
            vec![replacement_segment],
            None,
        )]
    }
}
