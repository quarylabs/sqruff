use ahash::AHashMap;
use itertools::chain;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::base::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::functional::segments::Segments;
use strum_macros::{AsRefStr, EnumString};

use crate::core::config::Value;
use crate::core::rules::base::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::utils::functional::context::FunctionalContext;

#[derive(Debug, Copy, Clone, AsRefStr, EnumString, PartialEq, Default)]
#[strum(serialize_all = "snake_case")]
enum TypeCastingStyle {
    #[default]
    Consistent,
    Cast,
    Convert,
    Shorthand,
    None,
}

#[derive(Copy, Clone)]
struct PreviousSkipped;

fn get_children(segments: Segments) -> Segments {
    segments.children(Some(|it: &ErasedSegment| {
        !it.is_meta()
            && !matches!(
                it.get_type(),
                SyntaxKind::StartBracket
                    | SyntaxKind::EndBracket
                    | SyntaxKind::Whitespace
                    | SyntaxKind::Newline
                    | SyntaxKind::CastingOperator
                    | SyntaxKind::Comma
                    | SyntaxKind::Keyword
            )
    }))
}

fn shorthand_fix_list(
    tables: &Tables,
    root_segment: ErasedSegment,
    shorthand_arg_1: ErasedSegment,
    shorthand_arg_2: ErasedSegment,
) -> Vec<LintFix> {
    let mut edits = if shorthand_arg_1.get_raw_segments().len() > 1 {
        vec![
            SegmentBuilder::token(tables.next_id(), "(", SyntaxKind::StartBracket).finish(),
            shorthand_arg_1,
            SegmentBuilder::token(tables.next_id(), ")", SyntaxKind::EndBracket).finish(),
        ]
    } else {
        vec![shorthand_arg_1]
    };

    edits.extend([
        SegmentBuilder::token(tables.next_id(), "::", SyntaxKind::CastingOperator).finish(),
        shorthand_arg_2,
    ]);

    vec![LintFix::replace(root_segment, edits, None)]
}

#[derive(Clone, Debug, Default)]
pub struct RuleCV11 {
    preferred_type_casting_style: TypeCastingStyle,
}

impl Rule for RuleCV11 {
    fn load_from_config(&self, config: &AHashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleCV11 {
            preferred_type_casting_style: config["preferred_type_casting_style"]
                .as_string()
                .unwrap()
                .parse()
                .unwrap(),
        }
        .erased())
    }

    fn name(&self) -> &'static str {
        "convention.casting_style"
    }

    fn description(&self) -> &'static str {
        "Enforce consistent type casting style."
    }

    fn long_description(&self) -> &'static str {
        r"
**Anti-pattern**

Using a mixture of `CONVERT`, `::`, and `CAST` when `preferred_type_casting_style` config is set to `consistent` (default).

```sql
SELECT
    CONVERT(int, 1) AS bar,
    100::int::text,
    CAST(10 AS text) AS coo
FROM foo;
```

**Best Practice**

Use a consistent type casting style.

```sql
SELECT
    CAST(1 AS int) AS bar,
    CAST(CAST(100 AS int) AS text),
    CAST(10 AS text) AS coo
FROM foo;
```
"
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Convention]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let current_type_casting_style = if context.segment.is_type(SyntaxKind::Function) {
            let Some(function_name) = context
                .segment
                .child(const { &SyntaxSet::new(&[SyntaxKind::FunctionName]) })
            else {
                return Vec::new();
            };
            if function_name.raw().eq_ignore_ascii_case("CAST") {
                TypeCastingStyle::Cast
            } else if function_name.raw().eq_ignore_ascii_case("CONVERT") {
                TypeCastingStyle::Convert
            } else {
                TypeCastingStyle::None
            }
        } else if context.segment.is_type(SyntaxKind::CastExpression) {
            TypeCastingStyle::Shorthand
        } else {
            TypeCastingStyle::None
        };

        let functional_context = FunctionalContext::new(context);
        match self.preferred_type_casting_style {
            TypeCastingStyle::Consistent => {
                let Some(prior_type_casting_style) = context.try_get::<TypeCastingStyle>() else {
                    context.set(current_type_casting_style);
                    return Vec::new();
                };
                let previous_skipped = context.try_get::<PreviousSkipped>();

                let mut fixes = Vec::new();
                match prior_type_casting_style {
                    TypeCastingStyle::Cast => match current_type_casting_style {
                        TypeCastingStyle::Convert => {
                            let convert_content =
                                get_children(functional_context.segment().children(Some(
                                    |it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed),
                                )));
                            if convert_content.len() > 2 {
                                if previous_skipped.is_none() {
                                    context.set(PreviousSkipped);
                                }
                                return Vec::new();
                            }

                            fixes = cast_fix_list(
                                context.tables,
                                context.segment.clone(),
                                &[convert_content[1].clone()],
                                convert_content[0].clone(),
                                None,
                            );
                        }
                        TypeCastingStyle::Shorthand => {
                            let expression_datatype_segment =
                                get_children(functional_context.segment());

                            fixes = cast_fix_list(
                                context.tables,
                                context.segment.clone(),
                                &[expression_datatype_segment[0].clone()],
                                expression_datatype_segment[1].clone(),
                                Some(Segments::from_vec(
                                    expression_datatype_segment.base[2..].to_vec(),
                                    None,
                                )),
                            )
                        }
                        _ => {}
                    },
                    TypeCastingStyle::Convert => match current_type_casting_style {
                        TypeCastingStyle::Cast => {
                            let cast_content = get_children(functional_context.segment().children(
                                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)),
                            ));

                            if cast_content.len() > 2 {
                                return Vec::new();
                            }

                            fixes = convert_fix_list(
                                context.tables,
                                context.segment.clone(),
                                cast_content[1].clone(),
                                cast_content[0].clone(),
                                None,
                            );
                        }
                        TypeCastingStyle::Shorthand => {
                            let expression_datatype_segment =
                                get_children(functional_context.segment());

                            fixes = convert_fix_list(
                                context.tables,
                                context.segment.clone(),
                                expression_datatype_segment[1].clone(),
                                expression_datatype_segment[0].clone(),
                                Some(Segments::from_vec(
                                    expression_datatype_segment.base[2..].to_vec(),
                                    None,
                                )),
                            );
                        }
                        _ => (),
                    },
                    TypeCastingStyle::Shorthand => {
                        if current_type_casting_style == TypeCastingStyle::Cast {
                            // Get the content of CAST
                            let cast_content = get_children(functional_context.segment().children(
                                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)),
                            ));
                            if cast_content.len() > 2 {
                                return Vec::new();
                            }

                            fixes = shorthand_fix_list(
                                context.tables,
                                context.segment.clone(),
                                cast_content[0].clone(),
                                cast_content[1].clone(),
                            );
                        } else if current_type_casting_style == TypeCastingStyle::Convert {
                            let convert_content =
                                get_children(functional_context.segment().children(Some(
                                    |it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed),
                                )));
                            if convert_content.len() > 2 {
                                return Vec::new();
                            }

                            fixes = shorthand_fix_list(
                                context.tables,
                                context.segment.clone(),
                                convert_content[1].clone(),
                                convert_content[0].clone(),
                            );
                        }
                    }
                    _ => {}
                }

                if prior_type_casting_style != current_type_casting_style {
                    return vec![LintResult::new(
                        context.segment.clone().into(),
                        fixes,
                        "Inconsistent type casting styles found.".to_owned().into(),
                        None,
                    )];
                }
            }
            _ if current_type_casting_style != self.preferred_type_casting_style => {
                let mut convert_content = None;
                let mut cast_content = None;
                let mut fixes = Vec::new();

                match self.preferred_type_casting_style {
                    TypeCastingStyle::Cast => match current_type_casting_style {
                        TypeCastingStyle::Convert => {
                            let segments = get_children(functional_context.segment().children(
                                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)),
                            ));
                            fixes = cast_fix_list(
                                context.tables,
                                context.segment.clone(),
                                &[segments[1].clone()],
                                segments[0].clone(),
                                None,
                            );
                            convert_content = Some(segments);
                        }
                        TypeCastingStyle::Shorthand => {
                            let expression_datatype_segment =
                                get_children(functional_context.segment());
                            let data_type_idx = expression_datatype_segment
                                .iter()
                                .position(|seg| seg.is_type(SyntaxKind::DataType))
                                .unwrap();

                            fixes = cast_fix_list(
                                context.tables,
                                context.segment.clone(),
                                &expression_datatype_segment[..data_type_idx],
                                expression_datatype_segment[data_type_idx].clone(),
                                Some(Segments::from_vec(
                                    expression_datatype_segment.base[data_type_idx + 1..].to_vec(),
                                    None,
                                )),
                            );
                        }
                        _ => {}
                    },
                    TypeCastingStyle::Convert => match current_type_casting_style {
                        TypeCastingStyle::Cast => {
                            let cast_content = get_children(functional_context.segment().children(
                                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)),
                            ));

                            fixes = convert_fix_list(
                                context.tables,
                                context.segment.clone(),
                                cast_content[1].clone(),
                                cast_content[0].clone(),
                                None,
                            );
                        }
                        TypeCastingStyle::Shorthand => {
                            let cast_content = get_children(functional_context.segment());

                            fixes = convert_fix_list(
                                context.tables,
                                context.segment.clone(),
                                cast_content[1].clone(),
                                cast_content[0].clone(),
                                Some(Segments::from_vec(cast_content.base[2..].to_vec(), None)),
                            )
                        }
                        _ => {}
                    },
                    TypeCastingStyle::Shorthand => match current_type_casting_style {
                        TypeCastingStyle::Cast => {
                            let segments = get_children(functional_context.segment().children(
                                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)),
                            ));

                            fixes = shorthand_fix_list(
                                context.tables,
                                context.segment.clone(),
                                segments[0].clone(),
                                segments[1].clone(),
                            );
                            cast_content = Some(segments);
                        }
                        TypeCastingStyle::Convert => {
                            let segments = get_children(functional_context.segment().children(
                                Some(|it: &ErasedSegment| it.is_type(SyntaxKind::Bracketed)),
                            ));

                            fixes = shorthand_fix_list(
                                context.tables,
                                context.segment.clone(),
                                segments[1].clone(),
                                segments[0].clone(),
                            );

                            convert_content = Some(segments);
                        }
                        _ => {}
                    },
                    _ => {}
                }

                if convert_content
                    .filter(|convert_content| convert_content.len() > 2)
                    .is_some()
                {
                    fixes.clear();
                }

                if cast_content
                    .filter(|cast_content| cast_content.len() > 2)
                    .is_some()
                {
                    fixes.clear();
                }

                return vec![LintResult::new(
                    context.segment.clone().into(),
                    fixes,
                    "Used type casting style is different from the preferred type casting style."
                        .to_owned()
                        .into(),
                    None,
                )];
            }

            _ => {}
        }

        Vec::new()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        SegmentSeekerCrawler::new(
            const { SyntaxSet::new(&[SyntaxKind::Function, SyntaxKind::CastExpression]) },
        )
        .into()
    }
}

fn convert_fix_list(
    tables: &Tables,
    root: ErasedSegment,
    convert_arg_1: ErasedSegment,
    convert_arg_2: ErasedSegment,
    later_types: Option<Segments>,
) -> Vec<LintFix> {
    use sqruff_lib_core::parser::segments::base::ErasedSegment;

    let mut edits: Vec<ErasedSegment> = vec![
        SegmentBuilder::token(
            tables.next_id(),
            "convert",
            SyntaxKind::FunctionNameIdentifier,
        )
        .finish(),
        SegmentBuilder::token(tables.next_id(), "(", SyntaxKind::StartBracket).finish(),
        convert_arg_1,
        SegmentBuilder::token(tables.next_id(), ",", SyntaxKind::Comma).finish(),
        SegmentBuilder::whitespace(tables.next_id(), " "),
        convert_arg_2,
        SegmentBuilder::token(tables.next_id(), ")", SyntaxKind::EndBracket).finish(),
    ];

    if let Some(later_types) = later_types {
        let pre_edits: Vec<ErasedSegment> = vec![
            SegmentBuilder::token(
                tables.next_id(),
                "convert",
                SyntaxKind::FunctionNameIdentifier,
            )
            .finish(),
            SegmentBuilder::symbol(tables.next_id(), "("),
        ];

        let in_edits: Vec<ErasedSegment> = vec![
            SegmentBuilder::symbol(tables.next_id(), ","),
            SegmentBuilder::whitespace(tables.next_id(), " "),
        ];

        let post_edits: Vec<ErasedSegment> = vec![SegmentBuilder::symbol(tables.next_id(), ")")];

        for _type in later_types.base {
            edits = chain(
                chain(pre_edits.clone(), vec![_type]),
                chain(in_edits.clone(), chain(edits, post_edits.clone())),
            )
            .collect();
        }
    }

    vec![LintFix::replace(root, edits, None)]
}

fn cast_fix_list(
    tables: &Tables,
    root: ErasedSegment,
    cast_arg_1: &[ErasedSegment],
    cast_arg_2: ErasedSegment,
    later_types: Option<Segments>,
) -> Vec<LintFix> {
    let mut edits = vec![
        SegmentBuilder::token(tables.next_id(), "cast", SyntaxKind::FunctionNameIdentifier)
            .finish(),
        SegmentBuilder::token(tables.next_id(), "(", SyntaxKind::StartBracket).finish(),
    ];
    edits.extend_from_slice(cast_arg_1);
    edits.extend([
        SegmentBuilder::whitespace(tables.next_id(), " "),
        SegmentBuilder::keyword(tables.next_id(), "as"),
        SegmentBuilder::whitespace(tables.next_id(), " "),
        cast_arg_2,
        SegmentBuilder::token(tables.next_id(), ")", SyntaxKind::EndBracket).finish(),
    ]);

    if let Some(later_types) = later_types {
        let pre_edits: Vec<ErasedSegment> = vec![
            SegmentBuilder::token(tables.next_id(), "cast", SyntaxKind::FunctionNameIdentifier)
                .finish(),
            SegmentBuilder::symbol(tables.next_id(), "("),
        ];

        let in_edits: Vec<ErasedSegment> = vec![
            SegmentBuilder::whitespace(tables.next_id(), " "),
            SegmentBuilder::keyword(tables.next_id(), "as"),
            SegmentBuilder::whitespace(tables.next_id(), " "),
        ];

        let post_edits: Vec<ErasedSegment> = vec![SegmentBuilder::symbol(tables.next_id(), ")")];

        for _type in later_types.base {
            let mut xs = Vec::new();
            xs.extend(pre_edits.clone());
            xs.extend(edits);
            xs.extend(in_edits.clone());
            xs.push(_type);
            xs.extend(post_edits.clone());
            edits = xs;
        }
    }

    vec![LintFix::replace(root, edits, None)]
}
