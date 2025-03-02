use itertools::Itertools;
use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::base::{Anything, Nothing, Ref};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use super::ansi::{self, raw_dialect};
use super::bigquery_keywords::{BIGQUERY_RESERVED_KEYWORDS, BIGQUERY_UNRESERVED_KEYWORDS};

pub fn dialect() -> Dialect {
    let mut dialect = raw_dialect();
    dialect.name = DialectKind::Bigquery;

    dialect.insert_lexer_matchers(
        vec![
            Matcher::string("right_arrow", "=>", SyntaxKind::RightArrow),
            Matcher::string("question_mark", "?", SyntaxKind::QuestionMark),
            Matcher::regex(
                "at_sign_literal",
                r"@[a-zA-Z_][\w]*",
                SyntaxKind::AtSignLiteral,
            ),
        ],
        "equals",
    );

    dialect.patch_lexer_matchers(vec![
        Matcher::legacy(
            "single_quote",
            |s| s.starts_with(['\'', 'R', 'r', 'B', 'b'].as_ref()),
            r"([rR]?[bB]?|[bB]?[rR]?)?('''((?<!\\)(\\{2})*\\'|'{,2}(?!')|[^'])*(?<!\\)(\\{2})*'''|'((?<!\\)(\\{2})*\\'|[^'])*(?<!\\)(\\{2})*')",
            SyntaxKind::SingleQuote
        ),
        Matcher::legacy(
            "double_quote",
            |s| s.starts_with(['"', 'R', 'r', 'B', 'b']),
            r#"([rR]?[bB]?|[bB]?[rR]?)?(\"\"\"((?<!\\)(\\{2})*\\\"|\"{,2}(?!\")|[^\"])*(?<!\\)(\\{2})*\"\"\"|"((?<!\\)(\\{2})*\\"|[^"])*(?<!\\)(\\{2})*")"#,
            SyntaxKind::DoubleQuote
        ),
    ]);

    dialect.add([
        (
            "DoubleQuotedLiteralSegment".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "SingleQuotedLiteralSegment".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "DoubleQuotedUDFBody".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::UdfBody)
                .to_matchable()
                .into(),
        ),
        (
            "SingleQuotedUDFBody".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::UdfBody)
                .to_matchable()
                .into(),
        ),
        (
            "StartAngleBracketSegment".into(),
            StringParser::new("<", SyntaxKind::StartAngleBracket)
                .to_matchable()
                .into(),
        ),
        (
            "EndAngleBracketSegment".into(),
            StringParser::new(">", SyntaxKind::EndAngleBracket)
                .to_matchable()
                .into(),
        ),
        (
            "RightArrowSegment".into(),
            StringParser::new("=>", SyntaxKind::RightArrow)
                .to_matchable()
                .into(),
        ),
        (
            "DashSegment".into(),
            StringParser::new("-", SyntaxKind::Dash)
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
            StringParser::new("?", SyntaxKind::QuestionMark)
                .to_matchable()
                .into(),
        ),
        (
            "AtSignLiteralSegment".into(),
            TypedParser::new(SyntaxKind::AtSignLiteral, SyntaxKind::AtSignLiteral)
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
                MultiStringParser::new(
                    dialect
                        .sets("extended_datetime_units")
                        .into_iter()
                        .map_into()
                        .collect_vec(),
                    SyntaxKind::DatePart,
                )
                .to_matchable()
            })
            .into(),
        ),
        (
            "NakedIdentifierFullSegment".into(),
            RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::NakedIdentifierAll)
                .to_matchable()
                .into(),
        ),
        (
            "NakedIdentifierPart".into(),
            RegexParser::new("[A-Z0-9_]+", SyntaxKind::NakedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "ProcedureNameIdentifierSegment".into(),
            one_of(vec_of_erased![
                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::ProcedureNameIdentifier)
                    .anti_template("STRUCT"),
                RegexParser::new("`[^`]*`", SyntaxKind::ProcedureNameIdentifier),
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

                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
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
                        one_of(vec_of_erased![
                            Ref::keyword("IGNORE"),
                            Ref::keyword("RESPECT")
                        ]),
                        Ref::keyword("NULLS")
                    ])
                    .config(|this| this.optional())
                ]),
                Ref::new("NamedArgumentSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TrimParametersGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "ParameterNameSegment".into(),
            one_of(vec_of_erased![
                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::Parameter),
                RegexParser::new("`[^`]*`", SyntaxKind::Parameter)
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
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
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
        (
            "NaturalJoinKeywordsGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
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
            Sequence::new(vec_of_erased![
                Ref::keyword("MERGE"),
                Ref::keyword("INTO").optional()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PrimaryKeyGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "ForeignKeyGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
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
    dialect
        .sets_mut("extended_datetime_units")
        .extend(["DATE", "DATETIME", "TIME"]);

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
        "angle",
        "StartAngleBracketSegment",
        "EndAngleBracketSegment",
        false,
    )]);

    dialect.add([
        (
            "ProcedureParameterListSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ProcedureParameterList,
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("ProcedureParameterGrammar")])
                        .config(|this| this.optional())
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureStatements".into(),
            NodeMatcher::new(
                SyntaxKind::ProcedureStatements,
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("DelimiterGrammar")
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![Ref::keyword("END")];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateProcedureStatement,
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
                        StringParser::new("strict_mode", SyntaxKind::ProcedureOption),
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CallStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CallStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CALL"),
                    Ref::new("ProcedureNameSegment"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
                            .config(|this| this.optional())
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ReturnStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ReturnStatement,
                Sequence::new(vec_of_erased![Ref::keyword("RETURN")]).to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "BreakStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::BreakStatement,
                Sequence::new(vec_of_erased![Ref::keyword("BREAK")]).to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LeaveStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LeaveStatement,
                Sequence::new(vec_of_erased![Ref::keyword("LEAVE")]).to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ContinueStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ContinueStatement,
                one_of(vec_of_erased![
                    Ref::keyword("CONTINUE"),
                    Ref::keyword("ITERATE")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RaiseStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RaiseStatement,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "ArrayTypeSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ARRAY"),
            Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]).config(|this| {
                this.bracket_type = "angle";
                this.bracket_pairs_set = "angle_bracket_pairs";
            })
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "QualifyClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::QualifyClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("QUALIFY"),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("UNION"),
                        one_of(vec_of_erased![
                            Ref::keyword("DISTINCT"),
                            Ref::keyword("ALL")
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INTERSECT"),
                        Ref::keyword("DISTINCT")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXCEPT"),
                        Ref::keyword("DISTINCT")
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar("SetExpressionSegment", {
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
    });

    dialect.replace_grammar("SelectStatementSegment", {
        ansi::select_statement().copy(
            Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
            None,
            Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        )
    });

    dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec![
                Ref::new("QualifyClauseSegment").optional().to_matchable(),
            ]),
            None,
            Some(Ref::new("OverlapsClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        ),
    );

    dialect.add([(
        "MultiStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::MultiStatementSegment, {
            one_of(vec_of_erased![
                Ref::new("ForInStatementSegment"),
                Ref::new("RepeatStatementSegment"),
                Ref::new("WhileStatementSegment"),
                Ref::new("LoopStatementSegment"),
                Ref::new("IfStatementSegment"),
                Ref::new("CreateProcedureStatementSegment"),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "FileSegment",
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
        .to_matchable(),
    );

    dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
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
        ),
    );

    dialect.add([(
        "AssertStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::AssertStatement,
            Sequence::new(vec_of_erased![
                Ref::keyword("ASSERT"),
                Ref::new("ExpressionSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::new("QuotedLiteralSegment")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ForInStatementsSegment".into(),
        NodeMatcher::new(
            SyntaxKind::ForInStatements,
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("MultiStatementSegment")
                ]),
                Ref::new("DelimiterGrammar")
            ])])
            .config(|this| {
                this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("END"),
                    Ref::keyword("FOR")
                ])];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ForInStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::ForInStatement,
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
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.add([
        (
            "RepeatStatementsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RepeatStatements,
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("MultiStatementSegment")
                    ]),
                    Ref::new("DelimiterGrammar")
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![Ref::keyword("UNTIL")];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RepeatStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RepeatStatement,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "IfStatementsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IfStatements,
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("MultiStatementSegment")
                    ]),
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "IfStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IfStatement,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LoopStatementsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LoopStatements,
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("MultiStatementSegment")
                    ]),
                    Ref::new("DelimiterGrammar")
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("END"),
                        Ref::keyword("LOOP")
                    ])];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "LoopStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::LoopStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("LOOP"),
                    MetaSegment::indent(),
                    Ref::new("LoopStatementsSegment"),
                    MetaSegment::dedent(),
                    Ref::keyword("END"),
                    Ref::keyword("LOOP")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WhileStatementsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WhileStatements,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "WhileStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::WhileStatement,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("DISTINCT"),
                Ref::keyword("ALL")
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    Ref::keyword("STRUCT"),
                    Ref::keyword("VALUE")
                ])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "IntervalExpressionSegment",
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
        .to_matchable(),
    );

    dialect.add([
        (
            "ExtractFunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionName,
                StringParser::new("EXTRACT", SyntaxKind::FunctionNameIdentifier).to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ArrayFunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionName,
                StringParser::new("ARRAY", SyntaxKind::FunctionNameIdentifier).to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatePartWeekSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DatePartWeek,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NormalizeFunctionNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FunctionName,
                one_of(vec_of_erased![
                    StringParser::new("NORMALIZE", SyntaxKind::FunctionNameIdentifier),
                    StringParser::new("NORMALIZE_AND_CASEFOLD", SyntaxKind::FunctionNameIdentifier),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "FunctionNameSegment",
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
        .to_matchable(),
    );

    dialect.replace_grammar(
        "FunctionSegment",
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
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionContentsGrammar").optional()
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                ]),
                Ref::new("ArrayAccessorSegment").optional(),
                Ref::new("SemiStructuredAccessorSegment").optional(),
                Ref::new("PostFunctionGrammar").optional()
            ]),
        ])])
        .config(|this| this.allow_gaps = false)
        .to_matchable(),
    );

    dialect.replace_grammar(
        "FunctionDefinitionGrammar",
        Sequence::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![
            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                Ref::keyword("DETERMINISTIC"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("DETERMINISTIC")
                ])
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
        .to_matchable(),
    );

    dialect.replace_grammar(
        "WildcardExpressionSegment",
        ansi::wildcard_expression_segment().copy(
            Some(vec_of_erased![
                Ref::new("ExceptClauseSegment").optional(),
                Ref::new("ReplaceClauseSegment").optional(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    dialect.add([
        (
            "ExceptClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectExceptClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    )])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ReplaceClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SelectReplaceClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SelectClauseElementSegment"
                    )])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar("DatatypeSegment", {
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
    });

    dialect.replace_grammar(
        "StructTypeSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("STRUCT"),
            Ref::new("StructTypeSchemaSegment").optional(),
        ])
        .to_matchable(),
    );

    dialect.add([(
        "StructTypeSchemaSegment".into(),
        NodeMatcher::new(
            SyntaxKind::StructTypeSchema,
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("DatatypeSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Ref::new("DatatypeSegment"),
                        ]),
                    ]),
                    AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")]),
                    Ref::new("OptionsSegment").optional(),
                ])
            ])])
            .config(|this| {
                this.bracket_type = "angle";
                this.bracket_pairs_set = "angle_bracket_pairs";
            })
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "ArrayExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::new("ArrayFunctionNameSegment"),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "TupleSegment".into(),
            NodeMatcher::new(
                SyntaxKind::Tuple,
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "BaseExpressionElementGrammar"
                )])])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NamedArgumentSegment".into(),
            NodeMatcher::new(
                SyntaxKind::NamedArgument,
                Sequence::new(vec_of_erased![
                    Ref::new("NakedIdentifierSegment"),
                    Ref::new("RightArrowSegment"),
                    Ref::new("ExpressionSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "SemiStructuredAccessorSegment".into(),
        NodeMatcher::new(
            SyntaxKind::SemiStructuredExpression,
            Sequence::new(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        one_of(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("StarSegment")
                        ])
                    ])
                    .config(|this| this.allow_gaps = true),
                    Ref::new("ArrayAccessorSegment").optional()
                ])
                .config(|this| {
                    this.allow_gaps = true;
                    this.min_times = 1;
                })
            ])
            .config(|this| this.allow_gaps = true)
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "ColumnReferenceSegment",
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
        .to_matchable(),
    );

    dialect.replace_grammar("TableReferenceSegment", {
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
    });

    dialect.add([
        (
            "DeclareStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DeclareSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SetStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SetSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "PartitionBySegment".into(),
            NodeMatcher::new(
                SyntaxKind::PartitionBySegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Ref::keyword("BY"),
                    Ref::new("ExpressionSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ClusterBySegment".into(),
            NodeMatcher::new(
                SyntaxKind::ClusterBySegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTER"),
                    Ref::keyword("BY"),
                    Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "OptionsSegment".into(),
            NodeMatcher::new(
                SyntaxKind::OptionsSegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("OPTIONS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ParameterNameSegment"),
                            Ref::new("EqualsSegment"),
                            Ref::new("BaseExpressionElementGrammar")
                        ])
                    ])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "ColumnDefinitionSegment",
        Sequence::new(vec_of_erased![
            Ref::new("SingleIdentifierGrammar"), // Column name
            Ref::new("DatatypeSegment"),         // Column type
            AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")]),
            Ref::new("OptionsSegment").optional()
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "CreateTableStatementSegment",
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
        .to_matchable(),
    );

    dialect.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                // SET OPTIONS
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("OptionsSegment")
                ]),
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
        .to_matchable(),
    );

    dialect.add([(
        "CreateExternalTableStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::CreateExternalTableStatement,
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
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "CreateExternalTableStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::CreateExternalTableStatement,
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
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "CreateViewStatementSegment",
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
        .to_matchable(),
    );

    dialect.add([
        (
            "AlterViewStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AlterViewStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("VIEW"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::keyword("SET"),
                    Ref::new("OptionsSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "CreateMaterializedViewStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateMaterializedViewStatement,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "AlterMaterializedViewStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AlterMaterializedViewSetOptionsStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("MATERIALIZED"),
                    Ref::keyword("VIEW"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::keyword("SET"),
                    Ref::new("OptionsSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropMaterializedViewStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropMaterializedViewStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("MATERIALIZED"),
                    Ref::keyword("VIEW"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ParameterizedExpression,
                one_of(vec_of_erased![
                    Ref::new("AtSignLiteralSegment"),
                    Ref::new("QuestionMarkSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "PivotForClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PivotForClause,
                Sequence::new(vec_of_erased![Ref::new("BaseExpressionElementGrammar")])
                    .config(|this| {
                        this.terminators = vec_of_erased![Ref::keyword("IN")];
                        this.parse_mode(ParseMode::Greedy);
                    })
                    .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "FromPivotExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::FromPivotExpression,
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
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("LiteralGrammar"),
                                Ref::new("AliasExpressionSegment").optional(),
                            ])
                        ])])
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UnpivotAliasExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::AliasExpression,
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::keyword("AS").optional(),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                    MetaSegment::dedent(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "FromUnpivotExpressionSegment".into(),
        NodeMatcher::new(
            SyntaxKind::FromUnpivotExpression,
            Sequence::new(vec_of_erased![
                Ref::keyword("UNPIVOT"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("INCLUDE"),
                        Ref::keyword("EXCLUDE"),
                    ]),
                    Ref::keyword("NULLS"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    // single column unpivot
                    Bracketed::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::keyword("FOR"),
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]),
                                Ref::new("UnpivotAliasExpressionSegment").optional(),
                            ]),
                        ])]),
                    ]),
                    // multi column unpivot
                    Bracketed::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar"),])
                                .config(|this| this.min_delimiters = 1)
                        ]),
                        Ref::keyword("FOR"),
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Bracketed::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![Ref::new(
                                        "SingleIdentifierGrammar"
                                    ),])
                                    .config(|this| this.min_delimiters = 1)
                                ]),
                                Ref::new("UnpivotAliasExpressionSegment").optional(),
                            ]),
                        ])]),
                    ]),
                ]),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.add([
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("INTO").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    Ref::new("SelectableGrammar")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SampleExpression,
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESAMPLE"),
                    Ref::keyword("SYSTEM"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::keyword("PERCENT")
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeMatchSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeMatch,
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("MergeMatchedClauseSegment"),
                    Ref::new("MergeNotMatchedByTargetClauseSegment"),
                    Ref::new("MergeNotMatchedBySourceClauseSegment"),
                ])
                .config(|this| this.min_times = 1)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedByTargetClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::NotMatchedByTargetClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHEN"),
                    Ref::keyword("NOT"),
                    Ref::keyword("MATCHED"),
                    Sequence::new(vec_of_erased![Ref::keyword("BY"), Ref::keyword("TARGET")])
                        .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AND"),
                        Ref::new("ExpressionSegment"),
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("THEN"),
                    MetaSegment::indent(),
                    Ref::new("MergeInsertClauseSegment"),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedBySourceClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeWhenMatchedClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHEN"),
                    Ref::keyword("NOT"),
                    Ref::keyword("MATCHED"),
                    Ref::keyword("BY"),
                    Ref::keyword("SOURCE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AND"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("THEN"),
                    MetaSegment::indent(),
                    one_of(vec_of_erased![
                        Ref::new("MergeUpdateClauseSegment"),
                        Ref::new("MergeDeleteClauseSegment")
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MergeInsertClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MergeInsertClause,
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INSERT"),
                        MetaSegment::indent(),
                        Ref::new("BracketedColumnReferenceListGrammar").optional(),
                        MetaSegment::dedent(),
                        Ref::new("ValuesClauseSegment").optional(),
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("INSERT"), Ref::keyword("ROW"),]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DeleteStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DELETE"),
                    Ref::keyword("FROM").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("AliasExpressionSegment").optional(),
                    Ref::new("WhereClauseSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ExportStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ExportStatement,
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
                                StringParser::new("compression", SyntaxKind::ExportOption),
                                StringParser::new("field_delimiter", SyntaxKind::ExportOption),
                                StringParser::new("format", SyntaxKind::ExportOption),
                                StringParser::new("uri", SyntaxKind::ExportOption),
                            ]),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                StringParser::new("header", SyntaxKind::ExportOption),
                                StringParser::new("overwrite", SyntaxKind::ExportOption),
                                StringParser::new(
                                    "use_avro_logical_types",
                                    SyntaxKind::ExportOption
                                ),
                            ]),
                            Ref::new("EqualsSegment"),
                            one_of(vec_of_erased![Ref::keyword("TRUE"), Ref::keyword("FALSE"),]),
                        ]),
                    ])]),
                    Ref::keyword("AS"),
                    Ref::new("SelectableGrammar")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureNameSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ProcedureName,
                Sequence::new(vec_of_erased![
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("DotSegment"),
                    ])]),
                    one_of(vec_of_erased![
                        Ref::new("ProcedureNameIdentifierSegment"),
                        Ref::new("QuotedIdentifierSegment"),
                    ])
                ])
                .config(|this| this.allow_gaps = false)
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "QuotedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::BackQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "NumericLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
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
                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::FunctionNameIdentifier)
                    .anti_template("^(STRUCT|ARRAY)$"),
                RegexParser::new("`[^`]*`", SyntaxKind::FunctionNameIdentifier),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.expand();
    dialect
}
