use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Nothing, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::StringParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use crate::{ansi, postgres};
use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(DuckDBDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    // Parse and validate dialect configuration, falling back to defaults on failure
    let _dialect_config: DuckDBDialectConfig = config
        .map(DuckDBDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let ansi_dialect = ansi::raw_dialect();
    let postgres_dialect = postgres::dialect(None);
    let postgres_non_set_selectable = postgres_dialect.grammar("NonSetSelectableGrammar");
    let mut duckdb_dialect = postgres_dialect;
    duckdb_dialect.name = DialectKind::Duckdb;

    duckdb_dialect.add_keyword_to_set("reserved_keywords", "SUMMARIZE");
    duckdb_dialect.add_keyword_to_set("reserved_keywords", "MACRO");
    duckdb_dialect.add_keyword_to_set("reserved_keywords", "PIVOT");
    duckdb_dialect.add_keyword_to_set("reserved_keywords", "PIVOT_LONGER");
    duckdb_dialect.add_keyword_to_set("reserved_keywords", "PIVOT_WIDER");
    duckdb_dialect.add_keyword_to_set("reserved_keywords", "UNPIVOT");
    duckdb_dialect.add_keyword_to_set("unreserved_keywords", "ANTI");
    duckdb_dialect.add_keyword_to_set("unreserved_keywords", "ASOF");
    duckdb_dialect.add_keyword_to_set("unreserved_keywords", "POSITIONAL");
    duckdb_dialect.add_keyword_to_set("unreserved_keywords", "SEMI");
    duckdb_dialect.add_keyword_to_set("unreserved_keywords", "VIRTUAL");

    duckdb_dialect.add([
        (
            "LambdaArrowSegment".into(),
            StringParser::new("->", SyntaxKind::LambdaArrow)
                .to_matchable()
                .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("QuotedIdentifierSegment").to_matchable(),
                Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DivideSegment".into(),
            one_of(vec![
                StringParser::new("//", SyntaxKind::BinaryOperator).to_matchable(),
                StringParser::new("/", SyntaxKind::BinaryOperator).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CreateTableAsStatementSegment".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "UnionGrammar".into(),
            ansi_dialect
                .grammar("UnionGrammar")
                .copy(
                    Some(vec![
                        Sequence::new(vec![
                            Ref::keyword("BY").to_matchable(),
                            Ref::keyword("NAME").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "LoadStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("LOAD").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SummarizeStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("SUMMARIZE").to_matchable(),
                one_of(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DescribeStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("DESCRIBE").to_matchable(),
                one_of(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CreateMacroStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                one_of(vec![
                    Ref::keyword("TEMP").to_matchable(),
                    Ref::keyword("TEMPORARY").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("MACRO").to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Ref::new("BaseExpressionElementGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("AS").to_matchable(),
                one_of(vec![
                    Ref::new("SelectStatementSegment").to_matchable(),
                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
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
    ]);

    duckdb_dialect.insert_lexer_matchers(
        vec![Matcher::string(
            "double_divide",
            "//",
            SyntaxKind::DoubleDivide,
        )],
        "divide",
    );

    duckdb_dialect.replace_grammar(
        "JoinLikeClauseGrammar",
        Sequence::new(vec![
            AnyNumberOf::new(vec![
                Ref::new("FromPivotExpressionSegment").to_matchable(),
                Ref::new("FromUnpivotExpressionSegment").to_matchable(),
            ])
            .config(|this| this.min_times = 1)
            .to_matchable(),
            Ref::new("AliasExpressionSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "NonSetSelectableGrammar",
        postgres_non_set_selectable.copy(
            Some(vec![
                Ref::new("SimplifiedPivotExpressionSegment").to_matchable(),
                Ref::new("SimplifiedUnpivotExpressionSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    duckdb_dialect.replace_grammar(
        "NonStandardJoinTypeKeywordsGrammar",
        one_of(vec![
            Ref::keyword("ANTI").to_matchable(),
            Ref::keyword("SEMI").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ASOF").to_matchable(),
                one_of(vec![
                    Ref::new("JoinTypeKeywordsGrammar").to_matchable(),
                    Ref::keyword("ANTI").to_matchable(),
                    Ref::keyword("SEMI").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "HorizontalJoinKeywordsGrammar",
        Ref::keyword("POSITIONAL").to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "FunctionContentsExpressionGrammar",
        one_of(vec![
            Ref::new("LambdaExpressionSegment").to_matchable(),
            Ref::new("NamedArgumentSegment").to_matchable(),
            Ref::new("ExpressionSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "ColumnsExpressionNameGrammar",
        Ref::keyword("COLUMNS").to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "ColumnsExpressionGrammar",
        Sequence::new(vec![
            Ref::new("ColumnsExpressionFunctionNameSegment").to_matchable(),
            Bracketed::new(vec![
                Ref::new("ColumnsExpressionFunctionContentsSegment").to_matchable(),
            ])
            .config(|this| this.parse_mode = ParseMode::Greedy)
            .to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "ColumnConstraintSegment",
        Sequence::new(vec![
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
                    Ref::keyword("DEFAULT").to_matchable(),
                    one_of(vec![
                        Ref::new("LiteralGrammar").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("UNIQUE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("PRIMARY").to_matchable(),
                    Ref::keyword("KEY").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("ReferenceDefinitionGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COLLATE").to_matchable(),
                    Ref::new("CollationReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            Ref::new("TemporaryGrammar").optional().to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                one_of(vec![
                                    Sequence::new(vec![
                                        Ref::new("DatatypeSegment").to_matchable(),
                                        AnyNumberOf::new(vec![
                                            Ref::new("ColumnConstraintSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        Ref::new("DatatypeSegment")
                                            .exclude(Ref::keyword("AS"))
                                            .optional()
                                            .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("GENERATED").to_matchable(),
                                            Ref::keyword("ALWAYS").to_matchable(),
                                        ])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                        Ref::keyword("AS").to_matchable(),
                                        Bracketed::new(vec![
                                            Ref::new("ExpressionSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        one_of(vec![
                                            Ref::keyword("STORED").to_matchable(),
                                            Ref::keyword("VIRTUAL").to_matchable(),
                                        ])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("TableConstraintSegment").to_matchable(),
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

    duckdb_dialect.add([
        (
            "WildcardExcludeExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::WildcardExclude, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXCLUDE").to_matchable(),
                    one_of(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
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
            "WildcardReplaceExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::WildcardReplace, |_| {
                Sequence::new(vec![
                    Ref::keyword("REPLACE").to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                                    Ref::new("AliasExpressionSegment").optional().to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("BaseExpressionElementGrammar").to_matchable(),
                            Ref::new("AliasExpressionSegment").optional().to_matchable(),
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
            "WildcardExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::WildcardExpression, |_| {
                Sequence::new(vec![
                    Ref::new("WildcardIdentifierSegment").to_matchable(),
                    Ref::new("WildcardExcludeExpressionSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("WildcardReplaceExpressionSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ColumnsExpressionFunctionContentsSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnsExpression, |_| {
                one_of(vec![
                    Ref::new("WildcardExpressionSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("LambdaExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LambdaExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::LambdaFunction, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("ParameterNameSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ParameterNameSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("LambdaArrowSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
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
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                                Ref::keyword("IN").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("GroupByClauseSegment").optional().to_matchable(),
                        Ref::new("OrderByClauseSegment").optional().to_matchable(),
                        Ref::new("LimitClauseSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SimplifiedPivotExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SimplifiedPivot, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("PIVOT").to_matchable(),
                        Ref::keyword("PIVOT_WIDER").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("TableExpressionSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ON").to_matchable(),
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("IN").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("FunctionSegment").to_matchable(),
                                Ref::new("AliasExpressionSegment").optional().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("GroupByClauseSegment").optional().to_matchable(),
                    Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    Ref::new("LimitClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "FromUnpivotExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::FromUnpivotExpression, |_| {
                Sequence::new(vec![
                    Ref::keyword("UNPIVOT").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("INCLUDE").to_matchable(),
                        Ref::keyword("NULLS").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                                Ref::keyword("IN").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Sequence::new(vec![
                                            optionally_bracketed(vec![
                                                Delimited::new(vec![
                                                    Ref::new("SingleIdentifierGrammar")
                                                        .to_matchable(),
                                                ])
                                                .to_matchable(),
                                            ])
                                            .to_matchable(),
                                            Ref::new("AliasExpressionSegment")
                                                .optional()
                                                .to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::new("ColumnsExpressionGrammar").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.min_times = 1)
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
            "SimplifiedUnpivotExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SimplifiedUnpivot, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("UNPIVOT").to_matchable(),
                        Ref::keyword("PIVOT_LONGER").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("TableExpressionSegment").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("ColumnReferenceSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("AliasExpressionSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ColumnsExpressionGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("INTO").to_matchable(),
                    Ref::keyword("NAME").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("VALUE").to_matchable(),
                    Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                        .to_matchable(),
                    Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    Ref::new("LimitClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    duckdb_dialect.replace_grammar(
        "SelectClauseElementSegment",
        one_of(vec![
            Ref::new("WildcardExpressionSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("BaseExpressionElementGrammar").to_matchable(),
                Ref::new("AliasExpressionSegment").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::select_statement().copy(
            Some(vec![
                Ref::new("QualifyClauseSegment").optional().to_matchable(),
            ]),
            None,
            Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        ),
    );

    duckdb_dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec![
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("SelectClauseSegment").to_matchable(),
                    Ref::new("FromClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("FromClauseSegment").to_matchable(),
                    Ref::new("SelectClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
            Ref::new("QualifyClauseSegment").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("OrderByClauseSegment").to_matchable(),
            Ref::new("LimitClauseSegment").to_matchable(),
        ])
        .config(|this| this.parse_mode(ParseMode::GreedyOnceStarted))
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "OrderByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("ORDER").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            Delimited::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("ALL").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ASC").to_matchable(),
                        Ref::keyword("DESC").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NULLS").to_matchable(),
                        one_of(vec![
                            Ref::keyword("FIRST").to_matchable(),
                            Ref::keyword("LAST").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| {
                config.allow_trailing = true;
                config.terminators = vec![Ref::new("OrderByClauseTerminators").to_matchable()];
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("GROUP").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            Delimited::new(vec![
                one_of(vec![
                    Ref::keyword("ALL").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| {
                config.allow_trailing = true;
                config.terminators =
                    vec![Ref::new("GroupByClauseTerminatorGrammar").to_matchable()];
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "ObjectLiteralElementSegment",
        Sequence::new(vec![
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("ColonSegment").to_matchable(),
            Ref::new("BaseExpressionElementGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    // DuckDB allows trailing commas in function argument lists, e.g.
    // `list_value(1, 2, 3,)`. Mirror the ansi `FunctionContentsGrammar`
    // but enable `allow_trailing` on the inner argument `Delimited`.
    duckdb_dialect.replace_grammar(
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
                    .optional()
                    .exclude(Ref::keyword("FROM"))
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
                    .config(|config| {
                        config.allow_trailing = true;
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("AggregateOrderByClause").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SEPARATOR").to_matchable(),
                Ref::new("LiteralGrammar").to_matchable(),
            ])
            .to_matchable(),
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
            Ref::new("IgnoreRespectNullsGrammar").to_matchable(),
            Ref::new("IndexColumnDefinitionSegment").to_matchable(),
            Ref::new("EmptyStructLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    duckdb_dialect.replace_grammar(
        "StatementSegment",
        postgres::statement_segment().copy(
            Some(vec![
                Ref::new("SimplifiedPivotExpressionSegment").to_matchable(),
                Ref::new("SimplifiedUnpivotExpressionSegment").to_matchable(),
                Ref::new("LoadStatementSegment").to_matchable(),
                Ref::new("SummarizeStatementSegment").to_matchable(),
                Ref::new("DescribeStatementSegment").to_matchable(),
                Ref::new("CreateMacroStatementSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    duckdb_dialect
}
