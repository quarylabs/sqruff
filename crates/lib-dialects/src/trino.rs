use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Nothing, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{StringParser, TypedParser};
use sqruff_lib_core::parser::segments::meta::MetaSegment;

pub fn dialect() -> Dialect {
    let ansi_dialect = super::ansi::raw_dialect();
    let mut trino_dialect = ansi_dialect;
    trino_dialect.name = DialectKind::Trino;

    trino_dialect.sets_mut("bare_functions").extend([
        "current_date",
        "current_time",
        "current_timestamp",
        "localtime",
        "localtimestamp",
    ]);

    trino_dialect.sets_mut("unreserved_keywords").clear();
    trino_dialect.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        super::trino_keywords::TRINO_UNRESERVED_KEYWORDS,
    );

    trino_dialect.sets_mut("reserved_keywords").clear();
    trino_dialect.update_keywords_set_from_multiline_string(
        "reserved_keywords",
        super::trino_keywords::TRINO_RESERVED_KEYWORDS,
    );

    trino_dialect.insert_lexer_matchers(
        // Regexp Replace w/ Lambda: https://trino.io/docs/422/functions/regexp.html
        vec![Matcher::string("right_arrow", "->", SyntaxKind::RightArrow)],
        "like_operator",
    );

    trino_dialect.add([
        (
            "RightArrowOperator".into(),
            StringParser::new("->", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "LambdaArrowSegment".into(),
            StringParser::new("->", SyntaxKind::Symbol)
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
            "FormatJsonEncodingGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("FORMAT").to_matchable(),
                Ref::keyword("JSON").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ENCODING").to_matchable(),
                    one_of(vec![
                        Ref::keyword("UTF8").to_matchable(),
                        Ref::keyword("UTF16").to_matchable(),
                        Ref::keyword("UTF32").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    trino_dialect.update_bracket_sets(
        "angle_bracket_pairs",
        vec![(
            "angle",
            "StartAngleBracketSegment",
            "EndAngleBracketSegment",
            false,
        )],
    );

    trino_dialect.add([
        (
            "DateTimeLiteralGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DATE").to_matchable(),
                        Ref::keyword("TIME").to_matchable(),
                        Ref::keyword("TIMESTAMP").to_matchable(),
                    ])
                    .to_matchable(),
                    TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("IntervalExpressionSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LikeGrammar".into(),
            Sequence::new(vec![Ref::keyword("LIKE").to_matchable()])
                .to_matchable()
                .into(),
        ),
        (
            "MLTableExpressionSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("WHERE").to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("GROUP").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::new("SetOperatorSegment").to_matchable(),
                Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
                Ref::new("WithDataClauseSegment").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrderByClauseTerminators".into(),
            one_of(vec![
                Ref::keyword("LIMIT").to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::new("FrameClauseUnitGrammar").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("FROM").to_matchable(),
                Ref::keyword("WHERE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Ref::new("SetOperatorSegment").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec![
                Ref::keyword("LIMIT").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("GROUP").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseTerminatorGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Ref::keyword("HAVING").to_matchable(),
                Ref::keyword("WINDOW").to_matchable(),
                Ref::keyword("FETCH").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_A_Unary_Operator_Grammar".into(),
            one_of(vec![
                Ref::new("SignedSegmentGrammar")
                    .exclude(Sequence::new(vec![
                        Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
                    ]))
                    .to_matchable(),
                Ref::new("TildeSegment").to_matchable(),
                Ref::new("NotOperatorGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionContentsGrammar".into(),
            AnyNumberOf::new(vec![
                Ref::new("ExpressionSegment").to_matchable(),
                // A Cast-like function
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                ])
                .to_matchable(),
                // A Trim function
                Sequence::new(vec![
                    Ref::new("TrimParametersGrammar").to_matchable(),
                    Ref::new("ExpressionSegment")
                        .optional()
                        .exclude(Ref::keyword("FROM"))
                        .to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                // An extract-like or substring-like function
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("DatetimeUnitSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    // Allow an optional DISTINCT keyword here.
                    Ref::keyword("DISTINCT").optional().to_matchable(),
                    one_of(vec![
                        // Most functions will be using the delimiited route
                        // but for COUNT(*) or similar we allow the star segement
                        // here.
                        Ref::new("StarSegment").to_matchable(),
                        Delimited::new(vec![
                            Ref::new("FunctionContentsExpressionGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("OrderByClauseSegment").to_matchable(),
                // # used by string_agg (postgres), group_concat (exasol),listagg (snowflake)
                // # like a function call: POSITION ( 'QL' IN 'SQL')
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("IN").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // For JSON_QUERY function
                // https://trino.io/docs/current/functions/json.html#json_query
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::new("FormatJsonEncodingGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("WITHOUT").to_matchable(),
                            Ref::keyword("ARRAY").optional().to_matchable(),
                            Ref::keyword("WRAPPER").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            one_of(vec![
                                Ref::keyword("CONDITIONAL").to_matchable(),
                                Ref::keyword("UNCONDITIONAL").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Ref::keyword("ARRAY").optional().to_matchable(),
                            Ref::keyword("WRAPPER").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("IgnoreRespectNullsGrammar").to_matchable(),
                Ref::new("IndexColumnDefinitionSegment").to_matchable(),
                Ref::new("EmptyStructLiteralSegment").to_matchable(),
                Ref::new("ListaggOverflowClauseSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FormatJsonEncodingGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("FORMAT").to_matchable(),
                Ref::keyword("JSON").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ENCODING").to_matchable(),
                    one_of(vec![
                        Ref::keyword("UTF8").to_matchable(),
                        Ref::keyword("UTF16").to_matchable(),
                        Ref::keyword("UTF32").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);
    trino_dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "ArrayTypeSegment",
        Sequence::new(vec![
            Ref::keyword("ARRAY").to_matchable(),
            Ref::new("ArrayTypeSchemaSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    trino_dialect.add([(
        "ArrayTypeSchemaSegment".into(),
        one_of(vec![
            Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()])
                .config(|config| {
                    config.bracket_pairs_set = "angle_bracket_pairs";
                    config.bracket_type = "angle";
                })
                .to_matchable(),
            Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()])
                .config(|config| {
                    config.bracket_type = "round";
                })
                .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    trino_dialect.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("GROUP").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            one_of(vec![
                Ref::keyword("ALL").to_matchable(),
                Ref::new("CubeRollupClauseSegment").to_matchable(),
                // Add GROUPING SETS support
                Ref::new("GroupingSetsClauseSegment").to_matchable(),
                Sequence::new(vec![
                    Delimited::new(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        // Can `GROUP BY 1`
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        // Can `GROUP BY coalesce(col, 1)`
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.terminators =
                            vec![Ref::new("GroupByClauseTerminatorGrammar").to_matchable()]
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    trino_dialect.add([
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec![
                Ref::new("LambdaExpressionSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_| {
                one_of(vec![
                    Ref::keyword("BOOLEAN").to_matchable(),
                    Ref::keyword("TINYINT").to_matchable(),
                    Ref::keyword("SMALLINT").to_matchable(),
                    Ref::keyword("INTEGER").to_matchable(),
                    Ref::keyword("INT").to_matchable(),
                    Ref::keyword("BIGINT").to_matchable(),
                    Ref::keyword("REAL").to_matchable(),
                    Ref::keyword("DOUBLE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DECIMAL").to_matchable(),
                        Ref::new("BracketedArguments").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("CHAR").to_matchable(),
                            Ref::keyword("VARCHAR").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("BracketedArguments").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("VARBINARY").to_matchable(),
                    Ref::keyword("JSON").to_matchable(),
                    Ref::keyword("DATE").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("TIME").to_matchable(),
                            Ref::keyword("TIMESTAMP").to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("WITH").to_matchable(),
                                Ref::keyword("WITHOUT").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("TIME").to_matchable(),
                            Ref::keyword("ZONE").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // Structural
                    Ref::new("ArrayTypeSegment").to_matchable(),
                    Ref::keyword("MAP").to_matchable(),
                    Ref::new("RowTypeSegment").to_matchable(),
                    // Others
                    Ref::keyword("IPADDRESS").to_matchable(),
                    Ref::keyword("UUID").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // Expression to construct a ROW datatype.
            "RowTypeSegment".into(),
            Sequence::new(vec![
                Ref::keyword("ROW").to_matchable(),
                Ref::new("RowTypeSchemaSegment").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec![
                Ref::new("ArrayAccessorSegment").to_matchable(),
                // Add in semi structured expressions
                Ref::new("SemiStructuredAccessorSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A semi-structured data accessor segment.
            "SemiStructuredAccessorSegment".into(),
            Sequence::new(vec![
                Ref::new("DotSegment").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("ArrayAccessorSegment").optional().to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("DotSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|config| {
                        config.allow_gaps = true;
                    })
                    .to_matchable(),
                    Ref::new("ArrayAccessorSegment").optional().to_matchable(),
                ])
                .config(|config| {
                    config.allow_gaps = true;
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // Expression to construct the schema of a ROW datatype.
            "RowTypeSchemaSegment".into(),
            Bracketed::new(vec![
                Delimited::new(vec![
                    // Comma-separated list of field names/types
                    Sequence::new(vec![
                        one_of(vec![
                            // ParameterNames can look like Datatypes so can't use
                            // Optional=True here and instead do a OneOf in order
                            // with DataType only first, followed by both.
                            Ref::new("DatatypeSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("DatatypeSegment").to_matchable(),
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
            .into(),
        ),
        (
            "OverlapsClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OverlapsClause, |_| {
                Nothing::new().to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    trino_dialect.replace_grammar(
        "ValuesClauseSegment",
        Sequence::new(vec![
            Ref::keyword("VALUES").to_matchable(),
            Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()]).to_matchable(),
        ])
        .to_matchable(),
    );

    trino_dialect.replace_grammar(
        "StatementSegment",
        super::ansi::statement_segment().copy(
            Some(vec![
                Ref::new("AnalyzeStatementSegment").to_matchable(),
                Ref::new("CommentOnStatementSegment").to_matchable(),
            ]),
            None,
            None,
            Some(vec![Ref::new("TransactionStatementSegment").to_matchable()]),
            Vec::new(),
            false,
        ),
    );

    trino_dialect.add([
        // An 'ANALYZE' statement.
        // As per docs https://trino.io/docs/current/sql/analyze.html
        (
            "AnalyzeStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("ANALYZE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("ParameterNameSegment").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            super::ansi::raw_dialect()
                .grammar("PostFunctionGrammar")
                .copy(
                    Some(vec![
                        Ref::new("WithinGroupClauseSegment").to_matchable(),
                        Ref::new("WithOrdinalityClauseSegment").to_matchable(),
                    ]),
                    None,
                    None,
                    Some(vec![Ref::new("TransactionStatementSegment").to_matchable()]),
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            // ON OVERFLOW clause of listagg function.
            // https://trino.io/docs/current/functions/aggregate.html#array_agg
            "ListaggOverflowClauseSegment".into(),
            Sequence::new(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("OVERFLOW").to_matchable(),
                one_of(vec![
                    Ref::keyword("ERROR").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TRUNCATE").to_matchable(),
                        Ref::new("SingleQuotedIdentifierSegment")
                            .optional()
                            .to_matchable(),
                        one_of(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Ref::keyword("WITHOUT").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                        Ref::keyword("COUNT").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // An WITHIN GROUP clause for window functions.
            // https://trino.io/docs/current/functions/aggregate.html#array_agg
            // Trino supports an optional FILTER during aggregation that comes
            // immediately after the WITHIN GROUP clause.
            // https://trino.io/docs/current/functions/aggregate.html#filtering-during-aggregation
            "WithinGroupClauseSegment".into(),
            Sequence::new(vec![
                Ref::keyword("WITHIN").to_matchable(),
                Ref::keyword("GROUP").to_matchable(),
                Bracketed::new(vec![Ref::new("OrderByClauseSegment").to_matchable()])
                    .to_matchable(),
                Ref::new("FilterClauseGrammar").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A WITH ORDINALITY clause for CROSS JOIN UNNEST(...).
            // https://trino.io/docs/current/sql/select.html#unnest
            // Trino supports an optional WITH ORDINALITY clause on UNNEST, which
            // adds a numerical ordinality column to the UNNEST result.
            "WithOrdinalityClauseSegment".into(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("ORDINALITY").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // `COMMENT ON` statement.
            // https://trino.io/docs/current/sql/comment.html
            "CommentOnStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("COMMENT").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("TABLE").to_matchable(),
                                // TODO: Create a ViewReferenceSegment
                                Ref::keyword("VIEW").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("COLUMN").to_matchable(),
                            // TODO: Does this correctly emit a Table Reference?
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("IS").to_matchable(),
                        one_of(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IntervalExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::IntervalExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("INTERVAL").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("YEAR").to_matchable(),
                        Ref::keyword("MONTH").to_matchable(),
                        Ref::keyword("DAY").to_matchable(),
                        Ref::keyword("HOUR").to_matchable(),
                        Ref::keyword("MINUTE").to_matchable(),
                        Ref::keyword("SECOND").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FrameClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FrameClause, |_| {
                let frame_extent = one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CURRENT").to_matchable(),
                        Ref::keyword("ROW").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("DateTimeLiteralGrammar").to_matchable(),
                            Ref::keyword("UNBOUNDED").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("PRECEDING").to_matchable(),
                            Ref::keyword("FOLLOWING").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ]);

                Sequence::new(vec![
                    Ref::new("FrameClauseUnitGrammar").to_matchable(),
                    one_of(vec![
                        frame_extent.clone().to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BETWEEN").to_matchable(),
                            frame_extent.clone().to_matchable(),
                            Ref::keyword("AND").to_matchable(),
                            frame_extent.to_matchable(),
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
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("UNION").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DISTINCT").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("INTERSECT").to_matchable(),
                            Ref::keyword("EXCEPT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("ALL").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.exclude = Sequence::new(vec![
                        Ref::keyword("EXCEPT").to_matchable(),
                        Bracketed::new(vec![Anything::new().to_matchable()]).to_matchable(),
                    ])
                    .to_matchable()
                    .into();
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LambdaExpressionSegment".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("ParameterNameSegment").to_matchable(),
                    Bracketed::new(vec![Ref::new("ParameterNameSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("LambdaArrowSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    trino_dialect.config(|dialect| dialect.expand())
}
