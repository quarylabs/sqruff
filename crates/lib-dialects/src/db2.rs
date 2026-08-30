use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{
    AnyNumberOf, any_set_of, one_of, optionally_bracketed,
};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use crate::db2_keywords::UNRESERVED_KEYWORDS;

use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(Db2DialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: Db2DialectConfig =
        config.map(Db2DialectConfig::from_value).unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let ansi_dialect = super::ansi::raw_dialect();
    let mut db2_dialect = super::ansi::dialect(None);
    db2_dialect.name = DialectKind::Db2;

    for kw in UNRESERVED_KEYWORDS {
        db2_dialect.add_keyword_to_set("unreserved_keywords", kw);
    }

    db2_dialect.replace_grammar(
        "FunctionContentsExpressionGrammar",
        one_of(vec![
            Ref::new("ExpressionSegment").to_matchable(),
            Ref::new("NamedArgumentSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    for terminator_grammar in [
        "FromClauseTerminatorGrammar",
        "WhereClauseTerminatorGrammar",
        "GroupByClauseTerminatorGrammar",
        "HavingClauseTerminatorGrammar",
        "OrderByClauseTerminators",
    ] {
        db2_dialect.replace_grammar(
            terminator_grammar,
            ansi_dialect.grammar(terminator_grammar).copy(
                Some(vec![Ref::keyword("OFFSET").to_matchable()]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
        );
    }

    db2_dialect.insert_lexer_matchers(
        vec![Matcher::string("right_arrow", "=>", SyntaxKind::RightArrow)],
        "equals",
    );

    // DB2 allows # in field names, and doesn't use it as a comment.
    db2_dialect.patch_lexer_matchers(vec![
        // Remove hash comments — DB2 only uses -- for inline comments.
        Matcher::regex("inline_comment", r"(--)[^\n]*", SyntaxKind::InlineComment),
        // Allow # in word tokens for identifiers.
        Matcher::regex("word", r"[0-9a-zA-Z_#]+", SyntaxKind::Word),
    ]);

    db2_dialect.add([
        (
            "RightArrowSegment".into(),
            StringParser::new("=>", SyntaxKind::RightArrow)
                .to_matchable()
                .into(),
        ),
        (
            "CallStoredProcedureSegment".into(),
            NodeMatcher::new(SyntaxKind::CallSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("CALL").to_matchable(),
                    one_of(vec![
                        Ref::new("FunctionSegment").to_matchable(),
                        Ref::new("FunctionNameSegment")
                            .reset_terminators()
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
            "CopyOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::CopyOptions, |_| {
                any_set_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("INCLUDING").to_matchable(),
                            Ref::keyword("EXCLUDING").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("COLUMN").optional().to_matchable(),
                        Ref::keyword("DEFAULTS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("INCLUDING").to_matchable(),
                            Ref::keyword("EXCLUDING").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("IDENTITY").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("COLUMN").to_matchable(),
                            Ref::keyword("ATTRIBUTES").to_matchable(),
                        ])
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
            "DeclareGlobalTempTableSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::keyword("GLOBAL").to_matchable(),
                    Ref::keyword("TEMPORARY").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("AS").to_matchable(),
                            optionally_bracketed(vec![
                                Ref::new("SelectableGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("WithDataClauseSegment").to_matchable(),
                            Ref::new("CopyOptionsSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("LIKE").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                            Ref::new("CopyOptionsSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    any_set_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ORGANIZE").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ROW").to_matchable(),
                                Ref::keyword("COLUMN").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("COMMIT").to_matchable(),
                            one_of(vec![
                                Ref::keyword("DELETE").to_matchable(),
                                Ref::keyword("PRESERVE").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("ROWS").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("LOGGED").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NOT").to_matchable(),
                                Ref::keyword("LOGGED").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("ROLLBACK").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("DELETE").to_matchable(),
                                        Ref::keyword("PRESERVE").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("ROWS").to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Ref::keyword("REPLACE").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("IN").to_matchable(),
                            Ref::new("TablespaceReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DeclareDistributionClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeclareDistributionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::DistributionSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DISTRIBUTE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BY").to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("HASH").optional().to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("RANDOM").to_matchable(),
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
        (
            "OffsetClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OffsetClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("OFFSET").to_matchable(),
                    one_of(vec![
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ROW").to_matchable(),
                        Ref::keyword("ROWS").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DB2 allows # in naked identifiers.
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({pattern})$");

                RegexParser::new("[A-Z0-9_#]*[A-Z#][A-Z0-9_#]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
            .into(),
        ),
        // DB2 PostFunctionGrammar: OVER or WITHIN GROUP (no FILTER).
        (
            "PostFunctionGrammar".into(),
            one_of(vec![
                Ref::new("OverClauseSegment").to_matchable(),
                Ref::new("WithinGroupClauseSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // DB2 Expression_C_Grammar: adds duration expressions (e.g. 1 DAYS, 1 DAY).
        (
            "Expression_C_Grammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("EXISTS").to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("Expression_D_Grammar").to_matchable(),
                        Ref::new("CaseExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("TimeZoneGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("ShorthandCastSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DAYS").to_matchable(),
                        Ref::keyword("DAY").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.terminators = vec![Ref::new("CommaSegment").to_matchable()])
            .to_matchable()
            .into(),
        ),
        // WithinGroupClauseSegment for DB2 window functions.
        (
            "WithinGroupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithingroupClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITHIN").to_matchable(),
                    Ref::keyword("GROUP").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    db2_dialect.replace_grammar(
        "LimitClauseSegment",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("LIMIT").to_matchable(),
                MetaSegment::indent().to_matchable(),
                optionally_bracketed(vec![
                    one_of(vec![
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                        Ref::keyword("ALL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("OFFSET").to_matchable(),
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("CommaSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::new("OffsetClauseSegment").optional().to_matchable(),
                Ref::new("FetchClauseSegment").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    db2_dialect.replace_grammar(
        "ValuesClauseSegment",
        Sequence::new(vec![
            Ref::keyword("VALUES").to_matchable(),
            Delimited::new(vec![
                one_of(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::keyword("DEFAULT").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                    .to_matchable(),
                    Ref::keyword("DEFAULT").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    db2_dialect.replace_grammar(
        "StatementSegment",
        super::ansi::statement_segment().copy(
            Some(vec![
                Ref::new("CallStoredProcedureSegment").to_matchable(),
                Ref::new("DeclareGlobalTempTableSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    db2_dialect
}
