use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;

pub fn raw_dialect() -> Dialect {
    let mut hive_dialect = super::ansi::dialect(None);

    hive_dialect.add([
        (
            "CommentGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("COMMENT").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LocationGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("LOCATION").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SerdePropertiesGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("SERDEPROPERTIES").to_matchable(),
                Ref::new("BracketedPropertyListGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StoredAsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("STORED").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("FileFormatGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StoredByGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("STORED").to_matchable(),
                Ref::keyword("BY").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
                Ref::new("SerdePropertiesGrammar").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StorageFormatGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("RowFormatClauseSegment").optional().to_matchable(),
                    Ref::new("StoredAsGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::new("StoredByGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TerminatedByGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("TERMINATED").to_matchable(),
                Ref::keyword("BY").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MsckRepairTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::MsckRepairTableStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("MSCK").to_matchable(),
                    Ref::keyword("REPAIR").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ADD").to_matchable(),
                            Ref::keyword("DROP").to_matchable(),
                            Ref::keyword("SYNC").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("PARTITIONS").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RowFormatClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::RowFormatClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ROW").to_matchable(),
                    Ref::keyword("FORMAT").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("DELIMITED").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FIELDS").to_matchable(),
                                Ref::new("TerminatedByGrammar").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("ESCAPED").to_matchable(),
                                    Ref::keyword("BY").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COLLECTION").to_matchable(),
                                Ref::keyword("ITEMS").to_matchable(),
                                Ref::new("TerminatedByGrammar").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MAP").to_matchable(),
                                Ref::keyword("KEYS").to_matchable(),
                                Ref::new("TerminatedByGrammar").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("LINES").to_matchable(),
                                Ref::new("TerminatedByGrammar").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NULL").to_matchable(),
                                Ref::keyword("DEFINED").to_matchable(),
                                Ref::keyword("AS").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SERDE").to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Ref::new("SerdePropertiesGrammar").optional().to_matchable(),
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
            "StructTypeSchemaSegment".into(),
            NodeMatcher::new(SyntaxKind::StructTypeSchema, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("ColonSegment").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                            Ref::new("CommentGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|_config| {
                        // config.bracket_type = "angle_bracket_pairs";
                    })
                    .to_matchable(),
                ])
                .config(|config| {
                    config.bracket_pairs_set = "angle_bracket_pairs";
                    config.bracket_type = "angle";
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SkewedByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SkewedByClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("SKEWED").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("LiteralGrammar").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                                        .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("STORED").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                        Ref::keyword("DIRECTORIES").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    hive_dialect.replace_grammar(
        "StructTypeSegment",
        Sequence::new(vec![
            Ref::keyword("STRUCT").to_matchable(),
            Ref::new("StructTypeSchemaSegment")
                .optional()
                .to_matchable(),
        ])
        .to_matchable(),
    );

    hive_dialect.replace_grammar(
        "ArrayTypeSegment",
        Sequence::new(vec![
            Ref::keyword("ARRAY").to_matchable(),
            Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()])
                .config(|config| {
                    config.bracket_type = "angle";
                    config.bracket_pairs_set = "angle_bracket_pairs";
                    config.optional();
                })
                .to_matchable(),
        ])
        .to_matchable(),
    );

    hive_dialect
}
