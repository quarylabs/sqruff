use hashbrown::HashMap;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::{SyntaxKind, SyntaxSet};
use sqruff_lib_core::lint_fix::LintFix;
use sqruff_lib_core::parser::segments::{ErasedSegment, SegmentBuilder, Tables};
use sqruff_lib_core::utils::functional::segments::Segments;
use strum_macros::{AsRefStr, EnumString};

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, SegmentSeekerCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
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
    segments.children_where(|it: &ErasedSegment| {
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
    })
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
    fn load_from_config(&self, config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
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

    fn dialect_skip(&self) -> &'static [DialectKind] {
        &[
            DialectKind::Athena,
            DialectKind::Teradata,
            DialectKind::Trino,
        ]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        // If we're in a templated section, don't consider the current location.
        // (i.e. if a cast happens in a macro, the end user writing the current
        // query may not know that or have control over it, so we should just
        // skip it).
        if let Some(pos_marker) = &context.segment.get_position_marker()
            && !pos_marker.is_literal()
        {
            return Vec::new();
        }

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
                // On the dialects whose CONVERT takes its arguments the other
                // way round, rewriting to CAST swaps them and produces valid SQL
                // that means something else: CONVERT(b, SIGNED) would become
                // cast(SIGNED as b). Leave CONVERT alone there rather than
                // corrupt it silently. CAST and :: are still linted as usual.
                if is_reversed_convert_dialect(context.dialect.name) {
                    return Vec::new();
                }
                TypeCastingStyle::Convert
            } else {
                TypeCastingStyle::None
            }
        } else if context.segment.is_type(SyntaxKind::CastExpression) {
            // Dialect-specific casts such as Databricks ?:: and Vertica ::!
            // cannot be rewritten as ordinary CAST or CONVERT expressions.
            if context
                .segment
                .child(const { &SyntaxSet::new(&[SyntaxKind::CastingOperator]) })
                .is_none()
            {
                return Vec::new();
            }
            TypeCastingStyle::Shorthand
        } else {
            TypeCastingStyle::None
        };

        let functional_context = FunctionalContext::new(context);
        match self.preferred_type_casting_style {
            TypeCastingStyle::Consistent => {
                // If current is None, it's not a cast operation (e.g., STRING_AGG, or
                // other non-CAST/CONVERT functions), so skip it entirely.
                if current_type_casting_style == TypeCastingStyle::None {
                    return Vec::new();
                }

                let Some(prior_type_casting_style) = context.try_get::<TypeCastingStyle>() else {
                    context.set(current_type_casting_style);
                    return Vec::new();
                };
                let previous_skipped = context.try_get::<PreviousSkipped>();

                let mut fixes = Vec::new();
                match prior_type_casting_style {
                    TypeCastingStyle::Cast => match current_type_casting_style {
                        TypeCastingStyle::Convert => {
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let convert_content = get_children(bracketed);
                            if convert_content.len() > 2 {
                                if previous_skipped.is_none() {
                                    context.set(PreviousSkipped);
                                }
                                return Vec::new();
                            }

                            fixes = cast_fix_list(
                                context.tables,
                                context.dialect.name,
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
                                context.dialect.name,
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
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let cast_content = get_children(bracketed);

                            if cast_content.len() > 2 {
                                return Vec::new();
                            }

                            fixes = convert_fix_list(
                                context.tables,
                                context.dialect.name,
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
                                context.dialect.name,
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
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let cast_content = get_children(bracketed);
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
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let convert_content = get_children(bracketed);
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
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let segments = get_children(bracketed);
                            fixes = cast_fix_list(
                                context.tables,
                                context.dialect.name,
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
                                context.dialect.name,
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
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let cast_content = get_children(bracketed);

                            fixes = convert_fix_list(
                                context.tables,
                                context.dialect.name,
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
                                context.dialect.name,
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
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let segments = get_children(bracketed);

                            fixes = shorthand_fix_list(
                                context.tables,
                                context.segment.clone(),
                                segments[0].clone(),
                                segments[1].clone(),
                            );
                            cast_content = Some(segments);
                        }
                        TypeCastingStyle::Convert => {
                            let bracketed = functional_context
                                .segment()
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::FunctionContents)
                                })
                                .children_where(|it: &ErasedSegment| {
                                    it.is_type(SyntaxKind::Bracketed)
                                });
                            let segments = get_children(bracketed);

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

/// Match the parser's function > function_name/function_contents > bracketed
/// structure so other rules in the same fix pass see a parse-shaped edit.
fn build_function(
    tables: &Tables,
    dialect: DialectKind,
    name: &str,
    contents: Vec<ErasedSegment>,
) -> ErasedSegment {
    let mut bracketed = Vec::with_capacity(contents.len() + 2);
    bracketed.push(SegmentBuilder::token(tables.next_id(), "(", SyntaxKind::StartBracket).finish());
    bracketed.extend(contents);
    bracketed.push(SegmentBuilder::token(tables.next_id(), ")", SyntaxKind::EndBracket).finish());

    let function_name = SegmentBuilder::node(
        tables.next_id(),
        SyntaxKind::FunctionName,
        dialect,
        vec![
            SegmentBuilder::token(tables.next_id(), name, SyntaxKind::FunctionNameIdentifier)
                .finish(),
        ],
    )
    .finish();
    let function_contents = SegmentBuilder::node(
        tables.next_id(),
        SyntaxKind::FunctionContents,
        dialect,
        vec![
            SegmentBuilder::node(tables.next_id(), SyntaxKind::Bracketed, dialect, bracketed)
                .finish(),
        ],
    )
    .finish();
    SegmentBuilder::node(
        tables.next_id(),
        SyntaxKind::Function,
        dialect,
        vec![function_name, function_contents],
    )
    .finish()
}

/// MySQL spells this `CONVERT(expr, type)`, the opposite way round from the
/// T-SQL `CONVERT(type, expr)` that this rule assumes. mariadb, doris and
/// starrocks all inherit the mysql dialect and so inherit the order too.
fn is_reversed_convert_dialect(dialect: DialectKind) -> bool {
    matches!(
        dialect,
        DialectKind::Mysql | DialectKind::Mariadb | DialectKind::Doris | DialectKind::Starrocks
    )
}

fn convert_fix_list(
    tables: &Tables,
    dialect: DialectKind,
    root: ErasedSegment,
    convert_arg_1: ErasedSegment,
    convert_arg_2: ErasedSegment,
    later_types: Option<Segments>,
) -> Vec<LintFix> {
    // Return no fixes on the dialects whose CONVERT takes its arguments the
    // other way round. The rewrite below emits the T-SQL order, so applying it
    // there would turn CAST(b AS SIGNED) into convert(SIGNED, b): valid SQL
    // that means something else. The violation is still reported by the caller,
    // it just cannot be auto-fixed.
    if is_reversed_convert_dialect(dialect) {
        return Vec::new();
    }

    let mut function = build_function(
        tables,
        dialect,
        "convert",
        vec![
            convert_arg_1,
            SegmentBuilder::token(tables.next_id(), ",", SyntaxKind::Comma).finish(),
            SegmentBuilder::whitespace(tables.next_id(), " "),
            convert_arg_2,
        ],
    );

    if let Some(later_types) = later_types {
        for data_type in later_types.base {
            function = build_function(
                tables,
                dialect,
                "convert",
                vec![
                    data_type,
                    SegmentBuilder::token(tables.next_id(), ",", SyntaxKind::Comma).finish(),
                    SegmentBuilder::whitespace(tables.next_id(), " "),
                    function,
                ],
            );
        }
    }

    vec![LintFix::replace(root, vec![function], None)]
}

fn cast_fix_list(
    tables: &Tables,
    dialect: DialectKind,
    root: ErasedSegment,
    cast_arg_1: &[ErasedSegment],
    cast_arg_2: ErasedSegment,
    later_types: Option<Segments>,
) -> Vec<LintFix> {
    let mut contents = cast_arg_1.to_vec();
    contents.extend([
        SegmentBuilder::whitespace(tables.next_id(), " "),
        SegmentBuilder::keyword(tables.next_id(), "as"),
        SegmentBuilder::whitespace(tables.next_id(), " "),
        cast_arg_2,
    ]);
    let mut function = build_function(tables, dialect, "cast", contents);

    if let Some(later_types) = later_types {
        for data_type in later_types.base {
            function = build_function(
                tables,
                dialect,
                "cast",
                vec![
                    function,
                    SegmentBuilder::whitespace(tables.next_id(), " "),
                    SegmentBuilder::keyword(tables.next_id(), "as"),
                    SegmentBuilder::whitespace(tables.next_id(), " "),
                    data_type,
                ],
            );
        }
    }

    vec![LintFix::replace(root, vec![function], None)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_cast_replacement_has_parse_shape() {
        let tables = Tables::default();
        let inner = build_function(
            &tables,
            DialectKind::Ansi,
            "cast",
            vec![
                SegmentBuilder::token(tables.next_id(), "1", SyntaxKind::NumericLiteral).finish(),
                SegmentBuilder::whitespace(tables.next_id(), " "),
                SegmentBuilder::keyword(tables.next_id(), "as"),
                SegmentBuilder::whitespace(tables.next_id(), " "),
                SegmentBuilder::token(tables.next_id(), "int", SyntaxKind::DataType).finish(),
            ],
        );
        let outer = build_function(
            &tables,
            DialectKind::Ansi,
            "cast",
            vec![
                inner,
                SegmentBuilder::whitespace(tables.next_id(), " "),
                SegmentBuilder::keyword(tables.next_id(), "as"),
                SegmentBuilder::whitespace(tables.next_id(), " "),
                SegmentBuilder::token(tables.next_id(), "text", SyntaxKind::DataType).finish(),
            ],
        );

        assert_eq!(outer.raw(), "cast(cast(1 as int) as text)");
        assert!(outer.segments()[0].is_type(SyntaxKind::FunctionName));
        assert!(outer.segments()[1].is_type(SyntaxKind::FunctionContents));
        let bracketed = &outer.segments()[1].segments()[0];
        assert!(bracketed.is_type(SyntaxKind::Bracketed));
        assert!(bracketed.segments()[1].is_type(SyntaxKind::Function));
    }
}
