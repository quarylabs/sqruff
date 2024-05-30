use std::sync::Arc;

use ahash::AHashSet;
use itertools::Itertools;

use super::ansi::{self, Node, NodeTrait};
use super::postgres_keywords::POSTGRES_POSTGIS_DATATYPE_KEYWORDS;
use crate::core::dialects::base::Dialect;
use crate::core::parser::grammar::anyof::{any_set_of, one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Anything, Ref};
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::Matcher;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, IdentifierSegment,
    NewlineSegment, NewlineSegmentNewArgs, Segment, SymbolSegment, SymbolSegmentNewArgs,
};
use crate::core::parser::segments::common::LiteralSegment;
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::meta::MetaSegment;
use crate::core::parser::types::ParseMode;
use crate::dialects::ansi::ansi_raw_dialect;
use crate::dialects::postgres_keywords::{get_keywords, postgres_keywords};
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

trait Boxed {
    fn boxed(self) -> Arc<Self>;
}

impl<T> Boxed for T {
    fn boxed(self) -> Arc<Self>
    where
        Self: Sized,
    {
        Arc::new(self)
    }
}

pub fn postgres_dialect() -> Dialect {
    let mut postgres = ansi_raw_dialect();
    postgres.name = "postgres";

    postgres.insert_lexer_matchers(
        vec![Matcher::string("right_arrow", "=>", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "right_arrow", ..Default::default() },
            )
        })],
        "equals",
    );

    postgres.insert_lexer_matchers(vec![
        Matcher::regex(
            "unicode_single_quote",
            r"(?s)U&(('')+?(?!')|('.*?(?<!')(?:'')*'(?!')))(\s*UESCAPE\s*'[^0-9A-Fa-f'+\-\s)]')?",
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "unicode_single_quote", ..Default::default() },
                )
            }
        ),
        Matcher::regex(
            "escaped_single_quote",
            r"(?s)E(('')+?(?!')|'.*?((?<!\\)(?:\\\\)*(?<!')(?:'')*|(?<!\\)(?:\\\\)*\\(?<!')(?:'')*')'(?!'))",
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "escaped_single_quote", ..Default::default() },
                )
            }
        ),
        Matcher::regex(
            "unicode_double_quote",
            r#"(?s)U&".+?"(\s*UESCAPE\s*\'[^0-9A-Fa-f\'+\-\s)]\')?"#,
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "unicode_double_quote", ..Default::default() },
                )
            }
        ),
        Matcher::regex(
            "json_operator",
            r#"->>|#>>|->|#>|@>|<@|\?\||\?|\?&|#-"#,
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "json_operator", ..Default::default() },
                )
            }
        ),
        Matcher::string(
            "at",
            "@",
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "at", ..Default::default() },
                )
            }
        ),
        Matcher::regex(
            "bit_string_literal",
            r#"[bBxX]'[0-9a-fA-F]*'"#,
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "bit_string_literal", ..Default::default() },
                )
            }
        ),
    ], "like_operator");

    postgres.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "meta_command",
                r"\\([^\\\r\n])+((\\\\)|(?=\n)|(?=\r\n))?",
                |slice, marker| {
                    CommentSegment::create(
                        slice,
                        marker.into(),
                        CommentSegmentNewArgs { r#type: "comment", trim_start: None },
                    )
                },
            ),
            Matcher::regex("dollar_numeric_literal", r"\$\d+", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs {
                        code_type: "dollar_numeric_literal",
                        ..Default::default()
                    },
                )
            }),
        ],
        "word",
    );

    postgres.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"(--)[^\n]*", |slice, marker| {
            CommentSegment::create(
                slice,
                marker.into(),
                CommentSegmentNewArgs { r#type: "inline_comment", trim_start: Some(vec!["--"]) },
            )
        }),
        Matcher::regex(
            "single_quote",
            r"(?s)('')+?(?!')|('.*?(?<!')(?:'')*'(?!'))",
            |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs {
                        code_type: "single_quote",
                        instance_types: vec![],
                        trim_start: None,
                        trim_chars: None,
                        source_fixes: None,
                    },
                )
            },
        ),
        Matcher::regex("double_quote", r#"(?s)".+?""#, |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs {
                    code_type: "double_quote",
                    instance_types: vec![],
                    trim_start: None,
                    trim_chars: None,
                    source_fixes: None,
                },
            )
        }),
        Matcher::regex("word", r"[a-zA-Z_][0-9a-zA-Z_$]*", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "word", ..Default::default() },
            )
        }),
    ]);

    let keywords = postgres_keywords();
    let not_keywords = get_keywords(&keywords, "not-keyword");

    postgres.sets_mut("reserved_keywords").extend(get_keywords(&keywords, "reserved"));
    postgres.sets_mut("unreserved_keywords").extend(get_keywords(&keywords, "non-reserved"));

    postgres.sets_mut("reserved_keywords").retain(|keyword| !not_keywords.contains(keyword));
    postgres.sets_mut("unreserved_keywords").retain(|keyword| !not_keywords.contains(keyword));

    // Add datetime units
    postgres.sets_mut("datetime_units").extend([
        "CENTURY",
        "DECADE",
        "DOW",
        "DOY",
        "EPOCH",
        "ISODOW",
        "ISOYEAR",
        "MICROSECONDS",
        "MILLENNIUM",
        "MILLISECONDS",
        "TIMEZONE",
        "TIMEZONE_HOUR",
        "TIMEZONE_MINUTE",
    ]);

    // Set the bare functions
    postgres.sets_mut("bare_functions").extend([
        "CURRENT_TIMESTAMP",
        "CURRENT_TIME",
        "CURRENT_DATE",
        "LOCALTIME",
        "LOCALTIMESTAMP",
    ]);

    // Postgres doesn't have a dateadd function
    // Also according to https://www.postgresql.org/docs/14/functions-datetime.html
    // It quotes dateparts. So don't need this.
    postgres.sets_mut("date_part_function_name").clear();

    // In Postgres, UNNEST() returns a "value table", similar to BigQuery
    postgres.sets_mut("value_table_functions").extend(["UNNEST", "GENERATE_SERIES"]);

    postgres.add([
        (
            "JsonOperatorSegment".into(),
            TypedParser::new(
                "json_operator",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "binary_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SimpleGeometryGrammar".into(),
            AnyNumberOf::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                .to_matchable()
                .into(),
        ),
        (
            "MultilineConcatenateNewline".into(),
            TypedParser::new(
                "newline",
                |segment: &dyn Segment| {
                    NewlineSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        NewlineSegmentNewArgs {},
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "MultilineConcatenateDelimiterGrammar".into(),
            AnyNumberOf::new(vec_of_erased![Ref::new("MultilineConcatenateNewline")])
                .config(|this| {
                    this.min_times(1);
                    this.disallow_gaps();
                })
                .to_matchable()
                .into(),
        ),
        (
            "NakedIdentifierFullSegment".into(),
            TypedParser::new(
                "word",
                |segment: &dyn Segment| {
                    IdentifierSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "naked_identifier", ..Default::default() },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "PropertiesNakedIdentifierSegment".into(),
            TypedParser::new(
                "word",
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "properties_naked_identifier",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SingleIdentifierFullGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("NakedIdentifierFullSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DefinitionArgumentValueGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("LiteralGrammar"),
                Ref::new("PropertiesNakedIdentifierSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CascadeRestrictGrammar".into(),
            one_of(vec_of_erased![Ref::keyword("CASCADE"), Ref::keyword("RESTRICT")])
                .to_matchable()
                .into(),
        ),
        (
            "ExtendedTableReferenceGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("TableReferenceSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ONLY"),
                    optionally_bracketed(vec_of_erased![Ref::new("TableReferenceSegment")])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::new("StarSegment")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "RightArrowSegment".into(),
            StringParser::new(
                "=>",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "right_arrow" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "OnKeywordAsIdentifierSegment".into(),
            StringParser::new(
                "ON",
                |segment: &dyn Segment| {
                    IdentifierSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "naked_identifier", ..Default::default() },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DollarNumericLiteralSegment".into(),
            TypedParser::new(
                "dollar_numeric_literal",
                |segment: &dyn Segment| {
                    LiteralSegment::create(&segment.raw(), &segment.get_position_marker().unwrap())
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ForeignDataWrapperGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("FOREIGN"),
                Ref::keyword("DATA"),
                Ref::keyword("WRAPPER"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OptionsListGrammar".into(),
            Sequence::new(vec_of_erased![Delimited::new(vec_of_erased![
                Ref::new("NakedIdentifierFullSegment"),
                Ref::new("QuotedLiteralSegment")
            ])])
            .to_matchable()
            .into(),
        ),
        (
            "OptionsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("OPTIONS"),
                Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![Ref::new(
                    "OptionsListGrammar"
                )])])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CreateUserMappingGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("USER"),
                Ref::keyword("MAPPING"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SessionInformationUserFunctionsGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("USER"),
                Ref::keyword("CURRENT_ROLE"),
                Ref::keyword("CURRENT_USER"),
                Ref::keyword("SESSION_USER"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ImportForeignSchemaGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("IMPORT"),
                Ref::keyword("FOREIGN"),
                Ref::keyword("SCHEMA"),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    postgres.add([
        (
            "LikeGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIKE"),
                Ref::keyword("ILIKE"),
                Sequence::new(vec_of_erased![Ref::keyword("SIMILAR"), Ref::keyword("TO")])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StringBinaryOperatorGrammar".into(),
            one_of(vec_of_erased![Ref::new("ConcatSegment"), Ref::keyword("COLLATE"),])
                .to_matchable()
                .into(),
        ),
        (
            "ComparisonOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("EqualsSegment"),
                Ref::new("GreaterThanSegment"),
                Ref::new("LessThanSegment"),
                Ref::new("GreaterThanOrEqualToSegment"),
                Ref::new("LessThanOrEqualToSegment"),
                Ref::new("NotEqualToSegment"),
                Ref::new("LikeOperatorSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("NOT"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM"),
                ]),
                Ref::new("OverlapSegment"),
                Ref::new("NotExtendRightSegment"),
                Ref::new("NotExtendLeftSegment"),
                Ref::new("AdjacentSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                // Generate the anti-template from the set of reserved keywords
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({})$", pattern);

                RegexParser::new(
                    r"([A-Z_]+|[0-9]+[A-Z_$])[A-Z0-9_$]*",
                    |segment| {
                        IdentifierSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "naked_identifier",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    anti_template.into(),
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            RegexParser::new(
                r#"[A-Z_][A-Z0-9_$]*|\"[^\"]*\""#,
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "parameter", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            RegexParser::new(
                r"[A-Z_][A-Z0-9_$]*",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "function_name_identifier",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec_of_erased![Ref::new("ExpressionSegment"), Ref::new("NamedArgumentSegment")])
                .to_matchable()
                .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    TypedParser::new(
                        "single_quote",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: "quoted_literal" },
                            )
                        },
                        None,
                        false,
                        None
                    ),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("MultilineConcatenateDelimiterGrammar"),
                        TypedParser::new(
                            "single_quote",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "quoted_literal" },
                                )
                            },
                            None,
                            false,
                            None
                        ),
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    TypedParser::new(
                        "bit_string_literal",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: "quoted_literal" },
                            )
                        },
                        None,
                        false,
                        None
                    ),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("MultilineConcatenateDelimiterGrammar"),
                        TypedParser::new(
                            "bit_string_literal",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "quoted_literal" },
                                )
                            },
                            None,
                            false,
                            None
                        ),
                    ])
                ]),
                Delimited::new(vec_of_erased![
                    TypedParser::new(
                        "unicode_single_quote",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: "quoted_literal" },
                            )
                        },
                        None,
                        false,
                        None
                    ),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("MultilineConcatenateDelimiterGrammar"),
                        TypedParser::new(
                            "unicode_single_quote",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "quoted_literal" },
                                )
                            },
                            None,
                            false,
                            None
                        ),
                    ])
                ]),
                Delimited::new(vec_of_erased![
                    TypedParser::new(
                        "escaped_single_quote",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: "quoted_literal" },
                            )
                        },
                        None,
                        false,
                        None
                    ),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("MultilineConcatenateDelimiterGrammar"),
                        TypedParser::new(
                            "escaped_single_quote",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "quoted_literal" },
                                )
                            },
                            None,
                            false,
                            None
                        ),
                    ])
                ]),
                Delimited::new(vec_of_erased![
                    TypedParser::new(
                        "dollar_quote",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: "quoted_literal" },
                            )
                        },
                        None,
                        false,
                        None
                    ),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("MultilineConcatenateDelimiterGrammar"),
                        TypedParser::new(
                            "dollar_quote",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "quoted_literal" },
                                )
                            },
                            None,
                            false,
                            None
                        ),
                    ])
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuotedIdentifierSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(
                    "double_quote",
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: "quoted_identifier" },
                        )
                    },
                    None,
                    false,
                    None
                ),
                TypedParser::new(
                    "unicode_double_quote",
                    |segment: &dyn Segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: "quoted_literal" },
                        )
                    },
                    None,
                    false,
                    None
                ),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("WithinGroupClauseSegment"),
                Ref::new("OverClauseSegment"),
                Ref::new("FilterClauseGrammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("ArithmeticBinaryOperatorGrammar"),
                Ref::new("StringBinaryOperatorGrammar"),
                Ref::new("BooleanBinaryOperatorGrammar"),
                Ref::new("ComparisonOperatorGrammar"),
                Ref::new("JsonOperatorSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionParameterGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("IN"),
                    Ref::keyword("OUT"),
                    Ref::keyword("INOUT"),
                    Ref::keyword("VARIADIC"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Ref::new("DatatypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Ref::new("DatatypeSegment"),
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("DEFAULT"), Ref::new("EqualsSegment"),]),
                    Ref::new("ExpressionSegment"),
                ])
                .config(|this| this.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FrameClauseUnitGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("RANGE"),
                Ref::keyword("ROWS"),
                Ref::keyword("GROUPS"),
            ])
            .to_matchable()
            .into(),
        ),
        ("IsNullGrammar".into(), Ref::keyword("ISNULL").to_matchable().into()),
        ("NotNullGrammar".into(), Ref::keyword("NOTNULL").to_matchable().into()),
        (
            "JoinKeywordsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("JOIN"),
                Sequence::new(vec_of_erased![Ref::keyword("LATERAL")])
                    .config(|this| this.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("INTO"),
                Ref::keyword("FROM"),
                Ref::keyword("WHERE"),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY"),]),
                Ref::keyword("LIMIT"),
                Ref::new("CommaSegment"),
                Ref::new("SetOperatorSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        // Assuming the existence of `ansi_dialect` in Rust and a way to manipulate its grammar:
        (
            "LiteralGrammar".into(),
            postgres
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec_of_erased![
                        Ref::new("DollarNumericLiteralSegment"),
                        Ref::new("PsqlVariableGrammar")
                    ]),
                    None,
                    Some(Ref::new("ArrayLiteralSegment").to_matchable()),
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            postgres
                .grammar("FromClauseTerminatorGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("ForClauseSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY"),]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY"),]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::keyword("OVERLAPS"),
                Ref::keyword("RETURNING"),
                Sequence::new(vec_of_erased![Ref::keyword("ON"), Ref::keyword("CONFLICT"),]),
                Ref::new("ForClauseSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrderByClauseTerminators".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::new("FrameClauseUnitGrammar"),
                Ref::keyword("SEPARATOR"),
                Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("DATA"),]),
                Ref::new("ForClauseSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("ArrayAccessorSegment"),
                Ref::new("SemiStructuredAccessorSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "NonWithSelectableGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("SetExpressionSegment"),
                optionally_bracketed(vec_of_erased![Ref::new("SelectStatementSegment")]),
                Ref::new("NonSetSelectableGrammar"),
                Ref::new("UpdateStatementSegment"),
                Ref::new("InsertStatementSegment"),
                Ref::new("DeleteStatementSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        ("NonWithNonSelectableGrammar".into(), one_of(vec_of_erased![]).to_matchable().into()),
    ]);

    macro_rules! add_segments {
        ($dialect:ident, $( $segment:ident ),*) => {
            $(
                $dialect.add([(
                    stringify!($segment).into(),
                    Node::<$segment>::new().to_matchable().into(),
                )]);
            )*
        }
    }

    add_segments!(
        postgres,
        OverlapSegment,
        NotExtendRightSegment,
        NotExtendLeftSegment,
        AdjacentSegment,
        PsqlVariableGrammar,
        ArrayAccessorSegment,
        DateTimeTypeIdentifier,
        DateTimeLiteralGrammar,
        DatatypeSegment,
        ArrayTypeSegment,
        IndexAccessMethodSegment,
        OperatorClassReferenceSegment,
        DefinitionParameterSegment,
        DefinitionParametersSegment,
        CreateCastStatementSegment,
        DropCastStatementSegment,
        RelationOptionSegment,
        RelationOptionsSegment,
        CreateFunctionStatementSegment,
        DropFunctionStatementSegment,
        AlterFunctionStatementSegment,
        AlterFunctionActionSegment,
        AlterProcedureActionSegment,
        AlterProcedureStatementSegment,
        CreateProcedureStatementSegment,
        DropProcedureStatementSegment,
        WellKnownTextGeometrySegment,
        SemiStructuredAccessorSegment,
        FunctionDefinitionGrammar,
        IntoClauseSegment,
        ForClauseSegment,
        UnorderedSelectStatementSegment,
        SelectStatementSegment,
        SelectClauseSegment,
        SelectClauseModifierSegment,
        WithinGroupClauseSegment,
        GroupByClauseSegment,
        CreateRoleStatementSegment,
        AlterRoleStatementSegment,
        ExplainStatementSegment,
        ExplainOptionSegment,
        CreateSchemaStatementSegment,
        CreateTableStatementSegment,
        CreateTableAsStatementSegment,
        AlterTableStatementSegment,
        AlterTableActionSegment,
        VersionIdentifierSegment,
        CreateExtensionStatementSegment,
        DropExtensionStatementSegment,
        PublicationReferenceSegment,
        PublicationTableSegment,
        PublicationObjectsSegment,
        CreatePublicationStatementSegment,
        AlterPublicationStatementSegment,
        DropPublicationStatementSegment,
        CreateMaterializedViewStatementSegment,
        AlterMaterializedViewStatementSegment,
        AlterMaterializedViewActionSegment,
        RefreshMaterializedViewStatementSegment,
        DropMaterializedViewStatementSegment,
        WithCheckOptionSegment,
        AlterPolicyStatementSegment,
        CreateViewStatementSegment,
        AlterViewStatementSegment,
        DropViewStatementSegment,
        CreateDatabaseStatementSegment,
        AlterDatabaseStatementSegment,
        DropDatabaseStatementSegment,
        VacuumStatementSegment,
        LikeOptionSegment,
        ColumnConstraintSegment,
        PartitionBoundSpecSegment,
        TableConstraintSegment,
        TableConstraintUsingIndexSegment,
        IndexParametersSegment,
        IndexElementOptionsSegment,
        IndexElementSegment,
        ExclusionConstraintElementSegment,
        AlterDefaultPrivilegesStatementSegment,
        AlterDefaultPrivilegesObjectPrivilegesSegment,
        AlterDefaultPrivilegesSchemaObjectsSegment,
        AlterDefaultPrivilegesToFromRolesSegment,
        AlterDefaultPrivilegesGrantSegment,
        AlterDefaultPrivilegesRevokeSegment,
        DropOwnedStatementSegment,
        ReassignOwnedStatementSegment,
        CommentOnStatementSegment,
        CreateIndexStatementSegment,
        AlterIndexStatementSegment,
        ReindexStatementSegment,
        DropIndexStatementSegment,
        FrameClauseSegment,
        CreateSequenceOptionsSegment,
        CreateSequenceStatementSegment,
        AlterSequenceOptionsSegment,
        AlterSequenceStatementSegment,
        DropSequenceStatementSegment,
        AnalyzeStatementSegment,
        StatementSegment,
        CreateTriggerStatementSegment,
        AlterTriggerStatementSegment,
        DropTriggerStatementSegment,
        AliasExpressionSegment,
        AsAliasExpressionSegment,
        OperationClassReferenceSegment,
        ConflictActionSegment,
        ConflictTargetSegment,
        InsertStatementSegment,
        DropTypeStatementSegment,
        SetStatementSegment,
        CreatePolicyStatementSegment,
        CallStoredProcedureSegment,
        CreateDomainStatementSegment,
        AlterDomainStatementSegment,
        DropDomainStatementSegment,
        DropPolicyStatementSegment,
        LoadStatementSegment,
        ResetStatementSegment,
        DiscardStatementSegment,
        ListenStatementSegment,
        NotifyStatementSegment,
        UnlistenStatementSegment,
        TruncateStatementSegment,
        CopyStatementSegment,
        LanguageClauseSegment,
        DoStatementSegment,
        CTEDefinitionSegment,
        ValuesClauseSegment,
        DeleteStatementSegment,
        SetClauseSegment,
        UpdateStatementSegment,
        CreateTypeStatementSegment,
        AlterTypeStatementSegment,
        CreateCollationStatementSegment,
        AlterSchemaStatementSegment,
        LockTableStatementSegment,
        ClusterStatementSegment,
        ColumnReferenceSegment,
        NamedArgumentSegment,
        TableExpressionSegment,
        ServerReferenceSegment,
        CreateServerStatementSegment,
        CreateUserMappingStatementSegment,
        ImportForeignSchemaStatementSegment
    );

    postgres.expand();
    postgres
}

pub struct OverlapSegment {}

impl NodeTrait for OverlapSegment {
    const TYPE: &'static str = "comparison_operator";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::new("AmpersandSegment"), Ref::new("AmpersandSegment")])
            .to_matchable()
    }
}

pub struct NotExtendRightSegment {}

impl NodeTrait for NotExtendRightSegment {
    const TYPE: &'static str = "comparison_operator";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("AmpersandSegment"),
            Ref::new("RawGreaterThanSegment")
        ])
        .allow_gaps(false)
        .to_matchable()
    }
}

pub struct NotExtendLeftSegment {}

impl NodeTrait for NotExtendLeftSegment {
    const TYPE: &'static str = "comparison_operator";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::new("AmpersandSegment"), Ref::new("RawLessThanSegment")])
            .allow_gaps(false)
            .to_matchable()
    }
}

pub struct AdjacentSegment {}

impl NodeTrait for AdjacentSegment {
    const TYPE: &'static str = "comparison_operator";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("MinusSegment"),
            Ref::new("PipeSegment"),
            Ref::new("MinusSegment")
        ])
        .allow_gaps(false)
        .to_matchable()
    }
}

pub struct PsqlVariableGrammar {}

impl NodeTrait for PsqlVariableGrammar {
    const TYPE: &'static str = "psql_variable";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![optionally_bracketed(vec_of_erased![
            Ref::new("ColonSegment"),
            one_of(vec_of_erased![
                Ref::new("ParameterNameSegment"),
                Ref::new("QuotedLiteralSegment")
            ])
        ])])
        .to_matchable()
    }
}

pub struct ArrayAccessorSegment;

impl NodeTrait for ArrayAccessorSegment {
    const TYPE: &'static str = "array_accessor";

    fn match_grammar() -> Arc<dyn Matchable> {
        Bracketed::new(vec_of_erased![one_of(vec_of_erased![
            // These three are for a single element access: [n]
            Ref::new("QualifiedNumericLiteralSegment"),
            Ref::new("NumericLiteralSegment"),
            Ref::new("ExpressionSegment"),
            // This is for slice access: [n:m], [:m], [n:], and [:]
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("QualifiedNumericLiteralSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment"),
                ])
                .config(|this| this.optional()),
                Ref::new("SliceSegment"),
                one_of(vec_of_erased![
                    Ref::new("QualifiedNumericLiteralSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment"),
                ])
                .config(|this| this.optional()),
            ]),
        ])])
        .config(|this| {
            this.bracket_type("square");
        })
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["array_accessor"].into()
    }
}

pub struct DateTimeTypeIdentifier;

impl NodeTrait for DateTimeTypeIdentifier {
    const TYPE: &'static str = "datetime_type_identifier";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::keyword("DATE"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("TIME"), Ref::keyword("TIMESTAMP")]),
                Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("WITHOUT")]),
                    Ref::keyword("TIME"),
                    Ref::keyword("ZONE")
                ])
                .config(|this| this.optional())
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("INTERVAL"),
                    Ref::keyword("TIMETZ"),
                    Ref::keyword("TIMESTAMPTZ")
                ]),
                Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                    .config(|this| this.optional())
            ])
        ])
        .to_matchable()
    }
}

pub struct DateTimeLiteralGrammar;

impl NodeTrait for DateTimeLiteralGrammar {
    const TYPE: &'static str = "datetime_literal";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("DateTimeTypeIdentifier").optional(),
            Ref::new("QuotedLiteralSegment")
        ])
        .to_matchable()
    }
}

pub struct DatatypeSegment;

impl NodeTrait for DatatypeSegment {
    const TYPE: &'static str = "data_type";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("DotSegment")
            ])
            .config(|this| {
                this.allow_gaps = false;
                this.optional();
            }),
            one_of(vec_of_erased![
                Ref::new("WellKnownTextGeometrySegment"),
                Ref::new("DateTimeTypeIdentifier"),
                Sequence::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::keyword("SMALLINT"),
                    Ref::keyword("INTEGER"),
                    Ref::keyword("INT"),
                    Ref::keyword("INT2"),
                    Ref::keyword("INT4"),
                    Ref::keyword("INT8"),
                    Ref::keyword("BIGINT"),
                    Ref::keyword("FLOAT4"),
                    Ref::keyword("FLOAT8"),
                    Ref::keyword("REAL"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DOUBLE"),
                        Ref::keyword("PRECISION")
                    ]),
                    Ref::keyword("SMALLSERIAL"),
                    Ref::keyword("SERIAL"),
                    Ref::keyword("SERIAL2"),
                    Ref::keyword("SERIAL4"),
                    Ref::keyword("SERIAL8"),
                    Ref::keyword("BIGSERIAL"),
                    // Numeric types [(precision)]
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("FLOAT")]),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    // Numeric types [precision ["," scale])]
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("DECIMAL"), Ref::keyword("NUMERIC")]),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    // Monetary type
                    Ref::keyword("MONEY"),
                    // Character types
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("BPCHAR"),
                                Ref::keyword("CHAR"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("CHAR"),
                                    Ref::keyword("VARYING")
                                ]),
                                Ref::keyword("CHARACTER"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("CHARACTER"),
                                    Ref::keyword("VARYING")
                                ]),
                                Ref::keyword("VARCHAR")
                            ]),
                            Ref::new("BracketedArguments").optional()
                        ]),
                        Ref::keyword("TEXT")
                    ]),
                    // Binary type
                    Ref::keyword("BYTEA"),
                    // Boolean types
                    one_of(vec_of_erased![Ref::keyword("BOOLEAN"), Ref::keyword("BOOL")]),
                    // Geometric types
                    one_of(vec_of_erased![
                        Ref::keyword("POINT"),
                        Ref::keyword("LINE"),
                        Ref::keyword("LSEG"),
                        Ref::keyword("BOX"),
                        Ref::keyword("PATH"),
                        Ref::keyword("POLYGON"),
                        Ref::keyword("CIRCLE")
                    ]),
                    // Network address types
                    one_of(vec_of_erased![
                        Ref::keyword("CIDR"),
                        Ref::keyword("INET"),
                        Ref::keyword("MACADDR"),
                        Ref::keyword("MACADDR8")
                    ]),
                    // Text search types
                    one_of(vec_of_erased![Ref::keyword("TSVECTOR"), Ref::keyword("TSQUERY")]),
                    // Bit string types
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BIT"),
                        one_of(vec_of_erased![Ref::keyword("VARYING")])
                            .config(|this| this.optional()),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    // UUID type
                    Ref::keyword("UUID"),
                    // XML type
                    Ref::keyword("XML"),
                    // JSON types
                    one_of(vec_of_erased![Ref::keyword("JSON"), Ref::keyword("JSONB")]),
                    // Range types
                    Ref::keyword("INT4RANGE"),
                    Ref::keyword("INT8RANGE"),
                    Ref::keyword("NUMRANGE"),
                    Ref::keyword("TSRANGE"),
                    Ref::keyword("TSTZRANGE"),
                    Ref::keyword("DATERANGE"),
                    // pg_lsn type
                    Ref::keyword("PG_LSN")
                ])]),
                Ref::new("DatatypeIdentifierSegment")
            ]),
            one_of(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment").optional()])
                        .config(|this| this.bracket_type("square"))
                ]),
                Ref::new("ArrayTypeSegment"),
                Ref::new("SizedArrayTypeSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct ArrayTypeSegment;

impl NodeTrait for ArrayTypeSegment {
    const TYPE: &'static str = "array_type";

    fn match_grammar() -> Arc<dyn Matchable> {
        Ref::keyword("ARRAY").to_matchable()
    }
}

pub struct IndexAccessMethodSegment;

impl NodeTrait for IndexAccessMethodSegment {
    const TYPE: &'static str = "index_access_method";

    fn match_grammar() -> Arc<dyn Matchable> {
        Ref::new("SingleIdentifierGrammar").to_matchable()
    }
}

pub struct OperatorClassReferenceSegment;

impl NodeTrait for OperatorClassReferenceSegment {
    const TYPE: &'static str = "operator_class_reference";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::ObjectReferenceSegment::match_grammar()
    }
}

pub struct DefinitionParameterSegment;

impl NodeTrait for DefinitionParameterSegment {
    const TYPE: &'static str = "definition_parameter";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("PropertiesNakedIdentifierSegment"),
            Sequence::new(vec_of_erased![
                Ref::new("EqualsSegment"),
                Ref::new("DefinitionArgumentValueGrammar").optional()
            ])
        ])
        .to_matchable()
    }
}

pub struct DefinitionParametersSegment;

impl NodeTrait for DefinitionParametersSegment {
    const TYPE: &'static str = "definition_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
            "DefinitionParameterSegment"
        )])])
        .to_matchable()
    }
}

pub struct CreateCastStatementSegment;

impl NodeTrait for CreateCastStatementSegment {
    const TYPE: &'static str = "create_cast_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("CAST"),
            Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("DatatypeSegment"),
                Ref::keyword("AS"),
                Ref::new("DatatypeSegment"),
            ])]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("FUNCTION"),
                    Ref::new("FunctionNameSegment"),
                    Ref::new("FunctionParameterListGrammar").optional(),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("WITHOUT"), Ref::keyword("FUNCTION")]),
                Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("INOUT")]),
            ]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::keyword("ASSIGNMENT")])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::keyword("IMPLICIT")])
                    .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct DropCastStatementSegment;

impl NodeTrait for DropCastStatementSegment {
    const TYPE: &'static str = "drop_cast_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("CAST"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS")])
                .config(|this| this.optional()),
            Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("DatatypeSegment"),
                Ref::keyword("AS"),
                Ref::new("DatatypeSegment"),
            ])]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct RelationOptionSegment;

impl NodeTrait for RelationOptionSegment {
    const TYPE: &'static str = "relation_option";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("PropertiesNakedIdentifierSegment"),
            Sequence::new(vec_of_erased![
                Ref::new("DotSegment"),
                Ref::new("PropertiesNakedIdentifierSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::new("EqualsSegment"),
                Ref::new("DefinitionArgumentValueGrammar").optional(),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct RelationOptionsSegment;

impl NodeTrait for RelationOptionsSegment {
    const TYPE: &'static str = "relation_options";

    fn match_grammar() -> Arc<dyn Matchable> {
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
            "RelationOptionSegment"
        )])])
        .to_matchable()
    }
}

pub struct CreateFunctionStatementSegment;

impl NodeTrait for CreateFunctionStatementSegment {
    const TYPE: &'static str = "create_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE")])
                .config(|this| this.optional()),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("FUNCTION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
            Sequence::new(vec_of_erased![
                Ref::keyword("RETURNS"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TABLE"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                            vec_of_erased![
                                Ref::new("DatatypeSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::new("ColumnReferenceSegment"),
                                    Ref::new("DatatypeSegment"),
                                ]),
                            ]
                        )])])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SETOF"),
                        Ref::new("DatatypeSegment"),
                    ]),
                    Ref::new("DatatypeSegment"),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
            Ref::new("FunctionDefinitionGrammar"),
        ])
        .to_matchable()
    }
}

pub struct DropFunctionStatementSegment;

impl NodeTrait for DropFunctionStatementSegment {
    const TYPE: &'static str = "drop_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("FUNCTION"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Ref::new("FunctionParameterListGrammar").optional(),
            ])]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterFunctionStatementSegment;

impl NodeTrait for AlterFunctionStatementSegment {
    const TYPE: &'static str = "alter_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("FUNCTION"),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Ref::new("FunctionParameterListGrammar").optional(),
            ])]),
            one_of(vec_of_erased![
                Ref::new("AlterFunctionActionSegment").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("FunctionNameSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Ref::new("QuotedIdentifierSegment")
                        ]),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER")
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NO").optional(),
                    Ref::keyword("DEPENDS"),
                    Ref::keyword("ON"),
                    Ref::keyword("EXTENSION"),
                    Ref::new("ExtensionReferenceSegment")
                ])
            ])
        ])
        .to_matchable()
    }
}

pub struct AlterFunctionActionSegment;

impl NodeTrait for AlterFunctionActionSegment {
    const TYPE: &'static str = "alter_function_action_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CALLED"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETURNS"),
                        Ref::keyword("NULL"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Ref::keyword("STRICT"),
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("IMMUTABLE"),
                    Ref::keyword("STABLE"),
                    Ref::keyword("VOLATILE"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT").optional(),
                    Ref::keyword("LEAKPROOF"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXTERNAL").optional(),
                    Ref::keyword("SECURITY"),
                    one_of(vec_of_erased![Ref::keyword("DEFINER"), Ref::keyword("INVOKER"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARALLEL"),
                    one_of(vec_of_erased![
                        Ref::keyword("UNSAFE"),
                        Ref::keyword("RESTRICTED"),
                        Ref::keyword("SAFE"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COST"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROWS"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SUPPORT"),
                    Ref::new("ParameterNameSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("ParameterNameSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("TO"), Ref::new("EqualsSegment"),]),
                            one_of(vec_of_erased![
                                Ref::new("LiteralGrammar"),
                                Ref::new("NakedIdentifierSegment"),
                                Ref::keyword("DEFAULT"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::keyword("CURRENT"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESET"),
                    one_of(vec_of_erased![Ref::keyword("ALL"), Ref::new("ParameterNameSegment"),]),
                ]),
            ]),
            Ref::keyword("RESTRICT").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterProcedureActionSegment;

impl NodeTrait for AlterProcedureActionSegment {
    const TYPE: &'static str = "alter_procedure_action_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXTERNAL").optional(),
                    Ref::keyword("SECURITY"),
                    one_of(vec_of_erased![Ref::keyword("DEFINER"), Ref::keyword("INVOKER"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("ParameterNameSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("TO"), Ref::new("EqualsSegment"),]),
                            one_of(vec_of_erased![
                                Ref::new("LiteralGrammar"),
                                Ref::new("NakedIdentifierSegment"),
                                Ref::keyword("DEFAULT"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::keyword("CURRENT"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESET"),
                    one_of(vec_of_erased![Ref::keyword("ALL"), Ref::new("ParameterNameSegment"),]),
                ]),
            ]),
            Ref::keyword("RESTRICT").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterProcedureStatementSegment;

impl NodeTrait for AlterProcedureStatementSegment {
    const TYPE: &'static str = "alter_procedure_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("PROCEDURE"),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Ref::new("FunctionParameterListGrammar").optional(),
            ])]),
            one_of(vec_of_erased![
                Ref::new("AlterProcedureActionSegment").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("FunctionNameSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("ParameterNameSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("TO"), Ref::new("EqualsSegment"),]),
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                                Ref::new("LiteralGrammar"),
                            ])]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::keyword("CURRENT"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Ref::new("QuotedIdentifierSegment"),
                        ]),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NO").optional(),
                    Ref::keyword("DEPENDS"),
                    Ref::keyword("ON"),
                    Ref::keyword("EXTENSION"),
                    Ref::new("ExtensionReferenceSegment"),
                ]),
            ])
        ])
        .to_matchable()
    }
}

pub struct CreateProcedureStatementSegment;

impl NodeTrait for CreateProcedureStatementSegment {
    const TYPE: &'static str = "create_procedure_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE")])
                .config(|this| this.optional()),
            Ref::keyword("PROCEDURE"),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
            Ref::new("FunctionDefinitionGrammar"),
        ])
        .to_matchable()
    }
}

pub struct DropProcedureStatementSegment;

impl NodeTrait for DropProcedureStatementSegment {
    const TYPE: &'static str = "drop_procedure_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("PROCEDURE"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Ref::new("FunctionParameterListGrammar").optional(),
            ])]),
            one_of(vec_of_erased![Ref::keyword("CASCADE"), Ref::keyword("RESTRICT")])
                .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct WellKnownTextGeometrySegment;

impl NodeTrait for WellKnownTextGeometrySegment {
    const TYPE: &'static str = "wkt_geometry_type";

    fn match_grammar() -> Arc<dyn Matchable> {
        let geometry_type_keywords = POSTGRES_POSTGIS_DATATYPE_KEYWORDS
            .iter()
            .map(|(kw, _)| Ref::keyword(kw).to_matchable())
            .collect_vec();

        let mut geometry_type_keywords0 = geometry_type_keywords.clone();
        geometry_type_keywords0.extend(
            ["GEOMETRY", "GEOGRAPHY"].into_iter().map(|it| Ref::keyword(it).to_matchable()),
        );

        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                one_of(geometry_type_keywords.clone()),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SimpleGeometryGrammar"
                    )])]),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Bracketed::new(
                        vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SimpleGeometryGrammar"
                        )])]
                    )])]),
                    Ref::new("WellKnownTextGeometrySegment"),
                ])]),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("GEOMETRY"), Ref::keyword("GEOGRAPHY")]),
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(geometry_type_keywords0),
                    Ref::new("CommaSegment"),
                    Ref::new("NumericLiteralSegment"),
                ])]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct SemiStructuredAccessorSegment;

impl NodeTrait for SemiStructuredAccessorSegment {
    const TYPE: &'static str = "semi_structured_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("DotSegment"),
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("ArrayAccessorSegment").optional(),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("DotSegment"),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                Ref::new("ArrayAccessorSegment").optional(),
            ])
        ])
        .to_matchable()
    }
}

pub struct FunctionDefinitionGrammar;

impl NodeTrait for FunctionDefinitionGrammar {
    const TYPE: &'static str = "function_definition";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            AnyNumberOf::new(vec_of_erased![
                Ref::new("LanguageClauseSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TRANSFORM"),
                    Ref::keyword("FOR"),
                    Ref::keyword("TYPE"),
                    Ref::new("ParameterNameSegment"),
                ]),
                Ref::keyword("WINDOW"),
                one_of(vec_of_erased![
                    Ref::keyword("IMMUTABLE"),
                    Ref::keyword("STABLE"),
                    Ref::keyword("VOLATILE"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT").optional(),
                    Ref::keyword("LEAKPROOF"),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CALLED"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETURNS"),
                        Ref::keyword("NULL"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Ref::keyword("STRICT"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXTERNAL").optional(),
                    Ref::keyword("SECURITY"),
                    one_of(vec_of_erased![Ref::keyword("INVOKER"), Ref::keyword("DEFINER"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARALLEL"),
                    one_of(vec_of_erased![
                        Ref::keyword("UNSAFE"),
                        Ref::keyword("RESTRICTED"),
                        Ref::keyword("SAFE"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COST"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROWS"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SUPPORT"),
                    Ref::new("ParameterNameSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("ParameterNameSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("TO"), Ref::new("EqualsSegment"),]),
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                                Ref::new("LiteralGrammar"),
                            ]),]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::keyword("CURRENT"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("CommaSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ParameterNameSegment"
                )])]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct IntoClauseSegment;

impl NodeTrait for IntoClauseSegment {
    const TYPE: &'static str = "into_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INTO"),
            one_of(vec_of_erased![
                Ref::keyword("TEMPORARY"),
                Ref::keyword("TEMP"),
                Ref::keyword("UNLOGGED"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("TABLE").optional(),
            Ref::new("TableReferenceSegment"),
        ])
        .to_matchable()
    }
}

pub struct ForClauseSegment;

impl NodeTrait for ForClauseSegment {
    const TYPE: &'static str = "for_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("FOR"),
            one_of(vec_of_erased![
                Ref::keyword("UPDATE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NO"),
                    Ref::keyword("KEY"),
                    Ref::keyword("UPDATE"),
                ]),
                Ref::keyword("SHARE"),
                Sequence::new(vec_of_erased![Ref::keyword("KEY"), Ref::keyword("SHARE")]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("OF"),
                Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment")]),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Ref::keyword("NOWAIT"),
                Sequence::new(vec_of_erased![Ref::keyword("SKIP"), Ref::keyword("LOCKED")]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct UnorderedSelectStatementSegment;

impl NodeTrait for UnorderedSelectStatementSegment {
    const TYPE: &'static str = "unordered_select_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::UnorderedSelectStatementSegment::match_grammar().copy(
            Some(vec![Ref::new("IntoClauseSegment").optional().to_matchable()]),
            None,
            Some(Ref::new("FromClauseSegment").optional().to_matchable()),
            None,
            vec![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("NO").optional(),
                    Ref::keyword("DATA")
                ])
                .to_matchable(),
                Sequence::new(vec_of_erased![Ref::keyword("ON"), Ref::keyword("CONFLICT")])
                    .to_matchable(),
                Ref::keyword("RETURNING").to_matchable(),
                Ref::new("WithCheckOptionSegment").to_matchable(),
            ],
            false,
        )
    }
}

pub struct SelectStatementSegment;

impl NodeTrait for SelectStatementSegment {
    const TYPE: &'static str = "select_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        let initial_copy = UnorderedSelectStatementSegment::match_grammar().copy(
            Some(vec![
                Ref::new("OrderByClauseSegment").optional().to_matchable(),
                Ref::new("LimitClauseSegment").optional().to_matchable(),
                Ref::new("NamedWindowSegment").optional().to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        );

        initial_copy.copy(
            Some(vec![Ref::new("ForClauseSegment").optional().to_matchable()]),
            None,
            Some(Ref::new("LimitClauseSegment").optional().to_matchable()),
            None,
            vec![
                Ref::new("SetOperatorSegment").to_matchable(),
                Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
                Ref::new("WithDataClauseSegment").to_matchable(),
                Sequence::new(vec_of_erased![Ref::keyword("ON"), Ref::keyword("CONFLICT")])
                    .to_matchable(),
                Ref::keyword("RETURNING").to_matchable(),
                Ref::new("WithCheckOptionSegment").to_matchable(),
            ],
            true,
        )
    }
}

pub struct SelectClauseSegment;

impl NodeTrait for SelectClauseSegment {
    const TYPE: &'static str = "select_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("SELECT"),
            Ref::new("SelectClauseModifierSegment").optional(),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")]).config(|this| {
                this.optional();
                this.allow_trailing = true;
            })
        ])
        .config(|this| {
            this.terminators = vec_of_erased![
                Ref::keyword("INTO"),
                Ref::keyword("FROM"),
                Ref::keyword("WHERE"),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("LIMIT"),
                Ref::keyword("OVERLAPS"),
                Ref::new("SetOperatorSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("NO").optional(),
                    Ref::keyword("DATA"),
                ]),
                Ref::new("WithCheckOptionSegment"),
            ];
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable()
    }
}

pub struct SelectClauseModifierSegment;

impl NodeTrait for SelectClauseModifierSegment {
    const TYPE: &'static str = "select_clause_modifier";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("DISTINCT"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    )])])
                    .config(|this| this.optional()),
                ]),
            ]),
            Ref::keyword("ALL"),
        ])
        .to_matchable()
    }
}

pub struct WithinGroupClauseSegment;

impl NodeTrait for WithinGroupClauseSegment {
    const TYPE: &'static str = "withingroup_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WITHIN"),
            Ref::keyword("GROUP"),
            Bracketed::new(vec_of_erased![Ref::new("OrderByClauseSegment").optional()])
        ])
        .to_matchable()
    }
}

pub struct GroupByClauseSegment;

impl NodeTrait for GroupByClauseSegment {
    const TYPE: &'static str = "groupby_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("GROUP"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("ColumnReferenceSegment"),
                Ref::new("NumericLiteralSegment"),
                Ref::new("CubeRollupClauseSegment"),
                Ref::new("GroupingSetsClauseSegment"),
                Ref::new("ExpressionSegment"),
                Bracketed::new(vec_of_erased![])
            ])])
            .config(|this| {
                this.terminators = vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                    Ref::keyword("LIMIT"),
                    Ref::keyword("HAVING"),
                    Ref::keyword("QUALIFY"),
                    Ref::keyword("WINDOW"),
                    Ref::new("SetOperatorSegment")
                ];
            }),
            MetaSegment::dedent()
        ])
        .to_matchable()
    }
}

pub struct CreateRoleStatementSegment;

impl NodeTrait for CreateRoleStatementSegment {
    const TYPE: &'static str = "create_role_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![Ref::keyword("ROLE"), Ref::keyword("USER"),]),
            Ref::new("RoleReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH").optional(),
                any_set_of(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("SUPERUSER"), Ref::keyword("NOSUPERUSER"),]),
                    one_of(vec_of_erased![Ref::keyword("CREATEDB"), Ref::keyword("NOCREATEDB"),]),
                    one_of(vec_of_erased![
                        Ref::keyword("CREATEROLE"),
                        Ref::keyword("NOCREATEROLE"),
                    ]),
                    one_of(vec_of_erased![Ref::keyword("INHERIT"), Ref::keyword("NOINHERIT"),]),
                    one_of(vec_of_erased![Ref::keyword("LOGIN"), Ref::keyword("NOLOGIN"),]),
                    one_of(vec_of_erased![
                        Ref::keyword("REPLICATION"),
                        Ref::keyword("NOREPLICATION"),
                    ]),
                    one_of(vec_of_erased![Ref::keyword("BYPASSRLS"), Ref::keyword("NOBYPASSRLS"),]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONNECTION"),
                        Ref::keyword("LIMIT"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PASSWORD"),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::keyword("NULL"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("VALID"),
                        Ref::keyword("UNTIL"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IN"),
                        Ref::keyword("ROLE"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IN"),
                        Ref::keyword("GROUP"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ROLE"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ADMIN"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USER"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SYSID"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterRoleStatementSegment;

impl NodeTrait for AlterRoleStatementSegment {
    const TYPE: &'static str = "alter_role_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            one_of(vec_of_erased![Ref::keyword("ROLE"), Ref::keyword("USER"),]),
            one_of(vec_of_erased![
                // role_specification
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Ref::keyword("WITH").optional(),
                    any_set_of(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("SUPERUSER"),
                            Ref::keyword("NOSUPERUSER"),
                        ]),
                        one_of(vec_of_erased![
                            Ref::keyword("CREATEDB"),
                            Ref::keyword("NOCREATEDB"),
                        ]),
                        one_of(vec_of_erased![
                            Ref::keyword("CREATEROLE"),
                            Ref::keyword("NOCREATEROLE"),
                        ]),
                        one_of(vec_of_erased![Ref::keyword("INHERIT"), Ref::keyword("NOINHERIT"),]),
                        one_of(vec_of_erased![Ref::keyword("LOGIN"), Ref::keyword("NOLOGIN"),]),
                        one_of(vec_of_erased![
                            Ref::keyword("REPLICATION"),
                            Ref::keyword("NOREPLICATION"),
                        ]),
                        one_of(vec_of_erased![
                            Ref::keyword("BYPASSRLS"),
                            Ref::keyword("NOBYPASSRLS"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CONNECTION"),
                            Ref::keyword("LIMIT"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ENCRYPTED").optional(),
                            Ref::keyword("PASSWORD"),
                            one_of(vec_of_erased![
                                Ref::new("QuotedLiteralSegment"),
                                Ref::keyword("NULL"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("VALID"),
                            Ref::keyword("UNTIL"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ]),
                ]),
                // name only
                Sequence::new(vec_of_erased![
                    Ref::new("RoleReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RENAME"),
                        Ref::keyword("TO"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                ]),
                // role_specification | all
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                        Ref::keyword("ALL"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IN"),
                        Ref::keyword("DATABASE"),
                        Ref::new("DatabaseReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::new("ParameterNameSegment"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("TO"),
                                        Ref::new("EqualsSegment"),
                                    ]),
                                    one_of(vec_of_erased![
                                        Ref::keyword("DEFAULT"),
                                        Delimited::new(vec_of_erased![
                                            Ref::new("LiteralGrammar"),
                                            Ref::new("NakedIdentifierSegment"),
                                            Ref::new("OnKeywordAsIdentifierSegment"),
                                        ]),
                                    ]),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FROM"),
                                    Ref::keyword("CURRENT"),
                                ]),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESET"),
                            one_of(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                                Ref::keyword("ALL"),
                            ]),
                        ]),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ExplainStatementSegment;

impl NodeTrait for ExplainStatementSegment {
    const TYPE: &'static str = "explain_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("EXPLAIN"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ANALYZE").optional(),
                        Ref::keyword("ANALYSE").optional(),
                    ]),
                    Ref::keyword("VERBOSE").optional(),
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ExplainOptionSegment"
                )])]),
            ])
            .config(|this| this.optional()),
            ansi::ExplainStatementSegment::explainable_stmt(),
        ])
        .to_matchable()
    }
}

pub struct ExplainOptionSegment;

impl NodeTrait for ExplainOptionSegment {
    const TYPE: &'static str = "explain_option";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ANALYZE"),
                    Ref::keyword("ANALYSE"),
                    Ref::keyword("VERBOSE"),
                    Ref::keyword("COSTS"),
                    Ref::keyword("SETTINGS"),
                    Ref::keyword("BUFFERS"),
                    Ref::keyword("WAL"),
                    Ref::keyword("TIMING"),
                    Ref::keyword("SUMMARY"),
                ]),
                Ref::new("BooleanLiteralGrammar").optional(),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FORMAT"),
                one_of(vec_of_erased![
                    Ref::keyword("TEXT"),
                    Ref::keyword("XML"),
                    Ref::keyword("JSON"),
                    Ref::keyword("YAML"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateSchemaStatementSegment;

impl NodeTrait for CreateSchemaStatementSegment {
    const TYPE: &'static str = "create_schema_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("SCHEMA"),
            Ref::new("IfNotExistsGrammar").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("SchemaReferenceSegment").optional(),
                    Ref::keyword("AUTHORIZATION"),
                    Ref::new("RoleReferenceSegment"),
                ]),
                Ref::new("SchemaReferenceSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateTableStatementSegment;

impl NodeTrait for CreateTableStatementSegment {
    const TYPE: &'static str = "create_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("GLOBAL"), Ref::keyword("LOCAL"),])
                        .config(|this| this.optional()),
                    Ref::new("TemporaryGrammar").optional(),
                ]),
                Ref::keyword("UNLOGGED"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("TABLE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                Ref::new("DatatypeSegment"),
                                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                                    Ref::new("ColumnConstraintSegment"),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("COLLATE"),
                                        Ref::new("CollationReferenceSegment"),
                                    ]),
                                ]),]),
                            ]),
                            Ref::new("TableConstraintSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("LIKE"),
                                Ref::new("TableReferenceSegment"),
                                AnyNumberOf::new(vec_of_erased![Ref::new("LikeOptionSegment"),])
                                    .config(|this| this.optional()),
                            ]),
                        ]),])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INHERITS"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "TableReferenceSegment"
                        ),]),]),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OF"),
                    Ref::new("ParameterNameSegment"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("WITH"),
                                    Ref::keyword("OPTIONS"),
                                ])
                                .config(|this| this.optional()),
                                AnyNumberOf::new(vec_of_erased![Ref::new(
                                    "ColumnConstraintSegment"
                                ),]),
                            ]),
                            Ref::new("TableConstraintSegment"),
                        ])
                        .config(|this| this.optional()),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Ref::keyword("OF"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Ref::keyword("OPTIONS"),
                            ])
                            .config(|this| this.optional()),
                            AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment"),]),
                        ]),
                        Ref::new("TableConstraintSegment"),
                    ]),])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FOR"),
                            Ref::keyword("VALUES"),
                            Ref::new("PartitionBoundSpecSegment"),
                        ]),
                        Ref::keyword("DEFAULT"),
                    ]),
                ]),
            ]),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Ref::keyword("RANGE"),
                        Ref::keyword("LIST"),
                        Ref::keyword("HASH"),
                    ]),
                    Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                Ref::new("FunctionSegment"),
                            ]),
                            AnyNumberOf::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("COLLATE"),
                                    Ref::new("CollationReferenceSegment"),
                                ])
                                .config(|this| this.optional()),
                                Ref::new("ParameterNameSegment").optional(),
                            ]),
                        ]),]),
                    ]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Ref::new("ParameterNameSegment"),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::new("RelationOptionsSegment"),
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("WITHOUT"), Ref::keyword("OIDS"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("COMMIT"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PRESERVE"),
                            Ref::keyword("ROWS"),
                        ]),
                        Sequence::new(
                            vec_of_erased![Ref::keyword("DELETE"), Ref::keyword("ROWS"),]
                        ),
                        Ref::keyword("DROP"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateTableAsStatementSegment;

impl NodeTrait for CreateTableAsStatementSegment {
    const TYPE: &'static str = "create_table_as_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("GLOBAL"), Ref::keyword("LOCAL")])
                        .config(|this| this.optional()),
                    Ref::new("TemporaryGrammar"),
                ]),
                Ref::keyword("UNLOGGED"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("TABLE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![Delimited::new(
                    vec_of_erased![Ref::new("ColumnReferenceSegment"),]
                )])])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Ref::new("ParameterNameSegment"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![
                                        Ref::new("LiteralGrammar"),
                                        Ref::new("NakedIdentifierSegment"),
                                    ]),
                                ])
                                .config(|this| this.optional()),
                            ]),
                        ])]),
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("WITHOUT"), Ref::keyword("OIDS")]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("COMMIT"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PRESERVE"),
                            Ref::keyword("ROWS"),
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("DELETE"), Ref::keyword("ROWS")]),
                        Ref::keyword("DROP"),
                    ]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                ])
                .config(|this| this.optional()),
            ]),
            Ref::keyword("AS"),
            one_of(vec_of_erased![
                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar"),]),
                optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                ])]),
                Ref::new("ValuesClauseSegment"),
                optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("EXECUTE"),
                    Ref::new("FunctionSegment"),
                ])]),
            ]),
            Ref::new("WithDataClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterTableStatementSegment;

impl NodeTrait for AlterTableStatementSegment {
    const TYPE: &'static str = "alter_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TABLE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::keyword("ONLY").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("StarSegment").optional(),
                    one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("AlterTableActionSegment")]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RENAME"),
                            Ref::keyword("COLUMN").optional(),
                            Ref::new("ColumnReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("ColumnReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RENAME"),
                            Ref::keyword("CONSTRAINT"),
                            Ref::new("ParameterNameSegment"),
                            Ref::keyword("TO"),
                            Ref::new("ParameterNameSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RENAME"),
                            Ref::keyword("TO"),
                            Ref::new("TableReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::keyword("SCHEMA"),
                            Ref::new("SchemaReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ATTACH"),
                            Ref::keyword("PARTITION"),
                            Ref::new("ParameterNameSegment"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FOR"),
                                    Ref::keyword("VALUES"),
                                    Ref::new("PartitionBoundSpecSegment"),
                                ]),
                                Ref::keyword("DEFAULT"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DETACH"),
                            Ref::keyword("PARTITION"),
                            Ref::new("ParameterNameSegment"),
                            Ref::keyword("CONCURRENTLY").optional(),
                            Ref::keyword("FINALIZE").optional(),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALL"),
                    Ref::keyword("IN"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OWNED"),
                        Ref::keyword("BY"),
                        Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")])
                            .config(|this| this.optional()),
                    ]),
                    Ref::keyword("SET"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                    Ref::keyword("NOWAIT").optional(),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterTableActionSegment;

impl NodeTrait for AlterTableActionSegment {
    const TYPE: &'static str = "alter_table_action_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::keyword("COLUMN").optional(),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DatatypeSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COLLATE"),
                    Ref::new("CollationReferenceSegment"),
                ])
                .config(|this| this.optional()),
                AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("COLUMN").optional(),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DropBehaviorGrammar").optional(),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("COLUMN").optional(),
                Ref::new("ColumnReferenceSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DATA")])
                            .config(|this| this.optional()),
                        Ref::keyword("TYPE"),
                        Ref::new("DatatypeSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLLATE"),
                            Ref::new("CollationReferenceSegment"),
                        ])
                        .config(|this| this.optional()),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("USING"),
                            one_of(vec_of_erased![Ref::new("ExpressionSegment")]),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::keyword("DEFAULT"),
                        one_of(vec_of_erased![
                            Ref::new("LiteralGrammar"),
                            Ref::new("FunctionSegment"),
                            Ref::new("BareFunctionSegment"),
                            Ref::new("ExpressionSegment"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("DROP"), Ref::keyword("DEFAULT")]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DROP")])
                            .config(|this| this.optional()),
                        Ref::keyword("NOT"),
                        Ref::keyword("NULL"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DROP"),
                        Ref::keyword("EXPRESSION"),
                        Ref::new("IfExistsGrammar").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ADD"),
                        Ref::keyword("GENERATED"),
                        one_of(vec_of_erased![
                            Ref::keyword("ALWAYS"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("BY"),
                                Ref::keyword("DEFAULT"),
                            ]),
                        ]),
                        Ref::keyword("AS"),
                        Ref::keyword("IDENTITY"),
                        Bracketed::new(vec_of_erased![
                            AnyNumberOf::new(vec_of_erased![Ref::new(
                                "AlterSequenceOptionsSegment"
                            )])
                            .config(|this| this.optional()),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::keyword("GENERATED"),
                            one_of(vec_of_erased![
                                Ref::keyword("ALWAYS"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("BY"),
                                    Ref::keyword("DEFAULT"),
                                ]),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::new("AlterSequenceOptionsSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESTART"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Ref::new("NumericLiteralSegment"),
                            ]),
                        ]),
                    ]),]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DROP"),
                        Ref::keyword("IDENTITY"),
                        Ref::new("IfExistsGrammar").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::keyword("STATISTICS"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::new("RelationOptionsSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESET"),
                        Ref::new("RelationOptionsSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::keyword("STORAGE"),
                        one_of(vec_of_erased![
                            Ref::keyword("PLAIN"),
                            Ref::keyword("EXTERNAL"),
                            Ref::keyword("EXTENDED"),
                            Ref::keyword("MAIN"),
                        ]),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("ADD"), Ref::new("TableConstraintSegment"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::new("TableConstraintUsingIndexSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("ParameterNameSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("DEFERRABLE"),
                    Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("DEFERRABLE"),])
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INITIALLY"),
                        Ref::keyword("DEFERRED"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INITIALLY"),
                        Ref::keyword("IMMEDIATE"),
                    ]),
                ])
                .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("VALIDATE"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("ParameterNameSegment")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("ParameterNameSegment"),
                Ref::new("DropBehaviorGrammar").optional(),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("ENABLE"), Ref::keyword("DISABLE"),]),
                Ref::keyword("TRIGGER"),
                one_of(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    Ref::keyword("ALL"),
                    Ref::keyword("USER"),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ENABLE"),
                one_of(vec_of_erased![Ref::keyword("REPLICA"), Ref::keyword("ALWAYS"),]),
                Ref::keyword("TRIGGER"),
                Ref::new("ParameterNameSegment"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ENABLE"),
                    Ref::keyword("DISABLE"),
                    Sequence::new(vec_of_erased![Ref::keyword("ENABLE"), Ref::keyword("REPLICA"),]),
                    Sequence::new(vec_of_erased![Ref::keyword("ENABLE"), Ref::keyword("RULE"),]),
                ]),
                Ref::keyword("RULE"),
                Ref::new("ParameterNameSegment"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DISABLE"),
                    Ref::keyword("ENABLE"),
                    Ref::keyword("FORCE"),
                    Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("FORCE"),]),
                ]),
                Ref::keyword("ROW"),
                Ref::keyword("LEVEL"),
                Ref::keyword("SECURITY"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("CLUSTER"),
                Ref::keyword("ON"),
                Ref::new("ParameterNameSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::keyword("WITHOUT"),
                one_of(vec_of_erased![Ref::keyword("CLUSTER"), Ref::keyword("OIDS"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::keyword("TABLESPACE"),
                Ref::new("TablespaceReferenceSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                one_of(vec_of_erased![Ref::keyword("LOGGED"), Ref::keyword("UNLOGGED"),]),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::new("RelationOptionsSegment"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("RESET"),
                Ref::new("RelationOptionsSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("NO").optional(),
                Ref::keyword("INHERIT"),
                Ref::new("TableReferenceSegment"),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("OF"), Ref::new("ParameterNameSegment"),]),
            Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("OF"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("OWNER"),
                Ref::keyword("TO"),
                one_of(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    Ref::keyword("CURRENT_ROLE"),
                    Ref::keyword("CURRENT_USER"),
                    Ref::keyword("SESSION_USER"),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("REPLICA"),
                Ref::keyword("IDENTITY"),
                one_of(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Ref::keyword("INDEX"),
                        Ref::new("IndexReferenceSegment"),
                    ]),
                    Ref::keyword("FULL"),
                    Ref::keyword("NOTHING"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct VersionIdentifierSegment;

impl NodeTrait for VersionIdentifierSegment {
    const TYPE: &'static str = "version_identifier";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(
            vec_of_erased![Ref::new("QuotedLiteralSegment"), Ref::new("NakedIdentifierSegment"),],
        )
        .to_matchable()
    }
}

pub struct CreateExtensionStatementSegment;

impl NodeTrait for CreateExtensionStatementSegment {
    const TYPE: &'static str = "create_extension_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("EXTENSION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ExtensionReferenceSegment"),
            Ref::keyword("WITH").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SCHEMA"),
                Ref::new("SchemaReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("VERSION"),
                Ref::new("VersionIdentifierSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("FROM"),
                Ref::new("VersionIdentifierSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct DropExtensionStatementSegment;

impl NodeTrait for DropExtensionStatementSegment {
    const TYPE: &'static str = "drop_extension_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("EXTENSION"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("ExtensionReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct PublicationReferenceSegment;

impl NodeTrait for PublicationReferenceSegment {
    const TYPE: &'static str = "publication_reference";

    fn match_grammar() -> Arc<dyn Matchable> {
        Ref::new("SingleIdentifierGrammar").to_matchable()
    }
}

pub struct PublicationTableSegment;

impl NodeTrait for PublicationTableSegment {
    const TYPE: &'static str = "publication_table";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("ExtendedTableReferenceGrammar"),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WHERE"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct PublicationObjectsSegment;

impl NodeTrait for PublicationObjectsSegment {
    const TYPE: &'static str = "publication_objects";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TABLE"),
                Delimited::new(vec_of_erased![Ref::new("PublicationTableSegment")]).config(
                    |this| {
                        this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("CommaSegment"),
                            one_of(vec_of_erased![Ref::keyword("TABLE"), Ref::keyword("TABLES")]),
                        ])];
                    }
                ),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("TABLES"),
                Ref::keyword("IN"),
                Ref::keyword("SCHEMA"),
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("SchemaReferenceSegment"),
                    Ref::keyword("CURRENT_SCHEMA"),
                ]),])
                .config(|this| {
                    this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("CommaSegment"),
                        one_of(vec_of_erased![Ref::keyword("TABLE"), Ref::keyword("TABLES"),]),
                    ]),];
                }),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreatePublicationStatementSegment;

impl NodeTrait for CreatePublicationStatementSegment {
    const TYPE: &'static str = "create_publication_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("PUBLICATION"),
            Ref::new("PublicationReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("FOR"),
                    Ref::keyword("ALL"),
                    Ref::keyword("TABLES"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FOR"),
                    Delimited::new(vec_of_erased![Ref::new("PublicationObjectsSegment"),]),
                ]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::new("DefinitionParametersSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterPublicationStatementSegment;

impl NodeTrait for AlterPublicationStatementSegment {
    const TYPE: &'static str = "alter_publication_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("PUBLICATION"),
            Ref::new("PublicationReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("DefinitionParametersSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Delimited::new(vec_of_erased![Ref::new("PublicationObjectsSegment")]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Delimited::new(vec_of_erased![Ref::new("PublicationObjectsSegment")]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Delimited::new(vec_of_erased![Ref::new("PublicationObjectsSegment")]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("PublicationReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                        Ref::new("RoleReferenceSegment"),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DropPublicationStatementSegment;

impl NodeTrait for DropPublicationStatementSegment {
    const TYPE: &'static str = "drop_publication_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("PUBLICATION"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("PublicationReferenceSegment"),]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct CreateMaterializedViewStatementSegment;

impl NodeTrait for CreateMaterializedViewStatementSegment {
    const TYPE: &'static str = "create_materialized_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Sequence::new(vec_of_erased![Ref::keyword("USING"), Ref::new("ParameterNameSegment"),])
                .config(|this| this.optional()),
            Sequence::new(
                vec_of_erased![Ref::keyword("WITH"), Ref::new("RelationOptionsSegment"),]
            )
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("TABLESPACE"),
                Ref::new("TablespaceReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("AS"),
            one_of(vec_of_erased![
                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar"),]),
                optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                ]),]),
                Ref::new("ValuesClauseSegment"),
                optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("EXECUTE"),
                    Ref::new("FunctionSegment"),
                ]),]),
            ]),
            Ref::new("WithDataClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterMaterializedViewStatementSegment;

impl NodeTrait for AlterMaterializedViewStatementSegment {
    const TYPE: &'static str = "alter_materialized_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new(
                            "AlterMaterializedViewActionSegment"
                        ),]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RENAME"),
                            Sequence::new(vec_of_erased![Ref::keyword("COLUMN"),])
                                .config(|this| this.optional()),
                            Ref::new("ColumnReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("ColumnReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RENAME"),
                            Ref::keyword("TO"),
                            Ref::new("TableReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::keyword("SCHEMA"),
                            Ref::new("SchemaReferenceSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::keyword("NO").optional(),
                    Ref::keyword("DEPENDS"),
                    Ref::keyword("ON"),
                    Ref::keyword("EXTENSION"),
                    Ref::new("ExtensionReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALL"),
                    Ref::keyword("IN"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OWNED"),
                        Ref::keyword("BY"),
                        Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment"),]),
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("SET"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                    Sequence::new(vec_of_erased![Ref::keyword("NOWAIT"),])
                        .config(|this| this.optional()),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterMaterializedViewActionSegment;

impl NodeTrait for AlterMaterializedViewActionSegment {
    const TYPE: &'static str = "alter_materialized_view_action_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("COLUMN").optional(),
                Ref::new("ColumnReferenceSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::keyword("STATISTICS"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                                Ref::new("EqualsSegment"),
                                Ref::new("LiteralGrammar"),
                            ]),
                        ])]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESET"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ParameterNameSegment"
                        )])]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::keyword("STORAGE"),
                        one_of(vec_of_erased![
                            Ref::keyword("PLAIN"),
                            Ref::keyword("EXTERNAL"),
                            Ref::keyword("EXTENDED"),
                            Ref::keyword("MAIN"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::keyword("COMPRESSION"),
                        Ref::new("ParameterNameSegment"),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("CLUSTER"),
                Ref::keyword("ON"),
                Ref::new("ParameterNameSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::keyword("WITHOUT"),
                Ref::keyword("CLUSTER"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("EqualsSegment"),
                            Ref::new("LiteralGrammar"),
                        ])
                        .config(|this| this.optional()),
                    ]
                )])]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("RESET"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ParameterNameSegment"
                )])]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("OWNER"),
                Ref::keyword("TO"),
                one_of(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("CURRENT_ROLE"),
                    Ref::keyword("CURRENT_USER"),
                    Ref::keyword("SESSION_USER"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct RefreshMaterializedViewStatementSegment;

impl NodeTrait for RefreshMaterializedViewStatementSegment {
    const TYPE: &'static str = "refresh_materialized_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REFRESH"),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            Ref::keyword("CONCURRENTLY").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("WithDataClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct DropMaterializedViewStatementSegment;

impl NodeTrait for DropMaterializedViewStatementSegment {
    const TYPE: &'static str = "drop_materialized_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment"),]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct WithCheckOptionSegment;

impl NodeTrait for WithCheckOptionSegment {
    const TYPE: &'static str = "with_check_option";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WITH"),
            one_of(vec_of_erased![Ref::keyword("CASCADED"), Ref::keyword("LOCAL"),]),
            Ref::keyword("CHECK"),
            Ref::keyword("OPTION"),
        ])
        .to_matchable()
    }
}

pub struct AlterPolicyStatementSegment;

impl NodeTrait for AlterPolicyStatementSegment {
    const TYPE: &'static str = "alter_policy_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("POLICY"),
            Ref::new("ObjectReferenceSegment"),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TO"),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("RoleReferenceSegment"),
                        Ref::keyword("PUBLIC"),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                    ])]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("CHECK"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                ])
                .config(|this| this.optional()),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateViewStatementSegment;

impl NodeTrait for CreateViewStatementSegment {
    const TYPE: &'static str = "create_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("RECURSIVE").optional(),
            Ref::keyword("VIEW"),
            Ref::new("TableReferenceSegment"),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::new("RelationOptionsSegment")])
                .config(|this| this.optional()),
            Ref::keyword("AS"),
            one_of(vec_of_erased![
                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                Ref::new("ValuesClauseSegment"),
            ]),
            Ref::new("WithCheckOptionSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterViewStatementSegment;

impl NodeTrait for AlterViewStatementSegment {
    const TYPE: &'static str = "alter_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("COLUMN").optional(),
                    Ref::new("ColumnReferenceSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::keyword("DEFAULT"),
                            one_of(vec_of_erased![
                                Ref::new("LiteralGrammar"),
                                Ref::new("FunctionSegment"),
                                Ref::new("BareFunctionSegment"),
                                Ref::new("ExpressionSegment"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DROP"),
                            Ref::keyword("DEFAULT"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("COLUMN").optional(),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::new("ColumnReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("EqualsSegment"),
                                Ref::new("LiteralGrammar"),
                            ])
                            .config(|this| this.optional()),
                        ]
                    ),]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESET"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ParameterNameSegment"
                    )])]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DropViewStatementSegment;

impl NodeTrait for DropViewStatementSegment {
    const TYPE: &'static str = "drop_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment"),]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct CreateDatabaseStatementSegment;

impl NodeTrait for CreateDatabaseStatementSegment {
    const TYPE: &'static str = "create_database_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("DATABASE"),
            Ref::new("DatabaseReferenceSegment"),
            Ref::keyword("WITH").optional(),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::new("EqualsSegment").optional(),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TEMPLATE"),
                    Ref::new("EqualsSegment").optional(),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENCODING"),
                    Ref::new("EqualsSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::keyword("DEFAULT"),
                    ]),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LOCALE"),
                        Ref::new("EqualsSegment").optional(),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LC_COLLATE"),
                            Ref::new("EqualsSegment").optional(),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LC_CTYPE"),
                            Ref::new("EqualsSegment").optional(),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESPACE"),
                    Ref::new("EqualsSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::new("TablespaceReferenceSegment"),
                        Ref::keyword("DEFAULT"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALLOW_CONNECTIONS"),
                    Ref::new("EqualsSegment").optional(),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONNECTION"),
                    Ref::keyword("LIMIT"),
                    Ref::new("EqualsSegment").optional(),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS_TEMPLATE"),
                    Ref::new("EqualsSegment").optional(),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterDatabaseStatementSegment;

impl NodeTrait for AlterDatabaseStatementSegment {
    const TYPE: &'static str = "alter_database_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("DATABASE"),
            Ref::new("DatabaseReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH").optional(),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ALLOW_CONNECTIONS"),
                            Ref::new("BooleanLiteralGrammar"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CONNECTION"),
                            Ref::keyword("LIMIT"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("IS_TEMPLATE"),
                            Ref::new("BooleanLiteralGrammar"),
                        ]),
                    ])
                    .config(|this| this.min_times(1)),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("DatabaseReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("ParameterNameSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("TO"), Ref::new("EqualsSegment"),]),
                            one_of(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("LiteralGrammar"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::keyword("CURRENT"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESET"),
                    one_of(vec_of_erased![Ref::keyword("ALL"), Ref::new("ParameterNameSegment"),]),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct DropDatabaseStatementSegment;

impl NodeTrait for DropDatabaseStatementSegment {
    const TYPE: &'static str = "drop_database_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("DATABASE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH").optional(),
                Bracketed::new(vec_of_erased![Ref::keyword("FORCE")]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct VacuumStatementSegment;

impl NodeTrait for VacuumStatementSegment {
    const TYPE: &'static str = "vacuum_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("VACUUM"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("FULL").optional(),
                    Ref::keyword("FREEZE").optional(),
                    Ref::keyword("VERBOSE").optional(),
                    one_of(vec_of_erased![Ref::keyword("ANALYZE"), Ref::keyword("ANALYSE")])
                        .config(|this| this.optional()),
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("FULL"),
                            Ref::keyword("FREEZE"),
                            Ref::keyword("VERBOSE"),
                            Ref::keyword("ANALYZE"),
                            Ref::keyword("ANALYSE"),
                            Ref::keyword("DISABLE_PAGE_SKIPPING"),
                            Ref::keyword("SKIP_LOCKED"),
                            Ref::keyword("INDEX_CLEANUP"),
                            Ref::keyword("PROCESS_TOAST"),
                            Ref::keyword("TRUNCATE"),
                            Ref::keyword("PARALLEL"),
                        ]),
                        one_of(vec_of_erased![
                            Ref::new("LiteralGrammar"),
                            Ref::new("NakedIdentifierSegment"),
                            Ref::new("OnKeywordAsIdentifierSegment"),
                        ])
                        .config(|this| this.optional()),
                    ]
                ),]),]),
            ])
            .config(|this| this.optional()),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("TableReferenceSegment"),
                Ref::new("BracketedColumnReferenceListGrammar").optional(),
            ])])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct LikeOptionSegment;

impl NodeTrait for LikeOptionSegment {
    const TYPE: &'static str = "like_option_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("INCLUDING"), Ref::keyword("EXCLUDING"),]),
            one_of(vec_of_erased![
                Ref::keyword("COMMENTS"),
                Ref::keyword("CONSTRAINTS"),
                Ref::keyword("DEFAULTS"),
                Ref::keyword("GENERATED"),
                Ref::keyword("IDENTITY"),
                Ref::keyword("INDEXES"),
                Ref::keyword("STATISTICS"),
                Ref::keyword("STORAGE"),
                Ref::keyword("ALL"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ColumnConstraintSegment;

impl NodeTrait for ColumnConstraintSegment {
    const TYPE: &'static str = "column_constraint";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment"),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(
                    vec_of_erased![Ref::keyword("NOT").optional(), Ref::keyword("NULL"),]
                ),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CHECK"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                    Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("INHERIT"),])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    one_of(vec_of_erased![
                        Ref::new("ShorthandCastSegment"),
                        Ref::new("LiteralGrammar"),
                        Ref::new("FunctionSegment"),
                        Ref::new("BareFunctionSegment"),
                        Ref::new("ExpressionSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("GENERATED"),
                    Ref::keyword("ALWAYS"),
                    Ref::keyword("AS"),
                    Ref::new("ExpressionSegment"),
                    Ref::keyword("STORED"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("GENERATED"),
                    one_of(vec_of_erased![
                        Ref::keyword("ALWAYS"),
                        Sequence::new(vec_of_erased![Ref::keyword("BY"), Ref::keyword("DEFAULT")]),
                    ]),
                    Ref::keyword("AS"),
                    Ref::keyword("IDENTITY"),
                    Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![Ref::new(
                        "AlterSequenceOptionsSegment"
                    )])])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNIQUE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NULLS"),
                        Ref::keyword("NOT").optional(),
                        Ref::keyword("DISTINCT"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::new("DefinitionParametersSegment"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Ref::keyword("INDEX"),
                        Ref::keyword("TABLESPACE"),
                        Ref::new("TablespaceReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PRIMARY"),
                    Ref::keyword("KEY"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::new("DefinitionParametersSegment"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Ref::keyword("INDEX"),
                        Ref::keyword("TABLESPACE"),
                        Ref::new("TablespaceReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Ref::new("ReferenceDefinitionGrammar"),
            ]),
            one_of(vec_of_erased![
                Ref::keyword("DEFERRABLE"),
                Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("DEFERRABLE")]),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("INITIALLY"), Ref::keyword("DEFERRED")]),
                Sequence::new(vec_of_erased![Ref::keyword("INITIALLY"), Ref::keyword("IMMEDIATE")]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct PartitionBoundSpecSegment;

impl NodeTrait for PartitionBoundSpecSegment {
    const TYPE: &'static str = "partition_bound_spec";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("IN"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ExpressionSegment"
                ),]),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FROM"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("MINVALUE"),
                        Ref::keyword("MAXVALUE"),
                    ]
                ),]),]),
                Ref::keyword("TO"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("MINVALUE"),
                        Ref::keyword("MAXVALUE"),
                    ]
                ),]),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("MODULUS"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("CommaSegment"),
                    Ref::keyword("REMAINDER"),
                    Ref::new("NumericLiteralSegment"),
                ]),]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct TableConstraintSegment;

impl NodeTrait for TableConstraintSegment {
    const TYPE: &'static str = "table_constraint";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment"),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CHECK"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                    Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("INHERIT"),])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNIQUE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NULLS"),
                        Ref::keyword("NOT").optional(),
                        Ref::keyword("DISTINCT"),
                    ])
                    .config(|this| this.optional()),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    Ref::new("IndexParametersSegment").optional(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PrimaryKeyGrammar"),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    Ref::new("IndexParametersSegment").optional(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCLUDE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Ref::new("IndexAccessMethodSegment"),
                    ])
                    .config(|this| this.optional()),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExclusionConstraintElementSegment"
                    )])]),
                    Ref::new("IndexParametersSegment").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WHERE"),
                        Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FOREIGN"),
                    Ref::keyword("KEY"),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    Ref::new("ReferenceDefinitionGrammar"),
                ]),
            ]),
            AnyNumberOf::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DEFERRABLE"),
                    Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("DEFERRABLE")]),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INITIALLY"),
                        Ref::keyword("DEFERRED"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INITIALLY"),
                        Ref::keyword("IMMEDIATE"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("VALID")]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("INHERIT")]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct TableConstraintUsingIndexSegment;

impl NodeTrait for TableConstraintUsingIndexSegment {
    const TYPE: &'static str = "table_constraint";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("UNIQUE"), Ref::new("PrimaryKeyGrammar"),]),
                Ref::keyword("USING"),
                Ref::keyword("INDEX"),
                Ref::new("IndexReferenceSegment"),
            ]),
            one_of(vec_of_erased![
                Ref::keyword("DEFERRABLE"),
                Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("DEFERRABLE"),]),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("INITIALLY"), Ref::keyword("DEFERRED"),]),
                Sequence::new(
                    vec_of_erased![Ref::keyword("INITIALLY"), Ref::keyword("IMMEDIATE"),]
                ),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct IndexParametersSegment;

impl NodeTrait for IndexParametersSegment {
    const TYPE: &'static str = "index_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("INCLUDE"),
                Ref::new("BracketedColumnReferenceListGrammar"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::new("DefinitionParametersSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::keyword("INDEX"),
                Ref::keyword("TABLESPACE"),
                Ref::new("TablespaceReferenceSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct ReferentialActionSegment;

impl NodeTrait for ReferentialActionSegment {
    const TYPE: &'static str = "referential_action";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::keyword("CASCADE"),
            Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::keyword("NULL"),]),
            Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DEFAULT"),]),
            Ref::keyword("RESTRICT"),
            Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("ACTION"),]),
        ])
        .to_matchable()
    }
}

pub struct IndexElementOptionsSegment;

impl NodeTrait for IndexElementOptionsSegment {
    const TYPE: &'static str = "index_element_options";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("COLLATE"),
                Ref::new("CollationReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::new("OperatorClassReferenceSegment").config(|this| {
                    this.exclude = Some(
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NULLS"),
                            one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST")]),
                        ])
                        .to_matchable(),
                    );
                }),
                Ref::new("RelationOptionsSegment").optional(),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULLS"),
                one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST"),]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct IndexElementSegment;

impl NodeTrait for IndexElementSegment {
    const TYPE: &'static str = "index_element";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("ColumnReferenceSegment"),
                Ref::new("FunctionSegment"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
            ]),
            Ref::new("IndexElementOptionsSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct ExclusionConstraintElementSegment;

impl NodeTrait for ExclusionConstraintElementSegment {
    const TYPE: &'static str = "exclusion_constraint_element";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("IndexElementSegment"),
            Ref::keyword("WITH"),
            Ref::new("ComparisonOperatorGrammar"),
        ])
        .to_matchable()
    }
}

pub struct AlterDefaultPrivilegesStatementSegment;

impl NodeTrait for AlterDefaultPrivilegesStatementSegment {
    const TYPE: &'static str = "alter_default_privileges_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("DEFAULT"),
            Ref::keyword("PRIVILEGES"),
            Sequence::new(vec_of_erased![
                Ref::keyword("FOR"),
                one_of(vec_of_erased![Ref::keyword("ROLE"), Ref::keyword("USER"),]),
                Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment"),]).config(
                    |this| {
                        this.terminators = vec_of_erased![
                            Ref::keyword("IN"),
                            Ref::keyword("GRANT"),
                            Ref::keyword("REVOKE"),
                        ];
                    }
                ),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("IN"),
                Ref::keyword("SCHEMA"),
                Delimited::new(vec_of_erased![Ref::new("SchemaReferenceSegment"),]).config(
                    |this| {
                        this.terminators =
                            vec_of_erased![Ref::keyword("GRANT"), Ref::keyword("REVOKE"),];
                    }
                ),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Ref::new("AlterDefaultPrivilegesGrantSegment"),
                Ref::new("AlterDefaultPrivilegesRevokeSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterDefaultPrivilegesObjectPrivilegesSegment;

impl NodeTrait for AlterDefaultPrivilegesObjectPrivilegesSegment {
    const TYPE: &'static str = "alter_default_privileges_object_privilege";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("ALL"),
                Ref::keyword("PRIVILEGES").optional(),
            ]),
            Delimited::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("DELETE"),
                Ref::keyword("EXECUTE"),
                Ref::keyword("INSERT"),
                Ref::keyword("REFERENCES"),
                Ref::keyword("SELECT"),
                Ref::keyword("TRIGGER"),
                Ref::keyword("TRUNCATE"),
                Ref::keyword("UPDATE"),
                Ref::keyword("USAGE"),
            ])
            .config(|this| {
                this.terminators = vec_of_erased![Ref::keyword("ON"),];
            }),
        ])
        .to_matchable()
    }
}

pub struct AlterDefaultPrivilegesSchemaObjectsSegment;

impl NodeTrait for AlterDefaultPrivilegesSchemaObjectsSegment {
    const TYPE: &'static str = "alter_default_privileges_schema_object";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::keyword("TABLES"),
            Ref::keyword("FUNCTIONS"),
            Ref::keyword("ROUTINES"),
            Ref::keyword("SEQUENCES"),
            Ref::keyword("TYPES"),
            Ref::keyword("SCHEMAS"),
        ])
        .to_matchable()
    }
}

pub struct AlterDefaultPrivilegesToFromRolesSegment;

impl NodeTrait for AlterDefaultPrivilegesToFromRolesSegment {
    const TYPE: &'static str = "alter_default_privileges_to_from_roles";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("GROUP").optional(),
                Ref::new("RoleReferenceSegment"),
            ]),
            Ref::keyword("PUBLIC"),
        ])
        .to_matchable()
    }
}

pub struct AlterDefaultPrivilegesGrantSegment;

impl NodeTrait for AlterDefaultPrivilegesGrantSegment {
    const TYPE: &'static str = "alter_default_privileges_grant";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("GRANT"),
            Ref::new("AlterDefaultPrivilegesObjectPrivilegesSegment"),
            Ref::keyword("ON"),
            Ref::new("AlterDefaultPrivilegesSchemaObjectsSegment"),
            Ref::keyword("TO"),
            Delimited::new(vec_of_erased![Ref::new("AlterDefaultPrivilegesToFromRolesSegment"),])
                .config(|this| {
                    this.terminators = vec_of_erased![Ref::keyword("WITH"),];
                }),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("GRANT"),
                Ref::keyword("OPTION"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterDefaultPrivilegesRevokeSegment;

impl NodeTrait for AlterDefaultPrivilegesRevokeSegment {
    const TYPE: &'static str = "alter_default_privileges_revoke";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REVOKE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("GRANT"),
                Ref::keyword("OPTION"),
                Ref::keyword("FOR"),
            ])
            .config(|this| this.optional()),
            Ref::new("AlterDefaultPrivilegesObjectPrivilegesSegment"),
            Ref::keyword("ON"),
            Ref::new("AlterDefaultPrivilegesSchemaObjectsSegment"),
            Ref::keyword("FROM"),
            Delimited::new(vec_of_erased![Ref::new("AlterDefaultPrivilegesToFromRolesSegment"),])
                .config(|this| {
                    this.terminators =
                        vec_of_erased![Ref::keyword("RESTRICT"), Ref::keyword("CASCADE"),];
                }),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct DropOwnedStatementSegment;

impl NodeTrait for DropOwnedStatementSegment {
    const TYPE: &'static str = "drop_owned_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("OWNED"),
            Ref::keyword("BY"),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("CURRENT_ROLE"),
                Ref::keyword("CURRENT_USER"),
                Ref::keyword("SESSION_USER"),
                Ref::new("RoleReferenceSegment"),
            ]),]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct ReassignOwnedStatementSegment;

impl NodeTrait for ReassignOwnedStatementSegment {
    const TYPE: &'static str = "reassign_owned_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REASSIGN"),
            Ref::keyword("OWNED"),
            Ref::keyword("BY"),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("CURRENT_ROLE"),
                Ref::keyword("CURRENT_USER"),
                Ref::keyword("SESSION_USER"),
                Ref::new("RoleReferenceSegment"),
            ]),]),
            Ref::keyword("TO"),
            one_of(vec_of_erased![
                Ref::keyword("CURRENT_ROLE"),
                Ref::keyword("CURRENT_USER"),
                Ref::keyword("SESSION_USER"),
                Ref::new("RoleReferenceSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CommentOnStatementSegment;

impl NodeTrait for CommentOnStatementSegment {
    const TYPE: &'static str = "comment_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("COMMENT"),
            Ref::keyword("ON"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("TABLE"), Ref::keyword("VIEW"),]),
                        Ref::new("TableReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CAST"),
                        Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("ObjectReferenceSegment"),
                            Ref::keyword("AS"),
                            Ref::new("ObjectReferenceSegment"),
                        ]),]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ON"),
                            Ref::keyword("DOMAIN").optional(),
                            Ref::new("ObjectReferenceSegment"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DATABASE"),
                        Ref::new("DatabaseReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXTENSION"),
                        Ref::new("ExtensionReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FUNCTION"),
                        Ref::new("FunctionNameSegment"),
                        Sequence::new(vec_of_erased![Ref::new("FunctionParameterListGrammar")])
                            .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INDEX"),
                        Ref::new("IndexReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SCHEMA"),
                        Ref::new("SchemaReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("COLLATION"),
                            Ref::keyword("CONVERSION"),
                            Ref::keyword("DOMAIN"),
                            Ref::keyword("LANGUAGE"),
                            Ref::keyword("POLICY"),
                            Ref::keyword("PUBLICATION"),
                            Ref::keyword("ROLE"),
                            Ref::keyword("RULE"),
                            Ref::keyword("SEQUENCE"),
                            Ref::keyword("SERVER"),
                            Ref::keyword("STATISTICS"),
                            Ref::keyword("SUBSCRIPTION"),
                            Ref::keyword("TABLESPACE"),
                            Ref::keyword("TRIGGER"),
                            Ref::keyword("TYPE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ACCESS"),
                                Ref::keyword("METHOD"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("EVENT"),
                                Ref::keyword("TRIGGER"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FOREIGN"),
                                Ref::keyword("DATA"),
                                Ref::keyword("WRAPPER"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FOREIGN"),
                                Ref::keyword("TABLE"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MATERIALIZED"),
                                Ref::keyword("VIEW"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TEXT"),
                                Ref::keyword("SEARCH"),
                                Ref::keyword("CONFIGURATION"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TEXT"),
                                Ref::keyword("SEARCH"),
                                Ref::keyword("DICTIONARY"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TEXT"),
                                Ref::keyword("SEARCH"),
                                Ref::keyword("PARSER"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TEXT"),
                                Ref::keyword("SEARCH"),
                                Ref::keyword("TEMPLATE"),
                            ]),
                        ]),
                        Ref::new("ObjectReferenceSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ON"),
                            Ref::new("ObjectReferenceSegment"),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("AGGREGATE"),
                            Ref::keyword("PROCEDURE"),
                            Ref::keyword("ROUTINE"),
                        ]),
                        Ref::new("ObjectReferenceSegment"),
                        Bracketed::new(vec_of_erased![
                            Sequence::new(vec_of_erased![Anything::new()])
                                .config(|this| this.optional()),
                        ])
                        .config(|this| this.optional()),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    one_of(vec_of_erased![Ref::new("QuotedLiteralSegment"), Ref::keyword("NULL")]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateIndexStatementSegment;

impl NodeTrait for CreateIndexStatementSegment {
    const TYPE: &'static str = "create_index_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("UNIQUE").optional(),
            Ref::keyword("INDEX"),
            Ref::keyword("CONCURRENTLY").optional(),
            Sequence::new(vec_of_erased![
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("IndexReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("ON"),
            Ref::keyword("ONLY").optional(),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::new("IndexAccessMethodSegment"),
            ])
            .config(|this| this.optional()),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "IndexElementSegment"
            )])]),
            Sequence::new(vec_of_erased![
                Ref::keyword("INCLUDE"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "IndexElementSegment"
                )])]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULLS"),
                Ref::keyword("NOT").optional(),
                Ref::keyword("DISTINCT"),
            ])
            .config(|this| this.optional()),
            Sequence::new(
                vec_of_erased![Ref::keyword("WITH"), Ref::new("RelationOptionsSegment"),]
            )
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("TABLESPACE"),
                Ref::new("TablespaceReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("WHERE"), Ref::new("ExpressionSegment"),])
                .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterIndexStatementSegment;

impl NodeTrait for AlterIndexStatementSegment {
    const TYPE: &'static str = "alter_index_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("INDEX"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("IndexReferenceSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RENAME"),
                            Ref::keyword("TO"),
                            Ref::new("IndexReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::keyword("TABLESPACE"),
                            Ref::new("TablespaceReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ATTACH"),
                            Ref::keyword("PARTITION"),
                            Ref::new("IndexReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NO").optional(),
                            Ref::keyword("DEPENDS"),
                            Ref::keyword("ON"),
                            Ref::keyword("EXTENSION"),
                            Ref::new("ExtensionReferenceSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("ParameterNameSegment"),
                                    Sequence::new(vec_of_erased![
                                        Ref::new("EqualsSegment"),
                                        Ref::new("LiteralGrammar"),
                                    ])
                                    .config(|this| this.optional()),
                                ]),
                            ]),]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESET"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ParameterNameSegment"),
                            ]),]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ALTER"),
                            Ref::keyword("COLUMN").optional(),
                            Ref::new("NumericLiteralSegment"),
                            Ref::keyword("SET"),
                            Ref::keyword("STATISTICS"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALL"),
                    Ref::keyword("IN"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OWNED"),
                        Ref::keyword("BY"),
                        Delimited::new(vec_of_erased![Ref::new("RoleReferenceSegment"),]),
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("SET"),
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                    Ref::keyword("NOWAIT").optional(),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ReindexStatementSegment;

impl NodeTrait for ReindexStatementSegment {
    const TYPE: &'static str = "reindex_statement_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REINDEX"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONCURRENTLY"),
                    Ref::new("BooleanLiteralGrammar").optional(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESPACE"),
                    Ref::new("TablespaceReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("VERBOSE"),
                    Ref::new("BooleanLiteralGrammar").optional(),
                ]),
            ]),])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("INDEX"),
                    Ref::keyword("CONCURRENTLY").optional(),
                    Ref::new("IndexReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Ref::keyword("CONCURRENTLY").optional(),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEMA"),
                    Ref::keyword("CONCURRENTLY").optional(),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("DATABASE"), Ref::keyword("SYSTEM"),]),
                    Ref::keyword("CONCURRENTLY").optional(),
                    Ref::new("DatabaseReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DropIndexStatementSegment;

impl NodeTrait for DropIndexStatementSegment {
    const TYPE: &'static str = "drop_index_statement_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("INDEX"),
            Ref::keyword("CONCURRENTLY").optional(),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("IndexReferenceSegment")]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct FrameClauseSegment;

impl NodeTrait for FrameClauseSegment {
    const TYPE: &'static str = "frame_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        let frame_extent = ansi::FrameClauseSegment::frame_extent();

        let frame_exclusion = Sequence::new(vec_of_erased![
            Ref::keyword("EXCLUDE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("CURRENT"), Ref::keyword("ROW"),]),
                Ref::keyword("GROUP"),
                Ref::keyword("TIES"),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("OTHERS"),]),
            ])
        ])
        .config(|this| this.optional());

        Sequence::new(vec_of_erased![
            Ref::new("FrameClauseUnitGrammar"),
            one_of(vec_of_erased![
                frame_extent.clone(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BETWEEN"),
                    frame_extent.clone(),
                    Ref::keyword("AND"),
                    frame_extent.clone(),
                ]),
            ]),
            frame_exclusion,
        ])
        .to_matchable()
    }
}

pub struct CreateSequenceOptionsSegment;

impl NodeTrait for CreateSequenceOptionsSegment {
    const TYPE: &'static str = "create_sequence_options_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::new("DatatypeSegment"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("INCREMENT"),
                Ref::keyword("BY").optional(),
                Ref::new("NumericLiteralSegment"),
            ]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("MINVALUE"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MINVALUE"),]),
            ]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAXVALUE"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MAXVALUE"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("START"),
                Ref::keyword("WITH").optional(),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(
                vec_of_erased![Ref::keyword("CACHE"), Ref::new("NumericLiteralSegment"),]
            ),
            one_of(vec_of_erased![
                Ref::keyword("CYCLE"),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("CYCLE"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("OWNED"),
                Ref::keyword("BY"),
                one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("ColumnReferenceSegment"),]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateSequenceStatementSegment;

impl NodeTrait for CreateSequenceStatementSegment {
    const TYPE: &'static str = "create_sequence_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("SEQUENCE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("SequenceReferenceSegment"),
            AnyNumberOf::new(vec_of_erased![Ref::new("CreateSequenceOptionsSegment")])
                .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterSequenceOptionsSegment;

impl NodeTrait for AlterSequenceOptionsSegment {
    const TYPE: &'static str = "alter_sequence_options_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::new("DatatypeSegment"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("INCREMENT"),
                Ref::keyword("BY").optional(),
                Ref::new("NumericLiteralSegment"),
            ]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("MINVALUE"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MINVALUE"),]),
            ]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAXVALUE"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MAXVALUE"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SEQUENCE"),
                Ref::keyword("NAME"),
                Ref::new("SequenceReferenceSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("START"),
                Ref::keyword("WITH").optional(),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("RESTART"),
                Ref::keyword("WITH").optional(),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(
                vec_of_erased![Ref::keyword("CACHE"), Ref::new("NumericLiteralSegment"),]
            ),
            Sequence::new(vec_of_erased![Ref::keyword("NO").optional(), Ref::keyword("CYCLE"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("OWNED"),
                Ref::keyword("BY"),
                one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("ColumnReferenceSegment"),]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterSequenceStatementSegment;

impl NodeTrait for AlterSequenceStatementSegment {
    const TYPE: &'static str = "alter_sequence_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("SEQUENCE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("SequenceReferenceSegment"),
            one_of(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![Ref::new("AlterSequenceOptionsSegment")])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("SequenceReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
            ])
        ])
        .to_matchable()
    }
}

pub struct DropSequenceStatementSegment;

impl NodeTrait for DropSequenceStatementSegment {
    const TYPE: &'static str = "drop_sequence_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("SEQUENCE"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("SequenceReferenceSegment")]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct AnalyzeStatementSegment;

impl NodeTrait for AnalyzeStatementSegment {
    const TYPE: &'static str = "analyze_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        let option = Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("VERBOSE"), Ref::keyword("SKIP_LOCKED")]),
            Ref::new("BooleanLiteralGrammar").optional(),
        ]);

        let tables_and_columns = Sequence::new(vec_of_erased![
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ColumnReferenceSegment"
            )])])
            .config(|this| this.optional()),
        ]);

        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("ANALYZE"), Ref::keyword("ANALYSE")]),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![option])]),
                Ref::keyword("VERBOSE")
            ])
            .config(|this| this.optional()),
            Delimited::new(vec_of_erased![tables_and_columns]).config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct StatementSegment;

impl NodeTrait for StatementSegment {
    const TYPE: &'static str = "statement_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::StatementSegment::match_grammar().copy(
            Some(vec_of_erased![
                Ref::new("AlterDefaultPrivilegesStatementSegment"),
                Ref::new("DropOwnedStatementSegment"),
                Ref::new("ReassignOwnedStatementSegment"),
                Ref::new("CommentOnStatementSegment"),
                Ref::new("AnalyzeStatementSegment"),
                Ref::new("CreateTableAsStatementSegment"),
                Ref::new("AlterTriggerStatementSegment"),
                Ref::new("SetStatementSegment"),
                Ref::new("AlterPolicyStatementSegment"),
                Ref::new("CreatePolicyStatementSegment"),
                Ref::new("DropPolicyStatementSegment"),
                Ref::new("CreateDomainStatementSegment"),
                Ref::new("AlterDomainStatementSegment"),
                Ref::new("DropDomainStatementSegment"),
                Ref::new("CreateMaterializedViewStatementSegment"),
                Ref::new("AlterMaterializedViewStatementSegment"),
                Ref::new("DropMaterializedViewStatementSegment"),
                Ref::new("RefreshMaterializedViewStatementSegment"),
                Ref::new("AlterDatabaseStatementSegment"),
                Ref::new("DropDatabaseStatementSegment"),
                Ref::new("VacuumStatementSegment"),
                Ref::new("AlterFunctionStatementSegment"),
                Ref::new("CreateViewStatementSegment"),
                Ref::new("AlterViewStatementSegment"),
                Ref::new("ListenStatementSegment"),
                Ref::new("NotifyStatementSegment"),
                Ref::new("UnlistenStatementSegment"),
                Ref::new("LoadStatementSegment"),
                Ref::new("ResetStatementSegment"),
                Ref::new("DiscardStatementSegment"),
                Ref::new("AlterProcedureStatementSegment"),
                Ref::new("CreateProcedureStatementSegment"),
                Ref::new("DropProcedureStatementSegment"),
                Ref::new("CopyStatementSegment"),
                Ref::new("DoStatementSegment"),
                Ref::new("AlterIndexStatementSegment"),
                Ref::new("ReindexStatementSegment"),
                Ref::new("AlterRoleStatementSegment"),
                Ref::new("CreateExtensionStatementSegment"),
                Ref::new("DropExtensionStatementSegment"),
                Ref::new("CreatePublicationStatementSegment"),
                Ref::new("AlterPublicationStatementSegment"),
                Ref::new("DropPublicationStatementSegment"),
                Ref::new("CreateTypeStatementSegment"),
                Ref::new("AlterTypeStatementSegment"),
                Ref::new("AlterSchemaStatementSegment"),
                Ref::new("LockTableStatementSegment"),
                Ref::new("ClusterStatementSegment"),
                Ref::new("CreateCollationStatementSegment"),
                Ref::new("CallStoredProcedureSegment"),
                Ref::new("CreateServerStatementSegment"),
                Ref::new("CreateUserMappingStatementSegment"),
                Ref::new("ImportForeignSchemaStatementSegment"),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        )
    }
}

pub struct CreateTriggerStatementSegment;

impl NodeTrait for CreateTriggerStatementSegment {
    const TYPE: &'static str = "create_trigger_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Ref::keyword("CONSTRAINT").optional(),
            Ref::keyword("TRIGGER"),
            Ref::new("TriggerReferenceSegment"),
            one_of(vec_of_erased![
                Ref::keyword("BEFORE"),
                Ref::keyword("AFTER"),
                Sequence::new(vec_of_erased![Ref::keyword("INSTEAD"), Ref::keyword("OF"),]),
            ]),
            Delimited::new(vec_of_erased![
                Ref::keyword("INSERT"),
                Ref::keyword("DELETE"),
                Ref::keyword("TRUNCATE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OF"),
                        Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),]).config(
                            |this| {
                                this.terminators =
                                    vec_of_erased![Ref::keyword("OR"), Ref::keyword("ON")];
                                this.optional();
                            }
                        ),
                    ])
                    .config(|this| this.optional()),
                ]),
            ])
            .config(|this| this.delimiter(Ref::keyword("OR"))),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("FROM"),
                    Ref::new("TableReferenceSegment"),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("DEFERRABLE"),]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFERRABLE").optional(),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("INITIALLY"),
                                Ref::keyword("IMMEDIATE"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("INITIALLY"),
                                Ref::keyword("DEFERRED"),
                            ]),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFERENCING"),
                    one_of(vec_of_erased![Ref::keyword("OLD"), Ref::keyword("NEW")]),
                    Ref::keyword("TABLE"),
                    Ref::keyword("AS"),
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("OLD"), Ref::keyword("NEW")]),
                        Ref::keyword("TABLE"),
                        Ref::keyword("AS"),
                        Ref::new("TableReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FOR"),
                    Ref::keyword("EACH").optional(),
                    one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("STATEMENT"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHEN"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("EXECUTE"),
                one_of(vec_of_erased![Ref::keyword("FUNCTION"), Ref::keyword("PROCEDURE"),]),
                Ref::new("FunctionSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterTriggerStatementSegment;

impl NodeTrait for AlterTriggerStatementSegment {
    const TYPE: &'static str = "alter_trigger";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TRIGGER"),
            Ref::new("TriggerReferenceSegment"),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TriggerReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NO").optional(),
                    Ref::keyword("DEPENDS"),
                    Ref::keyword("ON"),
                    Ref::keyword("EXTENSION"),
                    Ref::new("ExtensionReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DropTriggerStatementSegment;

impl NodeTrait for DropTriggerStatementSegment {
    const TYPE: &'static str = "drop_trigger_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("TRIGGER"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TriggerReferenceSegment"),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct AliasExpressionSegment;

impl NodeTrait for AliasExpressionSegment {
    const TYPE: &'static str = "alias_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("AS").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar").optional(),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Ref::new("DatatypeSegment"),
                        ]
                    )])]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AsAliasExpressionSegment;

impl NodeTrait for AsAliasExpressionSegment {
    const TYPE: &'static str = "alias_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            MetaSegment::indent(),
            Ref::keyword("AS"),
            Ref::new("SingleIdentifierGrammar"),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct OperationClassReferenceSegment;

impl NodeTrait for OperationClassReferenceSegment {
    const TYPE: &'static str = "operation_class_reference";

    fn match_grammar() -> Arc<dyn Matchable> {
        Ref::new("ObjectReferenceSegment").to_matchable()
    }
}

pub struct ConflictActionSegment;

impl NodeTrait for ConflictActionSegment {
    const TYPE: &'static str = "conflict_action";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DO"),
            one_of(vec_of_erased![
                Ref::keyword("NOTHING"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    Ref::keyword("SET"),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("EqualsSegment"),
                            one_of(vec_of_erased![
                                Ref::new("ExpressionSegment"),
                                Ref::keyword("DEFAULT")
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment")
                            ])]),
                            Ref::new("EqualsSegment"),
                            Ref::keyword("ROW").optional(),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
                                    Ref::new("ExpressionSegment"),
                                    Ref::keyword("DEFAULT")
                                ]
                            )])])
                        ]),
                        Sequence::new(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment")
                            ])]),
                            Ref::new("EqualsSegment"),
                            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
                        ])
                    ])]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WHERE"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional()),
                ])
            ])
        ])
        .to_matchable()
    }
}

pub struct ConflictTargetSegment;

impl NodeTrait for ConflictTargetSegment {
    const TYPE: &'static str = "conflict_target";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLLATE"),
                            Ref::new("CollationReferenceSegment")
                        ])
                        .config(|this| this.optional()),
                        Ref::new("OperationClassReferenceSegment").optional()
                    ]
                )])]),
                Sequence::new(vec_of_erased![Ref::keyword("WHERE"), Ref::new("ExpressionSegment")])
                    .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("ParameterNameSegment")
            ])
        ])
        .to_matchable()
    }
}

pub struct InsertStatementSegment;

impl NodeTrait for InsertStatementSegment {
    const TYPE: &'static str = "insert_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INSERT"),
            Ref::keyword("INTO"),
            Ref::new("TableReferenceSegment"),
            Ref::new("AsAliasExpressionSegment").optional(),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("OVERRIDING"),
                one_of(vec_of_erased![Ref::keyword("SYSTEM"), Ref::keyword("USER")]),
                Ref::keyword("VALUE")
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("DEFAULT"), Ref::keyword("VALUES")]),
                Ref::new("SelectableGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("CONFLICT"),
                Ref::new("ConflictTargetSegment").optional(),
                Ref::new("ConflictActionSegment")
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("RETURNING"),
                one_of(vec_of_erased![
                    Ref::new("StarSegment"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::new("AsAliasExpressionSegment").optional(),
                    ])])
                ])
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct DropTypeStatementSegment;

impl NodeTrait for DropTypeStatementSegment {
    const TYPE: &'static str = "drop_type_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("TYPE"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("DatatypeSegment")]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct SetStatementSegment;

impl NodeTrait for SetStatementSegment {
    const TYPE: &'static str = "set_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            one_of(vec_of_erased![Ref::keyword("SESSION"), Ref::keyword("LOCAL")])
                .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    one_of(vec_of_erased![Ref::keyword("TO"), Ref::new("EqualsSegment")]),
                    one_of(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        Delimited::new(vec_of_erased![
                            Ref::new("LiteralGrammar"),
                            Ref::new("NakedIdentifierSegment"),
                            Ref::new("OnKeywordAsIdentifierSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TIME"),
                    Ref::keyword("ZONE"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::keyword("LOCAL"),
                        Ref::keyword("DEFAULT")
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEMA"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROLE"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("RoleReferenceSegment"),]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreatePolicyStatementSegment;

impl NodeTrait for CreatePolicyStatementSegment {
    const TYPE: &'static str = "create_policy_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("POLICY"),
            Ref::new("ObjectReferenceSegment"),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![Ref::keyword("PERMISSIVE"), Ref::keyword("RESTRICTIVE")])
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("FOR"),
                one_of(vec_of_erased![
                    Ref::keyword("ALL"),
                    Ref::keyword("SELECT"),
                    Ref::keyword("INSERT"),
                    Ref::keyword("UPDATE"),
                    Ref::keyword("DELETE")
                ])
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("TO"),
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("PUBLIC"),
                    Ref::keyword("CURRENT_ROLE"),
                    Ref::keyword("CURRENT_USER"),
                    Ref::keyword("SESSION_USER")
                ])])
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("CHECK"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
    }
}

pub struct CallStoredProcedureSegment;

impl NodeTrait for CallStoredProcedureSegment {
    const TYPE: &'static str = "call_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("CALL"), Ref::new("FunctionSegment")])
            .to_matchable()
    }
}

pub struct CreateDomainStatementSegment;

impl NodeTrait for CreateDomainStatementSegment {
    const TYPE: &'static str = "create_domain_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("DOMAIN"),
            Ref::new("ObjectReferenceSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("AS"),]).config(|this| this.optional()),
            Ref::new("DatatypeSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("COLLATE"),
                Ref::new("CollationReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("DEFAULT"), Ref::new("ExpressionSegment"),])
                .config(|this| this.optional()),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NOT").optional(),
                        Ref::keyword("NULL"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CHECK"),
                        Ref::new("ExpressionSegment"),
                    ]),
                ]),
            ])]),
        ])
        .to_matchable()
    }
}

pub struct AlterDomainStatementSegment;

impl NodeTrait for AlterDomainStatementSegment {
    const TYPE: &'static str = "alter_domain_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("DOMAIN"),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("DEFAULT"),
                    Ref::new("ExpressionSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("DROP"), Ref::keyword("DEFAULT"),]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DROP"),]),
                    Ref::keyword("NOT"),
                    Ref::keyword("NULL"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NOT").optional(),
                            Ref::keyword("NULL"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CHECK"),
                            Ref::new("ExpressionSegment"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("VALID"),])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                    one_of(vec_of_erased![Ref::keyword("RESTRICT"), Ref::keyword("CASCADE"),])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("VALIDATE"),
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DropDomainStatementSegment;

impl NodeTrait for DropDomainStatementSegment {
    const TYPE: &'static str = "drop_domain_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("DOMAIN"),
            Ref::new("IfExistsGrammar").optional(),
            Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")]),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct DropPolicyStatementSegment;

impl NodeTrait for DropPolicyStatementSegment {
    const TYPE: &'static str = "drop_policy_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("POLICY"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct LoadStatementSegment;

impl NodeTrait for LoadStatementSegment {
    const TYPE: &'static str = "load_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("LOAD"), Ref::new("QuotedLiteralSegment"),])
            .to_matchable()
    }
}

pub struct ResetStatementSegment;

impl NodeTrait for ResetStatementSegment {
    const TYPE: &'static str = "reset_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("RESET"),
            one_of(vec_of_erased![
                Ref::keyword("ALL"),
                Ref::keyword("ROLE"),
                Ref::new("ParameterNameSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DiscardStatementSegment;

impl NodeTrait for DiscardStatementSegment {
    const TYPE: &'static str = "discard_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DISCARD"),
            one_of(vec_of_erased![
                Ref::keyword("ALL"),
                Ref::keyword("PLANS"),
                Ref::keyword("SEQUENCES"),
                Ref::keyword("TEMPORARY"),
                Ref::keyword("TEMP"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ListenStatementSegment;

impl NodeTrait for ListenStatementSegment {
    const TYPE: &'static str = "listen_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("LISTEN"), Ref::new("SingleIdentifierGrammar"),])
            .to_matchable()
    }
}

pub struct NotifyStatementSegment;

impl NodeTrait for NotifyStatementSegment {
    const TYPE: &'static str = "notify_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("NOTIFY"),
            Ref::new("SingleIdentifierGrammar"),
            Sequence::new(vec_of_erased![
                Ref::new("CommaSegment"),
                Ref::new("QuotedLiteralSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct UnlistenStatementSegment;

impl NodeTrait for UnlistenStatementSegment {
    const TYPE: &'static str = "unlisten_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNLISTEN"),
            one_of(vec_of_erased![Ref::new("SingleIdentifierGrammar"), Ref::new("StarSegment"),]),
        ])
        .to_matchable()
    }
}

pub struct TruncateStatementSegment;

impl NodeTrait for TruncateStatementSegment {
    const TYPE: &'static str = "truncate_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("TRUNCATE"),
            Ref::keyword("TABLE").optional(),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("ONLY").optional(),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::new("StarSegment").optional(),
                ]),
            ])]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("RESTART"), Ref::keyword("CONTINUE")]),
                Ref::keyword("IDENTITY")
            ])
            .config(|this| this.optional()),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct CopyStatementSegment;

impl NodeTrait for CopyStatementSegment {
    const TYPE: &'static str = "copy_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        let _target_subset = one_of(vec_of_erased![
            Ref::new("QuotedLiteralSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("PROGRAM"),
                Ref::new("QuotedLiteralSegment")
            ])
        ]);

        let _table_definition = Sequence::new(vec_of_erased![
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ColumnReferenceSegment"
            )])])
            .config(|this| this.optional()),
        ]);

        let _option = Sequence::new(vec_of_erased![
            Ref::keyword("WITH").optional(),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![any_set_of(
                vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FORMAT"),
                        Ref::new("SingleIdentifierGrammar")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FREEZE"),
                        Ref::new("BooleanLiteralGrammar").optional()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DELIMITER"),
                        Ref::new("QuotedLiteralSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NULL"),
                        Ref::new("QuotedLiteralSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("HEADER"),
                        Ref::new("BooleanLiteralGrammar").optional()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("QUOTE"),
                        Ref::new("QuotedLiteralSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ESCAPE"),
                        Ref::new("QuotedLiteralSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FORCE_QUOTE"),
                        one_of(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment")
                            ])]),
                            Ref::new("StarSegment")
                        ])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FORCE_NOT_NULL"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FORCE_NULL"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ENCODING"),
                        Ref::new("QuotedLiteralSegment")
                    ])
                ]
            )])])
        ])
        .config(|this| this.optional());

        Sequence::new(vec_of_erased![
            Ref::keyword("COPY"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    _table_definition.clone(),
                    Ref::keyword("FROM"),
                    one_of(vec_of_erased![_target_subset.clone(), Ref::keyword("STDIN")]),
                    _option.clone(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WHERE"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional())
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        _table_definition.clone(),
                        Bracketed::new(vec_of_erased![Ref::new("UnorderedSelectStatementSegment")])
                    ]),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![_target_subset, Ref::keyword("STDOUT")]),
                    _option
                ])
            ])
        ])
        .to_matchable()
    }
}

pub struct AlterSchemaStatementSegment;

impl NodeTrait for AlterSchemaStatementSegment {
    const TYPE: &'static str = "alter_schema_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("SCHEMA"),
            Ref::new("SchemaReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    Ref::new("RoleReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct LockTableStatementSegment;

impl NodeTrait for LockTableStatementSegment {
    const TYPE: &'static str = "lock_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("LOCK"),
            Ref::keyword("TABLE").optional(),
            Ref::keyword("ONLY").optional(),
            one_of(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment")]),
                Ref::new("StarSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("IN"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("ACCESS"), Ref::keyword("SHARE")]),
                    Sequence::new(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("SHARE")]),
                    Sequence::new(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("EXCLUSIVE")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SHARE"),
                        Ref::keyword("UPDATE"),
                        Ref::keyword("EXCLUSIVE")
                    ]),
                    Ref::keyword("SHARE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SHARE"),
                        Ref::keyword("ROW"),
                        Ref::keyword("EXCLUSIVE")
                    ]),
                    Ref::keyword("EXCLUSIVE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ACCESS"),
                        Ref::keyword("EXCLUSIVE")
                    ]),
                ]),
                Ref::keyword("MODE"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("NOWAIT").optional(),
        ])
        .to_matchable()
    }
}

pub struct ClusterStatementSegment;

impl NodeTrait for ClusterStatementSegment {
    const TYPE: &'static str = "cluster_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CLUSTER"),
            Ref::keyword("VERBOSE").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Ref::new("IndexReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("IndexReferenceSegment"),
                    Ref::keyword("ON"),
                    Ref::new("TableReferenceSegment"),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct LanguageClauseSegment;

impl NodeTrait for LanguageClauseSegment {
    const TYPE: &'static str = "language_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("LANGUAGE"),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("SingleQuotedIdentifierSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DoStatementSegment;

impl NodeTrait for DoStatementSegment {
    const TYPE: &'static str = "do_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DO"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("LanguageClauseSegment").optional(),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("LanguageClauseSegment").optional(),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CTEDefinitionSegment;

impl NodeTrait for CTEDefinitionSegment {
    const TYPE: &'static str = "common_table_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("CTEColumnList").optional(),
            Ref::keyword("AS"),
            Sequence::new(vec_of_erased![
                Ref::keyword("NOT").optional(),
                Ref::keyword("MATERIALIZED"),
            ])
            .config(|this| this.optional()),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar"),])
                .config(|this| this.parse_mode = ParseMode::Greedy),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SEARCH"),
                    one_of(vec_of_erased![Ref::keyword("BREADTH"), Ref::keyword("DEPTH"),]),
                    Ref::keyword("FIRST"),
                    Ref::keyword("BY"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("SET"),
                    Ref::new("ColumnReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CYCLE"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("SET"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("USING"),
                    Ref::new("ColumnReferenceSegment"),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct ValuesClauseSegment;

impl NodeTrait for ValuesClauseSegment {
    const TYPE: &'static str = "values_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("VALUES"),
            Delimited::new(vec_of_erased![Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Ref::keyword("DEFAULT")
                ])
                .config(|this| this.parse_mode = ParseMode::Greedy),
            ])]),
            Ref::new("AliasExpressionSegment").optional(),
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("LimitClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct DeleteStatementSegment;

impl NodeTrait for DeleteStatementSegment {
    const TYPE: &'static str = "delete_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DELETE"),
            Ref::keyword("FROM"),
            Ref::keyword("ONLY").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("StarSegment").optional(),
            Ref::new("AliasExpressionSegment").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                MetaSegment::indent(),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("TableExpressionSegment"),
                    Ref::new("AliasExpressionSegment").optional(),
                ])]),
                MetaSegment::dedent(),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHERE"),
                    Ref::keyword("CURRENT"),
                    Ref::keyword("OF"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Ref::new("WhereClauseSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("RETURNING"),
                one_of(vec_of_erased![
                    Ref::new("StarSegment"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::new("AliasExpressionSegment").optional(),
                    ])]),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct SetClauseSegment;

impl NodeTrait for SetClauseSegment {
    const TYPE: &'static str = "set_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("ColumnReferenceSegment"),
                Ref::new("ArrayAccessorSegment").optional(),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::new("LiteralGrammar"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("ExpressionSegment"),
                    Ref::keyword("DEFAULT"),
                ]),
                AnyNumberOf::new(vec_of_erased![Ref::new("ShorthandCastSegment")]),
            ]),
            Sequence::new(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ColumnReferenceSegment"
                ),])]),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("SelectableGrammar"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("LiteralGrammar"),
                            Ref::new("BareFunctionSegment"),
                            Ref::new("FunctionSegment"),
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("ExpressionSegment"),
                            Ref::keyword("DEFAULT"),
                        ]),
                        AnyNumberOf::new(vec_of_erased![Ref::new("ShorthandCastSegment")]),
                    ])])
                ])]),
            ]),
        ]),])
        .to_matchable()
    }
}

pub struct UpdateStatementSegment;

impl NodeTrait for UpdateStatementSegment {
    const TYPE: &'static str = "update_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UPDATE"),
            Ref::keyword("ONLY").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("AliasExpressionSegment").exclude(Ref::keyword("SET")).optional(),
            Ref::new("SetClauseListSegment"),
            Ref::new("FromClauseSegment").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHERE"),
                    Ref::keyword("CURRENT"),
                    Ref::keyword("OF"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Ref::new("WhereClauseSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("RETURNING"),
                one_of(vec_of_erased![
                    Ref::new("StarSegment"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::new("AliasExpressionSegment").optional(),
                    ])])
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct CreateTypeStatementSegment;

impl NodeTrait for CreateTypeStatementSegment {
    const TYPE: &'static str = "create_type_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("TYPE"),
            Ref::new("ObjectReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![Ref::keyword("ENUM"), Ref::keyword("RANGE"),])
                    .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
            Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Anything::new()]).config(|this| this.optional())
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterTypeStatementSegment;

impl NodeTrait for AlterTypeStatementSegment {
    const TYPE: &'static str = "alter_type_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TYPE"),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("OWNER"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                        Ref::keyword("CURRENT_ROLE"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("VALUE"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::keyword("TO"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ADD"),
                        Ref::keyword("ATTRIBUTE"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("DatatypeSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLLATE"),
                            Ref::new("CollationReferenceSegment"),
                        ])
                        .config(|this| this.optional()),
                        Ref::new("CascadeRestrictGrammar").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALTER"),
                        Ref::keyword("ATTRIBUTE"),
                        Ref::new("ColumnReferenceSegment"),
                        Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DATA"),])
                            .config(|this| this.optional()),
                        Ref::keyword("TYPE"),
                        Ref::new("DatatypeSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLLATE"),
                            Ref::new("CollationReferenceSegment"),
                        ])
                        .config(|this| this.optional()),
                        Ref::new("CascadeRestrictGrammar").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DROP"),
                        Ref::keyword("ATTRIBUTE"),
                        Ref::new("IfExistsGrammar").optional(),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("CascadeRestrictGrammar").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RENAME"),
                        Ref::keyword("ATTRIBUTE"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("TO"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("CascadeRestrictGrammar").optional(),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("VALUE"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("BEFORE"), Ref::keyword("AFTER")]),
                        Ref::new("QuotedLiteralSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateCollationStatementSegment;

impl NodeTrait for CreateCollationStatementSegment {
    const TYPE: &'static str = "create_collation_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("COLLATION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LOCALE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LC_COLLATE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LC_CTYPE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PROVIDER"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![Ref::keyword("ICU"), Ref::keyword("LIBC")]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DETERMINISTIC"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("VERSION"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ])]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FROM"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ColumnReferenceSegment;

impl NodeTrait for ColumnReferenceSegment {
    const TYPE: &'static str = "column_reference";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("SingleIdentifierGrammar"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("DotSegment"),
                    Sequence::new(vec_of_erased![Ref::new("DotSegment"), Ref::new("DotSegment"),]),
                ]),
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierFullGrammar"),]).config(
                    |this| {
                        this.delimiter(one_of(vec_of_erased![
                            Ref::new("DotSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("DotSegment"),
                                Ref::new("DotSegment"),
                            ]),
                        ]));
                        this.terminators = vec_of_erased![
                            Ref::keyword("ON"),
                            Ref::keyword("AS"),
                            Ref::keyword("USING"),
                            Ref::new("CommaSegment"),
                            Ref::new("CastOperatorSegment"),
                            Ref::new("StartSquareBracketSegment"),
                            Ref::new("StartBracketSegment"),
                            Ref::new("BinaryOperatorGrammar"),
                            Ref::new("ColonSegment"),
                            Ref::new("DelimiterGrammar"),
                            Ref::new("JoinLikeClauseGrammar"),
                            Bracketed::new(vec![]),
                        ];
                        this.allow_gaps = false;
                    }
                ),
            ])
            .config(|this| {
                this.optional();
                this.allow_gaps = false
            }),
        ])
        .config(|this| this.allow_gaps = false)
        .to_matchable()
    }
}

pub struct NamedArgumentSegment;

impl NodeTrait for NamedArgumentSegment {
    const TYPE: &'static str = "named_argument";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("NakedIdentifierSegment"),
            Ref::new("RightArrowSegment"),
            Ref::new("ExpressionSegment"),
        ])
        .to_matchable()
    }
}

pub struct TableExpressionSegment;

impl NodeTrait for TableExpressionSegment {
    const TYPE: &'static str = "table_expression_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            Sequence::new(vec_of_erased![
                Ref::new("FunctionSegment"),
                Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("ORDINALITY"),])
                    .config(|this| this.optional()),
            ]),
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar"),]),
            Bracketed::new(vec_of_erased![Ref::new("MergeStatementSegment"),]),
        ])
        .to_matchable()
    }
}

pub struct ServerReferenceSegment;

impl NodeTrait for ServerReferenceSegment {
    const TYPE: &'static str = "server_reference";

    fn match_grammar() -> Arc<dyn Matchable> {
        Ref::new("ObjectReferenceSegment").to_matchable()
    }
}

pub struct CreateServerStatementSegment;

impl NodeTrait for CreateServerStatementSegment {
    const TYPE: &'static str = "create_server_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("SERVER"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ServerReferenceSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("TYPE"), Ref::new("QuotedLiteralSegment"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("VERSION"),
                Ref::new("VersionIdentifierSegment"),
            ])
            .config(|this| this.optional()),
            Ref::new("ForeignDataWrapperGrammar"),
            Ref::new("ObjectReferenceSegment"),
            Ref::new("OptionsGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct CreateUserMappingStatementSegment;

impl NodeTrait for CreateUserMappingStatementSegment {
    const TYPE: &'static str = "create_user_mapping_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("CreateUserMappingGrammar"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::keyword("FOR"),
            one_of(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("SessionInformationUserFunctionsGrammar"),
                Ref::keyword("PUBLIC"),
            ]),
            Ref::keyword("SERVER"),
            Ref::new("ServerReferenceSegment"),
            Ref::new("OptionsGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct ImportForeignSchemaStatementSegment;

impl NodeTrait for ImportForeignSchemaStatementSegment {
    const TYPE: &'static str = "import_foreign_schema_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("ImportForeignSchemaGrammar"),
            Ref::new("SchemaReferenceSegment"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("LIMIT"), Ref::keyword("TO"),]),
                    Ref::keyword("EXCEPT"),
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "NakedIdentifierFullSegment"
                ),]),]),
            ])
            .config(|this| this.optional()),
            Ref::keyword("FROM"),
            Ref::keyword("SERVER"),
            Ref::new("ServerReferenceSegment"),
            Ref::keyword("INTO"),
            Ref::new("SchemaReferenceSegment"),
            Ref::new("OptionsGrammar").optional(),
        ])
        .to_matchable()
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect_file;
    use itertools::Itertools;
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    use crate::core::config::{FluffConfig, Value};
    use crate::core::linter::linter::Linter;
    use crate::core::parser::segments::base::ErasedSegment;
    use crate::helpers;

    fn parse_sql(linter: &Linter, sql: &str) -> ErasedSegment {
        let parsed = linter.parse_string(sql.into(), None, None, None).unwrap();
        parsed.tree.unwrap()
    }

    #[test]
    fn base_parse_struct() {
        let linter = Linter::new(
            FluffConfig::new(
                [(
                    "core".into(),
                    Value::Map([("dialect".into(), Value::String("postgres".into()))].into()),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );

        let files =
            glob::glob("test/fixtures/dialects/postgres/*.sql").unwrap().flatten().collect_vec();

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tree = parse_sql(&linter, &sql);
                let tree = tree.to_serialised(true, true, false);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}
