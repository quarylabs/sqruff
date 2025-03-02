use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{
    AnyNumberOf, any_set_of, one_of, optionally_bracketed,
};
use sqruff_lib_core::parser::grammar::base::Ref;
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use super::ansi::{self, raw_dialect};
use crate::clickhouse_keywords::UNRESERVED_KEYWORDS;

pub fn dialect() -> Dialect {
    let mut clickhouse_dialect = raw_dialect();
    clickhouse_dialect.name = DialectKind::Clickhouse;
    clickhouse_dialect
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);

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

    clickhouse_dialect.add([
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("SingleQuotedIdentifierSegment"),
            ])
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
    ]);

    clickhouse_dialect.insert_lexer_matchers(
        vec![Matcher::string("lambda", "->", SyntaxKind::Lambda)],
        "newline",
    );

    clickhouse_dialect.add(vec![
        (
            "JoinTypeKeywords".into(),
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
                // This case ANY JOIN
                Ref::keyword("ANY"),
                // This case ALL JOIN
                Ref::keyword("ALL"),
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

    clickhouse_dialect.add([(
        "JoinLikeClauseGrammar".into(),
        Sequence::new(vec_of_erased![
            AnyNumberOf::new(vec_of_erased![Ref::new("ArrayJoinClauseSegment")])
                .config(|this| this.min_times(1)),
            Ref::new("AliasExpressionSegment").optional(),
        ])
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
        NodeMatcher::new(
            SyntaxKind::ArrayJoinClause,
            Sequence::new(vec_of_erased![
                Ref::keyword("LEFT").optional(),
                Ref::keyword("ARRAY"),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")]),
                MetaSegment::dedent(),
            ])
            .to_matchable(),
        )
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
            NodeMatcher::new(
                SyntaxKind::TableEngineFunction,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "OnClusterClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::OnClusterClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("CLUSTER"),
                    Ref::new("SingleIdentifierGrammar"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableEngineSegment".into(),
            NodeMatcher::new(
                SyntaxKind::Engine,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENGINE"),
                    Ref::new("EqualsSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("TableEngineFunctionSegment"),
                        any_set_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ORDER"),
                                Ref::keyword("BY"),
                                one_of(vec_of_erased![
                                    Ref::new("BracketedColumnReferenceListGrammar"),
                                    Ref::new("ColumnReferenceSegment"),
                                ]),
                            ]),
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
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SETTINGS"),
                                Delimited::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::new("NakedIdentifierSegment"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::new("NumericLiteralSegment"),
                                            Ref::new("QuotedLiteralSegment"),
                                        ]),
                                    ])
                                    .config(|this| this.optional())
                                ]),
                            ]),
                        ]),
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseEngineFunctionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::EngineFunction,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseEngineSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DatabaseEngine,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENGINE"),
                    Ref::new("EqualsSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("DatabaseEngineFunctionSegment"),
                        any_set_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ORDER"),
                                Ref::keyword("BY"),
                                one_of(vec_of_erased![
                                    Ref::new("BracketedColumnReferenceListGrammar"),
                                    Ref::new("ColumnReferenceSegment"),
                                ]),
                            ])
                            .config(|this| this.optional()),
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
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SETTINGS"),
                                Delimited::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::new("NakedIdentifierSegment"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::new("NumericLiteralSegment"),
                                            Ref::new("QuotedLiteralSegment"),
                                        ]),
                                    ])
                                    .config(|this| this.optional()),
                                ])])
                            ])
                            .config(|this| this.optional()),
                        ]),
                    ]),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColumnTTLSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ColumnTtlSegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("TTL"),
                    Ref::new("ExpressionSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "TableTTLSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableTtlSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ColumnConstraintSegment,
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
                .to_matchable(),
            )
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
                        .config(|this| this.optional())
                    ])
                    .config(|this| this.optional()),
                ]),
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

    clickhouse_dialect.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![
                Ref::new("OrReplaceGrammar"),
                Ref::keyword("TEMPORARY"),
            ])
            .config(|this| this.optional()),
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
        ])
        .to_matchable(),
    );

    clickhouse_dialect.add([(
        "CreateMaterializedViewStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::CreateMaterializedViewStatement,
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
                        Ref::new("TableEngineSegment").optional(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("TableEngineSegment").optional(),
                        Ref::keyword("POPULATE").optional(),
                    ]),
                ]),
                Ref::keyword("AS"),
                Ref::new("SelectableGrammar"),
                Ref::new("TableEndClauseSegment").optional(),
            ])
            .to_matchable(),
        )
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
        NodeMatcher::new(
            SyntaxKind::DropDictionaryStatement,
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("DICTIONARY"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("SYNC").optional(),
            ])
            .to_matchable(),
        )
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
            NodeMatcher::new(
                SyntaxKind::DropQuotaStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("QUOTA"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("OnClusterClauseSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DropSettingProfileStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DropSettingProfileStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Delimited::new(vec_of_erased![Ref::new("NakedIdentifierSegment")])
                        .config(|this| this.min_delimiters = 0),
                    Ref::keyword("PROFILE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("OnClusterClauseSegment").optional(),
                ])
                .to_matchable(),
            )
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
            NodeMatcher::new(
                SyntaxKind::SystemMergesSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemTTLMergesSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemTtlMergesSegment,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("TTL"),
                    Ref::keyword("MERGES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemMovesSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemMovesSegment,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("MOVES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicaSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemReplicaSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemFilesystemSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemFilesystemSegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("FILESYSTEM"),
                    Ref::keyword("CACHE"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicatedSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemReplicatedSegment,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("REPLICATED"),
                    Ref::keyword("SENDS"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemReplicationSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemReplicationSegment,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("REPLICATION"),
                    Ref::keyword("QUEUES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemFetchesSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemFetchesSegment,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("STOP"),]),
                    Ref::keyword("FETCHES"),
                    Ref::new("TableReferenceSegment").optional(),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemDistributedSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemDistributedSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemModelSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemModelSegment,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemFileSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemFileSegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SYNC"),
                    Ref::keyword("FILE"),
                    Ref::keyword("CACHE"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemUnfreezeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemUnfreezeSegment,
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNFREEZE"),
                    Ref::keyword("WITH"),
                    Ref::keyword("NAME"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SystemStatement,
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
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    clickhouse_dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec_of_erased![
                Ref::new("CreateMaterializedViewStatementSegment"),
                Ref::new("DropDictionaryStatementSegment"),
                Ref::new("DropQuotaStatementSegment"),
                Ref::new("DropSettingProfileStatementSegment"),
                Ref::new("SystemStatementSegment"),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    clickhouse_dialect.expand();
    clickhouse_dialect
}
