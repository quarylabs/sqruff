use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::base::{Anything, Nothing, Ref};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use crate::sqlite_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let sqlite_dialect = super::ansi::raw_dialect();
    let mut sqlite_dialect = sqlite_dialect;
    sqlite_dialect.name = DialectKind::Sqlite;

    sqlite_dialect.sets_mut("reserved_keywords").clear();
    sqlite_dialect
        .sets_mut("reserved_keywords")
        .extend(RESERVED_KEYWORDS);
    sqlite_dialect.sets_mut("unreserved_keywords").clear();
    sqlite_dialect
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);

    sqlite_dialect.add([
        (
            "ColumnConstraintDefaultGrammar".into(),
            Ref::new("ExpressionSegment").to_matchable().into(),
        ),
        (
            "BooleanBinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("AndOperatorGrammar"),
                Ref::new("OrOperatorGrammar"),
                Ref::keyword("REGEXP")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PrimaryKeyGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("PRIMARY"),
                Ref::keyword("KEY"),
                Sequence::new(vec_of_erased![Ref::keyword("AUTOINCREMENT")]).config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryTransientGrammar".into(),
            Ref::new("TemporaryGrammar").to_matchable().into(),
        ),
        (
            "DateTimeLiteralGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATE"),
                    Ref::keyword("DATETIME")
                ]),
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BaseExpressionElementGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("LiteralGrammar"),
                Ref::new("BareFunctionSegment"),
                Ref::new("FunctionSegment"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("ExpressionSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("DatatypeSegment"),
                    Ref::new("LiteralGrammar")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AutoIncrementGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "CommentClauseSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "IntervalExpressionSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "TimeZoneGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "FetchClauseSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "TrimParametersGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "LikeGrammar".into(),
            Sequence::new(vec_of_erased![Ref::keyword("LIKE")])
                .to_matchable()
                .into(),
        ),
        (
            "OverlapsClauseSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "MLTableExpressionSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "MergeIntoLiteralGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "SamplingExpressionSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "OrderByClauseTerminators".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Ref::keyword("WINDOW"),
                Ref::new("FrameClauseUnitGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("WINDOW")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("WHERE"),
                Ref::keyword("LIMIT"),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("WINDOW"),
                Ref::new("SetOperatorSegment"),
                Ref::new("WithNoSchemaBindingClauseSegment"),
                Ref::new("WithDataClauseSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("LIMIT"),
                Ref::keyword("HAVING"),
                Ref::keyword("WINDOW")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            Ref::new("FilterClauseGrammar").to_matchable().into(),
        ),
        (
            "IgnoreRespectNullsGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("FROM"),
                Ref::keyword("WHERE"),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("LIMIT"),
                Ref::new("SetOperatorSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionContentsGrammar".into(),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("ExpressionSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Ref::keyword("AS"),
                    Ref::new("DatatypeSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("TrimParametersGrammar"),
                    Ref::new("ExpressionSegment")
                        .optional()
                        .exclude(Ref::keyword("FROM"))
                        .config(|config| {
                            config.exclude = Ref::keyword("FROM").to_matchable().into();
                        }),
                    Ref::keyword("FROM"),
                    Ref::new("ExpressionSegment")
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("DatetimeUnitSegment"),
                        Ref::new("ExpressionSegment")
                    ]),
                    Ref::keyword("FROM"),
                    Ref::new("ExpressionSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISTINCT").optional(),
                    one_of(vec_of_erased![
                        Ref::new("StarSegment"),
                        Delimited::new(vec_of_erased![Ref::new(
                            "FunctionContentsExpressionGrammar"
                        )])
                    ])
                ]),
                Ref::new("OrderByClauseSegment"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("ColumnReferenceSegment")
                    ]),
                    Ref::keyword("IN"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("ColumnReferenceSegment")
                    ])
                ]),
                Ref::new("IndexColumnDefinitionSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "Expression_A_Unary_Operator_Grammar".into(),
            one_of(vec_of_erased![
                Ref::new("SignedSegmentGrammar")
                    .exclude(Sequence::new(vec_of_erased![Ref::new(
                        "QualifiedNumericLiteralSegment"
                    )]))
                    .config(|config| {
                        config.exclude = Sequence::new(vec_of_erased![Ref::new(
                            "QualifiedNumericLiteralSegment"
                        )])
                        .to_matchable()
                        .into();
                    }),
                Ref::new("TildeSegment"),
                Ref::new("NotOperatorGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IsClauseGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("NULL"),
                Ref::new("BooleanLiteralGrammar")
            ])
            .to_matchable()
            .into(),
        ),
    ]);
    sqlite_dialect.add([(
        "SetOperatorSegment".into(),
        NodeMatcher::new(
            SyntaxKind::SetOperator,
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNION"),
                    one_of(vec_of_erased![
                        Ref::keyword("DISTINCT"),
                        Ref::keyword("ALL")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("INTERSECT"),
                        Ref::keyword("EXCEPT")
                    ]),
                    Ref::keyword("ALL").optional()
                ])
            ])
            .config(|config| {
                config.exclude = Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Bracketed::new(vec_of_erased![Anything::new()])
                ])
                .to_matchable()
                .into();
            })
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.replace_grammar(
        "DatatypeSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("DOUBLE"),
                Ref::keyword("PRECISION")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("UNSIGNED"),
                Ref::keyword("BIG"),
                Ref::keyword("INT")
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("VARYING"),
                            Ref::keyword("NATIVE")
                        ]),
                        one_of(vec_of_erased![Ref::keyword("CHARACTER")])
                    ]),
                    Ref::new("DatatypeIdentifierSegment")
                ]),
                Ref::new("BracketedArguments").optional()
            ])
        ])
        .to_matchable(),
    );
    sqlite_dialect.add([(
        "TableEndClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::TableEndClauseSegment,
            Delimited::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITHOUT"),
                    Ref::keyword("ROWID")
                ]),
                Ref::keyword("STRICT")
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.replace_grammar(
        "ValuesClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("VALUES"),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    Ref::new("ExpressionSegment")
                ])])
                .config(|config| {
                    config.parse_mode(ParseMode::Greedy);
                })
            ])])
        ])
        .to_matchable(),
    );
    sqlite_dialect.add([
        (
            "IndexColumnDefinitionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IndexColumnDefinition,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("ExpressionSegment")
                    ]),
                    one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")]).config(
                        |config| {
                            config.optional();
                        }
                    )
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertStatement,
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("INSERT"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("OR"),
                                one_of(vec_of_erased![
                                    Ref::keyword("ABORT"),
                                    Ref::keyword("FAIL"),
                                    Ref::keyword("IGNORE"),
                                    Ref::keyword("REPLACE"),
                                    Ref::keyword("ROLLBACK")
                                ])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Ref::keyword("REPLACE")
                    ]),
                    Ref::keyword("INTO"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    one_of(vec_of_erased![
                        Ref::new("ValuesClauseSegment"),
                        optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                        Ref::new("DefaultValuesGrammar")
                    ]),
                    Ref::new("ReturningClauseSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    let column_constraint = sqlite_dialect
        .grammar("ColumnConstraintSegment")
        .match_grammar()
        .unwrap()
        .copy(
            Some(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DEFERRABLE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NOT"),
                        Ref::keyword("DEFERRABLE")
                    ])
                ])
                .config(|config| {
                    config.optional();
                }),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INITIALLY"),
                        Ref::keyword("DEFERRED")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INITIALLY"),
                        Ref::keyword("IMMEDIATE")
                    ])
                ])
                .config(|config| {
                    config.optional();
                })
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        );
    sqlite_dialect.replace_grammar("ColumnConstraintSegment", column_constraint);

    sqlite_dialect.replace_grammar(
        "TableConstraintSegment",
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment")
            ])
            .config(|config| {
                config.optional();
            }),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CHECK"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNIQUE"),
                    Ref::new("BracketedColumnReferenceListGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PrimaryKeyGrammar"),
                    Ref::new("BracketedColumnReferenceListGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("ForeignKeyGrammar"),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    Ref::new("ReferenceDefinitionGrammar")
                ])
            ]),
            one_of(vec_of_erased![
                Ref::keyword("DEFERRABLE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("DEFERRABLE")
                ])
            ])
            .config(|config| {
                config.optional();
            }),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("INITIALLY"),
                    Ref::keyword("DEFERRED")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INITIALLY"),
                    Ref::keyword("IMMEDIATE")
                ])
            ])
            .config(|config| {
                config.optional();
            })
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "TransactionStatementSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("BEGIN"),
                Ref::keyword("COMMIT"),
                Ref::keyword("ROLLBACK"),
                Ref::keyword("END")
            ]),
            one_of(vec_of_erased![Ref::keyword("TRANSACTION")]).config(|config| {
                config.optional();
            }),
            Sequence::new(vec_of_erased![
                Ref::keyword("TO"),
                Ref::keyword("SAVEPOINT"),
                Ref::new("ObjectReferenceSegment")
            ])
            .config(|config| {
                config.optional();
            })
        ])
        .to_matchable(),
    );

    sqlite_dialect.add([(
        "PragmaReferenceSegment".into(),
        NodeMatcher::new(
            SyntaxKind::PragmaReference,
            sqlite_dialect
                .grammar("ObjectReferenceSegment")
                .match_grammar()
                .unwrap(),
        )
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.add([(
        "PragmaStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::PragmaStatement, {
            let pragma_value = one_of(vec_of_erased![
                Ref::new("LiteralGrammar"),
                Ref::new("BooleanLiteralGrammar"),
                Ref::keyword("YES"),
                Ref::keyword("NO"),
                Ref::keyword("ON"),
                Ref::keyword("OFF"),
                Ref::keyword("NONE"),
                Ref::keyword("FULL"),
                Ref::keyword("INCREMENTAL"),
                Ref::keyword("DELETE"),
                Ref::keyword("TRUNCATE"),
                Ref::keyword("PERSIST"),
                Ref::keyword("MEMORY"),
                Ref::keyword("WAL"),
                Ref::keyword("NORMAL"),
                Ref::keyword("EXCLUSIVE"),
                Ref::keyword("FAST"),
                Ref::keyword("EXTRA"),
                Ref::keyword("DEFAULT"),
                Ref::keyword("FILE"),
                Ref::keyword("PASSIVE"),
                Ref::keyword("RESTART"),
                Ref::keyword("RESET")
            ]);

            Sequence::new(vec_of_erased![
                Ref::keyword("PRAGMA"),
                Ref::new("PragmaReferenceSegment"),
                Bracketed::new(vec_of_erased![pragma_value.clone()]).config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::new("EqualsSegment"),
                    optionally_bracketed(vec_of_erased![pragma_value])
                ])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.replace_grammar(
        "CreateTriggerStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("TRIGGER"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TriggerReferenceSegment"),
            one_of(vec_of_erased![
                Ref::keyword("BEFORE"),
                Ref::keyword("AFTER"),
                Sequence::new(vec_of_erased![Ref::keyword("INSTEAD"), Ref::keyword("OF")])
            ])
            .config(|config| {
                config.optional();
            }),
            one_of(vec_of_erased![
                Ref::keyword("DELETE"),
                Ref::keyword("INSERT"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OF"),
                        Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
            ]),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("FOR"),
                Ref::keyword("EACH"),
                Ref::keyword("ROW")
            ])
            .config(|config| {
                config.optional();
            }),
            Sequence::new(vec_of_erased![
                Ref::keyword("WHEN"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::keyword("BEGIN"),
            Delimited::new(vec_of_erased![
                Ref::new("UpdateStatementSegment"),
                Ref::new("InsertStatementSegment"),
                Ref::new("DeleteStatementSegment"),
                Ref::new("SelectableGrammar")
            ])
            .config(|config| {
                config.delimiter(
                    AnyNumberOf::new(vec_of_erased![Ref::new("DelimiterGrammar")]).config(
                        |config| {
                            config.min_times = 1;
                        },
                    ),
                );
                config.allow_trailing = true;
            }),
            Ref::keyword("END")
        ])
        .to_matchable(),
    );
    sqlite_dialect.add([(
        "UnorderedSelectStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::SelectStatement,
            Sequence::new(vec_of_erased![
                Ref::new("SelectClauseSegment"),
                MetaSegment::dedent(),
                Ref::new("FromClauseSegment").optional(),
                Ref::new("WhereClauseSegment").optional(),
                Ref::new("GroupByClauseSegment").optional(),
                Ref::new("HavingClauseSegment").optional(),
                Ref::new("OverlapsClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional()
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.add([(
        "SelectStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::SelectStatement,
            sqlite_dialect
                .grammar("UnorderedSelectStatementSegment")
                .match_grammar()
                .unwrap()
                .copy(
                    Some(vec_of_erased![
                        Ref::new("OrderByClauseSegment").optional(),
                        Ref::new("FetchClauseSegment").optional(),
                        Ref::new("LimitClauseSegment").optional(),
                        Ref::new("NamedWindowSegment").optional(),
                    ]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                ),
        )
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.replace_grammar(
        "CreateIndexStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("UNIQUE").optional(),
            Ref::keyword("INDEX"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("IndexReferenceSegment"),
            Ref::keyword("ON"),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("IndexColumnDefinitionSegment")])
            ])]),
            Ref::new("WhereClauseSegment").optional()
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "StatementSegment",
        one_of(vec_of_erased![
            Ref::new("AlterTableStatementSegment"),
            Ref::new("CreateIndexStatementSegment"),
            Ref::new("CreateTableStatementSegment"),
            Ref::new("CreateTriggerStatementSegment"),
            Ref::new("CreateViewStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            Ref::new("DropIndexStatementSegment"),
            Ref::new("DropTableStatementSegment"),
            Ref::new("DropTriggerStatementSegment"),
            Ref::new("DropViewStatementSegment"),
            Ref::new("ExplainStatementSegment"),
            Ref::new("InsertStatementSegment"),
            Ref::new("PragmaStatementSegment"),
            Ref::new("SelectableGrammar"),
            Ref::new("TransactionStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Bracketed::new(vec_of_erased![Ref::new("StatementSegment")])
        ])
        .to_matchable(),
    );

    sqlite_dialect.add([(
        "ReturningClauseSegment".into(),
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
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect.add([(
        "AsAliasExpressionSegment".into(),
        NodeMatcher::new(
            SyntaxKind::AliasExpression,
            Sequence::new(vec_of_erased![
                MetaSegment::indent(),
                Ref::keyword("AS"),
                Ref::new("SingleIdentifierGrammar"),
                MetaSegment::dedent(),
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    sqlite_dialect
}
