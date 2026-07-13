use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectConfig, DialectKind};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{StringParser, TypedParser};
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(HiveDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: HiveDialectConfig = config
        .map(HiveDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut hive_dialect = super::ansi::dialect(None);
    hive_dialect.name = DialectKind::Hive;

    // These keywords are referenced by Hive-specific grammars below but are not
    // part of ANSI. Registering them is required when Hive is expanded directly
    // rather than used only as a grammar source by SparkSQL.
    hive_dialect.sets_mut("unreserved_keywords").extend([
        "COLLECTION",
        "DELIMITED",
        "DIRECTORIES",
        "ITEMS",
        "MSCK",
        "PARTITIONS",
        "REPAIR",
        "SERDE",
        "SERDEPROPERTIES",
        "SKEWED",
        "STORED",
        "STRUCT",
        "SYNC",
    ]);

    hive_dialect.update_bracket_sets(
        "angle_bracket_pairs",
        vec![(
            "angle",
            "StartAngleBracketSegment",
            "EndAngleBracketSegment",
            false,
        )],
    );

    hive_dialect.add([
        (
            "QuotedLiteralSegment".into(),
            one_of(vec![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral).to_matchable(),
                TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedLiteral).to_matchable(),
            ])
            .to_matchable()
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
                Ref::new("BackQuotedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StartAngleBracketSegment".into(),
            StringParser::new("<", SyntaxKind::StartAngleBracket)
                .to_matchable()
                .into(),
        ),
        (
            "EndAngleBracketSegment".into(),
            StringParser::new(">", SyntaxKind::EndAngleBracket)
                .to_matchable()
                .into(),
        ),
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

    hive_dialect.add([
        (
            "PrimitiveTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::PrimitiveType, |_| {
                one_of(vec![
                    Ref::keyword("BOOLEAN").to_matchable(),
                    Ref::keyword("TINYINT").to_matchable(),
                    Ref::keyword("SMALLINT").to_matchable(),
                    Ref::keyword("INT").to_matchable(),
                    Ref::keyword("INTEGER").to_matchable(),
                    Ref::keyword("BIGINT").to_matchable(),
                    Ref::keyword("FLOAT").to_matchable(),
                    Ref::keyword("REAL").to_matchable(),
                    Ref::keyword("DOUBLE").to_matchable(),
                    Ref::keyword("DATE").to_matchable(),
                    Ref::keyword("TIMESTAMP").to_matchable(),
                    Ref::keyword("STRING").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("CHAR").to_matchable(),
                            Ref::keyword("VARCHAR").to_matchable(),
                            Ref::keyword("DECIMAL").to_matchable(),
                            Ref::keyword("DEC").to_matchable(),
                            Ref::keyword("NUMERIC").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("BracketedArguments").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("BINARY").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RowTypeSchemaSegment".into(),
            Bracketed::new(vec![
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::new("ParameterNameSegment").to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "RowTypeSegment".into(),
            Sequence::new(vec![
                Ref::keyword("ROW").to_matchable(),
                Ref::new("RowTypeSchemaSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("ColonDelimiterSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("LiteralGrammar").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    hive_dialect.replace_grammar(
        "DatatypeSegment",
        NodeMatcher::new(SyntaxKind::DataType, |_| {
            one_of(vec![
                Ref::new("PrimitiveTypeSegment").to_matchable(),
                Ref::new("SizedArrayTypeSegment").to_matchable(),
                Ref::new("ArrayTypeSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("MAP").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("DatatypeSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|config| {
                        config.bracket_pairs_set = "angle_bracket_pairs";
                        config.bracket_type = "angle";
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("StructTypeSegment").to_matchable(),
                Ref::new("RowTypeSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    hive_dialect.replace_grammar(
        "StatementSegment",
        super::ansi::statement_segment().copy(
            Some(vec![
                Ref::new("MsckRepairTableStatementSegment").to_matchable(),
                Ref::new("SetStatementSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    hive_dialect
}
