use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::base::Ref;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::vec_of_erased;

pub fn raw_dialect() -> Dialect {
    let mut hive_dialect = super::ansi::dialect();

    hive_dialect.add([
        (
            "CommentGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMMENT"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LocationGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("LOCATION"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SerdePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("SERDEPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StoredAsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("STORED"),
                Ref::keyword("AS"),
                Ref::new("FileFormatGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StoredByGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("STORED"),
                Ref::keyword("BY"),
                Ref::new("QuotedLiteralSegment"),
                Ref::new("SerdePropertiesGrammar").optional()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StorageFormatGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("RowFormatClauseSegment").optional(),
                    Ref::new("StoredAsGrammar").optional()
                ]),
                Ref::new("StoredByGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TerminatedByGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TERMINATED"),
                Ref::keyword("BY"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MsckRepairTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MsckRepairTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("MSCK"),
                    Ref::keyword("REPAIR"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("ADD"),
                            Ref::keyword("DROP"),
                            Ref::keyword("SYNC")
                        ]),
                        Ref::keyword("PARTITIONS")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "RowFormatClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::RowFormatClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROW"),
                    Ref::keyword("FORMAT"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DELIMITED"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FIELDS"),
                                Ref::new("TerminatedByGrammar"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ESCAPED"),
                                    Ref::keyword("BY"),
                                    Ref::new("QuotedLiteralSegment")
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COLLECTION"),
                                Ref::keyword("ITEMS"),
                                Ref::new("TerminatedByGrammar")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MAP"),
                                Ref::keyword("KEYS"),
                                Ref::new("TerminatedByGrammar")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("LINES"),
                                Ref::new("TerminatedByGrammar")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("NULL"),
                                Ref::keyword("DEFINED"),
                                Ref::keyword("AS"),
                                Ref::new("QuotedLiteralSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SERDE"),
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("SerdePropertiesGrammar").optional()
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "StructTypeSchemaSegment".into(),
            NodeMatcher::new(
                SyntaxKind::StructTypeSchema,
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("ColonSegment"),
                        Ref::new("DatatypeSegment"),
                        Ref::new("CommentGrammar").optional()
                    ])])
                    .config(|_config| {
                        // config.bracket_type = "angle_bracket_pairs";
                    })
                ])
                .config(|config| {
                    config.bracket_pairs_set = "angle_bracket_pairs";
                    config.bracket_type = "angle";
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "SkewedByClauseSegment".into(),
            NodeMatcher::new(
                SyntaxKind::SkewedByClause,
                Sequence::new(vec_of_erased![
                    Ref::keyword("SKEWED"),
                    Ref::keyword("BY"),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    Ref::keyword("ON"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("LiteralGrammar"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("LiteralGrammar")
                            ])])
                        ]
                    )])]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("STORED"),
                        Ref::keyword("AS"),
                        Ref::keyword("DIRECTORIES")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    hive_dialect.replace_grammar(
        "StructTypeSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("STRUCT"),
            Ref::new("StructTypeSchemaSegment").optional()
        ])
        .to_matchable(),
    );

    hive_dialect.replace_grammar(
        "ArrayTypeSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ARRAY"),
            Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]).config(|config| {
                config.bracket_type = "angle";
                config.bracket_pairs_set = "angle_bracket_pairs";
                config.optional();
            })
        ])
        .to_matchable(),
    );

    hive_dialect
}
