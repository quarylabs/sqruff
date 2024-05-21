use std::sync::Arc;

use ahash::AHashSet;
use itertools::Itertools;

use super::ansi::{self, ansi_dialect, Node, NodeTrait};
use super::bigquery_keywords::{BIGQUERY_RESERVED_KEYWORDS, BIGQUERY_UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Anything, Nothing, Ref};
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::{RegexLexer, StringLexer};
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, IdentifierSegment, Segment, SymbolSegment,
    SymbolSegmentNewArgs,
};
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::meta::MetaSegment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

pub fn bigquery_dialect() -> Dialect {
    let mut dialect = ansi_dialect();
    dialect.name = "bigquery";

    dialect.insert_lexer_matchers(
        vec![
            Box::new(StringLexer::new(
                "right_arrow",
                "=>",
                &CodeSegment::create,
                CodeSegmentNewArgs { code_type: "right_arrow", ..Default::default() },
                None,
                None,
            )),
            Box::new(StringLexer::new(
                "question_mark",
                "?",
                &CodeSegment::create,
                CodeSegmentNewArgs { code_type: "question_mark", ..Default::default() },
                None,
                None,
            )),
            Box::new(
                RegexLexer::new(
                    "at_sign_literal",
                    r#"@[a-zA-Z_][\w]*"#,
                    &CodeSegment::create,
                    CodeSegmentNewArgs { code_type: "at_sign_literal", ..Default::default() },
                    None,
                    None,
                )
                .unwrap(),
            ),
        ],
        "equals",
    );

    dialect.patch_lexer_matchers(vec![
        Box::new(RegexLexer::new(
            "single_quote",
            r"([rR]?[bB]?|[bB]?[rR]?)?('''((?<!\\)(\\{2})*\\'|'{,2}(?!')|[^'])*(?<!\\)(\\{2})*'''|'((?<!\\)(\\{2})*\\'|[^'])*(?<!\\)(\\{2})*')",
            &CodeSegment::create,
            CodeSegmentNewArgs { code_type: "single_quote", ..Default::default() },
            None,
            None,
        ).unwrap()),
        Box::new(
            RegexLexer::new(
                "double_quote",
                r#"([rR]?[bB]?|[bB]?[rR]?)?(\"\"\"((?<!\\)(\\{2})*\\\"|\"{,2}(?!\")|[^\"])*(?<!\\)(\\{2})*\"\"\"|"((?<!\\)(\\{2})*\\"|[^"])*(?<!\\)(\\{2})*")"#,
                &CodeSegment::create,
                CodeSegmentNewArgs { code_type: "double_quote", ..Default::default() },
                None,
                None,
            )
            .unwrap(),
        ),
    ]);

    dialect.add([
        (
            "DoubleQuotedLiteralSegment".into(),
            TypedParser::new(
                "double_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "quoted_literal" },
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
            "SingleQuotedLiteralSegment".into(),
            TypedParser::new(
                "single_quote",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "quoted_literal", ..Default::default() },
                    )
                },
                "quoted_literal".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DoubleQuotedUDFBody".into(),
            TypedParser::new(
                "double_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "udf_body" },
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
            "SingleQuotedUDFBody".into(),
            TypedParser::new(
                "single_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "udf_body" },
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
            "StartAngleBracketSegment".into(),
            StringParser::new(
                "<",
                |segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "start_angle_bracket" },
                    )
                },
                "start_angle_bracket".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndAngleBracketSegment".into(),
            StringParser::new(
                ">",
                |segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "end_angle_bracket" },
                    )
                },
                "end_angle_bracket".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "RightArrowSegment".into(),
            StringParser::new(
                "=>",
                |segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "remove me" },
                    )
                },
                "right_arrow".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DashSegment".into(),
            StringParser::new(
                "-",
                |segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "dash" },
                    )
                },
                "dash".to_owned().into(),
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
                Ref::new("NakedIdentifierFullSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuestionMarkSegment".into(),
            StringParser::new(
                "?",
                |segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "question_mark" },
                    )
                },
                "dash".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "AtSignLiteralSegment".into(),
            TypedParser::new(
                "at_sign_literal",
                |segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "at_sign_literal" },
                    )
                },
                "quoted_literal".to_owned().into(),
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DefaultDeclareOptionsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT"),
                one_of(vec_of_erased![
                    Ref::new("LiteralGrammar"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectStatementSegment")]),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::new("ArrayLiteralSegment"),
                    Ref::new("TupleSegment"),
                    Ref::new("BaseExpressionElementGrammar")
                ])
                .config(|this| {
                    this.terminators = vec_of_erased![Ref::new("SemicolonSegment")];
                })
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ExtendedDatetimeUnitSegment".into(),
            SegmentGenerator::new(|dialect| {
                Arc::new(MultiStringParser::new(
                    dialect
                        .sets("extended_datetime_units")
                        .into_iter()
                        .map(Into::into)
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs { code_type: "date_part", ..Default::default() },
                        )
                    },
                    None,
                    false,
                    None,
                ))
            })
            .into(),
        ),
        (
            "NakedIdentifierFullSegment".into(),
            RegexParser::new(
                "[A-Z_][A-Z0-9_]*",
                |segment| {
                    IdentifierSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "naked_identifier_all",
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
            "NakedIdentifierPart".into(),
            RegexParser::new(
                "[A-Z0-9_]+",
                |segment| {
                    IdentifierSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "naked_identifier", ..Default::default() },
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
            "ProcedureNameIdentifierSegment".into(),
            one_of(vec_of_erased![
                RegexParser::new(
                    "[A-Z_][A-Z0-9_]*",
                    |segment| {
                        IdentifierSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "procedure_name_identifier",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    "STRUCT".to_owned().into(),
                    None,
                ),
                RegexParser::new(
                    "`[^`]*`",
                    |segment| {
                        IdentifierSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "procedure_name_identifier",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                    None,
                ),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("IN"),
                        Ref::keyword("OUT"),
                        Ref::keyword("INOUT"),
                    ])
                    .config(|this| this.optional()),
                    Ref::new("ParameterNameSegment").optional(),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("TYPE")]),
                        Ref::new("DatatypeSegment")
                    ])
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("TYPE")]),
                    Ref::new("DatatypeSegment")
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                // Generate the anti template from the set of reserved keywords
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({})$", pattern);

                Arc::new(RegexParser::new(
                    "[A-Z_][A-Z0-9_]*",
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
                ))
            })
            .into(),
        ),
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("DatetimeUnitSegment"),
                Ref::new("DatePartWeekSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("IGNORE"), Ref::keyword("RESPECT")]),
                        Ref::keyword("NULLS")
                    ])
                    .config(|this| this.optional())
                ]),
                Ref::new("NamedArgumentSegment")
            ])
            .to_matchable()
            .into(),
        ),
        ("TrimParametersGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "ParameterNameSegment".into(),
            one_of(vec_of_erased![
                RegexParser::new(
                    "[A-Z_][A-Z0-9_]*",
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
                ),
                RegexParser::new(
                    "`[^`]*`",
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
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DateTimeLiteralGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATE"),
                    Ref::keyword("DATETIME"),
                    Ref::keyword("TIME"),
                    Ref::keyword("TIMESTAMP")
                ]),
                TypedParser::new(
                    "single_quote",
                    |segment| {
                        IdentifierSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "date_constructor_literal",
                                ..Default::default()
                            },
                        )
                    },
                    "quoted_identifier".to_owned().into(),
                    false,
                    vec!['`'].into(),
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "JoinLikeClauseGrammar".into(),
            Sequence::new(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("FromPivotExpressionSegment"),
                    Ref::new("FromUnpivotExpressionSegment")
                ])
                .config(|this| this.min_times = 1),
                Ref::new("AliasExpressionSegment").optional()
            ])
            .to_matchable()
            .into(),
        ),
        ("NaturalJoinKeywordsGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("ArrayAccessorSegment"),
                Ref::new("SemiStructuredAccessorSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MergeIntoLiteralGrammar".into(),
            Sequence::new(vec_of_erased![Ref::keyword("MERGE"), Ref::keyword("INTO").optional()])
                .to_matchable()
                .into(),
        ),
        ("PrimaryKeyGrammar".into(), Nothing::new().to_matchable().into()),
        ("ForeignKeyGrammar".into(), Nothing::new().to_matchable().into()),
    ]);

    // Set Keywords
    dialect.sets_mut("unreserved_keywords").clear();
    dialect.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        BIGQUERY_UNRESERVED_KEYWORDS,
    );

    dialect.sets_mut("reserved_keywords").clear();
    dialect
        .update_keywords_set_from_multiline_string("reserved_keywords", BIGQUERY_RESERVED_KEYWORDS);

    // Add additional datetime units
    // https://cloud.google.com/bigquery/docs/reference/standard-sql/timestamp_functions#extract
    dialect.sets_mut("datetime_units").extend([
        "MICROSECOND",
        "MILLISECOND",
        "SECOND",
        "MINUTE",
        "HOUR",
        "DAY",
        "DAYOFWEEK",
        "DAYOFYEAR",
        "WEEK",
        "ISOWEEK",
        "MONTH",
        "QUARTER",
        "YEAR",
        "ISOYEAR",
    ]);

    // Add additional datetime units only recognised in some functions (e.g.
    // extract)
    dialect.sets_mut("extended_datetime_units").extend(["DATE", "DATETIME", "TIME"]);

    dialect.sets_mut("date_part_function_name").clear();
    dialect.sets_mut("date_part_function_name").extend([
        "DATE_DIFF",
        "DATE_TRUNC",
        "DATETIME_DIFF",
        "DATETIME_TRUNC",
        "EXTRACT",
        "LAST_DAY",
        "TIME_DIFF",
        "TIME_TRUNC",
        "TIMESTAMP_DIFF",
        "TIMESTAMP_TRUNC",
    ]);

    // Set value table functions
    dialect.sets_mut("value_table_functions").extend(["UNNEST"]);

    // Set angle bracket pairs
    dialect.bracket_sets_mut("angle_bracket_pairs").extend([(
        "angle".to_string(),
        "StartAngleBracketSegment".to_string(),
        "EndAngleBracketSegment".to_string(),
        false,
    )]);

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
        dialect,
        ArrayTypeSegment,
        QualifyClauseSegment,
        SetOperatorSegment,
        SetExpressionSegment,
        SelectStatementSegment,
        UnorderedSelectStatementSegment,
        MultiStatementSegment,
        FileSegment,
        StatementSegment,
        AssertStatementSegment,
        ForInStatementsSegment,
        ForInStatementSegment,
        RepeatStatementsSegment,
        RepeatStatementSegment,
        IfStatementsSegment,
        IfStatementSegment,
        LoopStatementsSegment,
        LoopStatementSegment,
        WhileStatementsSegment,
        WhileStatementSegment,
        SelectClauseModifierSegment,
        IntervalExpressionSegment,
        ExtractFunctionNameSegment,
        ArrayFunctionNameSegment,
        DatePartWeekSegment,
        NormalizeFunctionNameSegment,
        FunctionNameSegment,
        FunctionSegment,
        FunctionDefinitionGrammar,
        WildcardExpressionSegment,
        ExceptClauseSegment,
        ReplaceClauseSegment,
        DatatypeSegment,
        StructTypeSegment,
        StructTypeSchemaSegment,
        ArrayExpressionSegment,
        TupleSegment,
        NamedArgumentSegment,
        SemiStructuredAccessorSegment,
        ColumnReferenceSegment,
        TableReferenceSegment,
        DeclareStatementSegment,
        SetStatementSegment,
        PartitionBySegment,
        ClusterBySegment,
        OptionsSegment,
        ColumnDefinitionSegment,
        CreateTableStatementSegment,
        AlterTableStatementSegment,
        CreateExternalTableStatementSegment,
        CreateViewStatementSegment,
        AlterViewStatementSegment,
        CreateMaterializedViewStatementSegment,
        AlterMaterializedViewStatementSegment,
        DropMaterializedViewStatementSegment,
        ParameterizedSegment,
        PivotForClauseSegment,
        FromPivotExpressionSegment,
        UnpivotAliasExpressionSegment,
        FromUnpivotExpressionSegment,
        InsertStatementSegment,
        SamplingExpressionSegment,
        MergeMatchSegment,
        MergeNotMatchedByTargetClauseSegment,
        MergeNotMatchedBySourceClauseSegment,
        MergeInsertClauseSegment,
        DeleteStatementSegment,
        ExportStatementSegment,
        ProcedureNameSegment,
        ProcedureParameterListSegment,
        ProcedureStatements,
        CallStatementSegment,
        ReturnStatementSegment,
        BreakStatementSegment,
        LeaveStatementSegment,
        ContinueStatementSegment,
        RaiseStatementSegment,
        CreateProcedureStatementSegment
    );

    dialect.add([
        (
            "QuotedIdentifierSegment".into(),
            TypedParser::new(
                "back_quote",
                |segment| {
                    IdentifierSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "naked_identifier", ..Default::default() },
                    )
                },
                "quoted_identifier".to_owned().into(),
                false,
                vec!['`'].into(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NumericLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(
                    "numeric_literal",
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "numeric_literal",
                                ..Default::default()
                            },
                        )
                    },
                    "numeric_literal".to_owned().into(),
                    false,
                    None,
                ),
                Ref::new("ParameterizedSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                Ref::new("SingleQuotedLiteralSegment"),
                Ref::new("DoubleQuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LiteralGrammar".into(),
            dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("ParameterizedSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "PostTableExpressionGrammar".into(),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("FOR"),
                    one_of(vec_of_erased![
                        Ref::keyword("SYSTEM_TIME"),
                        Sequence::new(vec_of_erased![Ref::keyword("SYSTEM"), Ref::keyword("TIME")]),
                    ]),
                    Ref::keyword("AS"),
                    Ref::keyword("OF"),
                    Ref::new("ExpressionSegment")
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("OFFSET"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("SingleIdentifierGrammar")
                    ])
                    .config(|this| this.optional()),
                ])
                .config(|this| this.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            one_of(vec_of_erased![
                RegexParser::new(
                    "[A-Z_][A-Z0-9_]*",
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
                    "^(STRUCT|ARRAY)$".to_owned().into(),
                    None,
                ),
                RegexParser::new(
                    "`[^`]*`",
                    |segment| {
                        IdentifierSegment::create(
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
                ),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.expand();
    dialect
}

pub struct ArrayTypeSegment;

impl NodeTrait for ArrayTypeSegment {
    const TYPE: &'static str = "array_type";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ARRAY"),
            Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]).config(|this| {
                this.bracket_type = "angle";
                this.bracket_pairs_set = "angle_bracket_pairs";
            })
        ])
        .to_matchable()
    }
}

pub struct QualifyClauseSegment;

impl NodeTrait for QualifyClauseSegment {
    const TYPE: &'static str = "qualify_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("QUALIFY"),
            MetaSegment::indent(),
            optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct SetOperatorSegment;

impl NodeTrait for SetOperatorSegment {
    const TYPE: &'static str = "set_operator";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("UNION"),
                one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")]),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("INTERSECT"), Ref::keyword("DISTINCT")]),
            Sequence::new(vec_of_erased![Ref::keyword("EXCEPT"), Ref::keyword("DISTINCT")]),
        ])
        .to_matchable()
    }
}

pub struct SetExpressionSegment;

impl NodeTrait for SetExpressionSegment {
    const TYPE: &'static str = "set_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("NonSetSelectableGrammar"),
                Bracketed::new(vec_of_erased![Ref::new("SetExpressionSegment")]),
            ]),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("SetOperatorSegment"),
                one_of(vec_of_erased![
                    Ref::new("NonSetSelectableGrammar"),
                    Bracketed::new(vec_of_erased![Ref::new("SetExpressionSegment")]),
                ]),
            ])])
            .config(|this| this.min_times = 1),
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("LimitClauseSegment").optional(),
            Ref::new("NamedWindowSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct SelectStatementSegment;

impl NodeTrait for SelectStatementSegment {
    const TYPE: &'static str = "select_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::SelectStatementSegment::match_grammar().copy(
            Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
            None,
            Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct UnorderedSelectStatementSegment;

impl NodeTrait for UnorderedSelectStatementSegment {
    const TYPE: &'static str = "unordered_select_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::UnorderedSelectStatementSegment::match_grammar().copy(
            Some(vec![Ref::new("QualifyClauseSegment").optional().to_matchable()]),
            None,
            Some(Ref::new("OverlapsClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct MultiStatementSegment;

impl NodeTrait for MultiStatementSegment {
    const TYPE: &'static str = "multi_statement_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::new("ForInStatementSegment"),
            Ref::new("RepeatStatementSegment"),
            Ref::new("WhileStatementSegment"),
            Ref::new("LoopStatementSegment"),
            Ref::new("IfStatementSegment"),
            Ref::new("CreateProcedureStatementSegment"),
        ])
        .to_matchable()
    }
}

pub struct FileSegment;

impl NodeTrait for FileSegment {
    const TYPE: &'static str = "file_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("MultiStatementSegment"),
                Ref::new("StatementSegment")
            ])]),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("DelimiterGrammar"),
                one_of(vec_of_erased![
                    Ref::new("MultiStatementSegment"),
                    Ref::new("StatementSegment")
                ])
            ]),
            Ref::new("DelimiterGrammar").optional()
        ])
        .to_matchable()
    }
}

pub struct StatementSegment;

impl NodeTrait for StatementSegment {
    const TYPE: &'static str = "statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::StatementSegment::match_grammar().copy(
            Some(vec_of_erased![
                Ref::new("DeclareStatementSegment"),
                Ref::new("SetStatementSegment"),
                Ref::new("ExportStatementSegment"),
                Ref::new("CreateExternalTableStatementSegment"),
                Ref::new("AssertStatementSegment"),
                Ref::new("CallStatementSegment"),
                Ref::new("ReturnStatementSegment"),
                Ref::new("BreakStatementSegment"),
                Ref::new("LeaveStatementSegment"),
                Ref::new("ContinueStatementSegment"),
                Ref::new("RaiseStatementSegment"),
                Ref::new("AlterViewStatementSegment"),
                Ref::new("CreateMaterializedViewStatementSegment"),
                Ref::new("AlterMaterializedViewStatementSegment"),
                Ref::new("DropMaterializedViewStatementSegment"),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct AssertStatementSegment;

impl NodeTrait for AssertStatementSegment {
    const TYPE: &'static str = "assert_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ASSERT"),
            Ref::new("ExpressionSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::new("QuotedLiteralSegment")])
                .config(|this| this.optional())
        ])
        .to_matchable()
    }
}

pub struct ForInStatementsSegment;

impl NodeTrait for ForInStatementsSegment {
    const TYPE: &'static str = "for_in_statements";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment")]),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Ref::keyword("FOR")
            ])];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct ForInStatementSegment;

impl NodeTrait for ForInStatementSegment {
    const TYPE: &'static str = "for_in_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("FOR"),
            Ref::new("SingleIdentifierGrammar"),
            Ref::keyword("IN"),
            MetaSegment::indent(),
            Ref::new("SelectableGrammar"),
            MetaSegment::dedent(),
            Ref::keyword("DO"),
            MetaSegment::indent(),
            Ref::new("ForInStatementsSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("FOR")
        ])
        .to_matchable()
    }
}

pub struct RepeatStatementsSegment;

impl NodeTrait for RepeatStatementsSegment {
    const TYPE: &'static str = "repeat_statements";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(
                vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment"),]
            ),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Ref::keyword("UNTIL")];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct RepeatStatementSegment;

impl NodeTrait for RepeatStatementSegment {
    const TYPE: &'static str = "repeat_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REPEAT"),
            MetaSegment::indent(),
            Ref::new("RepeatStatementsSegment"),
            Ref::keyword("UNTIL"),
            Ref::new("ExpressionSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("REPEAT")
        ])
        .to_matchable()
    }
}

pub struct IfStatementsSegment;

impl NodeTrait for IfStatementsSegment {
    const TYPE: &'static str = "if_statements";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment")]),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![
                Ref::keyword("ELSE"),
                Ref::keyword("ELSEIF"),
                Sequence::new(vec_of_erased![Ref::keyword("END"), Ref::keyword("IF")])
            ];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct IfStatementSegment;

impl NodeTrait for IfStatementSegment {
    const TYPE: &'static str = "if_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("IF"),
            Ref::new("ExpressionSegment"),
            Ref::keyword("THEN"),
            MetaSegment::indent(),
            Ref::new("IfStatementsSegment"),
            MetaSegment::dedent(),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("ELSEIF"),
                Ref::new("ExpressionSegment"),
                Ref::keyword("THEN"),
                MetaSegment::indent(),
                Ref::new("IfStatementsSegment"),
                MetaSegment::dedent()
            ])]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ELSE"),
                MetaSegment::indent(),
                Ref::new("IfStatementsSegment"),
                MetaSegment::dedent()
            ])
            .config(|this| this.optional()),
            Ref::keyword("END"),
            Ref::keyword("IF")
        ])
        .to_matchable()
    }
}

pub struct LoopStatementsSegment;

impl NodeTrait for LoopStatementsSegment {
    const TYPE: &'static str = "loop_statements";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("StatementSegment"), Ref::new("MultiStatementSegment")]),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Ref::keyword("LOOP")
            ])];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct LoopStatementSegment;

impl NodeTrait for LoopStatementSegment {
    const TYPE: &'static str = "loop_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("LOOP"),
            MetaSegment::indent(),
            Ref::new("LoopStatementsSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("LOOP")
        ])
        .to_matchable()
    }
}

pub struct WhileStatementsSegment;

impl NodeTrait for WhileStatementsSegment {
    const TYPE: &'static str = "while_statements";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            Ref::new("StatementSegment"),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Ref::keyword("WHILE")
            ])];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct WhileStatementSegment;

impl NodeTrait for WhileStatementSegment {
    const TYPE: &'static str = "while_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WHILE"),
            Ref::new("ExpressionSegment"),
            Ref::keyword("DO"),
            MetaSegment::indent(),
            Ref::new("WhileStatementsSegment"),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("WHILE")
        ])
        .to_matchable()
    }
}

pub struct SelectClauseModifierSegment;

impl NodeTrait for SelectClauseModifierSegment {
    const TYPE: &'static str = "select_clause_modifier";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL")])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![Ref::keyword("STRUCT"), Ref::keyword("VALUE")])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
    }
}

pub struct IntervalExpressionSegment;

impl NodeTrait for IntervalExpressionSegment {
    const TYPE: &'static str = "interval_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INTERVAL"),
            Ref::new("ExpressionSegment"),
            one_of(vec_of_erased![
                Ref::new("QuotedLiteralSegment"),
                Ref::new("DatetimeUnitSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("DatetimeUnitSegment"),
                    Ref::keyword("TO"),
                    Ref::new("DatetimeUnitSegment")
                ])
            ])
        ])
        .to_matchable()
    }
}

pub struct ExtractFunctionNameSegment;

impl NodeTrait for ExtractFunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Arc<dyn Matchable> {
        StringParser::new(
            "EXTRACT",
            |segment| {
                SymbolSegment::create(
                    &segment.raw(),
                    segment.get_position_marker(),
                    SymbolSegmentNewArgs { r#type: "function_name_identifier" },
                )
            },
            "function_name_identifier".to_owned().into(),
            false,
            None,
        )
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["function_name"].into()
    }
}

pub struct ArrayFunctionNameSegment;

impl NodeTrait for ArrayFunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Arc<dyn Matchable> {
        StringParser::new(
            "ARRAY",
            |segment| {
                SymbolSegment::create(
                    &segment.raw(),
                    segment.get_position_marker(),
                    SymbolSegmentNewArgs { r#type: "function_name_identifier" },
                )
            },
            "function_name_identifier".to_owned().into(),
            false,
            None,
        )
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["function_name"].into()
    }
}

pub struct DatePartWeekSegment;

impl NodeTrait for DatePartWeekSegment {
    const TYPE: &'static str = "date_part_week";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WEEK"),
            Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("SUNDAY"),
                Ref::keyword("MONDAY"),
                Ref::keyword("TUESDAY"),
                Ref::keyword("WEDNESDAY"),
                Ref::keyword("THURSDAY"),
                Ref::keyword("FRIDAY"),
                Ref::keyword("SATURDAY")
            ])])
        ])
        .to_matchable()
    }
}

pub struct NormalizeFunctionNameSegment;

impl NodeTrait for NormalizeFunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            StringParser::new(
                "NORMALIZE",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "function_name_identifier" },
                    )
                },
                None,
                false,
                None,
            ),
            StringParser::new(
                "NORMALIZE_AND_CASEFOLD",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "function_name_identifier" },
                    )
                },
                None,
                false,
                None,
            ),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["function_name"].into()
    }
}

pub struct FunctionNameSegment;

impl NodeTrait for FunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            // AnyNumberOf to handle project names, schemas, or the SAFE keyword
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("SAFE"),
                        Ref::new("SingleIdentifierGrammar")
                    ]),
                    Ref::new("DotSegment"),
                ])
                .terminators(vec_of_erased![Ref::new("BracketedSegment")])
            ]),
            // Base function name
            one_of(vec_of_erased![
                Ref::new("FunctionNameIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment")
            ])
            .config(|this| this.terminators = vec_of_erased![Ref::new("BracketedSegment")]),
        ])
        .allow_gaps(true)
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["function_name"].into()
    }
}

pub struct FunctionSegment;

impl NodeTrait for FunctionSegment {
    const TYPE: &'static str = "function";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                // BigQuery EXTRACT allows optional TimeZone
                Ref::new("ExtractFunctionNameSegment"),
                Bracketed::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("DatetimeUnitSegment"),
                        Ref::new("DatePartWeekSegment"),
                        Ref::new("ExtendedDatetimeUnitSegment")
                    ]),
                    Ref::keyword("FROM"),
                    Ref::new("ExpressionSegment")
                ])
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("NormalizeFunctionNameSegment"),
                Bracketed::new(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("CommaSegment"),
                        one_of(vec_of_erased![
                            Ref::keyword("NFC"),
                            Ref::keyword("NFKC"),
                            Ref::keyword("NFD"),
                            Ref::keyword("NFKD")
                        ])
                    ])
                    .config(|this| this.optional())
                ])
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("DatePartFunctionNameSegment")
                    .exclude(Ref::new("ExtractFunctionNameSegment")),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Ref::new("DatetimeUnitSegment"),
                    Ref::new("DatePartWeekSegment"),
                    Ref::new("FunctionContentsGrammar")
                ])])
                .config(|this| this.parse_mode(ParseMode::Greedy))
            ]),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("FunctionNameSegment").exclude(one_of(vec_of_erased![
                        Ref::new("DatePartFunctionNameSegment"),
                        Ref::new("NormalizeFunctionNameSegment"),
                        Ref::new("ValuesClauseSegment"),
                    ])),
                    Bracketed::new(vec_of_erased![Ref::new("FunctionContentsGrammar").optional()])
                        .config(|this| this.parse_mode(ParseMode::Greedy))
                ]),
                Ref::new("ArrayAccessorSegment").optional(),
                Ref::new("SemiStructuredAccessorSegment").optional(),
                Ref::new("PostFunctionGrammar").optional()
            ]),
        ])])
        .config(|this| this.allow_gaps = false)
        .to_matchable()
    }
}

pub struct FunctionDefinitionGrammar;

impl NodeTrait for FunctionDefinitionGrammar {
    const TYPE: &'static str = "function_definition";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![
            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("DETERMINISTIC"),
                Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("DETERMINISTIC")])
            ])])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("LANGUAGE"),
                Ref::new("NakedIdentifierSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OPTIONS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Ref::new("EqualsSegment"),
                            Anything::new()
                        ]),
                        Ref::new("CommaSegment")
                    ])])
                ])
                .config(|this| this.optional())
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    Ref::new("DoubleQuotedUDFBody"),
                    Ref::new("SingleQuotedUDFBody"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::new("SelectStatementSegment")
                    ])])
                ])
            ]),
        ])])
        .to_matchable()
    }
}

pub struct WildcardExpressionSegment;

impl NodeTrait for WildcardExpressionSegment {
    const TYPE: &'static str = "wildcard_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::WildcardExpressionSegment::match_grammar().copy(
            Some(vec_of_erased![
                Ref::new("ExceptClauseSegment").optional(),
                Ref::new("ReplaceClauseSegment").optional(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        )
    }
}

pub struct ExceptClauseSegment;

impl NodeTrait for ExceptClauseSegment {
    const TYPE: &'static str = "select_except_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("EXCEPT"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "SingleIdentifierGrammar"
            )])])
        ])
        .to_matchable()
    }
}

pub struct ReplaceClauseSegment;

impl NodeTrait for ReplaceClauseSegment {
    const TYPE: &'static str = "select_replace_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REPLACE"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "SelectClauseElementSegment"
            )])])
        ])
        .to_matchable()
    }
}

pub struct DatatypeSegment;

impl NodeTrait for DatatypeSegment {
    const TYPE: &'static str = "data_type";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("DatatypeIdentifierSegment"),
                Ref::new("BracketedArguments").optional(),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("TYPE")]),
            Ref::new("ArrayTypeSegment"),
            Ref::new("StructTypeSegment"),
        ])
        .to_matchable()
    }
}

pub struct StructTypeSegment;

impl NodeTrait for StructTypeSegment {
    const TYPE: &'static str = "struct_type";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("STRUCT"),
            Ref::new("StructTypeSchemaSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct StructTypeSchemaSegment;

impl NodeTrait for StructTypeSchemaSegment {
    const TYPE: &'static str = "struct_type_schema";

    fn match_grammar() -> Arc<dyn Matchable> {
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
            vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("DatatypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Ref::new("DatatypeSegment"),
                    ]),
                ]),
                AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")]),
                Ref::new("OptionsSegment").optional(),
            ]
        )])])
        .config(|this| {
            this.bracket_type = "angle";
            this.bracket_pairs_set = "angle_bracket_pairs";
        })
        .to_matchable()
    }
}

pub struct ArrayExpressionSegment;

impl NodeTrait for ArrayExpressionSegment {
    const TYPE: &'static str = "array_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("ArrayFunctionNameSegment"),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
        ])
        .to_matchable()
    }
}

pub struct TupleSegment;

impl NodeTrait for TupleSegment {
    const TYPE: &'static str = "tuple";

    fn match_grammar() -> Arc<dyn Matchable> {
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
            "BaseExpressionElementGrammar"
        )])])
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

pub struct SemiStructuredAccessorSegment;

impl NodeTrait for SemiStructuredAccessorSegment {
    const TYPE: &'static str = "semi_structured_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("DotSegment"),
                    one_of(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("StarSegment")
                    ])
                ]),
                Ref::new("ArrayAccessorSegment").optional()
            ])
            .config(|this| this.min_times(1))
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
                Ref::new("ObjectReferenceDelimiterGrammar"),
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierFullGrammar")]).config(
                    |this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
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
                            Ref::new("BracketedSegment")
                        ];
                        this.allow_gaps = false;
                    }
                )
            ])
            .allow_gaps(false)
            .config(|this| this.optional())
        ])
        .allow_gaps(false)
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["object_reference", "column_reference"].into()
    }
}

pub struct TableReferenceSegment;

impl NodeTrait for TableReferenceSegment {
    const TYPE: &'static str = "table_reference";

    fn match_grammar() -> Arc<dyn Matchable> {
        Delimited::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("DashSegment"),
                        Ref::new("NakedIdentifierPart")
                    ])
                    .config(|this| this.allow_gaps = false)
                ])
                .config(|this| this.optional())
            ])
            .config(|this| this.allow_gaps = false)
        ])
        .config(|this| {
            this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
            this.terminators = vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("AS"),
                Ref::keyword("USING"),
                Ref::new("CommaSegment"),
                Ref::new("CastOperatorSegment"),
                Ref::new("StartSquareBracketSegment"),
                Ref::new("StartBracketSegment"),
                Ref::new("ColonSegment"),
                Ref::new("DelimiterGrammar"),
                Ref::new("JoinLikeClauseGrammar"),
                Ref::new("BracketedSegment")
            ];
            this.allow_gaps = false;
        })
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["table_reference"].into_iter().collect()
    }
}

pub struct DeclareStatementSegment;

impl NodeTrait for DeclareStatementSegment {
    const TYPE: &'static str = "declare_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DECLARE"),
            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierFullGrammar")]),
            one_of(vec_of_erased![
                Ref::new("DefaultDeclareOptionsGrammar"),
                Sequence::new(vec_of_erased![
                    Ref::new("DatatypeSegment"),
                    Ref::new("DefaultDeclareOptionsGrammar").optional()
                ])
            ])
        ])
        .to_matchable()
    }
}

pub struct SetStatementSegment;

impl NodeTrait for SetStatementSegment {
    const TYPE: &'static str = "set_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "NakedIdentifierSegment"
                )])])
            ]),
            Ref::new("EqualsSegment"),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("LiteralGrammar"),
                Bracketed::new(vec_of_erased![Ref::new("SelectStatementSegment")]),
                Ref::new("BareFunctionSegment"),
                Ref::new("FunctionSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Bracketed::new(vec_of_erased![Ref::new("SelectStatementSegment")]),
                        Ref::new("BareFunctionSegment"),
                        Ref::new("FunctionSegment")
                    ]
                )])]),
                Ref::new("ArrayLiteralSegment"),
                Ref::new("ExpressionSegment")
            ])])
        ])
        .to_matchable()
    }
}

pub struct PartitionBySegment;

impl NodeTrait for PartitionBySegment {
    const TYPE: &'static str = "partition_by_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("PARTITION"),
            Ref::keyword("BY"),
            Ref::new("ExpressionSegment")
        ])
        .to_matchable()
    }
}

pub struct ClusterBySegment;

impl NodeTrait for ClusterBySegment {
    const TYPE: &'static str = "cluster_by_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CLUSTER"),
            Ref::keyword("BY"),
            Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
        ])
        .to_matchable()
    }
}

pub struct OptionsSegment;

impl NodeTrait for OptionsSegment {
    const TYPE: &'static str = "options_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("OPTIONS"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BaseExpressionElementGrammar")
                ]
            )])])
        ])
        .to_matchable()
    }
}

pub struct ColumnDefinitionSegment;

impl NodeTrait for ColumnDefinitionSegment {
    const TYPE: &'static str = "column_definition";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("SingleIdentifierGrammar"), // Column name
            Ref::new("DatatypeSegment"),         // Column type
            AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")]),
            Ref::new("OptionsSegment").optional()
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
            Ref::new("OrReplaceGrammar").optional(),
            Ref::new("TemporaryTransientGrammar").optional(),
            Ref::keyword("TABLE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("COPY"),
                    Ref::keyword("LIKE"),
                    Ref::keyword("CLONE")
                ]),
                Ref::new("TableReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("ColumnDefinitionSegment")],)
                    .config(|this| this.allow_trailing())
            ])])
            .config(|this| this.optional()),
            Ref::new("PartitionBySegment").optional(),
            Ref::new("ClusterBySegment").optional(),
            Ref::new("OptionsSegment").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
            ])
            .config(|this| this.optional()),
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
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                // SET OPTIONS
                Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::new("OptionsSegment"),]),
                // ADD COLUMN
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("ColumnDefinitionSegment"),
                ])])
                .config(|this| this.allow_trailing = true),
                // RENAME TO
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment"),
                ]),
                // RENAME COLUMN
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("TO"),
                    Ref::new("SingleIdentifierGrammar"),
                ])])
                .config(|this| this.allow_trailing = true),
                // DROP COLUMN
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                ])]),
                // ALTER COLUMN SET OPTIONS
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            one_of(vec_of_erased![
                                Ref::new("OptionsSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("DATA"),
                                    Ref::keyword("TYPE"),
                                    Ref::new("DatatypeSegment"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("DEFAULT"),
                                    one_of(vec_of_erased![
                                        Ref::new("LiteralGrammar"),
                                        Ref::new("FunctionSegment"),
                                    ]),
                                ]),
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DROP"),
                            one_of(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("NOT"),
                                    Ref::keyword("NULL"),
                                ]),
                            ]),
                        ]),
                    ]),
                ])])
            ])
        ])
        .to_matchable()
    }
}

pub struct CreateExternalTableStatementSegment;

impl NodeTrait for CreateExternalTableStatementSegment {
    const TYPE: &'static str = "create_external_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("OR").optional(),
                Ref::keyword("REPLACE").optional()
            ])
            .config(|this| this.optional()),
            Ref::keyword("EXTERNAL"),
            Ref::keyword("TABLE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF").optional(),
                Ref::keyword("NOT").optional(),
                Ref::keyword("EXISTS").optional()
            ])
            .config(|this| this.optional()),
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("ColumnDefinitionSegment")])
                    .config(|this| this.allow_trailing = true)
            ])
            .config(|this| this.optional()),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("CONNECTION"),
                    Ref::new("TableReferenceSegment")
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("PARTITION"),
                    Ref::keyword("COLUMNS"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("ColumnDefinitionSegment")])
                            .config(|this| this.allow_trailing = true)
                    ])
                    .config(|this| this.optional())
                ])
                .config(|this| this.optional()),
                Ref::new("OptionsSegment").optional()
            ])
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
            Ref::keyword("VIEW"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Ref::new("OptionsSegment").optional(),
            Ref::keyword("AS"),
            optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
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
            Ref::keyword("SET"),
            Ref::new("OptionsSegment"),
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
            Ref::new("PartitionBySegment").optional(),
            Ref::new("ClusterBySegment").optional(),
            Ref::new("OptionsSegment").optional(),
            Ref::keyword("AS"),
            optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
        ])
        .to_matchable()
    }
}

pub struct AlterMaterializedViewStatementSegment;

impl NodeTrait for AlterMaterializedViewStatementSegment {
    const TYPE: &'static str = "alter_materialized_view_set_options_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::keyword("SET"),
            Ref::new("OptionsSegment"),
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
            Ref::new("TableReferenceSegment"),
        ])
        .to_matchable()
    }
}

pub struct ParameterizedSegment;

impl NodeTrait for ParameterizedSegment {
    const TYPE: &'static str = "parameterized_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![Ref::new("AtSignLiteralSegment"), Ref::new("QuestionMarkSegment"),])
            .to_matchable()
    }
}

pub struct PivotForClauseSegment;

impl NodeTrait for PivotForClauseSegment {
    const TYPE: &'static str = "pivot_for_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::new("BaseExpressionElementGrammar")])
            .config(|this| {
                this.terminators = vec_of_erased![Ref::keyword("IN")];
                this.parse_mode(ParseMode::Greedy);
            })
            .to_matchable()
    }
}

pub struct FromPivotExpressionSegment;

impl NodeTrait for FromPivotExpressionSegment {
    const TYPE: &'static str = "from_pivot_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("PIVOT"),
            Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("FunctionSegment"),
                    Ref::new("AliasExpressionSegment").optional(),
                ])]),
                Ref::keyword("FOR"),
                Ref::new("PivotForClauseSegment"),
                Ref::keyword("IN"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Ref::new("AliasExpressionSegment").optional(),
                    ]
                )])])
            ]),
        ])
        .to_matchable()
    }
}

pub struct UnpivotAliasExpressionSegment;

impl NodeTrait for UnpivotAliasExpressionSegment {
    const TYPE: &'static str = "alias_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            MetaSegment::indent(),
            Ref::keyword("AS").optional(),
            one_of(vec_of_erased![
                Ref::new("QuotedLiteralSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alias_expression"].into()
    }
}

pub struct FromUnpivotExpressionSegment;

impl NodeTrait for FromUnpivotExpressionSegment {
    const TYPE: &'static str = "from_unpivot_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNPIVOT"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("INCLUDE"), Ref::keyword("EXCLUDE")]),
                Ref::keyword("NULLS"),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                // Single column unpivot
                Bracketed::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("FOR"),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("IN"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]),
                            Ref::new("UnpivotAliasExpressionSegment").optional(),
                        ]
                    )])])
                ]),
                // Multi column unpivot
                Bracketed::new(vec_of_erased![
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                            .config(|this| this.min_delimiters = 1.into()),
                    ]),
                    Ref::keyword("FOR"),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("IN"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Bracketed::new(vec_of_erased![
                                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                                    .config(|this| this.min_delimiters = 1.into()),
                            ]),
                            Ref::new("UnpivotAliasExpressionSegment").optional(),
                        ]
                    )])])
                ])
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
            Ref::keyword("INTO").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            Ref::new("SelectableGrammar")
        ])
        .to_matchable()
    }
}

pub struct SamplingExpressionSegment;

impl NodeTrait for SamplingExpressionSegment {
    const TYPE: &'static str = "sample_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("TABLESAMPLE"),
            Ref::keyword("SYSTEM"),
            Bracketed::new(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Ref::keyword("PERCENT")
            ]),
        ])
        .to_matchable()
    }
}

pub struct MergeMatchSegment;

impl NodeTrait for MergeMatchSegment {
    const TYPE: &'static str = "merge_match";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![
            Ref::new("MergeMatchedClauseSegment"),
            Ref::new("MergeNotMatchedByTargetClauseSegment"),
            Ref::new("MergeNotMatchedBySourceClauseSegment"),
        ])
        .config(|this| this.min_times = 1)
        .to_matchable()
    }
}

pub struct MergeNotMatchedByTargetClauseSegment;

impl NodeTrait for MergeNotMatchedByTargetClauseSegment {
    const TYPE: &'static str = "not_matched_by_target_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WHEN"),
            Ref::keyword("NOT"),
            Ref::keyword("MATCHED"),
            Sequence::new(vec_of_erased![Ref::keyword("BY"), Ref::keyword("TARGET")])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("AND"), Ref::new("ExpressionSegment"),])
                .config(|this| this.optional()),
            Ref::keyword("THEN"),
            MetaSegment::indent(),
            Ref::new("MergeInsertClauseSegment"),
            MetaSegment::dedent()
        ])
        .to_matchable()
    }
}

pub struct MergeNotMatchedBySourceClauseSegment;

impl NodeTrait for MergeNotMatchedBySourceClauseSegment {
    const TYPE: &'static str = "merge_when_matched_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WHEN"),
            Ref::keyword("NOT"),
            Ref::keyword("MATCHED"),
            Ref::keyword("BY"),
            Ref::keyword("SOURCE"),
            Sequence::new(vec_of_erased![Ref::keyword("AND"), Ref::new("ExpressionSegment")])
                .config(|s| s.optional()),
            Ref::keyword("THEN"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Ref::new("MergeUpdateClauseSegment"),
                Ref::new("MergeDeleteClauseSegment")
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable()
    }
}

pub struct MergeInsertClauseSegment;

impl NodeTrait for MergeInsertClauseSegment {
    const TYPE: &'static str = "merge_insert_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("INSERT"),
                MetaSegment::indent(),
                Ref::new("BracketedColumnReferenceListGrammar").optional(),
                MetaSegment::dedent(),
                Ref::new("ValuesClauseSegment").optional(),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("INSERT"), Ref::keyword("ROW"),])
        ])
        .to_matchable()
    }
}

pub struct DeleteStatementSegment;

impl NodeTrait for DeleteStatementSegment {
    const TYPE: &'static str = "delete_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            // The DELETE keyword
            Ref::keyword("DELETE"),
            // The optional FROM keyword
            Ref::keyword("FROM").optional(),
            // Table reference
            Ref::new("TableReferenceSegment"),
            // Optional alias expression
            Ref::new("AliasExpressionSegment").optional(),
            // Optional WHERE clause
            Ref::new("WhereClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct ExportStatementSegment;

impl NodeTrait for ExportStatementSegment {
    const TYPE: &'static str = "export_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("EXPORT"),
            Ref::keyword("DATA"),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("CONNECTION"),
                Ref::new("ObjectReferenceSegment")
            ])
            .config(|this| this.optional()),
            Ref::keyword("OPTIONS"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        StringParser::new(
                            "compression",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        ),
                        StringParser::new(
                            "field_delimiter",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        ),
                        StringParser::new(
                            "format",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        ),
                        StringParser::new(
                            "uri",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        )
                    ]),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        StringParser::new(
                            "header",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        ),
                        StringParser::new(
                            "overwrite",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        ),
                        StringParser::new(
                            "use_avro_logical_types",
                            |segment: &dyn Segment| {
                                SymbolSegment::create(
                                    &segment.raw(),
                                    segment.get_position_marker(),
                                    SymbolSegmentNewArgs { r#type: "export_option" },
                                )
                            },
                            None,
                            false,
                            None,
                        )
                    ]),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("TRUE"), Ref::keyword("FALSE"),])
                ]),
            ])]),
            Ref::keyword("AS"),
            Ref::new("SelectableGrammar")
        ])
        .to_matchable()
    }
}

pub struct ProcedureNameSegment;

impl NodeTrait for ProcedureNameSegment {
    const TYPE: &'static str = "procedure_name";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            // Project name, schema identifier, etc.
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("DotSegment"),
            ])]),
            // Base procedure name
            one_of(vec_of_erased![
                Ref::new("ProcedureNameIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
            ])
        ])
        .config(|this| this.allow_gaps = false)
        .to_matchable()
    }
}

pub struct ProcedureParameterListSegment;

impl NodeTrait for ProcedureParameterListSegment {
    const TYPE: &'static str = "procedure_parameter_list";

    fn match_grammar() -> Arc<dyn Matchable> {
        Bracketed::new(vec_of_erased![
            Delimited::new(vec_of_erased![Ref::new("ProcedureParameterGrammar")])
                .config(|this| this.optional())
        ])
        .to_matchable()
    }
}

pub struct ProcedureStatements;

impl NodeTrait for ProcedureStatements {
    const TYPE: &'static str = "procedure_statements";

    fn match_grammar() -> Arc<dyn Matchable> {
        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
            Ref::new("StatementSegment"),
            Ref::new("DelimiterGrammar")
        ])])
        .config(|this| {
            this.terminators = vec_of_erased![Ref::keyword("END")];
            this.parse_mode = ParseMode::Greedy;
        })
        .to_matchable()
    }
}

pub struct CreateProcedureStatementSegment;

impl NodeTrait for CreateProcedureStatementSegment {
    const TYPE: &'static str = "create_procedure_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::keyword("PROCEDURE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ProcedureNameSegment"),
            Ref::new("ProcedureParameterListSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("OPTIONS"),
                Ref::keyword("strict_mode"),
                StringParser::new(
                    "strict_mode",
                    |segment| {
                        SymbolSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            SymbolSegmentNewArgs { r#type: "procedure_option" },
                        )
                    },
                    "procedure_option".to_owned().into(),
                    false,
                    None,
                ),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar").optional(),
            ])
            .config(|this| this.optional()),
            Ref::keyword("BEGIN"),
            MetaSegment::indent(),
            Ref::new("ProcedureStatements"),
            MetaSegment::dedent(),
            Ref::keyword("END")
        ])
        .to_matchable()
    }
}

pub struct CallStatementSegment;

impl NodeTrait for CallStatementSegment {
    const TYPE: &'static str = "call_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CALL"),
            Ref::new("ProcedureNameSegment"),
            Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
                    .config(|this| this.optional())
            ])
        ])
        .to_matchable()
    }
}

pub struct ReturnStatementSegment;

impl NodeTrait for ReturnStatementSegment {
    const TYPE: &'static str = "return_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("RETURN")]).to_matchable()
    }
}

pub struct BreakStatementSegment;

impl NodeTrait for BreakStatementSegment {
    const TYPE: &'static str = "break_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("BREAK")]).to_matchable()
    }
}

pub struct LeaveStatementSegment;

impl NodeTrait for LeaveStatementSegment {
    const TYPE: &'static str = "leave_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("LEAVE")]).to_matchable()
    }
}

pub struct ContinueStatementSegment;

impl NodeTrait for ContinueStatementSegment {
    const TYPE: &'static str = "continue_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![Ref::keyword("CONTINUE"), Ref::keyword("ITERATE")]).to_matchable()
    }
}

pub struct RaiseStatementSegment;

impl NodeTrait for RaiseStatementSegment {
    const TYPE: &'static str = "raise_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("RAISE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::keyword("MESSAGE"),
                Ref::new("EqualsSegment"),
                Ref::new("ExpressionSegment").optional(),
            ])
            .config(|this| this.optional())
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
                    Value::Map([("dialect".into(), Value::String("bigquery".into()))].into()),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );

        let files =
            glob::glob("test/fixtures/dialects/bigquery/*.sql").unwrap().flatten().collect_vec();

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
