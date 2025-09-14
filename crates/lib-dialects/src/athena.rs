// The AWS Athena dialect.
// https://docs.aws.amazon.com/athena/latest/ug/what-is.html

use itertools::Itertools;
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
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::vec_of_erased;

pub fn dialect() -> Dialect {
    let ansi_dialect = super::ansi::dialect();
    let mut dialect = super::ansi::raw_dialect();
    dialect.name = DialectKind::Athena;

    dialect
        .sets_mut("unreserved_keywords")
        .extend(super::athena_keywords::ATHENA_UNRESERVED_KEYWORDS);
    dialect
        .sets_mut("reserved_keywords")
        .extend(super::athena_keywords::ATHENA_RESERVED_KEYWORDS);

    dialect.insert_lexer_matchers(
        // Array Operations: https://prestodb.io/docs/0.217/functions/array.html
        vec![Matcher::string("right_arrow", "->", SyntaxKind::RightArrow)],
        "like_operator",
    );

    dialect
        .bracket_sets_mut("angle_bracket_pairs")
        .extend(vec![(
            "angle",
            "StartAngleBracketSegment",
            "EndAngleBracketSegment",
            false,
        )]);

    dialect.add([
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
            "RightArrowOperator".into(),
            StringParser::new("->", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        (
            "JSONFILE".into(),
            StringParser::new("JSONFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "RCFILE".into(),
            StringParser::new("RCFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "ORC".into(),
            StringParser::new("ORCFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "PARQUET".into(),
            StringParser::new("PARQUETFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "AVRO".into(),
            StringParser::new("AVROFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "ION".into(),
            StringParser::new("IONFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "SEQUENCEFILE".into(),
            StringParser::new("SEQUENCEFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
        ),
        (
            "TEXTFILE".into(),
            StringParser::new("TEXTFILE", SyntaxKind::FileFormat)
                .to_matchable()
                .into(),
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
                    Ref::keyword("FORMAT"),
                    Ref::keyword("PARTITIONED_BY"),
                    Ref::keyword("BUCKETED_BY"),
                    Ref::keyword("BUCKET_COUNT"),
                    Ref::keyword("WRITE_COMPRESSION"),
                    Ref::keyword("ORC_COMPRESSION"),
                    Ref::keyword("PARQUET_COMPRESSION"),
                    Ref::keyword("COMPRESSION_LEVEL"),
                    Ref::keyword("FIELD_DELIMITER"),
                    Ref::keyword("IS_EXTERNAL"),
                    Ref::keyword("TABLE_TYPE"),
                    Ref::keyword("EXTERNAL_LOCATION")
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
                    Ref::keyword("FORMAT"),
                    Ref::keyword("PARTITIONED_BY"),
                    Ref::keyword("BUCKETED_BY"),
                    Ref::keyword("BUCKET_COUNT"),
                    Ref::keyword("WRITE_COMPRESSION"),
                    Ref::keyword("ORC_COMPRESSION"),
                    Ref::keyword("PARQUET_COMPRESSION"),
                    Ref::keyword("COMPRESSION_LEVEL"),
                    Ref::keyword("FIELD_DELIMITER"),
                    Ref::keyword("IS_EXTERNAL"),
                    Ref::keyword("TABLE_TYPE"),
                    // Iceberg-specific properties
                    Ref::keyword("LOCATION"),
                    Ref::keyword("PARTITIONING"),
                    Ref::keyword("VACUUM_MAX_SNAPSHOT_AGE_SECONDS"),
                    Ref::keyword("VACUUM_MIN_SNAPSHOTS_TO_KEEP"),
                    Ref::keyword("OPTIMIZE_REWRITE_MIN_DATA_FILE_SIZE_BYTES"),
                    Ref::keyword("OPTIMIZE_REWRITE_MAX_DATA_FILE_SIZE_BYTES"),
                    Ref::keyword("OPTIMIZE_REWRITE_DATA_FILE_THRESHOLD"),
                    Ref::keyword("OPTIMIZE_REWRITE_DELETE_FILE_THRESHOLD")
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
                    Ref::keyword("FORMAT"),
                    Ref::keyword("PARTITIONED_BY"),
                    Ref::keyword("COMPRESSION"),
                    Ref::keyword("FIELD_DELIMITER")
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
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("EqualsSegment"),
                            Ref::new("LiteralGrammar")
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])
                ])])
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
        (
            "TrimParametersGrammar".into(),
            Nothing::new().to_matchable().into(),
        ),
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({pattern})$");

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
                        .config(|config| config.optional()),
                        Ref::new("WithinGroupClauseSegment")
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
            "FunctionContentsGrammar".into(),
            ansi_dialect
                .grammar("FunctionContentsGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("ListaggOverflowClauseSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
    ]);

    // Add support for WITHIN GROUP and LISTAGG overflow clauses
    dialect.add([
        (
            "WithinGroupClauseSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITHIN"),
                Ref::keyword("GROUP"),
                Bracketed::new(vec_of_erased![Ref::new("OrderByClauseSegment")]),
                Ref::new("FilterClauseGrammar").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ListaggOverflowClauseSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("OVERFLOW"),
                one_of(vec_of_erased![
                    Ref::keyword("ERROR"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TRUNCATE"),
                        Ref::new("QuotedLiteralSegment").optional(),
                        one_of(vec_of_erased![
                            Ref::keyword("WITH"),
                            Ref::keyword("WITHOUT")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Ref::keyword("COUNT").optional()
                    ]),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ValuesClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ValuesClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("VALUES"),
                    Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
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
            NodeMatcher::new(SyntaxKind::MapType, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAP"),
                    Ref::new("MapTypeSchemaSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MapTypeSchemaSegment".into(),
            NodeMatcher::new(SyntaxKind::MapTypeSchema, |_| {
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("PrimitiveTypeSegment"),
                    Ref::new("CommaSegment"),
                    Ref::new("DatatypeSegment")
                ])])
                .config(|config| {
                    config.bracket_pairs_set = "angle_bracket_pairs";
                    config.bracket_type = "angle";
                })
                .to_matchable()
            })
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
            NodeMatcher::new(SyntaxKind::StructTypeSchema, |_| {
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"),
                        Ref::new("ColonSegment"),
                        Ref::new("DatatypeSegment"),
                        Ref::new("CommentGrammar").optional()
                    ])
                ])])
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
            "PrimitiveTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::PrimitiveType, |_| {
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
                    Ref::keyword("HYPERLOGLOG"),
                    Ref::keyword("P4HYPERLOGLOG")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_| {
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
                    Ref::new("TimeWithTZGrammar")
                ])
                .to_matchable()
            })
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
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
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
                                        one_of(vec_of_erased![
                                            // External tables expect types...
                                            Ref::new("ColumnDefinitionSegment"),
                                            // Iceberg tables don't expect types.
                                            Ref::new("SingleIdentifierGrammar"),
                                            // Iceberg tables also allow partition transforms
                                            Ref::new("FunctionSegment"),
                                        ]),
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "MsckRepairTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::MsckRepairTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("MSCK"),
                    Ref::keyword("REPAIR"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RowFormatClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::RowFormatClause, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UnloadStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UnloadStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PrepareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PrepareStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExecuteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ExecuteStatement, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IntervalExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::IntervalExpression, |_| {
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.add([
        (
            "AlterTableDropColumnGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("COLUMN"),
                Ref::new("SingleIdentifierGrammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ShowStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ShowStatement, |_| {
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
                            one_of(vec_of_erased![
                                Ref::keyword("DATABASES"),
                                Ref::keyword("SCHEMAS")
                            ]),
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
                            Bracketed::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                                .config(|config| {
                                    config.optional();
                                })
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
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    dialect.config(|this| this.expand())
}
