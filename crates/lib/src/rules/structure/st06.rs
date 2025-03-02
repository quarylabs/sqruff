use std::iter::zip;

use ahash::AHashMap;
use itertools::{Itertools, enumerate};
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::ErasedSegment;

use crate::core::config::Value;
use crate::core::rules::base::{CloneRule, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};

#[derive(Clone, Debug)]
pub struct RuleST06;

impl Rule for RuleST06 {
    fn load_from_config(&self, _config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
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
                Validate::Function { name: "cast" },
                Validate::Expression {
                    child_typ: SyntaxKind::CastExpression,
                },
            ],
        ];

        if context.parent_stack.len() >= 2
            && matches!(
                context.parent_stack[context.parent_stack.len() - 2].get_type(),
                SyntaxKind::InsertStatement | SyntaxKind::SetExpression
            )
        {
            return Vec::new();
        }

        if context.parent_stack.len() >= 3
            && matches!(
                context.parent_stack[context.parent_stack.len() - 3].get_type(),
                SyntaxKind::InsertStatement | SyntaxKind::SetExpression
            )
            && context.parent_stack[context.parent_stack.len() - 2].get_type()
                == SyntaxKind::WithCompoundStatement
        {
            return Vec::new();
        }

        if context.parent_stack.len() >= 3
            && matches!(
                context.parent_stack[context.parent_stack.len() - 3].get_type(),
                SyntaxKind::CreateTableStatement | SyntaxKind::MergeStatement
            )
        {
            return Vec::new();
        }

        if context.parent_stack.len() >= 4
            && matches!(
                context.parent_stack[context.parent_stack.len() - 4].get_type(),
                SyntaxKind::CreateTableStatement | SyntaxKind::MergeStatement
            )
            && context.parent_stack[context.parent_stack.len() - 2].get_type()
                == SyntaxKind::WithCompoundStatement
        {
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
                            if segment.child(types).is_some() {
                                validate(
                                    i,
                                    segment.clone(),
                                    &mut current_element_band,
                                    &mut violation_exists,
                                    &mut seen_band_elements,
                                );
                            }
                        }
                        Validate::Function { name } => {
                            (|| {
                                let function = segment
                                    .child(const { &SyntaxSet::new(&[SyntaxKind::Function]) })?;
                                let function_name = function.child(
                                    const { &SyntaxSet::new(&[SyntaxKind::FunctionName]) },
                                )?;
                                if function_name.raw() == *name {
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
                                if expression.child(&SyntaxSet::new(&[*child_typ])).is_some()
                                    && matches!(
                                        expression.segments()[0].get_type(),
                                        SyntaxKind::ColumnReference
                                            | SyntaxKind::ObjectReference
                                            | SyntaxKind::Literal
                                            | SyntaxKind::CastExpression
                                    )
                                    && expression.segments().len() == 2
                                    || expression.segments().len() == 1
                                {
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

enum Validate {
    Types(SyntaxSet),
    Function { name: &'static str },
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
