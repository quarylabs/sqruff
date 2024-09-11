// The AWS Athena dialect.
// https://docs.aws.amazon.com/athena/latest/ug/what-is.html

use std::sync::Arc;

use itertools::Itertools;

use super::ansi::NodeMatcher;
use super::SyntaxKind;
use crate::core::dialects::base::Dialect;
use crate::core::dialects::init::DialectKind;
use crate::core::parser::grammar::anyof::{one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Nothing, Ref};
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::Matcher;
use crate::core::parser::parsers::{RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::meta::MetaSegment;
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

pub fn dialect() -> Dialect {
    let ansi_dialect = super::ansi::dialect();
    let mut dialect = super::ansi::raw_dialect();
    dialect.name = DialectKind::Athena;

    dialect
        .sets_mut("unreserved_keywords")
        .extend(super::athena_keywords::ATHENA_UNRESERVED_KEYWORDS);
    dialect.sets_mut("reserved_keywords").extend(super::athena_keywords::ATHENA_RESERVED_KEYWORDS);

    dialect.insert_lexer_matchers(
        // Array Operations: https://prestodb.io/docs/0.217/functions/array.html
        vec![Matcher::string("right_arrow", "->", SyntaxKind::RightArrow)],
        "like_operator",
    );

    dialect.bracket_sets_mut("angle_bracket_pairs").extend(vec![(
        "angle",
        "StartAngleBracketSegment",
        "EndAngleBracketSegment",
        false,
    )]);

    dialect.add([
        (
            "StartAngleBracketSegment".into(),
            StringParser::new("<", SyntaxKind::StartAngleBracket).to_matchable().into(),
        ),
        (
            "EndAngleBracketSegment".into(),
            StringParser::new(">", SyntaxKind::EndAngleBracket).to_matchable().into(),
        ),
        (
            "RightArrowOperator".into(),
            StringParser::new("->", SyntaxKind::BinaryOperator).to_matchable().into(),
        ),
        (
            "JsonfileKeywordSegment".into(),
            StringParser::new("JSONFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "RcfileKeywordSegment".into(),
            StringParser::new("RCFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "OrcKeywordSegment".into(),
            StringParser::new("ORCFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "ParquetKeywordSegment".into(),
            StringParser::new("PARQUETFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "AvroKeywordSegment".into(),
            StringParser::new("AVROFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "IonKeywordSegment".into(),
            StringParser::new("IONFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "SequencefileKeywordSegment".into(),
            StringParser::new("SEQUENCEFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "TextfileKeywordSegment".into(),
            StringParser::new("TEXTFILE", SyntaxKind::FileFormat).to_matchable().into(),
        ),
        (
            "PropertyGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("QuotedLiteralSegment"),
                Ref::new("EqualsSegment"),
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
            "BracketedPropertyListGrammar".into(),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "PropertyGrammar"
            )])])
            .to_matchable()
            .into(),
        ),
        (
            "CTASPropertyGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("external_location"),
                    Ref::keyword("format"),
                    Ref::keyword("partitioned_by"),
                    Ref::keyword("bucketed_by"),
                    Ref::keyword("bucket_count"),
                    Ref::keyword("write_compression"),
                    Ref::keyword("orc_compression"),
                    Ref::keyword("parquet_compression"),
                    Ref::keyword("field_delimiter"),
                    Ref::keyword("location")
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CTASIcebergPropertyGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("external_location"),
                    Ref::keyword("format"),
                    Ref::keyword("partitioned_by"),
                    Ref::keyword("bucketed_by"),
                    Ref::keyword("bucket_count"),
                    Ref::keyword("write_compression"),
                    Ref::keyword("orc_compression"),
                    Ref::keyword("parquet_compression"),
                    Ref::keyword("field_delimiter"),
                    Ref::keyword("location"),
                    Ref::keyword("is_external"),
                    Ref::keyword("table_type"),
                    Ref::keyword("partitioning"),
                    Ref::keyword("vacuum_max_snapshot_age_ms"),
                    Ref::keyword("vacuum_min_snapshots_to_keep")
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BracketedCTASPropertyGrammar".into(),
            Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("CTASPropertyGrammar")]),
                Delimited::new(vec_of_erased![Ref::new("CTASIcebergPropertyGrammar")])
            ])])
            .to_matchable()
            .into(),
        ),
        (
            "UnloadPropertyGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("format"),
                    Ref::keyword("partitioned_by"),
                    Ref::keyword("compression"),
                    Ref::keyword("field_delimiter")
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BracketedUnloadPropertyGrammar".into(),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "UnloadPropertyGrammar"
            )])])
            .to_matchable()
            .into(),
        ),
        (
            "TablePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TBLPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
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
            "FileFormatGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("SEQUENCEFILE"),
                Ref::keyword("TEXTFILE"),
                Ref::keyword("RCFILE"),
                Ref::keyword("ORC"),
                Ref::keyword("PARQUET"),
                Ref::keyword("AVRO"),
                Ref::keyword("JSONFILE"),
                Ref::keyword("ION"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INPUTFORMAT"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::keyword("OUTPUTFORMAT"),
                    Ref::new("QuotedLiteralSegment")
                ])
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
            "CommentGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMMENT"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PartitionSpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("PARTITION"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("EqualsSegment"),
                            Ref::new("LiteralGrammar")
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ]
                )])])
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
            "DatetimeWithTZSegment".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("TIMESTAMP"), Ref::keyword("TIME")]),
                Ref::keyword("WITH"),
                Ref::keyword("TIME"),
                Ref::keyword("ZONE")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "LiteralGrammar".into(),
            ansi_dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("ParameterSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "AccessorGrammar".into(),
            Sequence::new(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![Ref::new("ArrayAccessorSegment")]).config(
                    |config| {
                        config.optional();
                    }
                ),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("ObjectReferenceDelimiterGrammar"),
                    Ref::new("ObjectReferenceSegment")
                ])])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral),
                TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedLiteral),
                TypedParser::new(SyntaxKind::BackQuote, SyntaxKind::QuotedLiteral)
            ])
            .to_matchable()
            .into(),
        ),
        ("TrimParametersGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({})$", pattern);

                RegexParser::new("[A-Z0-9_]*[A-Z_][A-Z0-9_]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
            .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            ansi_dialect
                .grammar("SingleIdentifierGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("BackQuotedIdentifierSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "BinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("ArithmeticBinaryOperatorGrammar"),
                Ref::new("StringBinaryOperatorGrammar"),
                Ref::new("BooleanBinaryOperatorGrammar"),
                Ref::new("ComparisonOperatorGrammar"),
                Ref::new("RightArrowOperator")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            ansi_dialect
                .grammar("PostFunctionGrammar")
                .copy(
                    Some(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Ref::keyword("ORDINALITY")
                        ])
                        .config(|config| config.optional())
                    ]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
    ]);

    dialect.replace_grammar(
        "ArrayTypeSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ARRAY"),
            Ref::new("ArrayTypeSchemaSegment").optional()
        ])
        .to_matchable(),
    );

    dialect.replace_grammar(
        "ArrayTypeSchemaSegment",
        Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")])
            .config(|config| {
                config.bracket_pairs_set = "angle_bracket_pairs";
                config.bracket_type = "angle";
            })
            .to_matchable(),
    );

    dialect.replace_grammar(
        "StructTypeSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("STRUCT"),
            Ref::new("StructTypeSchemaSegment").optional()
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "MapTypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MapType,
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAP"),
                    Ref::new("MapTypeSchemaSegment").optional()
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "MapTypeSchemaSegment".into(),
            NodeMatcher::new(
                SyntaxKind::MapTypeSchema,
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("PrimitiveTypeSegment"),
                    Ref::new("CommaSegment"),
                    Ref::new("DatatypeSegment")
                ])])
                .config(|config| {
                    config.bracket_pairs_set = "angle_bracket_pairs";
                    config.bracket_type = "angle";
                })
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "StatementSegment",
        super::ansi::statement_segment().copy(
            Some(vec_of_erased![
                Ref::new("MsckRepairTableStatementSegment"),
                Ref::new("UnloadStatementSegment"),
                Ref::new("PrepareStatementSegment"),
                Ref::new("ExecuteStatementSegment"),
                Ref::new("ShowStatementSegment"),
            ]),
            None,
            None,
            Some(vec_of_erased![
                Ref::new("TransactionStatementSegment"),
                Ref::new("CreateSchemaStatementSegment"),
                Ref::new("SetSchemaStatementSegment"),
                Ref::new("CreateModelStatementSegment"),
                Ref::new("DropModelStatementSegment"),
            ]),
            Vec::new(),
            false,
        ),
    );

    dialect.add([
        (
            "StructTypeSchemaSegment".into(),
            NodeMatcher::new(
                SyntaxKind::StructTypeSchema,
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("NakedIdentifierSegment"),
                        Ref::new("ColonSegment"),
                        Ref::new("DatatypeSegment"),
                        Ref::new("CommentGrammar").optional()
                    ]
                )])])
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
            "PrimitiveTypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PrimitiveType,
                one_of(vec_of_erased![
                    Ref::keyword("BOOLEAN"),
                    Ref::keyword("TINYINT"),
                    Ref::keyword("SMALLINT"),
                    Ref::keyword("INTEGER"),
                    Ref::keyword("INT"),
                    Ref::keyword("BIGINT"),
                    Ref::keyword("DOUBLE"),
                    Ref::keyword("FLOAT"),
                    Ref::keyword("REAL"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("DECIMAL"),
                            Ref::keyword("CHAR"),
                            Ref::keyword("VARCHAR")
                        ]),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    Ref::keyword("STRING"),
                    Ref::keyword("BINARY"),
                    Ref::keyword("DATE"),
                    Ref::keyword("TIMESTAMP"),
                    Ref::keyword("VARBINARY"),
                    Ref::keyword("JSON"),
                    Ref::keyword("TIME"),
                    Ref::keyword("IPADDRESS"),
                    Ref::keyword("HyperLogLog"),
                    Ref::keyword("P4HyperLogLog")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(
                SyntaxKind::DataType,
                one_of(vec_of_erased![
                    Ref::new("PrimitiveTypeSegment"),
                    Ref::new("StructTypeSegment"),
                    Ref::new("ArrayTypeSegment"),
                    Ref::new("MapTypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ROW"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            AnyNumberOf::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("NakedIdentifierSegment"),
                                    Ref::new("DatatypeSegment")
                                ]),
                                Ref::new("LiteralGrammar")
                            ])
                        ])])
                    ]),
                    Ref::new("DatetimeWithTZSegment")
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("GROUP"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("CubeRollupClauseSegment"),
                Ref::new("GroupingSetsClauseSegment"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("NumericLiteralSegment"),
                Ref::new("ExpressionSegment")
            ])]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    dialect.add([
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::CreateTableStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("EXTERNAL").optional(),
                    Ref::keyword("TABLE"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
                                    Ref::new("TableConstraintSegment").optional(),
                                    Sequence::new(vec_of_erased![
                                        Ref::new("ColumnDefinitionSegment"),
                                        Ref::new("CommentGrammar").optional()
                                    ])
                                ]
                            )])])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::new("CommentGrammar").optional(),
                            Ref::new("StoredAsGrammar").optional(),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITIONED"),
                                Ref::keyword("BY"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::new("ColumnDefinitionSegment"),
                                        Ref::new("CommentGrammar").optional()
                                    ])
                                ])])
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CLUSTERED"),
                                Ref::keyword("BY"),
                                Ref::new("BracketedColumnReferenceListGrammar"),
                                Ref::keyword("INTO"),
                                Ref::new("NumericLiteralSegment"),
                                Ref::keyword("BUCKETS")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::new("StoredAsGrammar").optional(),
                            Ref::new("StorageFormatGrammar").optional(),
                            Ref::new("LocationGrammar").optional(),
                            Ref::new("TablePropertiesGrammar").optional(),
                            Ref::new("CommentGrammar").optional()
                        ]),
                        Sequence::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Ref::new("BracketedCTASPropertyGrammar")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::keyword("AS"),
                            optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Ref::keyword("NO"),
                                Ref::keyword("DATA")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ])
                    ])
                ])
                .to_matchable(),
            )
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
                    Ref::new("TableReferenceSegment")
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
            "InsertStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::InsertStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("INTO"),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::keyword("VALUES")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::new("BracketedColumnReferenceListGrammar").optional(),
                            one_of(vec_of_erased![
                                Ref::new("ValuesClauseSegment"),
                                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                            ])
                        ])
                    ])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "UnloadStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::UnloadStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNLOAD"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
                    Ref::keyword("TO"),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::new("BracketedUnloadPropertyGrammar")
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
            "PrepareStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::PrepareStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("PREPARE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::keyword("FROM"),
                    optionally_bracketed(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"),
                        Ref::new("UnloadStatementSegment"),
                        Ref::new("InsertStatementSegment"),
                    ])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "ExecuteStatementSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ExecuteStatement,
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXECUTE"),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Delimited::new(vec_of_erased![Ref::new("LiteralGrammar")])
                    ])])
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
            "IntervalExpressionSegment".into(),
            NodeMatcher::new(
                SyntaxKind::IntervalExpression,
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTERVAL").optional(),
                    one_of(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                        ]),
                        Ref::new("DatetimeUnitSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TO"),
                            Ref::new("DatetimeUnitSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])])
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([(
        "ShowStatementSegment".into(),
        NodeMatcher::new(
            SyntaxKind::ShowStatement,
            Sequence::new(vec_of_erased![
                Ref::keyword("SHOW"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMNS"),
                        one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("DatabaseReferenceSegment"),
                                Ref::new("TableReferenceSegment")
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::new("TableReferenceSegment"),
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("FROM"),
                                        Ref::keyword("IN")
                                    ]),
                                    Ref::new("DatabaseReferenceSegment")
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                            ])
                        ])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CREATE"),
                        one_of(vec_of_erased![Ref::keyword("TABLE"), Ref::keyword("VIEW")]),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("DATABASES"), Ref::keyword("SCHEMAS")]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LIKE"),
                            Ref::new("QuotedLiteralSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PARTITIONS"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TABLES"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("IN"),
                            Ref::new("DatabaseReferenceSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Ref::new("QuotedLiteralSegment").optional()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TBLPROPERTIES"),
                        Ref::new("TableReferenceSegment"),
                        Bracketed::new(vec_of_erased![Ref::new("QuotedLiteralSegment")]).config(
                            |config| {
                                config.optional();
                            }
                        )
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("VIEWS"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("IN"),
                            Ref::new("DatabaseReferenceSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LIKE"),
                            Ref::new("QuotedLiteralSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])
                ])
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    dialect.config(|this| this.expand())
}