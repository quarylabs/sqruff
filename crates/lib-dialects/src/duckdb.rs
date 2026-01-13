use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::parsers::StringParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;

use crate::{ansi, postgres};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let ansi_dialect = ansi::raw_dialect();
    let postgres_dialect = postgres::dialect();
    let mut duckdb_dialect = postgres_dialect;
    duckdb_dialect.name = DialectKind::Duckdb;

    duckdb_dialect.add_keyword_to_set("reserved_keywords", "SUMMARIZE");
    duckdb_dialect.add_keyword_to_set("reserved_keywords", "MACRO");

    duckdb_dialect.add([
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
        "SelectClauseElementSegment",
        one_of(vec![
            Sequence::new(vec![
                Ref::new("WildcardExpressionSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("EXCLUDE").to_matchable(),
                        one_of(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("REPLACE").to_matchable(),
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
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
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

    duckdb_dialect.replace_grammar(
        "StatementSegment",
        postgres::statement_segment().copy(
            Some(vec![
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
