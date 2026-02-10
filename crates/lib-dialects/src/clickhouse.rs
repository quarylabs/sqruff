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

use super::ansi::{self, raw_dialect};
use crate::clickhouse_keywords::UNRESERVED_KEYWORDS;
use sqruff_lib_core::dialects::init::{DialectConfig, NullDialectConfig};
use sqruff_lib_core::value::Value;

/// Configuration for the ClickHouse dialect.
pub type ClickHouseDialectConfig = NullDialectConfig;

pub fn dialect(config: Option<&Value>) -> Dialect {
    // Parse and validate dialect configuration, falling back to defaults on failure
    let _dialect_config: ClickHouseDialectConfig = config
        .map(ClickHouseDialectConfig::from_value)
        .unwrap_or_default();
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

    // ClickHouse supports CTEs with DML statements (INSERT, UPDATE, DELETE)
    // We add these to NonWithSelectableGrammar so WithCompoundStatementSegment can use them
    clickhouse_dialect.add([(
        "NonWithSelectableGrammar".into(),
        one_of(vec![
            Ref::new("SetExpressionSegment").to_matchable(),
            optionally_bracketed(vec![Ref::new("SelectStatementSegment").to_matchable()])
                .to_matchable(),
            Ref::new("NonSetSelectableGrammar").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([
        (
            "SelectClauseTerminatorGrammar".into(),
            clickhouse_dialect
                .grammar("SelectClauseTerminatorGrammar")
                .copy(
                    Some(vec![
                        Ref::keyword("PREWHERE").to_matchable(),
                        Ref::keyword("INTO").to_matchable(),
                        Ref::keyword("FORMAT").to_matchable(),
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
                    Some(vec![
                        Ref::keyword("PREWHERE").to_matchable(),
                        Ref::keyword("INTO").to_matchable(),
                        Ref::keyword("FORMAT").to_matchable(),
                    ]),
                    None,
                    Some(Ref::keyword("WHERE").to_matchable()),
                    None,
                    Vec::new(),
                    false,
                )
                .copy(
                    Some(vec![Ref::new("SettingsClauseSegment").to_matchable()]),
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
            .to_matchable()
            .into(),
        ),
    ]);

    // Disambiguate wildcard EXCEPT from set operator EXCEPT.
    // Exclude patterns like `EXCEPT ( ... )` and `EXCEPT identifier` from
    // being parsed as a set operator to allow wildcard `* EXCEPT ...` to bind.
    clickhouse_dialect.replace_grammar(
        "SetOperatorSegment",
        one_of(vec![
            Ref::new("UnionGrammar").to_matchable(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("INTERSECT").to_matchable(),
                    Ref::keyword("EXCEPT").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("ALL").optional().to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("MINUS").to_matchable(),
        ])
        .config(|config| {
            config.exclude = Some(
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("EXCEPT").to_matchable(),
                        Bracketed::new(vec![Anything::new().to_matchable()]).to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXCEPT").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            );
        })
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "FromExpressionElementSegment",
        Sequence::new(vec![
            Ref::new("PreTableFunctionKeywordsGrammar")
                .optional()
                .to_matchable(),
            optionally_bracketed(vec![Ref::new("TableExpressionSegment").to_matchable()])
                .to_matchable(),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec![
                    Ref::new("FromClauseTerminatorGrammar").to_matchable(),
                    Ref::new("SamplingExpressionSegment").to_matchable(),
                    Ref::new("JoinLikeClauseGrammar").to_matchable(),
                    Ref::keyword("FINAL").to_matchable(),
                    Ref::new("JoinClauseSegment").to_matchable(),
                ]))
                .optional()
                .to_matchable(),
            Ref::keyword("FINAL").optional().to_matchable(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("OFFSET").to_matchable(),
                Ref::new("AliasExpressionSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("SamplingExpressionSegment")
                .optional()
                .to_matchable(),
            Ref::new("PostTableExpressionGrammar")
                .optional()
                .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec![
            Sequence::new(vec![
                Ref::new("JoinTypeKeywords").optional().to_matchable(),
                Ref::new("JoinKeywordsGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Conditional::new(MetaSegment::indent())
                    .indented_using_on()
                    .to_matchable(),
                one_of(vec![
                    Ref::new("JoinOnConditionSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Conditional::new(MetaSegment::indent())
                            .indented_using_on()
                            .to_matchable(),
                        Delimited::new(vec![
                            one_of(vec![
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|this| this.parse_mode(ParseMode::Greedy))
                                .to_matchable(),
                                Delimited::new(vec![
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Conditional::new(MetaSegment::dedent())
                            .indented_using_on()
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Conditional::new(MetaSegment::dedent())
                    .indented_using_on()
                    .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "SelectClauseModifierSegment",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("DISTINCT").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("ALL").to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "OrderByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("ORDER").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            Delimited::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ASC").to_matchable(),
                        Ref::keyword("DESC").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NULLS").to_matchable(),
                        one_of(vec![
                            Ref::keyword("FIRST").to_matchable(),
                            Ref::keyword("LAST").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("WithFillSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![
                    Ref::keyword("LIMIT").to_matchable(),
                    Ref::new("FrameClauseUnitGrammar").to_matchable(),
                ]
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
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
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
                Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                Ref::new("BackQuotedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // A Clickhouse SELECT EXCEPT clause.
        // https://clickhouse.com/docs/en/sql-reference/statements/select#except
        (
            "ExceptClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectExceptClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXCEPT").to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral).to_matchable(),
                TypedParser::new(SyntaxKind::DollarQuote, SyntaxKind::QuotedLiteral).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
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
    ]);

    clickhouse_dialect.replace_grammar(
        "WildcardExpressionSegment",
        ansi::wildcard_expression_segment().copy(
            Some(vec![
                Ref::new("ExceptClauseSegment").optional().to_matchable(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    clickhouse_dialect.add([(
        "SettingsClauseSegment".into(),
        Sequence::new(vec![
            Ref::keyword("SETTINGS").to_matchable(),
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("NakedIdentifierSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("BooleanLiteralGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("INTO").to_matchable(),
                Ref::keyword("OUTFILE").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
                Ref::new("FormatClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FormatClauseSegment".into(),
            Sequence::new(vec![
                Ref::keyword("FORMAT").to_matchable(),
                Ref::new("FormatValueSegment").to_matchable(),
                Ref::new("SettingsClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MergeTreesOrderByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::MergeTreeOrderByClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("TUPLE").to_matchable(),
                            Bracketed::new(vec![]).to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
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
            Sequence::new(vec![
                Ref::keyword("GLOBAL").optional().to_matchable(),
                one_of(vec![
                    // This case INNER [ANY,ALL] JOIN
                    Sequence::new(vec![
                        Ref::keyword("INNER").to_matchable(),
                        one_of(vec![
                            Ref::keyword("ALL").to_matchable(),
                            Ref::keyword("ANY").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // This case [ANY,ALL] INNER JOIN
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ALL").to_matchable(),
                            Ref::keyword("ANY").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("INNER").to_matchable(),
                    ])
                    .to_matchable(),
                    // This case FULL ALL OUTER JOIN
                    Sequence::new(vec![
                        Ref::keyword("FULL").to_matchable(),
                        Ref::keyword("ALL").optional().to_matchable(),
                        Ref::keyword("OUTER").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // This case ALL FULL OUTER JOIN
                    Sequence::new(vec![
                        Ref::keyword("ALL").optional().to_matchable(),
                        Ref::keyword("FULL").to_matchable(),
                        Ref::keyword("OUTER").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // This case LEFT [OUTER,ANTI,SEMI,ANY,ASOF] JOIN
                    Sequence::new(vec![
                        Ref::keyword("LEFT").to_matchable(),
                        one_of(vec![
                            Ref::keyword("ANTI").to_matchable(),
                            Ref::keyword("SEMI").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ANY").to_matchable(),
                                Ref::keyword("ALL").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::keyword("ASOF").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("OUTER").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // This case [ANTI,SEMI,ANY,ASOF] LEFT JOIN
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ANTI").to_matchable(),
                            Ref::keyword("SEMI").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ANY").to_matchable(),
                                Ref::keyword("ALL").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::keyword("ASOF").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("LEFT").to_matchable(),
                    ])
                    .to_matchable(),
                    // This case RIGHT [OUTER,ANTI,SEMI,ANY,ASOF] JOIN
                    Sequence::new(vec![
                        Ref::keyword("RIGHT").to_matchable(),
                        one_of(vec![
                            Ref::keyword("OUTER").to_matchable(),
                            Ref::keyword("ANTI").to_matchable(),
                            Ref::keyword("SEMI").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ANY").to_matchable(),
                                Ref::keyword("ALL").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("OUTER").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // This case [OUTER,ANTI,SEMI,ANY] RIGHT JOIN
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ANTI").to_matchable(),
                            Ref::keyword("SEMI").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ANY").to_matchable(),
                                Ref::keyword("ALL").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("RIGHT").to_matchable(),
                    ])
                    .to_matchable(),
                    // This case CROSS JOIN
                    Ref::keyword("CROSS").to_matchable(),
                    // This case PASTE JOIN
                    Ref::keyword("PASTE").to_matchable(),
                    // This case ASOF JOIN
                    Ref::keyword("ASOF").to_matchable(),
                    // This case ANY JOIN
                    Ref::keyword("ANY").to_matchable(),
                    // This case ALL JOIN
                    Ref::keyword("ALL").to_matchable(),
                ])
                .to_matchable(),
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
        one_of(vec![
            Ref::new("ArithmeticBinaryOperatorGrammar").to_matchable(),
            Ref::new("StringBinaryOperatorGrammar").to_matchable(),
            Ref::new("BooleanBinaryOperatorGrammar").to_matchable(),
            Ref::new("ComparisonOperatorGrammar").to_matchable(),
            // Add Lambda Function
            Ref::new("LambdaFunctionSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([
        (
            "JoinLikeClauseGrammar".into(),
            Sequence::new(vec![
                AnyNumberOf::new(vec![Ref::new("ArrayJoinClauseSegment").to_matchable()])
                    .config(|this| this.min_times(1))
                    .to_matchable(),
                Ref::new("AliasExpressionSegment").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "InOperatorGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("GLOBAL").optional().to_matchable(),
                Ref::keyword("NOT").optional().to_matchable(),
                Ref::keyword("IN").to_matchable(),
                one_of(vec![
                    Ref::new("FunctionSegment").to_matchable(), // IN tuple(1, 2)
                    Ref::new("ArrayLiteralSegment").to_matchable(), // IN [1, 2]
                    Ref::new("TupleSegment").to_matchable(),    // IN (1, 2)
                    Ref::new("SingleIdentifierGrammar").to_matchable(), // IN TABLE, IN CTE
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("Expression_A_Grammar").to_matchable()])
                            .to_matchable(),
                        Ref::new("SelectableGrammar").to_matchable(),
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.add([(
        "PreWhereClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::PreWhereClause, |_| {
            Sequence::new(vec![
                Ref::keyword("PREWHERE").to_matchable(),
                // NOTE: The indent here is implicit to allow
                // constructions like:
                //
                //    PREWHERE a
                //        AND b
                //
                // to be valid without forcing an indent between
                // "PREWHERE" and "a".
                MetaSegment::implicit_indent().to_matchable(),
                optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                    .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([(
        "QualifyClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::QualifyClause, |_| {
            Sequence::new(vec![
                Ref::keyword("QUALIFY").to_matchable(),
                MetaSegment::implicit_indent().to_matchable(),
                optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                    .to_matchable(),
                MetaSegment::dedent().to_matchable(),
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
                Some(vec![
                    Ref::new("PreWhereClauseSegment").optional().to_matchable(),
                ]),
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
                Some(vec![
                    Ref::new("PreWhereClauseSegment").optional().to_matchable(),
                ]),
                None,
                Some(Ref::new("WhereClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            )
            .copy(
                Some(vec![
                    Ref::new("QualifyClauseSegment").optional().to_matchable(),
                ]),
                None,
                Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            )
            .copy(
                Some(vec![
                    Ref::new("FormatClauseSegment").optional().to_matchable(),
                    Ref::new("IntoOutfileClauseSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("SettingsClauseSegment").optional().to_matchable(),
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
        Sequence::new(vec![
            Ref::keyword("WITH").to_matchable(),
            Ref::keyword("FILL").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("FROM").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("TO").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("STEP").to_matchable(),
                one_of(vec![
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("IntervalExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "DatatypeSegment",
        one_of(vec![
            // Nullable(Type)
            Sequence::new(vec![
                StringParser::new("NULLABLE", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable(),
            ])
            .to_matchable(),
            // LowCardinality(Type)
            Sequence::new(vec![
                StringParser::new("LOWCARDINALITY", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable(),
            ])
            .to_matchable(),
            // DateTime64(precision, 'timezone')
            Sequence::new(vec![
                StringParser::new("DATETIME64", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(), // precision
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // DateTime('timezone')
            Sequence::new(vec![
                StringParser::new("DATETIME", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![Ref::new("QuotedLiteralSegment").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
            ])
            .to_matchable(),
            // FixedString(length)
            Sequence::new(vec![
                StringParser::new("FIXEDSTRING", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable(),
            // Array(Type)
            Sequence::new(vec![
                StringParser::new("ARRAY", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable(),
            ])
            .to_matchable(),
            // Map(KeyType, ValueType)
            Sequence::new(vec![
                StringParser::new("MAP", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Tuple(Type1, Type2) or Tuple(name1 Type1, ...)
            Sequence::new(vec![
                StringParser::new("TUPLE", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            // Named tuple element: name Type
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("DatatypeSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            // Regular tuple element: just Type
                            Ref::new("DatatypeSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Nested(name1 Type1, name2 Type2)
            Sequence::new(vec![
                StringParser::new("NESTED", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // JSON data type
            StringParser::new("JSON", SyntaxKind::DataTypeIdentifier).to_matchable(),
            // Enum8('val1' = 1, 'val2' = 2)
            Sequence::new(vec![
                one_of(vec![
                    StringParser::new("ENUM8", SyntaxKind::DataTypeIdentifier).to_matchable(),
                    StringParser::new("ENUM16", SyntaxKind::DataTypeIdentifier).to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // double args
            Sequence::new(vec![
                one_of(vec![
                    StringParser::new("DECIMAL", SyntaxKind::DataTypeIdentifier).to_matchable(),
                    StringParser::new("NUMERIC", SyntaxKind::DataTypeIdentifier).to_matchable(),
                ])
                .to_matchable(),
                Ref::new("BracketedArguments").optional().to_matchable(),
            ])
            .to_matchable(),
            // single args
            Sequence::new(vec![
                one_of(vec![
                    StringParser::new("DECIMAL32", SyntaxKind::DataTypeIdentifier).to_matchable(),
                    StringParser::new("DECIMAL64", SyntaxKind::DataTypeIdentifier).to_matchable(),
                    StringParser::new("DECIMAL128", SyntaxKind::DataTypeIdentifier).to_matchable(),
                    StringParser::new("DECIMAL256", SyntaxKind::DataTypeIdentifier).to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("TupleTypeSegment").to_matchable(),
            Ref::new("DatatypeIdentifierSegment").to_matchable(),
            Ref::new("NumericLiteralSegment").to_matchable(),
            Sequence::new(vec![
                StringParser::new("DATETIME64", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("NumericLiteralSegment").to_matchable(), // precision
                        Ref::new("QuotedLiteralSegment").optional().to_matchable(),
                    ])
                    // The brackets might be empty as well
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                StringParser::new("ARRAY", SyntaxKind::DataTypeIdentifier).to_matchable(),
                Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "TupleTypeSegment".into(),
        Sequence::new(vec![
            Ref::keyword("TUPLE").to_matchable(),
            Ref::new("TupleTypeSchemaSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.add([(
        "TupleTypeSchemaSegment".into(),
        Bracketed::new(vec![
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "BracketedArguments",
        Bracketed::new(vec![
            Delimited::new(vec![
                one_of(vec![
                    Ref::new("DatatypeIdentifierSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "ArrayJoinClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::ArrayJoinClause, |_| {
            Sequence::new(vec![
                Ref::keyword("LEFT").optional().to_matchable(),
                Ref::keyword("ARRAY").to_matchable(),
                Ref::new("JoinKeywordsGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Delimited::new(vec![Ref::new("SelectClauseElementSegment").to_matchable()])
                    .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "CTEDefinitionSegment",
        one_of(vec![
            Sequence::new(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("CTEColumnList").optional().to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                    .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "AliasExpressionSegment",
        Sequence::new(vec![
            MetaSegment::indent().to_matchable(),
            Ref::keyword("AS").optional().to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Bracketed::new(vec![Ref::new("SingleIdentifierListSegment").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
            ])
            .config(|this| {
                this.exclude = one_of(vec![
                    Ref::keyword("LATERAL").to_matchable(),
                    Ref::keyword("WINDOW").to_matchable(),
                    Ref::keyword("KEYS").to_matchable(),
                ])
                .to_matchable()
                .into()
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "TableEngineFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableEngineFunction, |_| {
                Sequence::new(vec![
                    Ref::new("FunctionNameSegment")
                        .exclude(one_of(vec![
                            Ref::new("DatePartFunctionNameSegment").to_matchable(),
                            Ref::new("ValuesClauseSegment").to_matchable(),
                        ]))
                        .to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("FunctionContentsGrammar")
                            .optional()
                            .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                        this.parse_mode(ParseMode::Greedy)
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "OnClusterClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OnClusterClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("CLUSTER").to_matchable(),
                    one_of(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        // Support for placeholders like '{cluster}'
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableEngineSegment".into(),
            NodeMatcher::new(SyntaxKind::Engine, |_| {
                Sequence::new(vec![
                    Ref::keyword("ENGINE").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::new("TableEngineFunctionSegment").to_matchable(),
                        any_set_of(vec![
                            Ref::new("MergeTreesOrderByClauseSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("PARTITION").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("PRIMARY").to_matchable(),
                                Ref::keyword("KEY").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SAMPLE").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("SettingsClauseSegment").optional().to_matchable(),
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
            "DatabaseEngineFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::EngineFunction, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("ATOMIC").to_matchable(),
                        Ref::keyword("MYSQL").to_matchable(),
                        Ref::keyword("MATERIALIZEDMYSQL").to_matchable(),
                        Ref::keyword("LAZY").to_matchable(),
                        Ref::keyword("POSTGRESQL").to_matchable(),
                        Ref::keyword("MATERIALIZEDPOSTGRESQL").to_matchable(),
                        Ref::keyword("REPLICATED").to_matchable(),
                        Ref::keyword("SQLITE").to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("FunctionContentsGrammar")
                            .optional()
                            .to_matchable(),
                    ])
                    .config(|this| {
                        this.parse_mode(ParseMode::Greedy);
                        this.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseEngineSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseEngine, |_| {
                Sequence::new(vec![
                    Ref::keyword("ENGINE").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("DatabaseEngineFunctionSegment").to_matchable(),
                        any_set_of(vec![
                            Ref::new("MergeTreesOrderByClauseSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("PARTITION").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("PRIMARY").to_matchable(),
                                Ref::keyword("KEY").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SAMPLE").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::new("SettingsClauseSegment").optional().to_matchable(),
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
            "ColumnTTLSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnTtlSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("TTL").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableTTLSegment".into(),
            NodeMatcher::new(SyntaxKind::TableTtlSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("TTL").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("ExpressionSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("DELETE").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("TO").to_matchable(),
                                    Ref::keyword("VOLUME").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("TO").to_matchable(),
                                    Ref::keyword("DISK").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::new("WhereClauseSegment").optional().to_matchable(),
                            Ref::new("GroupByClauseSegment").optional().to_matchable(),
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
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
                any_set_of(vec![
                    Sequence::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("CONSTRAINT").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("NOT").optional().to_matchable(),
                                Ref::keyword("NULL").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CHECK").to_matchable(),
                                Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Ref::keyword("MATERIALIZED").to_matchable(),
                                    Ref::keyword("ALIAS").to_matchable(),
                                ])
                                .to_matchable(),
                                one_of(vec![
                                    Ref::new("LiteralGrammar").to_matchable(),
                                    Ref::new("FunctionSegment").to_matchable(),
                                    Ref::new("BareFunctionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("EPHEMERAL").to_matchable(),
                                one_of(vec![
                                    Ref::new("LiteralGrammar").to_matchable(),
                                    Ref::new("FunctionSegment").to_matchable(),
                                    Ref::new("BareFunctionSegment").to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("PrimaryKeyGrammar").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CODEC").to_matchable(),
                                Ref::new("FunctionContentsGrammar").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Ref::new("ColumnTTLSegment").to_matchable(),
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

    clickhouse_dialect.replace_grammar(
        "CreateDatabaseStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("DATABASE").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("DatabaseReferenceSegment").to_matchable(),
            any_set_of(vec![
                Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                Ref::new("DatabaseEngineSegment").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::keyword("TABLE").to_matchable(),
                Ref::keyword("OVERRIDE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("TableConstraintSegment").to_matchable(),
                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                        Ref::new("ColumnConstraintSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // https://clickhouse.com/docs/sql-reference/statements/rename
    clickhouse_dialect.add([(
        "RenameStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::RenameTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("RENAME").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("TABLE").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("TableReferenceSegment").to_matchable(),
                                Ref::keyword("TO").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DATABASE").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("DatabaseReferenceSegment").to_matchable(),
                                Ref::keyword("TO").to_matchable(),
                                Ref::new("DatabaseReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DICTIONARY").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                                Ref::keyword("TO").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("OnClusterClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "CreateTableStatementSegment",
        one_of(vec![
            // Regular CREATE TABLE statement
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::new("OrReplaceGrammar").optional().to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                one_of(vec![
                    // CREATE TABLE (...):
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("TableConstraintSegment").to_matchable(),
                                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                                    Ref::new("ColumnConstraintSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("TableEngineSegment").to_matchable(),
                        // CREATE TABLE (...) AS SELECT:
                        Sequence::new(vec![
                            Ref::keyword("AS").to_matchable(),
                            Ref::new("SelectableGrammar").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TABLE AS other_table:
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::new("TableEngineSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TABLE AS table_function():
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("FunctionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                any_set_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("COMMENT").to_matchable(),
                        one_of(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("QuotedIdentifierSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("TableTTLSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("TableEndClauseSegment").optional().to_matchable(),
            ])
            .to_matchable(),
            // CREATE TEMPORARY TABLE statement
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("TEMPORARY").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                one_of(vec![
                    // CREATE TEMPORARY TABLE (...):
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("TableConstraintSegment").to_matchable(),
                                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                                    Ref::new("ColumnConstraintSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("TableEngineSegment").to_matchable(),
                        // CREATE TEMPORARY TABLE (...) AS SELECT:
                        Sequence::new(vec![
                            Ref::keyword("AS").to_matchable(),
                            Ref::new("SelectableGrammar").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TEMPORARY TABLE AS other_table:
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::new("TableEngineSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TEMPORARY TABLE AS table_function():
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("FunctionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TEMPORARY TABLE AS SELECT (without column definitions)
                    Sequence::new(vec![
                        Ref::keyword("AS").to_matchable(),
                        Ref::new("SelectableGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                any_set_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("COMMENT").to_matchable(),
                        one_of(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("QuotedIdentifierSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("TableTTLSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::new("TableEndClauseSegment").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "CreateViewStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateViewStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::new("OrReplaceGrammar").optional().to_matchable(),
                Ref::keyword("VIEW").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("SelectableGrammar").to_matchable(),
                Ref::new("TableEndClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "CreateMaterializedViewStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateMaterializedViewStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("MATERIALIZED").to_matchable(),
                Ref::keyword("VIEW").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                        // Add support for column list in TO clause
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("TableEngineSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("TableEngineSegment").optional().to_matchable(),
                        // Add support for PARTITION BY clause
                        Sequence::new(vec![
                            Ref::keyword("PARTITION").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        // Add support for ORDER BY clause
                        Ref::new("MergeTreesOrderByClauseSegment")
                            .optional()
                            .to_matchable(),
                        // Add support for TTL clause
                        Ref::new("TableTTLSegment").optional().to_matchable(),
                        // Add support for SETTINGS clause
                        Ref::new("SettingsClauseSegment").optional().to_matchable(),
                        Ref::keyword("POPULATE").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("SelectableGrammar").to_matchable(),
                Ref::new("TableEndClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "DropTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("TEMPORARY").optional().to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
            Ref::keyword("SYNC").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "DropDatabaseStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("DATABASE").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("DatabaseReferenceSegment").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
            Ref::keyword("SYNC").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "DropDictionaryStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropDictionaryStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("DICTIONARY").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::keyword("SYNC").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "DropUserStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("USER").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("SingleIdentifierGrammar").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "DropRoleStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("ROLE").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("SingleIdentifierGrammar").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "DropQuotaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropQuotaStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("QUOTA").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropSettingProfileStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropSettingProfileStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Delimited::new(vec![Ref::new("NakedIdentifierSegment").to_matchable()])
                        .config(|this| this.min_delimiters = 0)
                        .to_matchable(),
                    Ref::keyword("PROFILE").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.replace_grammar(
        "DropViewStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("VIEW").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
            Ref::keyword("SYNC").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "DropFunctionStatementSegment",
        Sequence::new(vec![
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("FUNCTION").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("SingleIdentifierGrammar").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([
        (
            "SystemMergesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemMergesSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("STOP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("MERGES").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("VOLUME").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemTTLMergesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemTtlMergesSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("STOP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("TTL").to_matchable(),
                    Ref::keyword("MERGES").to_matchable(),
                    Ref::new("TableReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemMovesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemMovesSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("STOP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("MOVES").to_matchable(),
                    Ref::new("TableReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicaSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemReplicaSegment, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("SYNC").to_matchable(),
                        Ref::keyword("REPLICA").to_matchable(),
                        Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::keyword("STRICT").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DROP").to_matchable(),
                        Ref::keyword("REPLICA").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FROM").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("DATABASE").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("TABLE").to_matchable(),
                                    Ref::new("TableReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ZKPATH").to_matchable(),
                                    Ref::new("PathSegment").to_matchable(),
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
                        Ref::keyword("RESTART").to_matchable(),
                        Ref::keyword("REPLICA").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RESTORE").to_matchable(),
                        Ref::keyword("REPLICA").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemFilesystemSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemFilesystemSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("FILESYSTEM").to_matchable(),
                    Ref::keyword("CACHE").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicatedSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemReplicatedSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("STOP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("REPLICATED").to_matchable(),
                    Ref::keyword("SENDS").to_matchable(),
                    Ref::new("TableReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicationSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemReplicationSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("STOP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("REPLICATION").to_matchable(),
                    Ref::keyword("QUEUES").to_matchable(),
                    Ref::new("TableReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemFetchesSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemFetchesSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("STOP").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FETCHES").to_matchable(),
                    Ref::new("TableReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemDistributedSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemDistributedSegment, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("START").to_matchable(),
                                Ref::keyword("STOP").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("DISTRIBUTED").to_matchable(),
                            Ref::keyword("SENDS").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FLUSH").to_matchable(),
                            Ref::keyword("DISTRIBUTED").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
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
            "SystemModelSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemModelSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("RELOAD").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("MODELS").to_matchable(),
                            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MODEL").to_matchable(),
                            any_set_of(vec![
                                Ref::new("OnClusterClauseSegment").optional().to_matchable(),
                                Ref::new("PathSegment").to_matchable(),
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
            "SystemFileSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemFileSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("SYNC").to_matchable(),
                    Ref::keyword("FILE").to_matchable(),
                    Ref::keyword("CACHE").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemUnfreezeSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemUnfreezeSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("UNFREEZE").to_matchable(),
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("NAME").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SystemStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SystemStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("SYSTEM").to_matchable(),
                    one_of(vec![
                        Ref::new("SystemMergesSegment").to_matchable(),
                        Ref::new("SystemTTLMergesSegment").to_matchable(),
                        Ref::new("SystemMovesSegment").to_matchable(),
                        Ref::new("SystemReplicaSegment").to_matchable(),
                        Ref::new("SystemReplicatedSegment").to_matchable(),
                        Ref::new("SystemReplicationSegment").to_matchable(),
                        Ref::new("SystemFetchesSegment").to_matchable(),
                        Ref::new("SystemDistributedSegment").to_matchable(),
                        Ref::new("SystemFileSegment").to_matchable(),
                        Ref::new("SystemFilesystemSegment").to_matchable(),
                        Ref::new("SystemUnfreezeSegment").to_matchable(),
                        Ref::new("SystemModelSegment").to_matchable(),
                    ])
                    .to_matchable(),
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
        Sequence::new(vec![
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("IfExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("OnClusterClauseSegment").optional().to_matchable(),
            one_of(vec![
                // ALTER TABLE ... DROP COLUMN [IF EXISTS] name
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... ADD COLUMN [IF NOT EXISTS] name [type]
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    one_of(vec![
                        // Regular column with type
                        Sequence::new(vec![
                            Ref::new("DatatypeSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MATERIALIZED").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CODEC").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        one_of(vec![
                                            Ref::new("FunctionSegment").to_matchable(),
                                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Alias column with type
                        Sequence::new(vec![
                            Ref::new("DatatypeSegment").to_matchable(),
                            Ref::keyword("ALIAS").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        // Alias column without type
                        Sequence::new(vec![
                            Ref::keyword("ALIAS").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        // Default could also be used without type
                        Sequence::new(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        // Materialized could also be used without type
                        Sequence::new(vec![
                            Ref::keyword("MATERIALIZED").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("AFTER").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FIRST").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... ADD ALIAS name FOR column_name
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("ALIAS").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... RENAME COLUMN [IF EXISTS] name to new_name
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... COMMENT COLUMN [IF EXISTS] name 'Text comment'
                Sequence::new(vec![
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... COMMENT 'Text comment'
                Sequence::new(vec![
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... MODIFY COMMENT 'Text comment'
                Sequence::new(vec![
                    Ref::keyword("MODIFY").to_matchable(),
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... MODIFY COLUMN [IF EXISTS] name [TYPE] [type]
                Sequence::new(vec![
                    Ref::keyword("MODIFY").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    one_of(vec![
                        // Type modification with explicit TYPE keyword
                        Sequence::new(vec![
                            Ref::keyword("TYPE").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MATERIALIZED").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CODEC").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        one_of(vec![
                                            Ref::new("FunctionSegment").to_matchable(),
                                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Type modification without TYPE keyword
                        Sequence::new(vec![
                            Ref::new("DatatypeSegment").optional().to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MATERIALIZED").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ALIAS").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CODEC").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        one_of(vec![
                                            Ref::new("FunctionSegment").to_matchable(),
                                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Alias modification
                        Sequence::new(vec![
                            Ref::keyword("ALIAS").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        // Remove alias
                        Sequence::new(vec![
                            Ref::keyword("REMOVE").to_matchable(),
                            Ref::new("ALIAS").to_matchable(),
                        ])
                        .to_matchable(),
                        // Remove property
                        Sequence::new(vec![
                            Ref::keyword("REMOVE").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ALIAS").to_matchable(),
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::keyword("MATERIALIZED").to_matchable(),
                                Ref::keyword("CODEC").to_matchable(),
                                Ref::keyword("COMMENT").to_matchable(),
                                Ref::keyword("TTL").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Modify setting
                        Sequence::new(vec![
                            Ref::keyword("MODIFY").to_matchable(),
                            Ref::keyword("SETTING").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        // Reset setting
                        Sequence::new(vec![
                            Ref::keyword("RESET").to_matchable(),
                            Ref::keyword("SETTING").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("AFTER").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FIRST").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... ALTER COLUMN name [TYPE] [type]
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    one_of(vec![
                        // With TYPE keyword
                        Sequence::new(vec![
                            Ref::keyword("TYPE").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // Without TYPE keyword
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("AFTER").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FIRST").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... REMOVE TTL
                Sequence::new(vec![
                    Ref::keyword("REMOVE").to_matchable(),
                    Ref::keyword("TTL").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... MODIFY TTL expression
                Sequence::new(vec![
                    Ref::keyword("MODIFY").to_matchable(),
                    Ref::keyword("TTL").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... MODIFY QUERY select_statement
                Sequence::new(vec![
                    Ref::keyword("MODIFY").to_matchable(),
                    Ref::keyword("QUERY").to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... MATERIALIZE COLUMN col
                Sequence::new(vec![
                    Ref::keyword("MATERIALIZE").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("IN").to_matchable(),
                            Ref::keyword("PARTITION").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("IN").to_matchable(),
                            Ref::keyword("PARTITION").to_matchable(),
                            Ref::keyword("ID").to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... DROP PARTITION|PART partition_expr
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    one_of(vec![
                        Ref::keyword("PARTITION").to_matchable(),
                        Ref::keyword("PART").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
                // ALTER TABLE ... REPLACE PARTITION partition_expr FROM table1
                Sequence::new(vec![
                    Ref::keyword("REPLACE").to_matchable(),
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec![
                Ref::new("CreateMaterializedViewStatementSegment").to_matchable(),
                Ref::new("DropDictionaryStatementSegment").to_matchable(),
                Ref::new("DropQuotaStatementSegment").to_matchable(),
                Ref::new("DropSettingProfileStatementSegment").to_matchable(),
                Ref::new("SystemStatementSegment").to_matchable(),
                Ref::new("RenameStatementSegment").to_matchable(),
                Ref::new("AlterTableStatementSegment").to_matchable(),
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
        optionally_bracketed(vec![
            one_of(vec![
                Ref::new("NumericLiteralSegment").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    clickhouse_dialect.replace_grammar(
        "LimitClauseSegment",
        Sequence::new(vec![
            Ref::keyword("LIMIT").to_matchable(),
            MetaSegment::indent().to_matchable(),
            Sequence::new(vec![
                Ref::new("LimitClauseComponentSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("OFFSET").to_matchable(),
                        Ref::new("LimitClauseComponentSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        // LIMIT 1,2 only accepts constants
                        // and can't be bracketed like that LIMIT (1, 2)
                        // but can be bracketed like that LIMIT (1), (2)
                        Ref::new("CommaSegment").to_matchable(),
                        Ref::new("LimitClauseComponentSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("BY").to_matchable(),
                    one_of(vec![
                        Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    // https://clickhouse.com/docs/sql-reference/data-types/special-data-types/interval
    // https://clickhouse.com/docs/sql-reference/operators#interval
    clickhouse_dialect.replace_grammar(
        "IntervalExpressionSegment",
        Sequence::new(vec![
            Ref::keyword("INTERVAL").to_matchable(),
            one_of(vec![
                // The Numeric Version
                Sequence::new(vec![
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("DatetimeUnitSegment").to_matchable(),
                ])
                .to_matchable(),
                // The String version
                Ref::new("QuotedLiteralSegment").to_matchable(),
                // Combine version
                Sequence::new(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("DatetimeUnitSegment").to_matchable(),
                ])
                .to_matchable(),
                // With expression as value
                Sequence::new(vec![
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::new("DatetimeUnitSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.replace_grammar(
        "ColumnDefinitionSegment",
        Sequence::new(vec![
            one_of(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("DatatypeSegment").to_matchable(),
            AnyNumberOf::new(vec![
                one_of(vec![
                    // DEFAULT expression
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        one_of(vec![
                            Ref::new("LiteralGrammar").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // ALIAS expression
                    Sequence::new(vec![
                        Ref::keyword("ALIAS").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // MATERIALIZED expression
                    Sequence::new(vec![
                        Ref::keyword("MATERIALIZED").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // CODEC(...)
                    Sequence::new(vec![
                        Ref::keyword("CODEC").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("FunctionSegment").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // COMMENT 'text'
                    Sequence::new(vec![
                        Ref::keyword("COMMENT").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // Column constraint
                    Ref::new("ColumnConstraintSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    clickhouse_dialect.expand();
    clickhouse_dialect
}
