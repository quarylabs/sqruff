use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{
    AnyNumberOf, any_set_of, one_of, optionally_bracketed,
};
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use super::ansi::{self, raw_dialect};
use crate::clickhouse_keywords::UNRESERVED_KEYWORDS;

pub fn dialect() -> Dialect {
    let ansi_dialect = raw_dialect();

    let mut clickhouse_dialect = raw_dialect();
    clickhouse_dialect.name = DialectKind::Clickhouse;
    clickhouse_dialect
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);

    clickhouse_dialect.sets_mut("datetime_units").clear();
    clickhouse_dialect.sets_mut("datetime_units").extend([
        // https://github.com/ClickHouse/ClickHouse/blob/1cdccd527f0cbf5629b21d29970e28d5156003dc/src/Parsers/parseIntervalKind.cpp#L8
        "NANOSECOND",
        "NANOSECONDS",
        "SQL_TSI_NANOSECOND",
        "NS",
        "MICROSECOND",
        "MICROSECONDS",
        "SQL_TSI_MICROSECOND",
        "MCS",
        "MILLISECOND",
        "MILLISECONDS",
        "SQL_TSI_MILLISECOND",
        "MS",
        "SECOND",
        "SECONDS",
        "SQL_TSI_SECOND",
        "SS",
        "S",
        "MINUTE",
        "MINUTES",
        "SQL_TSI_MINUTE",
        "MI",
        "N",
        "HOUR",
        "HOURS",
        "SQL_TSI_HOUR",
        "HH",
        "H",
        "DAY",
        "DAYS",
        "SQL_TSI_DAY",
        "DD",
        "D",
        "WEEK",
        "WEEKS",
        "SQL_TSI_WEEK",
        "WK",
        "WW",
        "MONTH",
        "MONTHS",
        "SQL_TSI_MONTH",
        "MM",
        "M",
        "QUARTER",
        "QUARTERS",
        "SQL_TSI_QUARTER",
        "QQ",
        "Q",
        "YEAR",
        "YEARS",
        "SQL_TSI_YEAR",
        "YYYY",
        "YY",
    ]);

    clickhouse_dialect.add([
        (
            "SelectClauseTerminatorGrammar".into(),
            clickhouse_dialect
                .grammar("SelectClauseTerminatorGrammar")
                .copy(
                    Some(vec_of_erased![
                        Ref::keyword("PREWHERE"),
                        Ref::keyword("INTO"),
                        Ref::keyword("FORMAT"),
                    ]),
                    None,
                    Some(Ref::keyword("WHERE").to_matchable()),
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            clickhouse_dialect
                .grammar("FromClauseTerminatorGrammar")
                .copy(
                    Some(vec_of_erased![
                        Ref::keyword("PREWHERE"),
                        Ref::keyword("INTO"),
                        Ref::keyword("FORMAT"),
                    ]),
                    None,
                    Some(Ref::keyword("WHERE").to_matchable()),
                    None,
                    Vec::new(),
                    false,
                )
                .copy(
                    Some(vec_of_erased![Ref::new("SettingsClauseSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "DateTimeLiteralGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATE"),
                    Ref::keyword("TIME"),
                    Ref::keyword("TIMESTAMP"),
                ]),
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral,)
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Disambiguate wildcard EXCEPT from set operator EXCEPT.
    // Exclude patterns like `EXCEPT ( ... )` and `EXCEPT identifier` from
    // being parsed as a set operator to allow wildcard `* EXCEPT ...` to bind.
    clickhouse_dialect.replace_grammar(
        "SetOperatorSegment",
        one_of(vec_of_erased![
            Ref::new("UnionGrammar"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("INTERSECT"),
                    Ref::keyword("EXCEPT")
                ]),
                Ref::keyword("ALL").optional(),
            ]),
            Ref::keyword("MINUS"),
        ])
        .config(|config| {
            config.exclude = Some(
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXCEPT"),
                        Bracketed::new(vec_of_erased![Anything::new()]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXCEPT"),
                        Ref::new("SingleIdentifierGrammar"),
                    ]),
                ])
                .to_matchable(),
            );
        })
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "FromExpressionElementSegment",
        Sequence::new(vec_of_erased![
            Ref::new("PreTableFunctionKeywordsGrammar").optional(),
            optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::new("FromClauseTerminatorGrammar"),
                    Ref::new("SamplingExpressionSegment"),
                    Ref::new("JoinLikeClauseGrammar"),
                    Ref::keyword("FINAL"),
                    Ref::new("JoinClauseSegment"),
                ]))
                .optional(),
            Ref::keyword("FINAL").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("OFFSET"),
                Ref::new("AliasExpressionSegment"),
            ])
            .config(|this| this.optional()),
            Ref::new("SamplingExpressionSegment").optional(),
            Ref::new("PostTableExpressionGrammar").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec_of_erased![Sequence::new(vec_of_erased![
            Ref::new("JoinTypeKeywords").optional(),
            Ref::new("JoinKeywordsGrammar"),
            MetaSegment::indent(),
            Ref::new("FromExpressionElementSegment"),
            MetaSegment::dedent(),
            Conditional::new(MetaSegment::indent()).indented_using_on(),
            one_of(vec_of_erased![
                Ref::new("JoinOnConditionSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Conditional::new(MetaSegment::indent()).indented_using_on(),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SingleIdentifierGrammar"
                        )])])
                        .config(|this| this.parse_mode(ParseMode::Greedy)),
                        Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]),
                    ])]),
                    Conditional::new(MetaSegment::dedent()).indented_using_on(),
                ]),
            ])
            .config(|this| this.optional()),
            Conditional::new(MetaSegment::dedent()).indented_using_on(),
        ]),])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "SelectClauseModifierSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("DISTINCT"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    )])])
                ])
                .config(|this| this.optional())
            ]),
            Ref::keyword("ALL")
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "OrderByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ORDER"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment"),
                ]),
                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NULLS"),
                    one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST"),]),
                ])
                .config(|this| this.optional()),
                Ref::new("WithFillSegment").optional(),
            ])])
            .config(|this| this.terminators =
                vec_of_erased![Ref::keyword("LIMIT"), Ref::new("FrameClauseUnitGrammar")]),
            MetaSegment::dedent(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                // Generate the anti template from the set of reserved keywords
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.into_iter().collect::<Vec<_>>().join("|");
                let anti_template = format!("^({pattern})$");
                RegexParser::new("[a-zA-Z_][0-9a-zA-Z_]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
            .into(),
        ),
        (
            "BackQuotedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::BackQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("SingleQuotedIdentifierSegment"),
                Ref::new("BackQuotedIdentifierSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        // A Clickhouse SELECT EXCEPT clause.
        // https://clickhouse.com/docs/en/sql-reference/statements/select#except
        (
            "ExceptClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectExceptClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    one_of(vec_of_erased![
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SingleIdentifierGrammar"
                        )])]),
                        Ref::new("SingleIdentifierGrammar"),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral),
                TypedParser::new(SyntaxKind::DollarQuote, SyntaxKind::QuotedLiteral),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TupleSegment".into(),
            NodeMatcher::new(SyntaxKind::Tuple, |_| {
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "BaseExpressionElementGrammar"
                )])])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.replace_grammar(
        "WildcardExpressionSegment",
        ansi::wildcard_expression_segment().copy(
            Some(vec_of_erased![Ref::new("ExceptClauseSegment").optional()]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    clickhouse_dialect.add([(
        "SettingsClauseSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("SETTINGS"),
            Delimited::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("NakedIdentifierSegment"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"),
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ]),
                ])
                .config(|this| this.optional()),
            ]),
        ])
        .config(|this| this.optional())
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([
        (
            // https://clickhouse.com/docs/interfaces/formats
            "FormatValueSegment".into(),
            RegexParser::new("[a-zA-Z]*", SyntaxKind::Word)
                .to_matchable()
                .into(),
        ),
        (
            "IntoOutfileClauseSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("INTO"),
                Ref::keyword("OUTFILE"),
                Ref::new("QuotedLiteralSegment"),
                Ref::new("FormatClauseSegment").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FormatClauseSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("FORMAT"),
                Ref::new("FormatValueSegment"),
                Ref::new("SettingsClauseSegment").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MergeTreesOrderByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeTreeOrderByClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ORDER"),
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TUPLE"),
                            Bracketed::new(vec_of_erased![]), // tuple() not tuple
                        ]),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("ExpressionSegment"),
                        ])]),
                        Ref::new("ColumnReferenceSegment"),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.insert_lexer_matchers(
        vec![Matcher::string("lambda", "->", SyntaxKind::Lambda)],
        "newline",
    );

    clickhouse_dialect.add(vec![
        (
            "JoinTypeKeywords".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("GLOBAL").optional(),
                one_of(vec_of_erased![
                    // This case INNER [ANY,ALL] JOIN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INNER"),
                        one_of(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("ANY")])
                            .config(|this| this.optional()),
                    ]),
                    // This case [ANY,ALL] INNER JOIN
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("ANY")])
                            .config(|this| this.optional()),
                        Ref::keyword("INNER"),
                    ]),
                    // This case FULL ALL OUTER JOIN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FULL"),
                        Ref::keyword("ALL").optional(),
                        Ref::keyword("OUTER").optional(),
                    ]),
                    // This case ALL FULL OUTER JOIN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALL").optional(),
                        Ref::keyword("FULL"),
                        Ref::keyword("OUTER").optional(),
                    ]),
                    // This case LEFT [OUTER,ANTI,SEMI,ANY,ASOF] JOIN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LEFT"),
                        one_of(vec_of_erased![
                            Ref::keyword("ANTI"),
                            Ref::keyword("SEMI"),
                            one_of(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("ALL")])
                                .config(|this| this.optional()),
                            Ref::keyword("ASOF"),
                        ])
                        .config(|this| this.optional()),
                        Ref::keyword("OUTER").optional(),
                    ]),
                    // This case [ANTI,SEMI,ANY,ASOF] LEFT JOIN
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("ANTI"),
                            Ref::keyword("SEMI"),
                            one_of(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("ALL")])
                                .config(|this| this.optional()),
                            Ref::keyword("ASOF"),
                        ]),
                        Ref::keyword("LEFT"),
                    ]),
                    // This case RIGHT [OUTER,ANTI,SEMI,ANY,ASOF] JOIN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RIGHT"),
                        one_of(vec_of_erased![
                            Ref::keyword("OUTER"),
                            Ref::keyword("ANTI"),
                            Ref::keyword("SEMI"),
                            one_of(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("ALL")])
                                .config(|this| this.optional()),
                        ])
                        .config(|this| this.optional()),
                        Ref::keyword("OUTER").optional(),
                    ]),
                    // This case [OUTER,ANTI,SEMI,ANY] RIGHT JOIN
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("ANTI"),
                            Ref::keyword("SEMI"),
                            one_of(vec_of_erased![Ref::keyword("ANY"), Ref::keyword("ALL")])
                                .config(|this| this.optional()),
                        ]),
                        Ref::keyword("RIGHT"),
                    ]),
                    // This case CROSS JOIN
                    Ref::keyword("CROSS"),
                    // This case PASTE JOIN
                    Ref::keyword("PASTE"),
                    // This case ASOF JOIN
                    Ref::keyword("ASOF"),
                    // This case ANY JOIN
                    Ref::keyword("ANY"),
                    // This case ALL JOIN
                    Ref::keyword("ALL"),
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LambdaFunctionSegment".into(),
            TypedParser::new(SyntaxKind::Lambda, SyntaxKind::Lambda)
                .to_matchable()
                .into(),
        ),
    ]);

    clickhouse_dialect.add(vec![(
        "BinaryOperatorGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("ArithmeticBinaryOperatorGrammar"),
            Ref::new("StringBinaryOperatorGrammar"),
            Ref::new("BooleanBinaryOperatorGrammar"),
            Ref::new("ComparisonOperatorGrammar"),
            // Add Lambda Function
            Ref::new("LambdaFunctionSegment"),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([
        (
            "JoinLikeClauseGrammar".into(),
            Sequence::new(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![Ref::new("ArrayJoinClauseSegment")])
                    .config(|this| this.min_times(1)),
                Ref::new("AliasExpressionSegment").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "InOperatorGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("GLOBAL").optional(),
                Ref::keyword("NOT").optional(),
                Ref::keyword("IN"),
                one_of(vec_of_erased![
                    Ref::new("FunctionSegment"),         // IN tuple(1, 2)
                    Ref::new("ArrayLiteralSegment"),     // IN [1, 2]
                    Ref::new("TupleSegment"),            // IN (1, 2)
                    Ref::new("SingleIdentifierGrammar"), // IN TABLE, IN CTE
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("Expression_A_Grammar")]),
                        Ref::new("SelectableGrammar"),
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy)),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.add([(
        "PreWhereClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::PreWhereClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("PREWHERE"),
                // NOTE: The indent here is implicit to allow
                // constructions like:
                //
                //    PREWHERE a
                //        AND b
                //
                // to be valid without forcing an indent between
                // "PREWHERE" and "a".
                MetaSegment::implicit_indent(),
                optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                MetaSegment::dedent(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // We need to replace the UnorderedSelectStatementSegment to include PREWHERE
    clickhouse_dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        ansi_dialect
            .grammar("UnorderedSelectStatementSegment")
            .match_grammar(&ansi_dialect)
            .unwrap()
            .copy(
                Some(vec_of_erased![Ref::new("PreWhereClauseSegment").optional()]),
                None,
                Some(Ref::new("WhereClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            ),
    );

    clickhouse_dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::select_statement()
            .copy(
                Some(vec_of_erased![Ref::new("PreWhereClauseSegment").optional(),]),
                None,
                Some(Ref::new("WhereClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            )
            .copy(
                Some(vec_of_erased![
                    Ref::new("FormatClauseSegment").optional(),
                    Ref::new("IntoOutfileClauseSegment").optional(),
                    Ref::new("SettingsClauseSegment").optional(),
                ]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
    );

    clickhouse_dialect.add([(
        "WithFillSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("WITH"),
            Ref::keyword("FILL"),
            Sequence::new(vec_of_erased![
                Ref::keyword("FROM"),
                Ref::new("ExpressionSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("TO"),
                Ref::new("ExpressionSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("STEP"),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("IntervalExpressionSegment"),
                ])
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "DatatypeSegment",
        one_of(vec_of_erased![
            // Nullable(Type)
            Sequence::new(vec_of_erased![
                StringParser::new("NULLABLE", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]),
            ]),
            // LowCardinality(Type)
            Sequence::new(vec_of_erased![
                StringParser::new("LOWCARDINALITY", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]),
            ]),
            // DateTime64(precision, 'timezone')
            Sequence::new(vec_of_erased![
                StringParser::new("DATETIME64", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"), // precision
                        Ref::new("QuotedLiteralSegment"),  // timezone
                    ])])
                    .config(|this| {
                        this.optional();
                    }),
                ]),
            ]),
            // DateTime('timezone')
            Sequence::new(vec_of_erased![
                StringParser::new("DATETIME", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                    .config(|this| this.optional()),
            ]),
            // FixedString(length)
            Sequence::new(vec_of_erased![
                StringParser::new("FIXEDSTRING", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
            ]),
            // Array(Type)
            Sequence::new(vec_of_erased![
                StringParser::new("ARRAY", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]),
            ]),
            // Map(KeyType, ValueType)
            Sequence::new(vec_of_erased![
                StringParser::new("MAP", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "DatatypeSegment"
                )])]),
            ]),
            // Tuple(Type1, Type2) or Tuple(name1 Type1, ...)
            Sequence::new(vec_of_erased![
                StringParser::new("TUPLE", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        // Named tuple element: name Type
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("SingleIdentifierGrammar"),
                                Ref::new("QuotedIdentifierSegment"),
                            ]),
                            Ref::new("DatatypeSegment"),
                        ]),
                        // Regular tuple element: just Type
                        Ref::new("DatatypeSegment"),
                    ]
                )]),]),
            ]),
            // Nested(name1 Type1, name2 Type2)
            Sequence::new(vec_of_erased![
                StringParser::new("NESTED", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("DatatypeSegment"),
                    ])
                ]),]),
            ]),
            // JSON data type
            StringParser::new("JSON", SyntaxKind::DataTypeIdentifier),
            // Enum8('val1' = 1, 'val2' = 2)
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("ENUM8", SyntaxKind::DataTypeIdentifier),
                    StringParser::new("ENUM16", SyntaxKind::DataTypeIdentifier),
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("EqualsSegment"),
                        Ref::new("NumericLiteralSegment"),
                    ])
                ]),]),
            ]),
            // double args
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("DECIMAL", SyntaxKind::DataTypeIdentifier),
                    StringParser::new("NUMERIC", SyntaxKind::DataTypeIdentifier),
                ]),
                Ref::new("BracketedArguments").optional(),
            ]),
            // single args
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("DECIMAL32", SyntaxKind::DataTypeIdentifier),
                    StringParser::new("DECIMAL64", SyntaxKind::DataTypeIdentifier),
                    StringParser::new("DECIMAL128", SyntaxKind::DataTypeIdentifier),
                    StringParser::new("DECIMAL256", SyntaxKind::DataTypeIdentifier),
                ]),
                Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment"),]),
            ]),
            Ref::new("TupleTypeSegment"),
            Ref::new("DatatypeIdentifierSegment"),
            Ref::new("NumericLiteralSegment"),
            Sequence::new(vec_of_erased![
                StringParser::new("DATETIME64", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),           // precision
                        Ref::new("QuotedLiteralSegment").optional(), // timezone
                    ])
                    // The brackets might be empty as well
                    .config(|this| {
                        this.optional();
                    }),
                ])
                .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![
                StringParser::new("ARRAY", SyntaxKind::DataTypeIdentifier),
                Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment"),]),
            ])
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "TupleTypeSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("TUPLE"),
            Ref::new("TupleTypeSchemaSegment"), // Tuple() can't be empty
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([(
        "TupleTypeSchemaSegment".into(),
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("DatatypeSegment"),
            ])
        ])])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "BracketedArguments",
        Bracketed::new(vec_of_erased![
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("DatatypeIdentifierSegment"),
                Ref::new("NumericLiteralSegment"),
            ])])
            .config(|this| this.optional())
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "ArrayJoinClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::ArrayJoinClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("LEFT").optional(),
                Ref::keyword("ARRAY"),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")]),
                MetaSegment::dedent(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "CTEDefinitionSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("CTEColumnList").optional(),
                Ref::keyword("AS"),
                Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
                    .config(|this| this.parse_mode(ParseMode::Greedy)),
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("ExpressionSegment"),
                Ref::keyword("AS"),
                Ref::new("SingleIdentifierGrammar"),
            ]),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "AliasExpressionSegment",
        Sequence::new(vec_of_erased![
            MetaSegment::indent(),
            Ref::keyword("AS").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                        .config(|this| this.optional()),
                ]),
                Ref::new("SingleQuotedIdentifierSegment"),
            ])
            .config(|this| this.exclude = one_of(vec_of_erased![
                Ref::keyword("LATERAL"),
                Ref::keyword("WINDOW"),
                Ref::keyword("KEYS"),
            ])
            .to_matchable()
            .into()),
            MetaSegment::dedent(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "TableEngineFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableEngineFunction, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("FunctionNameSegment").exclude(one_of(vec_of_erased![
                        Ref::new("DatePartFunctionNameSegment"),
                        Ref::new("ValuesClauseSegment"),
                    ])),
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionContentsGrammar").optional()
                    ])
                    .config(|this| {
                        this.optional();
                        this.parse_mode(ParseMode::Greedy)
                    }),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OnClusterClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OnClusterClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("CLUSTER"),
                    one_of(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        // Support for placeholders like '{cluster}'
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableEngineSegment".into(),
            NodeMatcher::new(SyntaxKind::Engine, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENGINE"),
                    Ref::new("EqualsSegment").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::new("TableEngineFunctionSegment"),
                        any_set_of(vec_of_erased![
                            Ref::new("MergeTreesOrderByClauseSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITION"),
                                Ref::keyword("BY"),
                                Ref::new("ExpressionSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PRIMARY"),
                                Ref::keyword("KEY"),
                                Ref::new("ExpressionSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SAMPLE"),
                                Ref::keyword("BY"),
                                Ref::new("ExpressionSegment"),
                            ]),
                            Ref::new("SettingsClauseSegment").optional(),
                        ]),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseEngineFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::EngineFunction, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ATOMIC"),
                        Ref::keyword("MYSQL"),
                        Ref::keyword("MATERIALIZEDMYSQL"),
                        Ref::keyword("LAZY"),
                        Ref::keyword("POSTGRESQL"),
                        Ref::keyword("MATERIALIZEDPOSTGRESQL"),
                        Ref::keyword("REPLICATED"),
                        Ref::keyword("SQLITE"),
                    ]),
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionContentsGrammar").optional()
                    ])
                    .config(|this| {
                        this.parse_mode(ParseMode::Greedy);
                        this.optional();
                    }),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseEngineSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseEngine, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENGINE"),
                    Ref::new("EqualsSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("DatabaseEngineFunctionSegment"),
                        any_set_of(vec_of_erased![
                            Ref::new("MergeTreesOrderByClauseSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITION"),
                                Ref::keyword("BY"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PRIMARY"),
                                Ref::keyword("KEY"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SAMPLE"),
                                Ref::keyword("BY"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Ref::new("SettingsClauseSegment").optional(),
                        ]),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnTTLSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnTtlSegment, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("TTL"),
                    Ref::new("ExpressionSegment"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableTTLSegment".into(),
            NodeMatcher::new(SyntaxKind::TableTtlSegment, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("TTL"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        one_of(vec_of_erased![
                            Ref::keyword("DELETE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TO"),
                                Ref::keyword("VOLUME"),
                                Ref::new("QuotedLiteralSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TO"),
                                Ref::keyword("DISK"),
                                Ref::new("QuotedLiteralSegment"),
                            ]),
                        ])
                        .config(|this| this.optional()),
                        Ref::new("WhereClauseSegment").optional(),
                        Ref::new("GroupByClauseSegment").optional(),
                    ])]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
                any_set_of(vec_of_erased![Sequence::new(vec_of_erased![
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
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                        ]),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::keyword("MATERIALIZED"),
                                Ref::keyword("ALIAS"),
                            ]),
                            one_of(vec_of_erased![
                                Ref::new("LiteralGrammar"),
                                Ref::new("FunctionSegment"),
                                Ref::new("BareFunctionSegment"),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("EPHEMERAL"),
                            one_of(vec_of_erased![
                                Ref::new("LiteralGrammar"),
                                Ref::new("FunctionSegment"),
                                Ref::new("BareFunctionSegment"),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        Ref::new("PrimaryKeyGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CODEC"),
                            Ref::new("FunctionContentsGrammar"),
                        ])
                        .config(|this| this.optional()),
                        Ref::new("ColumnTTLSegment"),
                    ]),
                ]),])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.replace_grammar(
        "CreateDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("DATABASE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            any_set_of(vec_of_erased![
                Ref::new("OnClusterClauseSegment").optional(),
                Ref::new("DatabaseEngineSegment").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("SingleIdentifierGrammar"),
                ])
                .config(|this| this.optional()),
            ]),
            AnyNumberOf::new(vec_of_erased![
                Ref::keyword("TABLE"),
                Ref::keyword("OVERRIDE"),
                Ref::new("TableReferenceSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Ref::new("TableConstraintSegment"),
                    Ref::new("ColumnDefinitionSegment"),
                    Ref::new("ColumnConstraintSegment"),
                ])])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable(),
    );

    // https://clickhouse.com/docs/sql-reference/statements/rename
    clickhouse_dialect.add([(
        "RenameStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RenameTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("RENAME"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TABLE"),
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("TableReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("TableReferenceSegment"),
                        ])]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DATABASE"),
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DatabaseReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("DatabaseReferenceSegment"),
                        ])]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DICTIONARY"),
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("ObjectReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("ObjectReferenceSegment"),
                        ])]),
                    ]),
                ]),
                Ref::new("OnClusterClauseSegment").optional(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "CreateTableStatementSegment",
        one_of(vec_of_erased![
            // Regular CREATE TABLE statement
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::new("OrReplaceGrammar").optional(),
                Ref::keyword("TABLE"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("TableReferenceSegment"),
                Ref::new("OnClusterClauseSegment").optional(),
                one_of(vec_of_erased![
                    // CREATE TABLE (...):
                    Sequence::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                            vec_of_erased![
                                Ref::new("TableConstraintSegment"),
                                Ref::new("ColumnDefinitionSegment"),
                                Ref::new("ColumnConstraintSegment"),
                            ]
                        )])])
                        .config(|this| this.optional()),
                        Ref::new("TableEngineSegment"),
                        // CREATE TABLE (...) AS SELECT:
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS"),
                            Ref::new("SelectableGrammar"),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    // CREATE TABLE AS other_table:
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("TableEngineSegment").optional(),
                    ]),
                    // CREATE TABLE AS table_function():
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("FunctionSegment"),
                    ]),
                ]),
                any_set_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COMMENT"),
                        one_of(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("QuotedIdentifierSegment"),
                        ]),
                    ]),
                    Ref::new("TableTTLSegment"),
                ])
                .config(|this| this.optional()),
                Ref::new("TableEndClauseSegment").optional(),
            ]),
            // CREATE TEMPORARY TABLE statement
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("TEMPORARY"),
                Ref::keyword("TABLE"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("TableReferenceSegment"),
                one_of(vec_of_erased![
                    // CREATE TEMPORARY TABLE (...):
                    Sequence::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                            vec_of_erased![
                                Ref::new("TableConstraintSegment"),
                                Ref::new("ColumnDefinitionSegment"),
                                Ref::new("ColumnConstraintSegment"),
                            ]
                        )])])
                        .config(|this| this.optional()),
                        Ref::new("TableEngineSegment"),
                        // CREATE TEMPORARY TABLE (...) AS SELECT:
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS"),
                            Ref::new("SelectableGrammar"),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    // CREATE TEMPORARY TABLE AS other_table:
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("TableEngineSegment").optional(),
                    ]),
                    // CREATE TEMPORARY TABLE AS table_function():
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("FunctionSegment"),
                    ]),
                    // CREATE TEMPORARY TABLE AS SELECT (without column definitions)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("SelectableGrammar"),
                    ]),
                ]),
                any_set_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COMMENT"),
                        one_of(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("QuotedIdentifierSegment"),
                        ]),
                    ]),
                    Ref::new("TableTTLSegment"),
                ])
                .config(|this| this.optional()),
                Ref::new("TableEndClauseSegment").optional(),
            ]),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "CreateViewStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateViewStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::new("OrReplaceGrammar").optional(),
                Ref::keyword("VIEW"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("TableReferenceSegment"),
                Ref::new("OnClusterClauseSegment").optional(),
                Ref::keyword("AS"),
                Ref::new("SelectableGrammar"),
                Ref::new("TableEndClauseSegment").optional()
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "CreateMaterializedViewStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateMaterializedViewStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("MATERIALIZED"),
                Ref::keyword("VIEW"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("TableReferenceSegment"),
                Ref::new("OnClusterClauseSegment").optional(),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TO"),
                        Ref::new("TableReferenceSegment"),
                        // Add support for column list in TO clause
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SingleIdentifierGrammar"
                        ),])])
                        .config(|this| this.optional()),
                        Ref::new("TableEngineSegment").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("TableEngineSegment").optional(),
                        // Add support for PARTITION BY clause
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PARTITION"),
                            Ref::keyword("BY"),
                            Ref::new("ExpressionSegment"),
                        ])
                        .config(|this| this.optional()),
                        // Add support for ORDER BY clause
                        Ref::new("MergeTreesOrderByClauseSegment").optional(),
                        // Add support for TTL clause
                        Ref::new("TableTTLSegment").optional(),
                        // Add support for SETTINGS clause
                        Ref::new("SettingsClauseSegment").optional(),
                        Ref::keyword("POPULATE").optional(),
                    ]),
                ]),
                Ref::keyword("AS"),
                Ref::new("SelectableGrammar"),
                Ref::new("TableEndClauseSegment").optional(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "DropTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("TEMPORARY").optional(),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("OnClusterClauseSegment").optional(),
            Ref::keyword("SYNC").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "DropDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("DATABASE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            Ref::new("OnClusterClauseSegment").optional(),
            Ref::keyword("SYNC").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "DropDictionaryStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropDictionaryStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("DICTIONARY"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("SYNC").optional(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "DropUserStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("USER"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("OnClusterClauseSegment").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "DropRoleStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("ROLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("OnClusterClauseSegment").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "DropQuotaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropQuotaStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("QUOTA"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("OnClusterClauseSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropSettingProfileStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSettingProfileStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Delimited::new(vec_of_erased![Ref::new("NakedIdentifierSegment")])
                        .config(|this| this.min_delimiters = 0),
                    Ref::keyword("PROFILE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("OnClusterClauseSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.replace_grammar(
        "DropViewStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("OnClusterClauseSegment").optional(),
            Ref::keyword("SYNC").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "DropFunctionStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("FUNCTION"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("OnClusterClauseSegment").optional(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "SystemMergesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemMergesSegment, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("MERGES"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ON"),
                            Ref::keyword("VOLUME"),
                            Ref::new("ObjectReferenceSegment"),
                        ]),
                        Ref::new("TableReferenceSegment"),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemTTLMergesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemTtlMergesSegment, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("TTL"),
                    Ref::keyword("MERGES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemMovesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemMovesSegment, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("MOVES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicaSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemReplicaSegment, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SYNC"),
                        Ref::keyword("REPLICA"),
                        Ref::new("OnClusterClauseSegment").optional(),
                        Ref::new("TableReferenceSegment"),
                        Ref::keyword("STRICT").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DROP"),
                        Ref::keyword("REPLICA"),
                        Ref::new("SingleIdentifierGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("DATABASE"),
                                    Ref::new("ObjectReferenceSegment"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("TABLE"),
                                    Ref::new("TableReferenceSegment"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ZKPATH"),
                                    Ref::new("PathSegment"),
                                ]),
                            ]),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESTART"),
                        Ref::keyword("REPLICA"),
                        Ref::new("TableReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESTORE"),
                        Ref::keyword("REPLICA"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("OnClusterClauseSegment").optional(),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemFilesystemSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemFilesystemSegment, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("FILESYSTEM"),
                    Ref::keyword("CACHE"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicatedSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemReplicatedSegment, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("REPLICATED"),
                    Ref::keyword("SENDS"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicationSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemReplicationSegment, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("REPLICATION"),
                    Ref::keyword("QUEUES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemFetchesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemFetchesSegment, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("FETCHES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemDistributedSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemDistributedSegment, |_| {
                Sequence::new(vec_of_erased![one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                        Ref::keyword("DISTRIBUTED"),
                        Ref::keyword("SENDS"),
                        Ref::new("TableReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FLUSH"),
                        Ref::keyword("DISTRIBUTED"),
                        Ref::new("TableReferenceSegment"),
                    ]),
                ]),])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemModelSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemModelSegment, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("RELOAD"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MODELS"),
                            Ref::new("OnClusterClauseSegment").optional(),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MODEL"),
                            any_set_of(vec_of_erased![
                                Ref::new("OnClusterClauseSegment").optional(),
                                Ref::new("PathSegment"),
                            ]),
                        ]),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemFileSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemFileSegment, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SYNC"),
                    Ref::keyword("FILE"),
                    Ref::keyword("CACHE"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemUnfreezeSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemUnfreezeSegment, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNFREEZE"),
                    Ref::keyword("WITH"),
                    Ref::keyword("NAME"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SYSTEM"),
                    one_of(vec_of_erased![
                        Ref::new("SystemMergesSegment"),
                        Ref::new("SystemTTLMergesSegment"),
                        Ref::new("SystemMovesSegment"),
                        Ref::new("SystemReplicaSegment"),
                        Ref::new("SystemReplicatedSegment"),
                        Ref::new("SystemReplicationSegment"),
                        Ref::new("SystemFetchesSegment"),
                        Ref::new("SystemDistributedSegment"),
                        Ref::new("SystemFileSegment"),
                        Ref::new("SystemFilesystemSegment"),
                        Ref::new("SystemUnfreezeSegment"),
                        Ref::new("SystemModelSegment"),
                    ]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // https://clickhouse.com/docs/sql-reference/statements/alter
    clickhouse_dialect.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("OnClusterClauseSegment").optional(),
            one_of(vec_of_erased![
                // ALTER TABLE ... DROP COLUMN [IF EXISTS] name
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                // ALTER TABLE ... ADD COLUMN [IF NOT EXISTS] name [type]
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    one_of(vec_of_erased![
                        // Regular column with type
                        Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MATERIALIZED"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CODEC"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::new("FunctionSegment"),
                                        Ref::new("SingleIdentifierGrammar"),
                                    ])
                                ])]),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        // Alias column with type
                        Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            Ref::keyword("ALIAS"),
                            Ref::new("ExpressionSegment"),
                        ]),
                        // Alias column without type
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ALIAS"),
                            Ref::new("ExpressionSegment"),
                        ]),
                        // Default could also be used without type
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::new("ExpressionSegment"),
                        ]),
                        // Materialized could also be used without type
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MATERIALIZED"),
                            Ref::new("ExpressionSegment"),
                        ]),
                    ])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AFTER"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                        Ref::keyword("FIRST"),
                    ])
                    .config(|this| this.optional()),
                ]),
                // ALTER TABLE ... ADD ALIAS name FOR column_name
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("ALIAS"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("FOR"),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                // ALTER TABLE ... RENAME COLUMN [IF EXISTS] name to new_name
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("TO"),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                // ALTER TABLE ... COMMENT COLUMN [IF EXISTS] name 'Text comment'
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                // ALTER TABLE ... COMMENT 'Text comment'
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                // ALTER TABLE ... MODIFY COMMENT 'Text comment'
                Sequence::new(vec_of_erased![
                    Ref::keyword("MODIFY"),
                    Ref::keyword("COMMENT"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                // ALTER TABLE ... MODIFY COLUMN [IF EXISTS] name [TYPE] [type]
                Sequence::new(vec_of_erased![
                    Ref::keyword("MODIFY"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    one_of(vec_of_erased![
                        // Type modification with explicit TYPE keyword
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TYPE"),
                            Ref::new("DatatypeSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MATERIALIZED"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CODEC"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::new("FunctionSegment"),
                                        Ref::new("SingleIdentifierGrammar"),
                                    ])
                                ])]),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        // Type modification without TYPE keyword
                        Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment").optional(),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MATERIALIZED"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ALIAS"),
                                Ref::new("ExpressionSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CODEC"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::new("FunctionSegment"),
                                        Ref::new("SingleIdentifierGrammar"),
                                    ])
                                ])]),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        // Alias modification
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ALIAS"),
                            Ref::new("ExpressionSegment"),
                        ]),
                        // Remove alias
                        Sequence::new(vec_of_erased![Ref::keyword("REMOVE"), Ref::new("ALIAS"),]),
                        // Remove property
                        Sequence::new(vec_of_erased![
                            Ref::keyword("REMOVE"),
                            one_of(vec_of_erased![
                                Ref::keyword("ALIAS"),
                                Ref::keyword("DEFAULT"),
                                Ref::keyword("MATERIALIZED"),
                                Ref::keyword("CODEC"),
                                Ref::keyword("COMMENT"),
                                Ref::keyword("TTL"),
                            ]),
                        ]),
                        // Modify setting
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MODIFY"),
                            Ref::keyword("SETTING"),
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("EqualsSegment"),
                            Ref::new("LiteralGrammar"),
                        ]),
                        // Reset setting
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESET"),
                            Ref::keyword("SETTING"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                    ])
                    .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AFTER"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                        Ref::keyword("FIRST"),
                    ])
                    .config(|this| this.optional()),
                ]),
                // ALTER TABLE ... ALTER COLUMN name [TYPE] [type]
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("COLUMN"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    one_of(vec_of_erased![
                        // With TYPE keyword
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TYPE"),
                            Ref::new("DatatypeSegment"),
                        ]),
                        Ref::new("DatatypeSegment"),
                    ]),
                    // Without TYPE keyword
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AFTER"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                        Ref::keyword("FIRST"),
                    ])
                    .config(|this| this.optional()),
                ]),
                // ALTER TABLE ... REMOVE TTL
                Sequence::new(vec_of_erased![Ref::keyword("REMOVE"), Ref::keyword("TTL"),]),
                // ALTER TABLE ... MODIFY TTL expression
                Sequence::new(vec_of_erased![
                    Ref::keyword("MODIFY"),
                    Ref::keyword("TTL"),
                    Ref::new("ExpressionSegment"),
                ]),
                // ALTER TABLE ... MODIFY QUERY select_statement
                Sequence::new(vec_of_erased![
                    Ref::keyword("MODIFY"),
                    Ref::keyword("QUERY"),
                    Ref::new("SelectStatementSegment"),
                ]),
                // ALTER TABLE ... MATERIALIZE COLUMN col
                Sequence::new(vec_of_erased![
                    Ref::keyword("MATERIALIZE"),
                    Ref::keyword("COLUMN"),
                    Ref::new("SingleIdentifierGrammar"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("IN"),
                            Ref::keyword("PARTITION"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("IN"),
                            Ref::keyword("PARTITION"),
                            Ref::keyword("ID"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ])
                    .config(|this| this.optional()),
                ]),
                // ALTER TABLE ... DROP PARTITION|PART partition_expr
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    one_of(vec_of_erased![
                        Ref::keyword("PARTITION"),
                        Ref::keyword("PART"),
                    ]),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                // ALTER TABLE ... REPLACE PARTITION partition_expr FROM table1
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Ref::keyword("PARTITION"),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("FROM"),
                    Ref::new("TableReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec_of_erased![
                Ref::new("CreateMaterializedViewStatementSegment"),
                Ref::new("DropDictionaryStatementSegment"),
                Ref::new("DropQuotaStatementSegment"),
                Ref::new("DropSettingProfileStatementSegment"),
                Ref::new("SystemStatementSegment"),
                Ref::new("RenameStatementSegment"),
                Ref::new("AlterTableStatementSegment"),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    clickhouse_dialect.add([(
        "LimitClauseComponentSegment".into(),
        optionally_bracketed(vec_of_erased![one_of(vec_of_erased![
            Ref::new("NumericLiteralSegment"),
            Ref::new("ExpressionSegment"),
        ])])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "LimitClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("LIMIT"),
            MetaSegment::indent(),
            Sequence::new(vec_of_erased![
                Ref::new("LimitClauseComponentSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OFFSET"),
                        Ref::new("LimitClauseComponentSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        // LIMIT 1,2 only accepts constants
                        // and can't be bracketed like that LIMIT (1, 2)
                        // but can be bracketed like that LIMIT (1), (2)
                        Ref::new("CommaSegment"),
                        Ref::new("LimitClauseComponentSegment"),
                    ]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        Ref::new("ColumnReferenceSegment"),
                    ]),
                ])
                .config(|this| this.optional()),
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable(),
    );

    // https://clickhouse.com/docs/sql-reference/data-types/special-data-types/interval
    // https://clickhouse.com/docs/sql-reference/operators#interval
    clickhouse_dialect.replace_grammar(
        "IntervalExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("INTERVAL"),
            one_of(vec_of_erased![
                // The Numeric Version
                Sequence::new(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("DatetimeUnitSegment"),
                ]),
                // The String version
                Ref::new("QuotedLiteralSegment"),
                // Combine version
                Sequence::new(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("DatetimeUnitSegment"),
                ]),
                // With expression as value
                Sequence::new(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Ref::new("DatetimeUnitSegment"),
                ]),
            ]),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "ColumnDefinitionSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("QuotedIdentifierSegment"),
            ]),
            Ref::new("DatatypeSegment"),
            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                // DEFAULT expression
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    one_of(vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Ref::new("FunctionSegment"),
                        Ref::new("ExpressionSegment"),
                    ]),
                ]),
                // ALIAS expression
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALIAS"),
                    Ref::new("ExpressionSegment"),
                ]),
                // MATERIALIZED expression
                Sequence::new(vec_of_erased![
                    Ref::keyword("MATERIALIZED"),
                    Ref::new("ExpressionSegment"),
                ]),
                // CODEC(...)
                Sequence::new(vec_of_erased![
                    Ref::keyword("CODEC"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Ref::new("FunctionSegment"),
                        Ref::new("SingleIdentifierGrammar"),
                    ]),]),
                ]),
                // COMMENT 'text'
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                // Column constraint
                Ref::new("ColumnConstraintSegment"),
            ])])
            .config(|this| this.optional()),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.expand();
    clickhouse_dialect
}
