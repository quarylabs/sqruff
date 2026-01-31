use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Nothing, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::ansi::{self, raw_dialect};
use super::bigquery_keywords::{BIGQUERY_RESERVED_KEYWORDS, BIGQUERY_UNRESERVED_KEYWORDS};
use sqruff_lib_core::dialects::init::{DialectConfig, NullDialectConfig};
use sqruff_lib_core::value::Value;

/// Configuration for the BigQuery dialect.
pub type BigQueryDialectConfig = NullDialectConfig;

pub fn dialect(config: Option<&Value>) -> Dialect {
    // Parse and validate dialect configuration, falling back to defaults on failure
    let _dialect_config: BigQueryDialectConfig = config
        .map(BigQueryDialectConfig::from_value)
        .unwrap_or_default();
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

    // BigQuery supports CTEs with DML statements (INSERT, UPDATE, DELETE, MERGE)
    // We add these to NonWithSelectableGrammar so WithCompoundStatementSegment can use them
    dialect.add([(
        "NonWithSelectableGrammar".into(),
        one_of(vec![
            Ref::new("SetExpressionSegment").to_matchable(),
            optionally_bracketed(vec![Ref::new("SelectStatementSegment").to_matchable()])
                .to_matchable(),
            Ref::new("NonSetSelectableGrammar").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
            Ref::new("MergeStatementSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

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
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
                Ref::new("NakedIdentifierFullSegment").to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("DEFAULT").to_matchable(),
                one_of(vec![
                    Ref::new("LiteralGrammar").to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectStatementSegment").to_matchable()])
                        .to_matchable(),
                    Ref::new("BareFunctionSegment").to_matchable(),
                    Ref::new("FunctionSegment").to_matchable(),
                    Ref::new("ArrayLiteralSegment").to_matchable(),
                    Ref::new("TupleSegment").to_matchable(),
                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![Ref::new("SemicolonSegment").to_matchable()];
                })
                .to_matchable(),
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
            one_of(vec![
                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::ProcedureNameIdentifier)
                    .anti_template("STRUCT")
                    .to_matchable(),
                RegexParser::new("`[^`]*`", SyntaxKind::ProcedureNameIdentifier).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("IN").to_matchable(),
                        Ref::keyword("OUT").to_matchable(),
                        Ref::keyword("INOUT").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("ParameterNameSegment").optional().to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ANY").to_matchable(),
                            Ref::keyword("TYPE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("ANY").to_matchable(),
                        Ref::keyword("TYPE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                ])
                .to_matchable(),
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
                let anti_template = format!("^({pattern})$");

                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
            .into(),
        ),
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec![
                Ref::new("DatetimeUnitSegment").to_matchable(),
                Ref::new("DatePartWeekSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("IGNORE").to_matchable(),
                            Ref::keyword("RESPECT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("NULLS").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("HAVING").to_matchable(),
                    one_of(vec![
                        Ref::keyword("MIN").to_matchable(),
                        Ref::keyword("MAX").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("NamedArgumentSegment").to_matchable(),
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
            one_of(vec![
                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::Parameter).to_matchable(),
                RegexParser::new("`[^`]*`", SyntaxKind::Parameter).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DateTimeLiteralGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("DATE").to_matchable(),
                    Ref::keyword("DATETIME").to_matchable(),
                    Ref::keyword("TIME").to_matchable(),
                    Ref::keyword("TIMESTAMP").to_matchable(),
                ])
                .to_matchable(),
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "JoinLikeClauseGrammar".into(),
            Sequence::new(vec![
                AnyNumberOf::new(vec![
                    Ref::new("FromPivotExpressionSegment").to_matchable(),
                    Ref::new("FromUnpivotExpressionSegment").to_matchable(),
                ])
                .config(|this| this.min_times = 1)
                .to_matchable(),
                Ref::new("AliasExpressionSegment").optional().to_matchable(),
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
            AnyNumberOf::new(vec![
                Ref::new("ArrayAccessorSegment").to_matchable(),
                Ref::new("SemiStructuredAccessorSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MergeIntoLiteralGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("MERGE").to_matchable(),
                Ref::keyword("INTO").optional().to_matchable(),
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
            NodeMatcher::new(SyntaxKind::ProcedureParameterList, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ProcedureParameterGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureStatements".into(),
            NodeMatcher::new(SyntaxKind::ProcedureStatements, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("StatementSegment").to_matchable(),
                        Ref::new("DelimiterGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![Ref::keyword("END").to_matchable()];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::keyword("PROCEDURE").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("ProcedureNameSegment").to_matchable(),
                    Ref::new("ProcedureParameterListSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OPTIONS").to_matchable(),
                        Ref::keyword("STRICT_MODE").to_matchable(),
                        StringParser::new("strict_mode", SyntaxKind::ProcedureOption)
                            .to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("BooleanLiteralGrammar").optional().to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("BEGIN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("ProcedureStatements").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CallStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CallStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CALL").to_matchable(),
                    Ref::new("ProcedureNameSegment").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .config(|this| this.optional())
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ReturnStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ReturnStatement, |_| {
                Sequence::new(vec![Ref::keyword("RETURN").to_matchable()]).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BreakStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::BreakStatement, |_| {
                Sequence::new(vec![Ref::keyword("BREAK").to_matchable()]).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LeaveStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::LeaveStatement, |_| {
                Sequence::new(vec![Ref::keyword("LEAVE").to_matchable()]).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ContinueStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ContinueStatement, |_| {
                one_of(vec![
                    Ref::keyword("CONTINUE").to_matchable(),
                    Ref::keyword("ITERATE").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RaiseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RaiseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("RAISE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Ref::keyword("MESSAGE").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("ExpressionSegment").optional().to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "ArrayTypeSegment",
        Sequence::new(vec![
            Ref::keyword("ARRAY").to_matchable(),
            Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()])
                .config(|this| {
                    this.bracket_type = "angle";
                    this.bracket_pairs_set = "angle_bracket_pairs";
                })
                .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "QualifyClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::QualifyClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("QUALIFY").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("UNION").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DISTINCT").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("INTERSECT").to_matchable(),
                        Ref::keyword("DISTINCT").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXCEPT").to_matchable(),
                        Ref::keyword("DISTINCT").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar("SetExpressionSegment", {
        Sequence::new(vec![
            one_of(vec![
                Ref::new("NonSetSelectableGrammar").to_matchable(),
                Bracketed::new(vec![Ref::new("SetExpressionSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("SetOperatorSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("NonSetSelectableGrammar").to_matchable(),
                        Bracketed::new(vec![Ref::new("SetExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.min_times = 1)
            .to_matchable(),
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
        ])
        .to_matchable()
    });

    dialect.replace_grammar("SelectStatementSegment", {
        ansi::select_statement().copy(
            Some(vec![
                Ref::new("QualifyClauseSegment").optional().to_matchable(),
            ]),
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
        NodeMatcher::new(SyntaxKind::MultiStatementSegment, |_| {
            one_of(vec![
                Ref::new("ForInStatementSegment").to_matchable(),
                Ref::new("RepeatStatementSegment").to_matchable(),
                Ref::new("WhileStatementSegment").to_matchable(),
                Ref::new("LoopStatementSegment").to_matchable(),
                Ref::new("IfStatementSegment").to_matchable(),
                Ref::new("CreateProcedureStatementSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "FileSegment",
        Sequence::new(vec![
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("MultiStatementSegment").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::new("DelimiterGrammar").to_matchable(),
                one_of(vec![
                    Ref::new("MultiStatementSegment").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("DelimiterGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec![
                Ref::new("DeclareStatementSegment").to_matchable(),
                Ref::new("SetStatementSegment").to_matchable(),
                Ref::new("ExportStatementSegment").to_matchable(),
                Ref::new("CreateExternalTableStatementSegment").to_matchable(),
                Ref::new("AssertStatementSegment").to_matchable(),
                Ref::new("CallStatementSegment").to_matchable(),
                Ref::new("ReturnStatementSegment").to_matchable(),
                Ref::new("BreakStatementSegment").to_matchable(),
                Ref::new("LeaveStatementSegment").to_matchable(),
                Ref::new("ContinueStatementSegment").to_matchable(),
                Ref::new("RaiseStatementSegment").to_matchable(),
                Ref::new("AlterViewStatementSegment").to_matchable(),
                Ref::new("CreateMaterializedViewStatementSegment").to_matchable(),
                Ref::new("AlterMaterializedViewStatementSegment").to_matchable(),
                Ref::new("DropMaterializedViewStatementSegment").to_matchable(),
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
        NodeMatcher::new(SyntaxKind::AssertStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ASSERT").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ForInStatementsSegment".into(),
        NodeMatcher::new(SyntaxKind::ForInStatements, |_| {
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("StatementSegment").to_matchable(),
                        Ref::new("MultiStatementSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DelimiterGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![
                    Sequence::new(vec![
                        Ref::keyword("END").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                    ])
                    .to_matchable(),
                ];
                this.parse_mode = ParseMode::Greedy;
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ForInStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ForInStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::keyword("IN").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("SelectableGrammar").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("DO").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("ForInStatementsSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::keyword("FOR").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([
        (
            "RepeatStatementsSegment".into(),
            NodeMatcher::new(SyntaxKind::RepeatStatements, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("StatementSegment").to_matchable(),
                            Ref::new("MultiStatementSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DelimiterGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![Ref::keyword("UNTIL").to_matchable()];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RepeatStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RepeatStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("REPEAT").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("RepeatStatementsSegment").to_matchable(),
                    Ref::keyword("UNTIL").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("REPEAT").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IfStatementsSegment".into(),
            NodeMatcher::new(SyntaxKind::IfStatements, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("StatementSegment").to_matchable(),
                            Ref::new("MultiStatementSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DelimiterGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        Ref::keyword("ELSE").to_matchable(),
                        Ref::keyword("ELSEIF").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("END").to_matchable(),
                            Ref::keyword("IF").to_matchable(),
                        ])
                        .to_matchable(),
                    ];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IfStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::IfStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("IF").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("IfStatementsSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ELSEIF").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::keyword("THEN").to_matchable(),
                            MetaSegment::indent().to_matchable(),
                            Ref::new("IfStatementsSegment").to_matchable(),
                            MetaSegment::dedent().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ELSE").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("IfStatementsSegment").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("IF").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LoopStatementsSegment".into(),
            NodeMatcher::new(SyntaxKind::LoopStatements, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("StatementSegment").to_matchable(),
                            Ref::new("MultiStatementSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DelimiterGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        Sequence::new(vec![
                            Ref::keyword("END").to_matchable(),
                            Ref::keyword("LOOP").to_matchable(),
                        ])
                        .to_matchable(),
                    ];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LoopStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::LoopStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("LOOP").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("LoopStatementsSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WhileStatementsSegment".into(),
            NodeMatcher::new(SyntaxKind::WhileStatements, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("StatementSegment").to_matchable(),
                        Ref::new("DelimiterGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        Sequence::new(vec![
                            Ref::keyword("END").to_matchable(),
                            Ref::keyword("WHILE").to_matchable(),
                        ])
                        .to_matchable(),
                    ];
                    this.parse_mode = ParseMode::Greedy;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WhileStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WhileStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("WHILE").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("DO").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("WhileStatementsSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("WHILE").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("DISTINCT").to_matchable(),
                Ref::keyword("ALL").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("AS").to_matchable(),
                one_of(vec![
                    Ref::keyword("STRUCT").to_matchable(),
                    Ref::keyword("VALUE").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "IntervalExpressionSegment",
        Sequence::new(vec![
            Ref::keyword("INTERVAL").to_matchable(),
            Ref::new("ExpressionSegment").to_matchable(),
            one_of(vec![
                Ref::new("QuotedLiteralSegment").to_matchable(),
                Ref::new("DatetimeUnitSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("DatetimeUnitSegment").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("DatetimeUnitSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "DateTimeFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("DatetimeUnitSegment").to_matchable(),
                        Ref::new("DatePartWeekSegment").to_matchable(),
                        Ref::new("FunctionContentsGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExtractFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                Bracketed::new(vec![
                    one_of(vec![
                        Ref::new("DatetimeUnitSegment").to_matchable(),
                        Ref::new("DatePartWeekSegment").to_matchable(),
                        Ref::new("ExtendedDatetimeUnitSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NormalizeFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
                Bracketed::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("CommaSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("NFC").to_matchable(),
                            Ref::keyword("NFKC").to_matchable(),
                            Ref::keyword("NFD").to_matchable(),
                            Ref::keyword("NFKD").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExtractFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                StringParser::new("EXTRACT", SyntaxKind::FunctionNameIdentifier).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArrayFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                StringParser::new("ARRAY", SyntaxKind::FunctionNameIdentifier).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatePartWeekSegment".into(),
            NodeMatcher::new(SyntaxKind::DatePartWeek, |_| {
                Sequence::new(vec![
                    Ref::keyword("WEEK").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::keyword("SUNDAY").to_matchable(),
                            Ref::keyword("MONDAY").to_matchable(),
                            Ref::keyword("TUESDAY").to_matchable(),
                            Ref::keyword("WEDNESDAY").to_matchable(),
                            Ref::keyword("THURSDAY").to_matchable(),
                            Ref::keyword("FRIDAY").to_matchable(),
                            Ref::keyword("SATURDAY").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NormalizeFunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                one_of(vec![
                    StringParser::new("NORMALIZE", SyntaxKind::FunctionNameIdentifier)
                        .to_matchable(),
                    StringParser::new("NORMALIZE_AND_CASEFOLD", SyntaxKind::FunctionNameIdentifier)
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "FunctionNameSegment",
        Sequence::new(vec![
            // AnyNumberOf to handle project names, schemas, or the SAFE keyword
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("SAFE").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DotSegment").to_matchable(),
                ])
                .terminators(vec![Ref::new("BracketedSegment").to_matchable()])
                .to_matchable(),
            ])
            .to_matchable(),
            // Base function name
            one_of(vec![
                Ref::new("FunctionNameIdentifierSegment").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
            ])
            .config(|this| this.terminators = vec![Ref::new("BracketedSegment").to_matchable()])
            .to_matchable(),
        ])
        .allow_gaps(true)
        .to_matchable(),
    );

    dialect.replace_grammar(
        "FunctionSegment",
        Sequence::new(vec![
            one_of(vec![
                Sequence::new(vec![
                    // BigQuery EXTRACT allows optional TimeZone
                    Ref::new("ExtractFunctionNameSegment").to_matchable(),
                    Ref::new("ExtractFunctionContentsSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    // BigQuery NORMALIZE allows optional normalization_mode
                    Ref::new("NormalizeFunctionNameSegment").to_matchable(),
                    Ref::new("NormalizeFunctionContentsSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    // Treat functions which take date parts separately
                    Ref::new("DatePartFunctionNameSegment")
                        .exclude(Ref::new("ExtractFunctionNameSegment"))
                        .to_matchable(),
                    Ref::new("DateTimeFunctionContentsSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::new("FunctionNameSegment")
                            .exclude(one_of(vec![
                                Ref::new("DatePartFunctionNameSegment").to_matchable(),
                                Ref::new("NormalizeFunctionNameSegment").to_matchable(),
                                Ref::new("ValuesClauseSegment").to_matchable(),
                            ]))
                            .to_matchable(),
                        Ref::new("FunctionContentsSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ArrayAccessorSegment").optional().to_matchable(),
                    Ref::new("SemiStructuredAccessorSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("PostFunctionGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .config(|this| this.allow_gaps = false)
        .to_matchable(),
    );

    dialect.replace_grammar(
        "FunctionDefinitionGrammar",
        Sequence::new(vec![
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DETERMINISTIC").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("DETERMINISTIC").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LANGUAGE").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OPTIONS").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    Anything::new().to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("CommaSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    one_of(vec![
                        Ref::new("DoubleQuotedUDFBody").to_matchable(),
                        Ref::new("SingleQuotedUDFBody").to_matchable(),
                        Bracketed::new(vec![
                            one_of(vec![
                                Ref::new("ExpressionSegment").to_matchable(),
                                Ref::new("SelectStatementSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "WildcardExpressionSegment",
        ansi::wildcard_expression_segment().copy(
            Some(vec![
                Ref::new("ExceptClauseSegment").optional().to_matchable(),
                Ref::new("ReplaceClauseSegment").optional().to_matchable(),
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
            NodeMatcher::new(SyntaxKind::SelectExceptClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXCEPT").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ReplaceClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectReplaceClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("REPLACE").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("SelectClauseElementSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar("DatatypeSegment", {
        one_of(vec![
            Sequence::new(vec![
                Ref::new("DatatypeIdentifierSegment").to_matchable(),
                Ref::new("BracketedArguments").optional().to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ANY").to_matchable(),
                Ref::keyword("TYPE").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("ArrayTypeSegment").to_matchable(),
            Ref::new("StructTypeSegment").to_matchable(),
        ])
        .to_matchable()
    });

    dialect.replace_grammar(
        "StructTypeSegment",
        Sequence::new(vec![
            Ref::keyword("STRUCT").to_matchable(),
            Ref::new("StructTypeSchemaSegment")
                .optional()
                .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([(
        "StructTypeSchemaSegment".into(),
        NodeMatcher::new(SyntaxKind::StructTypeSchema, |_| {
            Bracketed::new(vec![
                Delimited::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("DatatypeSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("DatatypeSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        AnyNumberOf::new(vec![Ref::new("ColumnConstraintSegment").to_matchable()])
                            .to_matchable(),
                        Ref::new("OptionsSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.bracket_type = "angle";
                this.bracket_pairs_set = "angle_bracket_pairs";
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ArrayFunctionContentsSegment".into(),
        NodeMatcher::new(SyntaxKind::FunctionContents, |_| {
            Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()]).to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "ArrayExpressionSegment",
        Sequence::new(vec![
            Ref::new("ArrayFunctionNameSegment").to_matchable(),
            Ref::new("ArrayFunctionContentsSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "TupleSegment".into(),
            NodeMatcher::new(SyntaxKind::Tuple, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("BaseExpressionElementGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NamedArgumentSegment".into(),
            NodeMatcher::new(SyntaxKind::NamedArgument, |_| {
                Sequence::new(vec![
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::new("RightArrowSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "SemiStructuredAccessorSegment".into(),
        NodeMatcher::new(SyntaxKind::SemiStructuredExpression, |_| {
            Sequence::new(vec![
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("DotSegment").to_matchable(),
                        one_of(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("StarSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.allow_gaps = true)
                    .to_matchable(),
                    Ref::new("ArrayAccessorSegment").optional().to_matchable(),
                ])
                .config(|this| {
                    this.allow_gaps = true;
                    this.min_times = 1;
                })
                .to_matchable(),
            ])
            .config(|this| this.allow_gaps = true)
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "ColumnReferenceSegment",
        Sequence::new(vec![
            Ref::new("SingleIdentifierGrammar").to_matchable(),
            Sequence::new(vec![
                Ref::new("ObjectReferenceDelimiterGrammar").to_matchable(),
                Delimited::new(vec![Ref::new("SingleIdentifierFullGrammar").to_matchable()])
                    .config(|this| {
                        this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
                        this.terminators = vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("AS").to_matchable(),
                            Ref::keyword("USING").to_matchable(),
                            Ref::new("CommaSegment").to_matchable(),
                            Ref::new("CastOperatorSegment").to_matchable(),
                            Ref::new("StartSquareBracketSegment").to_matchable(),
                            Ref::new("StartBracketSegment").to_matchable(),
                            Ref::new("BinaryOperatorGrammar").to_matchable(),
                            Ref::new("ColonSegment").to_matchable(),
                            Ref::new("DelimiterGrammar").to_matchable(),
                            Ref::new("BracketedSegment").to_matchable(),
                        ];
                        this.allow_gaps = false;
                    })
                    .to_matchable(),
            ])
            .allow_gaps(false)
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .allow_gaps(false)
        .to_matchable(),
    );

    dialect.replace_grammar("TableReferenceSegment", {
        Delimited::new(vec![
            Sequence::new(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("DashSegment").to_matchable(),
                        Ref::new("NakedIdentifierPart").to_matchable(),
                    ])
                    .config(|this| this.allow_gaps = false)
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .config(|this| this.allow_gaps = false)
            .to_matchable(),
        ])
        .config(|this| {
            this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
            this.terminators = vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::keyword("USING").to_matchable(),
                Ref::new("CommaSegment").to_matchable(),
                Ref::new("CastOperatorSegment").to_matchable(),
                Ref::new("StartSquareBracketSegment").to_matchable(),
                Ref::new("StartBracketSegment").to_matchable(),
                Ref::new("ColonSegment").to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
                Ref::new("JoinLikeClauseGrammar").to_matchable(),
                Ref::new("BracketedSegment").to_matchable(),
            ];
            this.allow_gaps = false;
        })
        .to_matchable()
    });

    dialect.add([
        (
            "DeclareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Delimited::new(vec![Ref::new("SingleIdentifierFullGrammar").to_matchable()])
                        .to_matchable(),
                    one_of(vec![
                        Ref::new("DefaultDeclareOptionsGrammar").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("DatatypeSegment").to_matchable(),
                            Ref::new("DefaultDeclareOptionsGrammar")
                                .optional()
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    one_of(vec![
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("NakedIdentifierSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("LiteralGrammar").to_matchable(),
                            Bracketed::new(vec![Ref::new("SelectStatementSegment").to_matchable()])
                                .to_matchable(),
                            Ref::new("BareFunctionSegment").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    one_of(vec![
                                        Ref::new("LiteralGrammar").to_matchable(),
                                        Bracketed::new(vec![
                                            Ref::new("SelectStatementSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::new("BareFunctionSegment").to_matchable(),
                                        Ref::new("FunctionSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("ArrayLiteralSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PartitionBySegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionBySegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ClusterBySegment".into(),
            NodeMatcher::new(SyntaxKind::ClusterBySegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("CLUSTER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::OptionsSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("OPTIONS").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("BaseExpressionElementGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "ColumnDefinitionSegment",
        Sequence::new(vec![
            Ref::new("SingleIdentifierGrammar").to_matchable(), // Column name
            Ref::new("DatatypeSegment").to_matchable(),         // Column type
            AnyNumberOf::new(vec![Ref::new("ColumnConstraintSegment").to_matchable()])
                .to_matchable(),
            Ref::new("OptionsSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            Ref::new("TemporaryTransientGrammar")
                .optional()
                .to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("COPY").to_matchable(),
                    Ref::keyword("LIKE").to_matchable(),
                    Ref::keyword("CLONE").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnDefinitionSegment").to_matchable()])
                        .config(|this| this.allow_trailing())
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("PartitionBySegment").optional().to_matchable(),
            Ref::new("ClusterBySegment").optional().to_matchable(),
            Ref::new("OptionsSegment").optional().to_matchable(),
            Sequence::new(vec![
                Ref::keyword("AS").to_matchable(),
                optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                    .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            one_of(vec![
                // SET OPTIONS
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("OptionsSegment").to_matchable(),
                ])
                .to_matchable(),
                // ADD COLUMN
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("ADD").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.allow_trailing = true)
                .to_matchable(),
                // RENAME TO
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                // RENAME COLUMN
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("RENAME").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("IfExistsGrammar").optional().to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.allow_trailing = true)
                .to_matchable(),
                // DROP COLUMN
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("DROP").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("IfExistsGrammar").optional().to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // ALTER COLUMN SET OPTIONS
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("ALTER").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("IfExistsGrammar").optional().to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("SET").to_matchable(),
                                one_of(vec![
                                    Ref::new("OptionsSegment").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("DATA").to_matchable(),
                                        Ref::keyword("TYPE").to_matchable(),
                                        Ref::new("DatatypeSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("DEFAULT").to_matchable(),
                                        one_of(vec![
                                            Ref::new("LiteralGrammar").to_matchable(),
                                            Ref::new("FunctionSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DROP").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("NOT").to_matchable(),
                                        Ref::keyword("NULL").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([(
        "CreateExternalTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OR").optional().to_matchable(),
                    Ref::keyword("REPLACE").optional().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("EXTERNAL").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IF").optional().to_matchable(),
                    Ref::keyword("NOT").optional().to_matchable(),
                    Ref::keyword("EXISTS").optional().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnDefinitionSegment").to_matchable()])
                        .config(|this| this.allow_trailing = true)
                        .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("CONNECTION").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("PARTITION").to_matchable(),
                        Ref::keyword("COLUMNS").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                            ])
                            .config(|this| this.allow_trailing = true)
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("OptionsSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "CreateExternalTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OR").optional().to_matchable(),
                    Ref::keyword("REPLACE").optional().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("EXTERNAL").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IF").optional().to_matchable(),
                    Ref::keyword("NOT").optional().to_matchable(),
                    Ref::keyword("EXISTS").optional().to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnDefinitionSegment").to_matchable()])
                        .config(|this| this.allow_trailing = true)
                        .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("CONNECTION").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("PARTITION").to_matchable(),
                        Ref::keyword("COLUMNS").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                            ])
                            .config(|this| this.allow_trailing = true)
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("OptionsSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.replace_grammar(
        "CreateViewStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            Ref::keyword("VIEW").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("BracketedColumnReferenceListGrammar")
                .optional()
                .to_matchable(),
            Ref::new("OptionsSegment").optional().to_matchable(),
            Ref::keyword("AS").to_matchable(),
            optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()]).to_matchable(),
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "AlterViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterViewStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("OptionsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateMaterializedViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateMaterializedViewStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::keyword("MATERIALIZED").to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("PartitionBySegment").optional().to_matchable(),
                    Ref::new("ClusterBySegment").optional().to_matchable(),
                    Ref::new("OptionsSegment").optional().to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AlterMaterializedViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterMaterializedViewSetOptionsStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("MATERIALIZED").to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("OptionsSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropMaterializedViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropMaterializedViewStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("MATERIALIZED").to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(SyntaxKind::ParameterizedExpression, |_| {
                one_of(vec![
                    Ref::new("AtSignLiteralSegment").to_matchable(),
                    Ref::new("QuestionMarkSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PivotForClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotForClause, |_| {
                Sequence::new(vec![
                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![Ref::keyword("IN").to_matchable()];
                    this.parse_mode(ParseMode::Greedy);
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FromPivotExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::FromPivotExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("PIVOT").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("FunctionSegment").to_matchable(),
                                Ref::new("AliasExpressionSegment").optional().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::new("PivotForClauseSegment").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("LiteralGrammar").to_matchable(),
                                    Ref::new("AliasExpressionSegment").optional().to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UnpivotAliasExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::AliasExpression, |_| {
                Sequence::new(vec![
                    MetaSegment::indent().to_matchable(),
                    Ref::keyword("AS").optional().to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "FromUnpivotExpressionSegment".into(),
        NodeMatcher::new(SyntaxKind::FromUnpivotExpression, |_| {
            Sequence::new(vec![
                Ref::keyword("UNPIVOT").to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("INCLUDE").to_matchable(),
                        Ref::keyword("EXCLUDE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("NULLS").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    // single column unpivot
                    Bracketed::new(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("UnpivotAliasExpressionSegment")
                                        .optional()
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // multi column unpivot
                    Bracketed::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .config(|this| this.min_delimiters = 1)
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Bracketed::new(vec![
                                        Delimited::new(vec![
                                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        ])
                                        .config(|this| this.min_delimiters = 1)
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("UnpivotAliasExpressionSegment")
                                        .optional()
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("INSERT").to_matchable(),
                    Ref::keyword("INTO").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("TABLESAMPLE").to_matchable(),
                    Ref::keyword("SYSTEM").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::keyword("PERCENT").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeMatchSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeMatch, |_| {
                AnyNumberOf::new(vec![
                    Ref::new("MergeMatchedClauseSegment").to_matchable(),
                    Ref::new("MergeNotMatchedByTargetClauseSegment").to_matchable(),
                    Ref::new("MergeNotMatchedBySourceClauseSegment").to_matchable(),
                ])
                .config(|this| this.min_times = 1)
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedByTargetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::NotMatchedByTargetClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WHEN").to_matchable(),
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("MATCHED").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BY").to_matchable(),
                        Ref::keyword("TARGET").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AND").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("MergeInsertClauseSegment").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeNotMatchedBySourceClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeWhenMatchedClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WHEN").to_matchable(),
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("MATCHED").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::keyword("SOURCE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AND").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Ref::new("MergeUpdateClauseSegment").to_matchable(),
                        Ref::new("MergeDeleteClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MergeInsertClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeInsertClause, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("INSERT").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("BracketedColumnReferenceListGrammar")
                            .optional()
                            .to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                        Ref::new("ValuesClauseSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("INSERT").to_matchable(),
                        Ref::keyword("ROW").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DELETE").to_matchable(),
                    Ref::keyword("FROM").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("AliasExpressionSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExportStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ExportStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXPORT").to_matchable(),
                    Ref::keyword("DATA").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("CONNECTION").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("OPTIONS").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    StringParser::new("compression", SyntaxKind::ExportOption)
                                        .to_matchable(),
                                    StringParser::new("field_delimiter", SyntaxKind::ExportOption)
                                        .to_matchable(),
                                    StringParser::new("format", SyntaxKind::ExportOption)
                                        .to_matchable(),
                                    StringParser::new("uri", SyntaxKind::ExportOption)
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    StringParser::new("header", SyntaxKind::ExportOption)
                                        .to_matchable(),
                                    StringParser::new("overwrite", SyntaxKind::ExportOption)
                                        .to_matchable(),
                                    StringParser::new(
                                        "use_avro_logical_types",
                                        SyntaxKind::ExportOption,
                                    )
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("TRUE").to_matchable(),
                                    Ref::keyword("FALSE").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureNameSegment".into(),
            NodeMatcher::new(SyntaxKind::ProcedureName, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("DotSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("ProcedureNameIdentifierSegment").to_matchable(),
                        Ref::new("QuotedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.allow_gaps = false)
                .to_matchable()
            })
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
            one_of(vec![
                TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral)
                    .to_matchable(),
                Ref::new("ParameterizedSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec![
                Ref::new("SingleQuotedLiteralSegment").to_matchable(),
                Ref::new("DoubleQuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LiteralGrammar".into(),
            dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec![Ref::new("ParameterizedSegment").to_matchable()]),
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
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::keyword("SYSTEM_TIME").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SYSTEM").to_matchable(),
                            Ref::keyword("TIME").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("OF").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("OFFSET").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionNameIdentifierSegment".into(),
            one_of(vec![
                RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::FunctionNameIdentifier)
                    .anti_template("^(STRUCT|ARRAY)$")
                    .to_matchable(),
                RegexParser::new("`[^`]*`", SyntaxKind::FunctionNameIdentifier).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.expand();
    dialect
}
