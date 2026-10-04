use std::iter::zip;

use hashbrown::HashMap;
use itertools::{Itertools, enumerate};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased as _, ErasedRule, LintResult, Rule, RuleGroups};

#[derive(Clone, Debug)]
pub struct RuleST06;

impl Rule for RuleST06 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleST06.erased())
    }

    fn name(&self) -> &'static str {
        "structure.column_order"
    }

    fn description(&self) -> &'static str {
        "Select wildcards then simple targets before calculations and aggregates."
    }

    fn long_description(&self) -> &'static str {
        r"
**Anti-pattern**

```sql
select
    a,
    *,
    row_number() over (partition by id order by date) as y,
    b
from x
```

**Best practice**

Order `select` targets in ascending complexity

```sql
select
    *,
    a,
    b,
    row_number() over (partition by id order by date) as y
from x
```"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Structure]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let mut violation_exists = false;

        static SELECT_ELEMENT_ORDER_PREFERENCE: &[&[Validate]] = &[
            &[Validate::Types(
                const { SyntaxSet::new(&[SyntaxKind::WildcardExpression]) },
            )],
            &[
                Validate::Types(
                    const { SyntaxSet::new(&[SyntaxKind::ObjectReference, SyntaxKind::ColumnReference]) },
                ),
                Validate::Types(const { SyntaxSet::new(&[SyntaxKind::Literal]) }),
                Validate::Types(const { SyntaxSet::new(&[SyntaxKind::CastExpression]) }),
                Validate::Function,
                Validate::Expression {
                    child_typ: SyntaxKind::CastExpression,
                },
            ],
        ];

        // Inserts, merges, CREATE TABLE statements, and set expressions are
        // order-sensitive regardless of how deeply the SELECT is nested.
        if context.parent_stack.iter().rev().any(|segment| {
            matches!(
                segment.get_type(),
                SyntaxKind::InsertStatement
                    | SyntaxKind::SetExpression
                    | SyntaxKind::CreateTableStatement
                    | SyntaxKind::MergeStatement
            )
        }) {
            return Vec::new();
        }

        // A CTE is order-sensitive when it is referenced by SELECT * in a set
        // expression, because reordering its columns changes union semantics.
        for (cte_index, cte) in context.parent_stack.iter().enumerate().rev() {
            if !cte.is_type(SyntaxKind::CommonTableExpression) {
                continue;
            }

            let cte_identifier = cte
                .child(
                    const {
                        &SyntaxSet::new(&[
                            SyntaxKind::Identifier,
                            SyntaxKind::NakedIdentifier,
                            SyntaxKind::QuotedIdentifier,
                        ])
                    },
                )
                .expect("common table expression should have an identifier");
            let Some(with_compound_statement) = cte_index
                .checked_sub(1)
                .and_then(|parent_index| context.parent_stack.get(parent_index))
            else {
                break;
            };

            for table_reference in with_compound_statement.recursive_crawl(
                const { &SyntaxSet::new(&[SyntaxKind::TableReference]) },
                true,
                &SyntaxSet::EMPTY,
                false,
            ) {
                if !table_reference
                    .raw()
                    .eq_ignore_ascii_case(cte_identifier.raw())
                {
                    continue;
                }

                let path = with_compound_statement.path_to(&table_reference);
                let used_in_set_expression = path
                    .iter()
                    .any(|step| step.segment.is_type(SyntaxKind::SetExpression));
                let selected_with_wildcard = path.iter().any(|step| {
                    matches!(
                        step.segment.get_type(),
                        SyntaxKind::SelectStatement | SyntaxKind::UnorderedSelectStatementSegment
                    ) && step
                        .segment
                        .descendant_type_set()
                        .contains(SyntaxKind::WildcardExpression)
                });

                if used_in_set_expression && selected_with_wildcard {
                    return Vec::new();
                }
            }
        }

        // Reordering the SELECT targets would corrupt the positional mapping
        // declared by an explicit CREATE VIEW column list.
        if is_view_with_explicit_columns(context) {
            return Vec::new();
        }

        let select_clause_segment = context.segment.clone();
        let select_target_elements: Vec<_> = select_clause_segment
            .children(const { &SyntaxSet::new(&[SyntaxKind::SelectClauseElement]) })
            .collect();

        if select_target_elements.is_empty() {
            return Vec::new();
        }

        let mut seen_band_elements: Vec<Vec<ErasedSegment>> = SELECT_ELEMENT_ORDER_PREFERENCE
            .iter()
            .map(|_| Vec::new())
            .collect();
        seen_band_elements.push(Vec::new());

        for &segment in &select_target_elements {
            let mut current_element_band: Option<usize> = None;

            for (i, band) in enumerate(SELECT_ELEMENT_ORDER_PREFERENCE) {
                for e in *band {
                    match e {
                        Validate::Types(types) => {
                            if let Some(child) = segment.child(types)
                                && (!child.is_type(SyntaxKind::CastExpression)
                                    || is_simple_cast_expression(&child))
                            {
                                validate(
                                    i,
                                    segment.clone(),
                                    &mut current_element_band,
                                    &mut violation_exists,
                                    &mut seen_band_elements,
                                );
                            }
                        }
                        Validate::Function => {
                            (|| {
                                let function = segment
                                    .child(const { &SyntaxSet::new(&[SyntaxKind::Function]) })?;
                                if is_simple_function(&function) {
                                    validate(
                                        i,
                                        segment.clone(),
                                        &mut current_element_band,
                                        &mut violation_exists,
                                        &mut seen_band_elements,
                                    );
                                }

                                Some(())
                            })();
                        }
                        Validate::Expression { child_typ } => {
                            (|| {
                                let expression = segment
                                    .child(const { &SyntaxSet::new(&[SyntaxKind::Expression]) })?;
                                if is_simple_expression(&expression, *child_typ) {
                                    validate(
                                        i,
                                        segment.clone(),
                                        &mut current_element_band,
                                        &mut violation_exists,
                                        &mut seen_band_elements,
                                    );
                                }

                                Some(())
                            })();
                        }
                    }
                }
            }

            if current_element_band.is_none() {
                seen_band_elements.last_mut().unwrap().push(segment.clone());
            }
        }

        if violation_exists {
            if context
                .parent_stack
                .last()
                .is_some_and(implicit_column_references)
            {
                return vec![LintResult::new(
                    select_clause_segment.into(),
                    Vec::new(),
                    None,
                    None,
                )];
            }

            let ordered_select_target_elements =
                seen_band_elements.into_iter().flatten().collect_vec();

            let fixes = zip(select_target_elements, ordered_select_target_elements)
                .filter_map(
                    |(initial_select_target_element, replace_select_target_element)| {
                        (initial_select_target_element != &replace_select_target_element).then(
                            || {
                                LintFix::replace(
                                    initial_select_target_element.clone(),
                                    vec![replace_select_target_element],
                                    None,
                                )
                            },
                        )
                    },
                )
                .collect_vec();

            return vec![LintResult::new(
                select_clause_segment.into(),
                fixes,
                None,
                None,
            )];
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(const { SyntaxSet::new(&[SyntaxKind::SelectClause]) }).into()
    }
}

fn is_simple_function(segment: &ErasedSegment) -> bool {
    segment
        .child(const { &SyntaxSet::new(&[SyntaxKind::FunctionName]) })
        .is_some_and(|function_name| function_name.raw().eq_ignore_ascii_case("CAST"))
}

fn is_simple_cast_expression(segment: &ErasedSegment) -> bool {
    segment.is_type(SyntaxKind::CastExpression)
        && segment
            .segments()
            .first()
            .is_some_and(is_simple_expression_segment)
}

fn is_simple_expression_segment(segment: &ErasedSegment) -> bool {
    match segment.get_type() {
        SyntaxKind::ColumnReference
        | SyntaxKind::ObjectReference
        | SyntaxKind::Literal
        | SyntaxKind::NumericLiteral
        | SyntaxKind::QuotedLiteral
        | SyntaxKind::BooleanLiteral
        | SyntaxKind::NullLiteral => true,
        SyntaxKind::Function => is_simple_function(segment),
        SyntaxKind::CastExpression => is_simple_cast_expression(segment),
        _ => false,
    }
}

fn is_simple_expression(segment: &ErasedSegment, child_type: SyntaxKind) -> bool {
    segment.is_type(SyntaxKind::Expression)
        && segment.child(&SyntaxSet::new(&[child_type])).is_some()
        && matches!(segment.segments().len(), 1 | 2)
        && segment
            .segments()
            .first()
            .is_some_and(is_simple_expression_segment)
}

fn is_view_with_explicit_columns(context: &RuleContext) -> bool {
    for parent in &context.parent_stack {
        if !parent.is_type(SyntaxKind::CreateViewStatement) {
            continue;
        }

        if parent.segments().iter().any(|child| {
            if !child.is_type(SyntaxKind::Bracketed) {
                return false;
            }

            let descendant_types = child.descendant_type_set();
            descendant_types.contains(SyntaxKind::ColumnReference)
                || descendant_types.contains(SyntaxKind::IndexColumnDefinition)
                || descendant_types.contains(SyntaxKind::ColumnDefinition)
                || child.segments().iter().any(|segment| {
                    segment.is_type(SyntaxKind::NakedIdentifier)
                        || segment.is_type(SyntaxKind::QuotedIdentifier)
                })
        }) {
            return true;
        }
    }

    false
}

enum Validate {
    Types(SyntaxSet),
    Function,
    Expression { child_typ: SyntaxKind },
}

fn validate(
    i: usize,
    segment: ErasedSegment,
    current_element_band: &mut Option<usize>,
    violation_exists: &mut bool,
    seen_band_elements: &mut [Vec<ErasedSegment>],
) {
    if seen_band_elements[i + 1..] != vec![Vec::new(); seen_band_elements[i + 1..].len()] {
        *violation_exists = true;
    }

    *current_element_band = Some(1);
    seen_band_elements[i].push(segment);
}

fn implicit_column_references(segment: &ErasedSegment) -> bool {
    if !matches!(
        segment.get_type(),
        SyntaxKind::WithingroupClause | SyntaxKind::WindowSpecification
    ) {
        if matches!(
            segment.get_type(),
            SyntaxKind::GroupbyClause | SyntaxKind::OrderbyClause
        ) {
            for seg in segment.segments() {
                if seg.is_type(SyntaxKind::NumericLiteral) {
                    return true;
                }
            }
        } else {
            for seg in segment.segments() {
                if implicit_column_references(seg) {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use sqruff_lib_core::parser::segments::Tables;

    use super::*;
    use crate::core::config::FluffConfig;
    use crate::core::linter::core::Linter;

    #[test]
    fn simple_expression_helpers_reject_non_simple_segments() {
        let config = FluffConfig::from_source("[sqruff]\ndialect = ansi\n", None);
        let linter = Linter::new(config, None, None, false).unwrap();
        let tables = Tables::default();
        let parsed = linter
            .parse_string(&tables, "SELECT a + 1 AS sum FROM foo", None)
            .unwrap();
        assert!(parsed.violations.is_empty());
        let expression = parsed
            .tree
            .unwrap()
            .recursive_crawl(
                &SyntaxSet::single(SyntaxKind::Expression),
                true,
                &SyntaxSet::EMPTY,
                true,
            )
            .into_iter()
            .next()
            .unwrap();

        assert!(!is_simple_cast_expression(&expression));
        assert!(!is_simple_expression_segment(&expression));
    }

    #[test]
    fn shorthand_casts_only_remain_simple_for_simple_operands() {
        let config = FluffConfig::from_source("[sqruff]\ndialect = postgres\n", None);
        let linter = Linter::new(config, None, None, false).unwrap();
        let tables = Tables::default();
        let parsed = linter
            .parse_string(
                &tables,
                "SELECT 2::INT, a::DATE, MIN(c)::DATE FROM foo",
                None,
            )
            .unwrap();
        assert!(parsed.violations.is_empty());
        let cast_expressions = parsed.tree.unwrap().recursive_crawl(
            &SyntaxSet::single(SyntaxKind::CastExpression),
            true,
            &SyntaxSet::EMPTY,
            true,
        );

        assert_eq!(cast_expressions.len(), 3);
        assert!(is_simple_cast_expression(&cast_expressions[0]));
        assert!(is_simple_cast_expression(&cast_expressions[1]));
        assert!(!is_simple_cast_expression(&cast_expressions[2]));
    }
}
