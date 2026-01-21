use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Nothing, Ref};
use sqruff_lib_core::parser::lexer::{Matcher, Pattern};
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{StringParser, TypedParser};
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::sqlite_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};

pub fn dialect() -> Dialect {
    raw_dialect().config(|this| this.expand())
}

pub fn raw_dialect() -> Dialect {
    let ansi_dialect = super::ansi::raw_dialect();
    let mut sqlite_dialect = ansi_dialect.clone();
    sqlite_dialect.name = DialectKind::Sqlite;

    // Add lexer matcher for SQLite blob literals (X'...' or x'...').
    // Insert before single_quote to take precedence over string literals.
    sqlite_dialect.insert_lexer_matchers(
        vec![Matcher::regex(
            "bytes_single_quote",
            r#"[xX]'[0-9A-Fa-f]*'"#,
            SyntaxKind::BytesSingleQuote,
        )],
        "single_quote",
    );

    sqlite_dialect.sets_mut("reserved_keywords").clear();

    sqlite_dialect
        .sets_mut("reserved_keywords")
        .extend(RESERVED_KEYWORDS);

    sqlite_dialect.sets_mut("unreserved_keywords").clear();

    sqlite_dialect
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);

    sqlite_dialect.patch_lexer_matchers(vec![
        Matcher::legacy(
            "block_comment",
            |s| s.starts_with("/*"),
            r#"\/\*([^\*]|\*(?!\/))*(\*\/|\Z)"#,
            SyntaxKind::BlockComment,
        )
        .subdivider(Pattern::legacy(
            "newline",
            |_| true,
            r#"\r\n|\n"#,
            SyntaxKind::Newline,
        ))
        .post_subdivide(Pattern::legacy(
            "whitespace",
            |_| true,
            r#"[^\S\r\n]+"#,
            SyntaxKind::Whitespace,
        )),
        Matcher::regex("single_quote", r#"'([^']|'')*'"#, SyntaxKind::SingleQuote),
        Matcher::regex("double_quote", r#""([^"]|"")*""#, SyntaxKind::DoubleQuote),
        Matcher::regex("back_quote", r#"`([^`]|``)*`"#, SyntaxKind::BackQuote),
    ]);

    sqlite_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "at_sign_literal",
                r#"@[a-zA-Z0-9_]+"#,
                SyntaxKind::AtSignLiteral,
            ),
            Matcher::regex(
                "colon_literal",
                r#":[a-zA-Z0-9_]+"#,
                SyntaxKind::ColonLiteral,
            ),
            Matcher::regex(
                "question_literal",
                r#"\?[0-9]+"#,
                SyntaxKind::QuestionLiteral,
            ),
            Matcher::regex(
                "dollar_literal",
                r#"\$[a-zA-Z0-9_]+"#,
                SyntaxKind::DollarLiteral,
            ),
        ],
        "question",
    );

    sqlite_dialect.insert_lexer_matchers(
        vec![
            Matcher::string(
                "inline_path_operator",
                r#"->>"#,
                SyntaxKind::InlinePathOperator,
            ),
            Matcher::string(
                "column_path_operator",
                r#"->"#,
                SyntaxKind::ColumnPathOperator,
            ),
        ],
        "greater_than",
    );

    sqlite_dialect.add([
        // SQLite blob literal segment (X'...' or x'...')
        (
            "BytesQuotedLiteralSegment".into(),
            TypedParser::new(SyntaxKind::BytesSingleQuote, SyntaxKind::BytesQuotedLiteral)
                .to_matchable()
                .into(),
        ),
        // Extend LiteralGrammar to include blob literals
        (
            "LiteralGrammar".into(),
            sqlite_dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec![Ref::new("BytesQuotedLiteralSegment").to_matchable()]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "BackQuotedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::BackQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "ColumnPathOperatorSegment".into(),
            StringParser::new("->", SyntaxKind::ColumnPathOperator)
                .to_matchable()
                .into(),
        ),
        (
            "InlinePathOperatorSegment".into(),
            StringParser::new("->>", SyntaxKind::ColumnPathOperator)
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
            "ColonLiteralSegment".into(),
            TypedParser::new(SyntaxKind::ColonLiteral, SyntaxKind::ColonLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "QuestionLiteralSegment".into(),
            TypedParser::new(SyntaxKind::QuestionLiteral, SyntaxKind::QuestionLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "DollarLiteralSegment".into(),
            TypedParser::new(SyntaxKind::DollarLiteral, SyntaxKind::DollarLiteral)
                .to_matchable()
                .into(),
        ),
        (
            "BytesQuotedLiteralSegment".into(),
            TypedParser::new(SyntaxKind::BytesSingleQuote, SyntaxKind::BytesQuotedLiteral)
                .to_matchable()
                .into(),
        ),
    ]);

    sqlite_dialect.replace_grammar(
        "PrimaryKeyGrammar",
        Sequence::new(vec![
            Ref::keyword("PRIMARY").to_matchable(),
            Ref::keyword("KEY").to_matchable(),
            one_of(vec![
                Ref::keyword("ASC").to_matchable(),
                Ref::keyword("DESC").to_matchable(),
            ])
            .config(|this| {
                this.optional();
            })
            .to_matchable(),
            Ref::new("ConflictClauseSegment").optional().to_matchable(),
            Sequence::new(vec![Ref::keyword("AUTOINCREMENT").to_matchable()])
                .config(|this| {
                    this.optional();
                })
                .to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "NumericLiteralSegment",
        one_of(vec![
            TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral).to_matchable(),
            Ref::new("ParameterizedSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "LiteralGrammar",
        ansi_dialect.grammar("LiteralGrammar").copy(
            Some(vec![
                Ref::new("ParameterizedSegment").to_matchable(),
                Ref::new("BytesQuotedLiteralSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    sqlite_dialect.replace_grammar(
        "TemporaryTransientGrammar",
        Ref::new("TemporaryGrammar").to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "DateTimeLiteralGrammar",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("DATE").to_matchable(),
                Ref::keyword("DATETIME").to_matchable(),
            ])
            .to_matchable(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral)
                .to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "BaseExpressionElementGrammar",
        one_of(vec![
            Ref::new("LiteralGrammar").to_matchable(),
            Ref::new("BareFunctionSegment").to_matchable(),
            Ref::new("FunctionSegment").to_matchable(),
            Ref::new("ColumnReferenceSegment").to_matchable(),
            Ref::new("ExpressionSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("DatatypeSegment").to_matchable(),
                Ref::new("LiteralGrammar").to_matchable(),
            ])
            .to_matchable(),
        ])
        .config(|this| {
            this.terminators = vec![
                Ref::new("CommaSegment").to_matchable(),
                Ref::keyword("AS").to_matchable(),
            ];
        })
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "AlterTableOptionsGrammar",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("RENAME").to_matchable(),
                Ref::keyword("TO").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("RENAME").to_matchable(),
                Sequence::new(vec![Ref::keyword("COLUMN").to_matchable()])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                Ref::new("ColumnReferenceSegment").to_matchable(),
                Ref::keyword("TO").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ADD").to_matchable(),
                Sequence::new(vec![Ref::keyword("COLUMN").to_matchable()])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                Ref::new("ColumnDefinitionSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Sequence::new(vec![Ref::keyword("COLUMN").to_matchable()])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                Ref::new("ColumnReferenceSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar("AutoIncrementGrammar", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("CommentClauseSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("IntervalExpressionSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("TimeZoneGrammar", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("FetchClauseSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("TrimParametersGrammar", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar(
        "LikeGrammar",
        Sequence::new(vec![Ref::keyword("LIKE").to_matchable()]).to_matchable(),
    );

    sqlite_dialect.replace_grammar("OverlapsClauseSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("MLTableExpressionSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("MergeIntoLiteralGrammar", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar("SamplingExpressionSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar(
        "BinaryOperatorGrammar",
        ansi_dialect.grammar("BinaryOperatorGrammar").copy(
            Some(vec![
                Ref::new("ColumnPathOperatorSegment").to_matchable(),
                Ref::new("InlinePathOperatorSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    sqlite_dialect.replace_grammar(
        "OrderByClauseTerminators",
        one_of(vec![
            Ref::keyword("LIMIT").to_matchable(),
            Ref::keyword("WINDOW").to_matchable(),
            Ref::new("FrameClauseUnitGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "WhereClauseTerminatorGrammar",
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
            Ref::keyword("WINDOW").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "FromClauseTerminatorGrammar",
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
            Ref::keyword("WINDOW").to_matchable(),
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "GroupByClauseTerminatorGrammar",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("ORDER").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("LIMIT").to_matchable(),
            Ref::keyword("HAVING").to_matchable(),
            Ref::keyword("WINDOW").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "PostFunctionGrammar",
        Sequence::new(vec![
            Ref::new("FilterClauseGrammar").optional().to_matchable(),
            Ref::new("OverClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar("IgnoreRespectNullsGrammar", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar(
        "SelectClauseTerminatorGrammar",
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
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "FunctionContentsGrammar",
        AnyNumberOf::new(vec![
            Ref::new("ExpressionSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::new("TrimParametersGrammar").to_matchable(),
                Ref::new("ExpressionSegment")
                    .exclude(Ref::keyword("FROM"))
                    .optional()
                    .to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable(),
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
                Ref::keyword("DISTINCT").optional().to_matchable(),
                one_of(vec![
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
            Ref::new("IndexColumnDefinitionSegment").to_matchable(),
            one_of(vec![
                Ref::keyword("IGNORE").to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("ABORT").to_matchable(),
                        Ref::keyword("FAIL").to_matchable(),
                        Ref::keyword("ROLLBACK").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("CommaSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "Expression_A_Unary_Operator_Grammar",
        one_of(vec![
            Ref::new("SignedSegmentGrammar")
                .exclude(Sequence::new(vec![
                    Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
                ]))
                .to_matchable(),
            Ref::new("TildeSegment").to_matchable(),
            Ref::new("NotOperatorGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "IsDistinctFromGrammar",
        Sequence::new(vec![
            Ref::keyword("IS").to_matchable(),
            Ref::keyword("NOT").optional().to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DISTINCT").to_matchable(),
                Ref::keyword("FROM").to_matchable(),
            ])
            .config(|this| {
                this.optional();
            })
            .to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar("NanLiteralSegment", Nothing::new().to_matchable());

    sqlite_dialect.replace_grammar(
        "PatternMatchingGrammar",
        Sequence::new(vec![
            Ref::keyword("NOT").optional().to_matchable(),
            one_of(vec![
                Ref::keyword("GLOB").to_matchable(),
                Ref::keyword("REGEXP").to_matchable(),
                Ref::keyword("MATCH").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "SingleIdentifierGrammar",
        one_of(vec![
            Ref::new("NakedIdentifierSegment").to_matchable(),
            Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
            Ref::new("QuotedIdentifierSegment").to_matchable(),
            Ref::new("BackQuotedIdentifierSegment").to_matchable(),
        ])
        .config(|this| {
            this.terminators = vec![Ref::new("DotSegment").to_matchable()];
        })
        .to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "QuotedIdentifierSegment",
        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier).to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "SingleQuotedIdentifierSegment",
        TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedIdentifier).to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "ColumnConstraintDefaultGrammar",
        Ref::new("ExpressionSegment").to_matchable(),
    );

    sqlite_dialect.replace_grammar(
        "FrameClauseUnitGrammar",
        one_of(vec![
            Ref::keyword("ROWS").to_matchable(),
            Ref::keyword("RANGE").to_matchable(),
            Ref::keyword("GROUPS").to_matchable(),
        ])
        .to_matchable(),
    );

    sqlite_dialect.add([
        (
            "FrameClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FrameClause, |_dialect| {
                Sequence::new(vec![
                    Ref::new("FrameClauseUnitGrammar").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("UNBOUNDED").to_matchable(),
                            Ref::keyword("PRECEDING").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CURRENT").to_matchable(),
                            Ref::keyword("ROW").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::keyword("PRECEDING").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BETWEEN").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("UNBOUNDED").to_matchable(),
                                    Ref::keyword("PRECEDING").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("CURRENT").to_matchable(),
                                    Ref::keyword("ROW").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("ExpressionSegment").to_matchable(),
                                    Ref::keyword("FOLLOWING").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("ExpressionSegment").to_matchable(),
                                    Ref::keyword("PRECEDING").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("AND").to_matchable(),
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("UNBOUNDED").to_matchable(),
                                    Ref::keyword("FOLLOWING").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("CURRENT").to_matchable(),
                                    Ref::keyword("ROW").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("ExpressionSegment").to_matchable(),
                                    Ref::keyword("FOLLOWING").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("ExpressionSegment").to_matchable(),
                                    Ref::keyword("PRECEDING").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXCLUDE").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("NO").to_matchable(),
                                Ref::keyword("OTHERS").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CURRENT").to_matchable(),
                                Ref::keyword("ROW").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("TIES").to_matchable(),
                            Ref::keyword("GROUP").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
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
            "ParameterizedSegment".into(),
            NodeMatcher::new(SyntaxKind::ParameterizedExpression, |_dialect| {
                one_of(vec![
                    Ref::new("AtSignLiteralSegment").to_matchable(),
                    Ref::new("QuestionMarkSegment").to_matchable(),
                    Ref::new("ColonLiteralSegment").to_matchable(),
                    Ref::new("QuestionLiteralSegment").to_matchable(),
                    Ref::new("DollarLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::SetOperator, |_dialect| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("UNION").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DISTINCT").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
                        ])
                        .config(|this| {
                            this.optional();
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
                .config(|this| {
                    this.exclude = Some(
                        Sequence::new(vec![
                            Ref::keyword("EXCEPT").to_matchable(),
                            Bracketed::new(vec![Anything::new().to_matchable()]).to_matchable(),
                        ])
                        .to_matchable(),
                    );
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnReference, |_dialect| {
                {
                    let dialect = super::ansi::raw_dialect();
                    dialect
                        .grammar("ObjectReferenceSegment")
                        .match_grammar(&dialect)
                        .unwrap()
                }
                .copy(
                    Some(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                {
                                    let dialect = super::ansi::raw_dialect();
                                    dialect
                                        .grammar("ObjectReferenceSegment")
                                        .match_grammar(&dialect)
                                        .unwrap()
                                }
                                .copy(
                                    None,
                                    None,
                                    None,
                                    None,
                                    vec![],
                                    false,
                                ),
                                Ref::new("FunctionSegment").to_matchable(),
                                Ref::new("BareFunctionSegment").to_matchable(),
                                Ref::new("LiteralGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ]),
                    None,
                    None,
                    None,
                    vec![],
                    false,
                )
            })
            .to_matchable()
            .into(),
        ),
        (
            "TableReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |_dialect| {
                {
                    let dialect = super::ansi::raw_dialect();
                    dialect
                        .grammar("ObjectReferenceSegment")
                        .match_grammar(&dialect)
                        .unwrap()
                }
                .copy(
                    Some(vec![
                        Sequence::new(vec![
                            {
                                let dialect = super::ansi::raw_dialect();
                                dialect
                                    .grammar("ObjectReferenceSegment")
                                    .match_grammar(&dialect)
                                    .unwrap()
                            }
                            .copy(
                                None,
                                None,
                                None,
                                None,
                                vec![],
                                false,
                            ),
                            one_of(vec![
                                Ref::new("ColumnPathOperatorSegment").to_matchable(),
                                Ref::new("InlinePathOperatorSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![Ref::new("LiteralGrammar").to_matchable()]).to_matchable(),
                        ])
                        .to_matchable(),
                    ]),
                    None,
                    None,
                    None,
                    vec![],
                    false,
                )
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_dialect| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("DOUBLE").to_matchable(),
                        Ref::keyword("PRECISION").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("UNSIGNED").to_matchable(),
                        Ref::keyword("BIG").to_matchable(),
                        Ref::keyword("INT").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("VARYING").to_matchable(),
                                    Ref::keyword("NATIVE").to_matchable(),
                                ])
                                .to_matchable(),
                                one_of(vec![Ref::keyword("CHARACTER").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![Ref::keyword("CHARACTER").to_matchable()])
                                    .to_matchable(),
                                one_of(vec![
                                    Ref::keyword("VARYING").to_matchable(),
                                    Ref::keyword("NATIVE").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("DatatypeIdentifierSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("BracketedArguments").optional().to_matchable(),
                        one_of(vec![Ref::keyword("UNSIGNED").to_matchable()])
                            .config(|this| {
                                this.optional();
                            })
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
            "TableEndClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::TableEndClauseSegment, |_dialect| {
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("WITHOUT").to_matchable(),
                        Ref::keyword("ROWID").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("STRICT").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ValuesClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ValuesClause, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("VALUES").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| {
                                this.parse_mode(ParseMode::Greedy);
                            })
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
            "IndexColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexColumnDefinition, |_dialect| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ASC").to_matchable(),
                        Ref::keyword("DESC").to_matchable(),
                    ])
                    .config(|this| {
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
            "ReturningClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ReturningClause, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("RETURNING").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Delimited::new(vec![
                        Ref::new("WildcardExpressionSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::new("AliasExpressionSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
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
            "ConflictTargetSegment".into(),
            NodeMatcher::new(SyntaxKind::ConflictTarget, |_dialect| {
                Sequence::new(vec![
                    Delimited::new(vec![
                        Ref::new("IndexColumnDefinitionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHERE").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|this| {
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
            "UpsertClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::UpsertClause, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("CONFLICT").to_matchable(),
                    Ref::new("ConflictTargetSegment").optional().to_matchable(),
                    Ref::keyword("DO").to_matchable(),
                    one_of(vec![
                        Ref::keyword("NOTHING").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("UPDATE").to_matchable(),
                            Ref::keyword("SET").to_matchable(),
                            MetaSegment::indent().to_matchable(),
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    one_of(vec![
                                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        Ref::new("BracketedColumnReferenceListGrammar")
                                            .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            MetaSegment::dedent().to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("WHERE").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|this| {
                                this.optional();
                            })
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
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_dialect| {
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("INSERT").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("OR").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ABORT").to_matchable(),
                                    Ref::keyword("FAIL").to_matchable(),
                                    Ref::keyword("IGNORE").to_matchable(),
                                    Ref::keyword("REPLACE").to_matchable(),
                                    Ref::keyword("ROLLBACK").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| {
                                this.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("INTO").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("ValuesClauseSegment").to_matchable(),
                            Ref::new("UpsertClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            optionally_bracketed(vec![
                                Ref::new("SelectableGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("UpsertClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DefaultValuesGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ReturningClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ConflictClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ConflictClause, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("CONFLICT").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ROLLBACK").to_matchable(),
                        Ref::keyword("ABORT").to_matchable(),
                        Ref::keyword("FAIL").to_matchable(),
                        Ref::keyword("IGNORE").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
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
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_dialect| {
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("NOT").optional().to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                            Ref::new("ConflictClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CHECK").to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            Ref::new("ColumnConstraintDefaultGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("PrimaryKeyGrammar").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("UniqueKeyGrammar").to_matchable(),
                            Ref::new("ConflictClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("AutoIncrementGrammar").to_matchable(),
                        Ref::new("ReferenceDefinitionGrammar").to_matchable(),
                        Ref::new("CommentClauseSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("COLLATE").to_matchable(),
                            Ref::new("CollationReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("GENERATED").to_matchable(),
                                Ref::keyword("ALWAYS").to_matchable(),
                            ])
                            .config(|this| {
                                this.optional();
                            })
                            .to_matchable(),
                            Ref::keyword("AS").to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                            one_of(vec![
                                Ref::keyword("STORED").to_matchable(),
                                Ref::keyword("VIRTUAL").to_matchable(),
                            ])
                            .config(|this| {
                                this.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFERRABLE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("DEFERRABLE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("INITIALLY").to_matchable(),
                            Ref::keyword("DEFERRED").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INITIALLY").to_matchable(),
                            Ref::keyword("IMMEDIATE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
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
            "TableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::TableConstraint, |_dialect| {
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("CHECK").to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("UNIQUE").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            Ref::new("ConflictClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("PrimaryKeyGrammar").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            Ref::new("ConflictClauseSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ForeignKeyGrammar").to_matchable(),
                            Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                            Ref::new("ReferenceDefinitionGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFERRABLE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("DEFERRABLE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("INITIALLY").to_matchable(),
                            Ref::keyword("DEFERRED").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INITIALLY").to_matchable(),
                            Ref::keyword("IMMEDIATE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
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
            "TransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::TransactionStatement, |_dialect| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("BEGIN").to_matchable(),
                        Ref::keyword("COMMIT").to_matchable(),
                        Ref::keyword("ROLLBACK").to_matchable(),
                        Ref::keyword("END").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![Ref::keyword("TRANSACTION").to_matchable()])
                        .config(|this| {
                            this.optional();
                        })
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TO").to_matchable(),
                        Ref::keyword("SAVEPOINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|this| {
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
            "PragmaReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::PragmaReference, |_dialect| {
                let dialect = super::ansi::raw_dialect();
                dialect
                    .grammar("ObjectReferenceSegment")
                    .match_grammar(&dialect)
                    .unwrap()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PragmaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PragmaStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("PRAGMA").to_matchable(),
                    Ref::new("PragmaReferenceSegment").to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            one_of(vec![
                                Ref::new("LiteralGrammar").to_matchable(),
                                Ref::new("BooleanLiteralGrammar").to_matchable(),
                                Ref::keyword("YES").to_matchable(),
                                Ref::keyword("NO").to_matchable(),
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
                                Ref::keyword("NONE").to_matchable(),
                                Ref::keyword("FULL").to_matchable(),
                                Ref::keyword("INCREMENTAL").to_matchable(),
                                Ref::keyword("DELETE").to_matchable(),
                                Ref::keyword("TRUNCATE").to_matchable(),
                                Ref::keyword("PERSIST").to_matchable(),
                                Ref::keyword("MEMORY").to_matchable(),
                                Ref::keyword("WAL").to_matchable(),
                                Ref::keyword("NORMAL").to_matchable(),
                                Ref::keyword("EXCLUSIVE").to_matchable(),
                                Ref::keyword("FAST").to_matchable(),
                                Ref::keyword("EXTRA").to_matchable(),
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::keyword("FILE").to_matchable(),
                                Ref::keyword("PASSIVE").to_matchable(),
                                Ref::keyword("RESTART").to_matchable(),
                                Ref::keyword("RESET").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![Ref::new("ObjectReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("EqualsSegment").to_matchable(),
                        optionally_bracketed(vec![
                            one_of(vec![
                                Ref::new("LiteralGrammar").to_matchable(),
                                Ref::new("BooleanLiteralGrammar").to_matchable(),
                                Ref::keyword("YES").to_matchable(),
                                Ref::keyword("NO").to_matchable(),
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
                                Ref::keyword("NONE").to_matchable(),
                                Ref::keyword("FULL").to_matchable(),
                                Ref::keyword("INCREMENTAL").to_matchable(),
                                Ref::keyword("DELETE").to_matchable(),
                                Ref::keyword("TRUNCATE").to_matchable(),
                                Ref::keyword("PERSIST").to_matchable(),
                                Ref::keyword("MEMORY").to_matchable(),
                                Ref::keyword("WAL").to_matchable(),
                                Ref::keyword("NORMAL").to_matchable(),
                                Ref::keyword("EXCLUSIVE").to_matchable(),
                                Ref::keyword("FAST").to_matchable(),
                                Ref::keyword("EXTRA").to_matchable(),
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::keyword("FILE").to_matchable(),
                                Ref::keyword("PASSIVE").to_matchable(),
                                Ref::keyword("RESTART").to_matchable(),
                                Ref::keyword("RESET").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
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
            "CreateTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTriggerStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("TemporaryGrammar").optional().to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TriggerReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BEFORE").to_matchable(),
                        Ref::keyword("AFTER").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INSTEAD").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("DELETE").to_matchable(),
                        Ref::keyword("INSERT").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("UPDATE").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("OF").to_matchable(),
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
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
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("EACH").to_matchable(),
                        Ref::keyword("ROW").to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHEN").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("BEGIN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Delimited::new(vec![
                        Ref::new("UpdateStatementSegment").to_matchable(),
                        Ref::new("InsertStatementSegment").to_matchable(),
                        Ref::new("DeleteStatementSegment").to_matchable(),
                        Ref::new("SelectableGrammar").to_matchable(),
                    ])
                    .config(|this| {
                        this.allow_trailing();
                        this.delimiter(
                            AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").to_matchable()])
                                .config(|this| {
                                    this.min_times(1);
                                }),
                        );
                    })
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateViewStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("TemporaryGrammar").optional().to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
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
            "UnorderedSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::new("SelectClauseSegment").to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                    Ref::new("GroupByClauseSegment").optional().to_matchable(),
                    Ref::new("HavingClauseSegment").optional().to_matchable(),
                    Ref::new("OverlapsClauseSegment").optional().to_matchable(),
                    Ref::new("NamedWindowSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("DELETE").to_matchable(),
                    Ref::new("FromClauseSegment").to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                    Ref::new("ReturningClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UpdateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("UPDATE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        one_of(vec![
                            Ref::keyword("ABORT").to_matchable(),
                            Ref::keyword("FAIL").to_matchable(),
                            Ref::keyword("IGNORE").to_matchable(),
                            Ref::keyword("REPLACE").to_matchable(),
                            Ref::keyword("ROLLBACK").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("AliasExpressionSegment").optional().to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::new("SetClauseListSegment").to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                    Ref::new("ReturningClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SetClause, |_dialect| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_dialect| {
                get_unordered_select_statement_segment_grammar().copy(
                    Some(vec![
                        Ref::new("OrderByClauseSegment").optional().to_matchable(),
                        Ref::new("FetchClauseSegment").optional().to_matchable(),
                        Ref::new("LimitClauseSegment").optional().to_matchable(),
                        Ref::new("NamedWindowSegment").optional().to_matchable(),
                    ]),
                    None,
                    None,
                    None,
                    vec![],
                    false,
                )
            })
            .to_matchable()
            .into(),
        ),
        (
            "GroupingSetsClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::GroupingSetsClause, |_dialect| {
                Nothing::new().to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("UNIQUE").optional().to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("IndexReferenceSegment").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("IndexColumnDefinitionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("WhereClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateVirtualTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateVirtualTableStatement, |_dialect| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("VIRTUAL").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::keyword("USING").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| {
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
            "StatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, |_dialect| {
                one_of(vec![
                    Ref::new("AlterTableStatementSegment").to_matchable(),
                    Ref::new("CreateIndexStatementSegment").to_matchable(),
                    Ref::new("CreateTableStatementSegment").to_matchable(),
                    Ref::new("CreateVirtualTableStatementSegment").to_matchable(),
                    Ref::new("CreateTriggerStatementSegment").to_matchable(),
                    Ref::new("CreateViewStatementSegment").to_matchable(),
                    Ref::new("DeleteStatementSegment").to_matchable(),
                    Ref::new("DropIndexStatementSegment").to_matchable(),
                    Ref::new("DropTableStatementSegment").to_matchable(),
                    Ref::new("DropTriggerStatementSegment").to_matchable(),
                    Ref::new("DropViewStatementSegment").to_matchable(),
                    Ref::new("ExplainStatementSegment").to_matchable(),
                    Ref::new("InsertStatementSegment").to_matchable(),
                    Ref::new("PragmaStatementSegment").to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                    Ref::new("TransactionStatementSegment").to_matchable(),
                    Ref::new("UpdateStatementSegment").to_matchable(),
                    Bracketed::new(vec![Ref::new("StatementSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sqlite_dialect
}

pub fn get_unordered_select_statement_segment_grammar() -> Matchable {
    Sequence::new(vec![
        Ref::new("SelectClauseSegment").to_matchable(),
        Ref::new("FromClauseSegment").optional().to_matchable(),
        Ref::new("WhereClauseSegment").optional().to_matchable(),
        Ref::new("GroupByClauseSegment").optional().to_matchable(),
        Ref::new("HavingClauseSegment").optional().to_matchable(),
        Ref::new("OverlapsClauseSegment").optional().to_matchable(),
        Ref::new("NamedWindowSegment").optional().to_matchable(),
    ])
    .to_matchable()
}
