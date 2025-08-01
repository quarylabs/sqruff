use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{
    AnyNumberOf, any_set_of, one_of, optionally_bracketed,
};
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::bracketed::BracketedSegmentMatcher;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use super::sparksql_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::ansi;

pub fn raw_dialect() -> Dialect {
    let ansi_dialect = ansi::raw_dialect();
    let hive_dialect = super::hive::raw_dialect();
    let mut sparksql_dialect = ansi_dialect;
    sparksql_dialect.name = DialectKind::Sparksql;

    sparksql_dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"(--)[^\n]*", SyntaxKind::InlineComment),
        Matcher::regex("equals", r"==|<=>|=", SyntaxKind::RawComparisonOperator),
        Matcher::regex("back_quote", r"`([^`]|``)*`", SyntaxKind::BackQuote),
        Matcher::legacy("numeric_literal", |s| s.starts_with(|ch: char| ch == '.' || ch == '-' || ch.is_ascii_alphanumeric()), r#"(?>(?>\d+\.\d+|\d+\.|\.\d+)([eE][+-]?\d+)?([dDfF]|BD|bd)?|\d+[eE][+-]?\d+([dDfF]|BD|bd)?|\d+([dDfFlLsSyY]|BD|bd)?)((?<=\.)|(?=\b))"#, SyntaxKind::NumericLiteral),
    ]);

    sparksql_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "bytes_single_quote",
                r"X'([^'\\]|\\.)*'",
                SyntaxKind::BytesSingleQuote,
            ),
            Matcher::regex(
                "bytes_double_quote",
                r#"X"([^"\\]|\\.)*""#,
                SyntaxKind::BytesDoubleQuote,
            ),
        ],
        "single_quote",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "bytes_single_quote",
                r"X'([^'\\]|\\.)*'",
                SyntaxKind::BytesSingleQuote,
            ),
            Matcher::regex(
                "bytes_double_quote",
                r#"X"([^"\\]|\\.)*""#,
                SyntaxKind::BytesDoubleQuote,
            ),
        ],
        "single_quote",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::regex(
            "at_sign_literal",
            r"@\w*",
            SyntaxKind::AtSignLiteral,
        )],
        "word",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![
            Matcher::regex("file_literal", r#"[a-zA-Z0-9]*:?([a-zA-Z0-9\-_\.]*(/|\\)){2,}((([a-zA-Z0-9\-_\.]*(:|\?|=|&)[a-zA-Z0-9\-_\.]*)+)|([a-zA-Z0-9\-_\.]*\.[a-z]+))"#, SyntaxKind::FileLiteral),
        ],
        "newline",
    );

    sparksql_dialect.sets_mut("bare_functions").clear();
    sparksql_dialect.sets_mut("bare_functions").extend([
        "CURRENT_DATE",
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
    ]);

    sparksql_dialect.sets_mut("date_part_function_name").clear();
    sparksql_dialect
        .sets_mut("date_part_function_name")
        .extend([
            "DATE_ADD",
            "DATE_DIFF",
            "DATEADD",
            "DATEDIFF",
            "TIMESTAMPADD",
            "TIMESTAMPDIFF",
        ]);

    sparksql_dialect.sets_mut("datetime_units").clear();
    sparksql_dialect.sets_mut("datetime_units").extend([
        "YEAR",
        "YEARS",
        "YYYY",
        "YY",
        "QUARTER",
        "QUARTERS",
        "MONTH",
        "MONTHS",
        "MON",
        "MM",
        "WEEK",
        "WEEKS",
        "DAY",
        "DAYS",
        "DD",
        "HOUR",
        "HOURS",
        "MINUTE",
        "MINUTES",
        "SECOND",
        "SECONDS",
        "MILLISECOND",
        "MILLISECONDS",
        "MICROSECOND",
        "MICROSECONDS",
    ]);

    sparksql_dialect
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);
    sparksql_dialect
        .sets_mut("reserved_keywords")
        .extend(RESERVED_KEYWORDS);

    sparksql_dialect.update_bracket_sets(
        "angle_bracket_pairs",
        vec![(
            "angle",
            "StartAngleBracketSegment",
            "EndAngleBracketSegment",
            false,
        )],
    );

    sparksql_dialect.add([
        (
            "SelectClauseTerminatorGrammar".into(),
            ansi::raw_dialect()
                .grammar("SelectClauseTerminatorGrammar")
                .copy(
                    Some(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("CLUSTER"), Ref::keyword("BY")]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DISTRIBUTE"),
                            Ref::keyword("BY")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("SORT"), Ref::keyword("BY")]),
                        Ref::keyword("QUALIFY"),
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
            "ComparisonOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("EqualsSegment"),
                Ref::new("EqualsSegment_a"),
                Ref::new("EqualsSegment_b"),
                Ref::new("GreaterThanSegment"),
                Ref::new("LessThanSegment"),
                Ref::new("GreaterThanOrEqualToSegment"),
                Ref::new("LessThanOrEqualToSegment"),
                Ref::new("NotEqualToSegment"),
                Ref::new("LikeOperatorSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IS"),
                    Ref::keyword("NOT"),
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("FROM")
                ])
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
                Sequence::new(vec_of_erased![Ref::keyword("CLUSTER"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISTRIBUTE"),
                    Ref::keyword("BY")
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("SORT"), Ref::keyword("BY")]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::new("SetOperatorSegment"),
                Ref::new("WithNoSchemaBindingClauseSegment"),
                Ref::new("WithDataClauseSegment"),
                Ref::keyword("KEYS")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryGrammar".into(),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("GLOBAL")]).config(|config| {
                    config.optional();
                }),
                one_of(vec_of_erased![
                    Ref::keyword("TEMP"),
                    Ref::keyword("TEMPORARY")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral),
                TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedLiteral)
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LiteralGrammar".into(),
            sparksql_dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("BytesQuotedLiteralSegment")]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "NaturalJoinKeywordsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("NATURAL"),
                Ref::new("JoinTypeKeywords").optional()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LikeGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("LIKE"), Ref::keyword("ILIKE")]),
                    one_of(vec_of_erased![
                        Ref::keyword("ALL"),
                        Ref::keyword("ANY"),
                        Ref::keyword("SOME")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ]),
                Ref::keyword("RLIKE"),
                Ref::keyword("REGEXP")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("SingleQuotedIdentifierSegment"),
                Ref::new("BackQuotedIdentifierSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("DISTRIBUTE"),
                        Ref::keyword("GROUP"),
                        Ref::keyword("ORDER"),
                        Ref::keyword("SORT")
                    ]),
                    Ref::keyword("BY")
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISTRIBUTE"),
                    Ref::keyword("BY")
                ]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::keyword("OVERLAPS"),
                Ref::keyword("APPLY")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Ref::keyword("DISTRIBUTE"),
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("SORT")
                    ]),
                    Ref::keyword("BY")
                ]),
                Ref::keyword("LIMIT"),
                Ref::keyword("HAVING"),
                Ref::keyword("WINDOW")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("DISTRIBUTE"),
                        Ref::keyword("SORT")
                    ]),
                    Ref::keyword("BY")
                ]),
                Ref::keyword("LIMIT"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ArithmeticBinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("PlusSegment"),
                Ref::new("MinusSegment"),
                Ref::new("DivideSegment"),
                Ref::new("MultiplySegment"),
                Ref::new("ModuloSegment"),
                Ref::new("BitwiseAndSegment"),
                Ref::new("BitwiseOrSegment"),
                Ref::new("BitwiseXorSegment"),
                Ref::new("BitwiseLShiftSegment"),
                Ref::new("BitwiseRShiftSegment"),
                Ref::new("DivBinaryOperatorSegment")
            ])
            .to_matchable()
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
            "AccessorGrammar".into(),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("ArrayAccessorSegment"),
                Ref::new("SemiStructuredAccessorSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ObjectReferenceTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("AS"),
                Ref::keyword("USING"),
                Ref::new("CommaSegment"),
                Ref::new("CastOperatorSegment"),
                Ref::new("StartSquareBracketSegment"),
                Ref::new("StartBracketSegment"),
                Ref::new("BinaryOperatorGrammar"),
                Ref::new("DelimiterGrammar"),
                Ref::new("JoinLikeClauseGrammar"),
                BracketedSegmentMatcher::new()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("ExpressionSegment"),
                Ref::new("StarSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            // An `IDENTIFIER` clause segment.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-names-identifier-clause.html
            "IdentifierClauseSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("IDENTIFIER"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.add([
        (
            "FileLiteralSegment".into(),
            TypedParser::new(SyntaxKind::FileLiteral, SyntaxKind::FileLiteral)
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
            "NakedSemiStructuredElementSegment".into(),
            RegexParser::new("[A-Z0-9_]*", SyntaxKind::SemiStructuredElement)
                .to_matchable()
                .into(),
        ),
        (
            "QuotedSemiStructuredElementSegment".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::SemiStructuredElement)
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
            "BINARYFILE".into(),
            StringParser::new("BINARYFILE", SyntaxKind::FileFormat)
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
            "EqualsSegment_a".into(),
            StringParser::new("==", SyntaxKind::ComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "EqualsSegment_b".into(),
            StringParser::new("<=>", SyntaxKind::ComparisonOperator)
                .to_matchable()
                .into(),
        ),
        (
            "FILE".into(),
            MultiStringParser::new(vec!["FILE".into(), "FILES".into()], SyntaxKind::FileKeyword)
                .to_matchable()
                .into(),
        ),
        (
            "JAR".into(),
            MultiStringParser::new(vec!["JAR".into(), "JARS".into()], SyntaxKind::FileKeyword)
                .to_matchable()
                .into(),
        ),
        (
            "NOSCAN".into(),
            StringParser::new("NOSCAN", SyntaxKind::Keyword)
                .to_matchable()
                .into(),
        ),
        (
            "WHL".into(),
            StringParser::new("WHL", SyntaxKind::FileKeyword)
                .to_matchable()
                .into(),
        ),
        (
            "CommentGrammar".into(),
            hive_dialect.grammar("CommentGrammar").into(),
        ),
        (
            "LocationGrammar".into(),
            hive_dialect.grammar("LocationGrammar").into(),
        ),
        (
            "SerdePropertiesGrammar".into(),
            hive_dialect.grammar("SerdePropertiesGrammar").into(),
        ),
        (
            "StoredAsGrammar".into(),
            hive_dialect.grammar("StoredAsGrammar").into(),
        ),
        (
            "StoredByGrammar".into(),
            hive_dialect.grammar("StoredByGrammar").into(),
        ),
        (
            "StorageFormatGrammar".into(),
            hive_dialect.grammar("StorageFormatGrammar").into(),
        ),
        (
            "TerminatedByGrammar".into(),
            hive_dialect.grammar("TerminatedByGrammar").into(),
        ),
        (
            "PropertyGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("PropertyNameSegment"),
                Ref::new("EqualsSegment").optional(),
                one_of(vec_of_erased![
                    Ref::new("LiteralGrammar"),
                    // when property value is Java Class Name
                    Delimited::new(vec_of_erased![Ref::new("PropertiesNakedIdentifierSegment"),])
                        .config(|config| {
                            config.delimiter(Ref::new("DotSegment"));
                        })
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PropertyNameListGrammar".into(),
            Delimited::new(vec_of_erased![Ref::new("PropertyNameSegment")])
                .to_matchable()
                .into(),
        ),
        (
            "BracketedPropertyNameListGrammar".into(),
            Bracketed::new(vec_of_erased![Ref::new("PropertyNameListGrammar")])
                .to_matchable()
                .into(),
        ),
        (
            "PropertyListGrammar".into(),
            Delimited::new(vec_of_erased![Ref::new("PropertyGrammar")])
                .to_matchable()
                .into(),
        ),
        (
            "BracketedPropertyListGrammar".into(),
            Bracketed::new(vec_of_erased![Ref::new("PropertyListGrammar")])
                .to_matchable()
                .into(),
        ),
        (
            "OptionsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("OPTIONS"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BucketSpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("ClusteredBySpecGrammar"),
                Ref::new("SortedBySpecGrammar").optional(),
                Ref::keyword("INTO"),
                Ref::new("NumericLiteralSegment"),
                Ref::keyword("BUCKETS")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ClusteredBySpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CLUSTERED"),
                Ref::keyword("BY"),
                Ref::new("BracketedColumnReferenceListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DatabasePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DBPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DataSourcesV2FileTypeGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("AVRO"),
                Ref::keyword("CSV"),
                Ref::keyword("JSON"),
                Ref::keyword("PARQUET"),
                Ref::keyword("ORC"),
                Ref::keyword("DELTA"),
                Ref::keyword("CSV"),
                Ref::keyword("ICEBERG"),
                Ref::keyword("TEXT"),
                Ref::keyword("BINARYFILE")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FileFormatGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("DataSourcesV2FileTypeGrammar"),
                Ref::keyword("SEQUENCEFILE"),
                Ref::keyword("TEXTFILE"),
                Ref::keyword("RCFILE"),
                Ref::keyword("JSONFILE"),
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
            "TimestampAsOfGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TIMESTAMP"),
                Ref::keyword("AS"),
                Ref::keyword("OF"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("BareFunctionSegment"),
                    Ref::new("FunctionSegment")
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "VersionAsOfGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("VERSION"),
                Ref::keyword("AS"),
                Ref::keyword("OF"),
                Ref::new("NumericLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "StartHintSegment".into(),
            StringParser::new("/*+", SyntaxKind::StartHint)
                .to_matchable()
                .into(),
        ),
        (
            "EndHintSegment".into(),
            StringParser::new("*/", SyntaxKind::EndHint)
                .to_matchable()
                .into(),
        ),
        (
            "PartitionSpecGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PARTITIONED"),
                        Ref::keyword("BY")
                    ])
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        Ref::new("ColumnDefinitionSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("EqualsSegment").optional(),
                            Ref::new("LiteralGrammar").optional(),
                            Ref::new("CommentGrammar").optional()
                        ]),
                        Ref::new("IcebergTransformationSegment").optional()
                    ]
                )])])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PartitionFieldGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("PARTITION"),
                Ref::keyword("FIELD"),
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("ColumnDefinitionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("EqualsSegment").optional(),
                        Ref::new("LiteralGrammar").optional(),
                        Ref::new("CommentGrammar").optional()
                    ]),
                    Ref::new("IcebergTransformationSegment").optional()
                ])]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH").optional(),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("ColumnDefinitionSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("EqualsSegment").optional(),
                            Ref::new("LiteralGrammar").optional(),
                            Ref::new("CommentGrammar").optional()
                        ]),
                        Ref::new("IcebergTransformationSegment").optional()
                    ])])
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::new("NakedIdentifierSegment")
                ])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PropertiesNakedIdentifierSegment".into(),
            RegexParser::new(
                "[A-Z0-9]*[A-Z][A-Z0-9]*",
                SyntaxKind::PropertiesNakedIdentifier,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ResourceFileGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("JAR"),
                Ref::new("WHL"),
                Ref::new("FILE")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ResourceLocationGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::new("ResourceFileGrammar"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SortedBySpecGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SORTED"),
                Ref::keyword("BY"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")]).config(
                            |config| {
                                config.optional();
                            }
                        )
                    ])
                ])])
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable()
            .into(),
        ),
        (
            "UnsetTablePropertiesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("UNSET"),
                Ref::keyword("TBLPROPERTIES"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("BracketedPropertyNameListGrammar")
            ])
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
            "BytesQuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::BytesSingleQuote, SyntaxKind::BytesQuotedLiteral,),
                TypedParser::new(SyntaxKind::BytesDoubleQuote, SyntaxKind::BytesQuotedLiteral,)
            ])
            .to_matchable()
            .into(),
        ),
        (
            "JoinTypeKeywords".into(),
            one_of(vec_of_erased![
                Ref::keyword("CROSS"),
                Ref::keyword("INNER"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("FULL"),
                        Ref::keyword("LEFT"),
                        Ref::keyword("RIGHT")
                    ]),
                    Ref::keyword("OUTER").optional()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LEFT").optional(),
                    Ref::keyword("SEMI")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LEFT").optional(),
                    Ref::keyword("ANTI")
                ])
            ])
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
            "SignedQuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::SignedQuotedLiteral),
                TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::SignedQuotedLiteral)
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrRefreshGrammar".into(),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REFRESH")])
                .to_matchable()
                .into(),
        ),
        (
            "WidgetNameIdentifierSegment".into(),
            RegexParser::new("[A-Z][A-Z0-9_]*", SyntaxKind::WidgetNameIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "WidgetDefaultGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT"),
                Ref::new("QuotedLiteralSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TableDefinitionSegment".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("OrReplaceGrammar"),
                    Ref::new("OrRefreshGrammar")
                ])
                .config(|config| {
                    config.optional();
                }),
                Ref::new("TemporaryGrammar").optional(),
                Ref::keyword("EXTERNAL").optional(),
                Ref::keyword("STREAMING").optional(),
                Ref::keyword("LIVE").optional(),
                Ref::keyword("TABLE"),
                Ref::new("IfNotExistsGrammar").optional(),
                one_of(vec_of_erased![
                    Ref::new("FileReferenceSegment"),
                    Ref::new("TableReferenceSegment")
                ]),
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("ColumnDefinitionSegment"),
                                Ref::new("GeneratedColumnDefinitionSegment")
                            ]),
                            Ref::new("CommentGrammar").optional()
                        ]),
                        Ref::new("ConstraintStatementSegment").optional(),
                    ])]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LIKE"),
                        one_of(vec_of_erased![
                            Ref::new("FileReferenceSegment"),
                            Ref::new("TableReferenceSegment")
                        ])
                    ])
                ])
                .config(|config| {
                    config.optional();
                }),
                Ref::new("UsingClauseSegment").optional(),
                any_set_of(vec_of_erased![
                    Ref::new("RowFormatClauseSegment"),
                    Ref::new("StoredAsGrammar"),
                    Ref::new("CommentGrammar"),
                    Ref::new("OptionsGrammar"),
                    Ref::new("PartitionSpecGrammar"),
                    Ref::new("BucketSpecGrammar"),
                    Ref::new("LocationGrammar"),
                    Ref::new("CommentGrammar"),
                    Ref::new("TablePropertiesGrammar"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CLUSTER"),
                        Ref::keyword("BY"),
                        Ref::new("BracketedColumnReferenceListGrammar")
                    ])
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS").optional(),
                    optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
                ])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::legacy(
            "start_hint",
            |s| s.starts_with("/*+"),
            r"\/\*\+",
            SyntaxKind::StartHint,
        )],
        "block_comment",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::regex("end_hint", r"\*\/", SyntaxKind::EndHint)],
        "single_quote",
    );

    sparksql_dialect.insert_lexer_matchers(
        vec![Matcher::string("end_hint", r"->", SyntaxKind::RightArrow)],
        "like_operator",
    );

    sparksql_dialect.add([
        (
            "SQLConfPropertiesSegment".into(),
            NodeMatcher::new(SyntaxKind::SqlConfOption, |_| {
                Sequence::new(vec_of_erased![
                    StringParser::new("-", SyntaxKind::Dash),
                    StringParser::new("v", SyntaxKind::SqlConfOption)
                ])
                .config(|config| {
                    config.disallow_gaps();
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DivBinaryOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Ref::keyword("DIV").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "QualifyClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::QualifyClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("QUALIFY"),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.add([
        (
            "PrimitiveTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::PrimitiveType, |_| {
                one_of(vec_of_erased![
                    Ref::keyword("BOOLEAN"),
                    Ref::keyword("TINYINT"),
                    Ref::keyword("LONG"),
                    Ref::keyword("SMALLINT"),
                    Ref::keyword("INT"),
                    Ref::keyword("INTEGER"),
                    Ref::keyword("BIGINT"),
                    Ref::keyword("FLOAT"),
                    Ref::keyword("REAL"),
                    Ref::keyword("DOUBLE"),
                    Ref::keyword("DATE"),
                    Ref::keyword("TIMESTAMP"),
                    Ref::keyword("TIMESTAMP_LTZ"),
                    Ref::keyword("TIMESTAMP_NTZ"),
                    Ref::keyword("STRING"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("CHAR"),
                            Ref::keyword("CHARACTER"),
                            Ref::keyword("VARCHAR"),
                            Ref::keyword("DECIMAL"),
                            Ref::keyword("DEC"),
                            Ref::keyword("NUMERIC")
                        ]),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    Ref::keyword("BINARY"),
                    Ref::keyword("INTERVAL"),
                    Ref::keyword("VARIANT"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArrayTypeSegment".into(),
            hive_dialect.grammar("ArrayTypeSegment").into(),
        ),
        (
            "StructTypeSegment".into(),
            hive_dialect.grammar("StructTypeSegment").into(),
        ),
        (
            "StructTypeSchemaSegment".into(),
            hive_dialect.grammar("StructTypeSchemaSegment").into(),
        ),
    ]);

    sparksql_dialect.add([
        (
            "SemiStructuredAccessorSegment".into(),
            NodeMatcher::new(SyntaxKind::SemiStructuredExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("ColonSegment"),
                    one_of(vec_of_erased![
                        Ref::new("NakedSemiStructuredElementSegment"),
                        Bracketed::new(vec_of_erased![Ref::new(
                            "QuotedSemiStructuredElementSegment"
                        )])
                        .config(|config| {
                            config.bracket_type = "square";
                        })
                    ]),
                    Ref::new("ArrayAccessorSegment").optional(),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("DotSegment"),
                                Ref::new("ColonSegment")
                            ]),
                            one_of(vec_of_erased![
                                Ref::new("NakedSemiStructuredElementSegment"),
                                Bracketed::new(vec_of_erased![Ref::new(
                                    "QuotedSemiStructuredElementSegment"
                                )])
                                .config(|config| {
                                    config.bracket_type = "square";
                                })
                            ])
                        ]),
                        Ref::new("ArrayAccessorSegment").optional()
                    ])
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
                    Ref::new("ArrayTypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MAP"),
                        Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            Ref::new("CommaSegment"),
                            Ref::new("DatatypeSegment")
                        ])])
                        .config(|config| {
                            config.bracket_pairs_set = "angle_bracket_pairs";
                            config.bracket_type = "angle";
                        })
                    ]),
                    Ref::new("StructTypeSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // An `ALTER DATABASE/SCHEMA` statement.
            // http://spark.apache.org/docs/latest/sql-ref-syntax-ddl-alter-database.html
            "AlterDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterDatabaseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    one_of(vec_of_erased![
                        Ref::keyword("DATABASE"),
                        Ref::keyword("SCHEMA")
                    ]),
                    Ref::new("DatabaseReferenceSegment"),
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("DatabasePropertiesGrammar"),
                        Ref::new("LocationGrammar")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // A `SET VARIABLE` statement used to set session variables.
            // https://spark.apache.org/docs/4.0.0-preview2/sql-ref-syntax-aux-set-var.html
            "SetVariableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                one_of(vec_of_erased![
                    Ref::keyword("VAR"),
                    Ref::keyword("VARIABLE")
                ]),
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    ),])]),
                    Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar"),])
                ]),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    Ref::new("ExpressionSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                ])
            ])
            .allow_gaps(true)
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TABLE"),
            Ref::new("TableReferenceSegment"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar"),
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("PartitionSpecGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("COLUMN"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::new("ColumnReferenceSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    one_of(vec_of_erased![
                        Ref::keyword("COLUMNS"),
                        Ref::keyword("COLUMN")
                    ]),
                    MetaSegment::indent(),
                    optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnFieldDefinitionSegment"),
                            one_of(vec_of_erased![
                                Ref::keyword("FIRST"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("AFTER"),
                                    Ref::new("ColumnReferenceSegment")
                                ])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ])
                    ])]),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ALTER"),
                        Ref::keyword("CHANGE")
                    ]),
                    Ref::keyword("COLUMN").optional(),
                    MetaSegment::indent(),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment")
                            .exclude(one_of(vec_of_erased![
                                Ref::keyword("COMMENT"),
                                Ref::keyword("TYPE"),
                                Ref::new("DatatypeSegment"),
                                Ref::keyword("FIRST"),
                                Ref::keyword("AFTER"),
                                Ref::keyword("SET"),
                                Ref::keyword("DROP")
                            ]))
                            .config(|config| {
                                config.exclude = one_of(vec_of_erased![
                                    Ref::keyword("COMMENT"),
                                    Ref::keyword("TYPE"),
                                    Ref::new("DatatypeSegment"),
                                    Ref::keyword("FIRST"),
                                    Ref::keyword("AFTER"),
                                    Ref::keyword("SET"),
                                    Ref::keyword("DROP")
                                ])
                                .to_matchable()
                                .into();
                            })
                    ])
                    .config(|config| {
                        config.max_times = Some(2);
                    }),
                    Ref::keyword("TYPE").optional(),
                    Ref::new("DatatypeSegment").optional(),
                    Ref::new("CommentGrammar").optional(),
                    one_of(vec_of_erased![
                        Ref::keyword("FIRST"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AFTER"),
                            Ref::new("ColumnReferenceSegment")
                        ])
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DROP")]),
                        Ref::keyword("NOT"),
                        Ref::keyword("NULL")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Ref::keyword("COLUMNS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnDefinitionSegment"),
                            Ref::new("CommentGrammar").optional()
                        ])
                    ])])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMN"),
                            Ref::new("ColumnReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMNS"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                AnyNumberOf::new(vec_of_erased![Ref::new(
                                    "ColumnReferenceSegment"
                                )])
                            ])])
                        ])
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("PartitionSpecGrammar"),
                        Ref::new("PartitionFieldGrammar")
                    ])
                    .config(|config| {
                        config.min_times = 1;
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::new("IfExistsGrammar").optional(),
                    one_of(vec_of_erased![
                        Ref::new("PartitionSpecGrammar"),
                        Ref::new("PartitionFieldGrammar")
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("PURGE")]).config(|config| {
                        config.optional();
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Ref::new("PartitionFieldGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RECOVER"),
                    Ref::keyword("PARTITIONS")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("TablePropertiesGrammar")
                ]),
                Ref::new("UnsetTablePropertiesGrammar"),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SERDEPROPERTIES"),
                            Ref::new("BracketedPropertyListGrammar")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SERDE"),
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("SerdePropertiesGrammar").optional()
                        ])
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::keyword("SET"),
                    Ref::keyword("FILEFORMAT"),
                    Ref::new("DataSourceFormatSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::keyword("SET"),
                    Ref::new("LocationGrammar")
                ]),
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("DROP")]),
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ColumnReferenceSegment")
                        .exclude(Ref::keyword("CHECK"))
                        .config(|config| {
                            config.exclude = Ref::keyword("CHECK").to_matchable().into();
                        }),
                    Ref::keyword("CHECK").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]).config(
                        |config| {
                            config.optional();
                        }
                    ),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WRITE"),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DISTRIBUTED"),
                            Ref::keyword("BY"),
                            Ref::keyword("PARTITION")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LOCALLY").optional(),
                            Ref::keyword("ORDERED"),
                            Ref::keyword("BY"),
                            MetaSegment::indent(),
                            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|config| {
                                        config.optional();
                                    }),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("NULLS"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("FIRST"),
                                        Ref::keyword("LAST")
                                    ])
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                            ])])
                            .config(|config| {
                                config.optional();
                            }),
                            MetaSegment::dedent()
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])
                    .config(|config| {
                        config.min_times = 1;
                        config.max_times_per_element = Some(1);
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("IDENTIFIER"),
                    Ref::keyword("FIELDS"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])]),
                    MetaSegment::dedent()
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("IDENTIFIER"),
                    Ref::keyword("FIELDS"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])]),
                    MetaSegment::dedent()
                ])
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    sparksql_dialect.add([(
        "ColumnFieldDefinitionSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DatatypeSegment"),
                Bracketed::new(vec_of_erased![Anything::new()]).config(|config| {
                    config.optional();
                }),
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("ColumnConstraintSegment").optional()
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.add([(
        "AlterViewStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterViewStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("VIEW"),
                Ref::new("TableReferenceSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RENAME"),
                        Ref::keyword("TO"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Ref::new("TablePropertiesGrammar")
                    ]),
                    Ref::new("UnsetTablePropertiesGrammar"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        optionally_bracketed(vec_of_erased![Ref::new("SelectStatementSegment")])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.add([(
        "JoinLikeClauseGrammar".into(),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("PivotClauseSegment"),
                Ref::new("UnpivotClauseSegment"),
                Ref::new("LateralViewClauseSegment"),
            ]),
            Ref::new("AliasExpressionSegment").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.add([
        // An Unpivot segment.
        // https://spark.apache.org/docs/latest/sql-ref-syntax-qry-select-unpivot.html
        (
            "UnpivotClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::UnpivotClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNPIVOT"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("INCLUDE"),
                            Ref::keyword("EXCLUDE")
                        ]),
                        Ref::keyword("NULLS")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    MetaSegment::indent(),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("SingleValueColumnUnpivotSegment"),
                        Ref::new("MultiValueColumnUnpivotSegment")
                    ])]),
                    MetaSegment::dedent(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SingleValueColumnUnpivotSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("FOR"),
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("IN"),
                Bracketed::new(vec_of_erased![
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("AliasExpressionSegment").optional()
                    ])]),
                    MetaSegment::dedent()
                ])
                .config(|config| {
                    config.parse_mode = ParseMode::Greedy;
                }),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "MultiValueColumnUnpivotSegment".into(),
            Sequence::new(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "SingleIdentifierGrammar"
                )])]),
                MetaSegment::indent(),
                Ref::keyword("FOR"),
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("IN"),
                Bracketed::new(vec_of_erased![
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![
                            MetaSegment::indent(),
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                        ]),
                        Ref::new("AliasExpressionSegment").optional()
                    ])]),
                ])
                .config(|config| {
                    config.parse_mode = ParseMode::Greedy;
                }),
                MetaSegment::dedent()
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "CreateDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![
                Ref::keyword("DATABASE"),
                Ref::keyword("SCHEMA")
            ]),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            Ref::new("CommentGrammar").optional(),
            Ref::new("LocationGrammar").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("DBPROPERTIES"),
                Ref::new("BracketedPropertyListGrammar")
            ])
            .config(|config| {
                config.optional();
            })
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "CreateFunctionStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE")]).config(
                |config| {
                    config.optional();
                }
            ),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("FUNCTION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("FunctionNameIdentifierSegment"),
            Ref::keyword("AS"),
            Ref::new("QuotedLiteralSegment"),
            Ref::new("ResourceLocationGrammar").optional()
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("TableDefinitionSegment")
        ])
        .to_matchable(),
    );

    sparksql_dialect.add([(
        "CreateHiveFormatTableStatementSegment".into(),
        hive_dialect.grammar("CreateTableStatementSegment").into(),
    )]);

    sparksql_dialect.add([(
        "NonWithNonSelectableGrammar".into(),
        ansi::raw_dialect()
            .grammar("NonWithNonSelectableGrammar")
            .copy(
                Some(vec_of_erased![Ref::new("InsertOverwriteDirectorySegment")]),
                None,
                None,
                None,
                Vec::new(),
                false,
            )
            .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "CreateViewStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            one_of(vec_of_erased![
                Ref::new("OrReplaceGrammar"),
                Ref::new("OrRefreshGrammar")
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("STREAMING").optional(),
            Ref::keyword("LIVE").optional(),
            Ref::keyword("MATERIALIZED").optional(),
            Ref::keyword("VIEW"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("CommentGrammar").optional()
                    ]),
                    Ref::new("ConstraintStatementSegment").optional()
                ])
            ])])
            .config(|config| {
                config.optional();
            }),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                Ref::new("DataSourceFormatSegment")
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("OptionsGrammar").optional(),
            Ref::new("CommentGrammar").optional(),
            Ref::new("TablePropertiesGrammar").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("WithNoSchemaBindingClauseSegment").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "CreateWidgetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateWidgetStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("WIDGET"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DROPDOWN"),
                            Ref::new("WidgetNameIdentifierSegment"),
                            Ref::new("WidgetDefaultGrammar"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CHOICES"),
                                Ref::new("SelectStatementSegment")
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TEXT"),
                            Ref::new("WidgetNameIdentifierSegment"),
                            Ref::new("WidgetDefaultGrammar")
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ReplaceTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ReplaceTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("REPLACE"),
                    Ref::new("TableDefinitionSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RemoveWidgetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RemoveWidgetStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("REMOVE"),
                    Ref::keyword("WIDGET"),
                    Ref::new("WidgetNameIdentifierSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "DropDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            one_of(vec_of_erased![
                Ref::keyword("DATABASE"),
                Ref::keyword("SCHEMA")
            ]),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("DatabaseReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([(
        "DropFunctionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropFunctionStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::new("TemporaryGrammar").optional(),
                Ref::keyword("FUNCTION"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("FunctionNameSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.add([(
        "MsckRepairTableStatementSegment".into(),
        hive_dialect
            .grammar("MsckRepairTableStatementSegment")
            .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "TruncateStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("TRUNCATE"),
            Ref::keyword("TABLE"),
            Ref::new("TableReferenceSegment"),
            Ref::new("PartitionSpecGrammar").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "UseDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UseDatabaseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("USE"),
                    Ref::new("DatabaseReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "InsertBracketedColumnReferenceListGrammar".into(),
            Ref::new("BracketedColumnReferenceListGrammar")
                .to_matchable()
                .into(),
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    one_of(vec_of_erased![
                        Ref::keyword("INTO"),
                        Ref::keyword("OVERWRITE")
                    ]),
                    Ref::keyword("TABLE").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("PartitionSpecGrammar").optional(),
                    Ref::new("InsertBracketedColumnReferenceListGrammar").optional(),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("ValuesClauseSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        ),
                        Ref::new("SelectableGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE").optional(),
                            Ref::new("TableReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::new("TableReferenceSegment"),
                            Ref::keyword("SELECT"),
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                            Ref::new("WhereClauseSegment").optional(),
                            Ref::new("GroupByClauseSegment").optional(),
                            Ref::new("OrderByClauseSegment").optional(),
                            Ref::new("LimitClauseSegment").optional()
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "InsertOverwriteDirectorySegment".into(),
            NodeMatcher::new(SyntaxKind::InsertOverwriteDirectoryStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("OVERWRITE"),
                    Ref::keyword("LOCAL").optional(),
                    Ref::keyword("DIRECTORY"),
                    Ref::new("QuotedLiteralSegment").optional(),
                    Ref::keyword("USING"),
                    Ref::new("DataSourceFormatSegment"),
                    Ref::new("OptionsGrammar").optional(),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("ValuesClauseSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        ),
                        Ref::new("SelectableGrammar")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "InsertOverwriteDirectoryHiveFmtSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertOverwriteDirectoryHiveFmtStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("OVERWRITE"),
                    Ref::keyword("LOCAL").optional(),
                    Ref::keyword("DIRECTORY"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("RowFormatClauseSegment").optional(),
                    Ref::new("StoredAsGrammar").optional(),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("ValuesClauseSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        ),
                        Ref::new("SelectableGrammar")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LoadDataSegment".into(),
            NodeMatcher::new(SyntaxKind::LoadDataStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("LOAD"),
                    Ref::keyword("DATA"),
                    Ref::keyword("LOCAL").optional(),
                    Ref::keyword("INPATH"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::keyword("OVERWRITE").optional(),
                    Ref::keyword("INTO"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("PartitionSpecGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ClusterByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ClusterByClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTER"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment")
                        ]
                    )])])
                    .config(|config| {
                        config.terminators = vec_of_erased![
                            Ref::keyword("LIMIT"),
                            Ref::keyword("HAVING"),
                            Ref::keyword("WINDOW"),
                            Ref::new("FrameClauseUnitGrammar"),
                            Ref::keyword("SEPARATOR")
                        ];
                    }),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DistributeByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::DistributeByClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISTRIBUTE"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment")
                        ]
                    )])])
                    .config(|config| {
                        config.terminators = vec_of_erased![
                            Ref::keyword("SORT"),
                            Ref::keyword("LIMIT"),
                            Ref::keyword("HAVING"),
                            Ref::keyword("WINDOW"),
                            Ref::new("FrameClauseUnitGrammar"),
                            Ref::keyword("SEPARATOR")
                        ];
                    }),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "HintFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::HintFunction, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("FunctionNameSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("ColumnReferenceSegment")
                        ])
                        .config(|config| {
                            config.min_times = 1;
                        })
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
            "SelectHintSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectHint, |_| {
                Sequence::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("StartHintSegment"),
                    Delimited::new(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![Ref::new("HintFunctionSegment")]).config(
                            |config| {
                                config.min_times = 1;
                            }
                        )
                    ])
                    .config(|config| {
                        config.terminators = vec_of_erased![Ref::new("EndHintSegment")];
                    }),
                    Ref::new("EndHintSegment")
                ])])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "LimitClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("LIMIT"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Ref::keyword("ALL"),
                Ref::new("FunctionSegment")
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "SetOperatorSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Ref::keyword("MINUS")
                ]),
                Ref::keyword("ALL").optional()
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("UNION"),
                    Ref::keyword("INTERSECT")
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("DISTINCT"),
                    Ref::keyword("ALL")
                ])
                .config(|config| {
                    config.optional();
                })
            ])
        ])
        .config(|config| {
            config.exclude = Some(
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Bracketed::new(vec_of_erased![Anything::new()]),
                ])
                .to_matchable(),
            )
        })
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "SelectClauseModifierSegment",
        Sequence::new(vec_of_erased![
            Ref::new("SelectHintSegment").optional(),
            one_of(vec_of_erased![
                Ref::keyword("DISTINCT"),
                Ref::keyword("ALL")
            ])
            .config(|config| {
                config.optional();
            })
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec_of_erased![
                Ref::new("QualifyClauseSegment").optional(),
                Ref::new("ClusterByClauseSegment").optional(),
                Ref::new("DistributeByClauseSegment").optional(),
                Ref::new("SortByClauseSegment").optional(),
            ]),
            None,
            None,
            Some(vec_of_erased![Ref::new("OverlapsClauseSegment").optional()]),
            Vec::new(),
            false,
        ),
    );

    sparksql_dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::select_statement()
            .copy(
                Some(vec_of_erased![
                    Ref::new("ClusterByClauseSegment",).optional(),
                    Ref::new("DistributeByClauseSegment").optional(),
                    Ref::new("SortByClauseSegment").optional(),
                ]),
                None,
                Some(Ref::new("LimitClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            )
            .copy(
                Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
                None,
                Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
                None,
                Vec::new(),
                false,
            ),
    );

    sparksql_dialect.replace_grammar(
        // Enhance `GROUP BY` clause like in `SELECT` for 'CUBE' and 'ROLLUP`.
        // https://spark.apache.org/docs/latest/sql-ref-syntax-qry-select-groupby.html
        "GroupByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("GROUP"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Delimited::new(vec_of_erased![
                    Ref::new("CubeRollupClauseSegment"),
                    Ref::new("GroupingSetsClauseSegment"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("ExpressionSegment")
                    ]),
                    one_of(vec_of_erased![
                        Ref::new("WithCubeRollupClauseSegment"),
                        Ref::new("GroupingSetsClauseSegment")
                    ])
                ])
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "WithCubeRollupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCubeRollupClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    one_of(vec_of_erased![Ref::keyword("CUBE"), Ref::keyword("ROLLUP")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SortByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SortByClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SORT"),
                    Ref::keyword("BY"),
                    MetaSegment::indent(),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("ExpressionSegment")
                        ]),
                        one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")]).config(
                            |config| {
                                config.optional();
                            }
                        ),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NULLS"),
                            one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST")])
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ])])
                    .config(|config| {
                        config.terminators = vec_of_erased![
                            Ref::keyword("LIMIT"),
                            Ref::keyword("HAVING"),
                            Ref::keyword("QUALIFY"),
                            Ref::keyword("WINDOW"),
                            Ref::new("FrameClauseUnitGrammar"),
                            Ref::keyword("SEPARATOR")
                        ];
                    }),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // A `TABLESAMPLE` clause following a table identifier.
    // https://spark.apache.org/docs/latest/sql-ref-syntax-qry-select-sampling.html
    sparksql_dialect.replace_grammar(
        "SamplingExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("TABLESAMPLE"),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("PERCENT"),
                        Ref::keyword("ROWS")
                    ])
                ]),
                Bracketed::new(vec_of_erased![
                    Ref::keyword("BUCKET"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::keyword("OUT"),
                    Ref::keyword("OF"),
                    Ref::new("NumericLiteralSegment")
                ])
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.add([
        (
            "LateralViewClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::LateralViewClause, |_| {
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::keyword("LATERAL"),
                    Ref::keyword("VIEW"),
                    Ref::keyword("OUTER").optional(),
                    Ref::new("FunctionSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AS").optional(),
                                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS").optional(),
                            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                        ])
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PivotClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotClause, |_| {
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::keyword("PIVOT"),
                    Bracketed::new(vec_of_erased![
                        MetaSegment::indent(),
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("BaseExpressionElementGrammar"),
                            Ref::new("AliasExpressionSegment").optional()
                        ])]),
                        Ref::keyword("FOR"),
                        optionally_bracketed(vec_of_erased![one_of(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
                        ])]),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                        Ref::new("ExpressionSegment")
                                    ])])
                                    .config(|config| {
                                        config.parse_mode(ParseMode::Greedy);
                                    }),
                                    Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")])
                                ]),
                                Ref::new("AliasExpressionSegment").optional()
                            ])
                        ])]),
                        MetaSegment::dedent()
                    ]),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TransformClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::TransformClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("TRANSFORM"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    )])])
                    .config(|config| {
                        config.parse_mode(ParseMode::Greedy);
                    }),
                    MetaSegment::indent(),
                    Ref::new("RowFormatClauseSegment").optional(),
                    Ref::keyword("USING"),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            AnyNumberOf::new(vec_of_erased![
                                Ref::new("SingleIdentifierGrammar"),
                                Ref::new("DatatypeSegment")
                            ])
                        ])])
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Ref::new("RowFormatClauseSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RowFormatClauseSegment".into(),
            hive_dialect.grammar("RowFormatClauseSegment").into(),
        ),
        (
            "SkewedByClauseSegment".into(),
            hive_dialect.grammar("SkewedByClauseSegment").into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "ExplainStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("EXPLAIN"),
            one_of(vec_of_erased![
                Ref::keyword("EXTENDED"),
                Ref::keyword("CODEGEN"),
                Ref::keyword("COST"),
                Ref::keyword("FORMATTED")
            ])
            .config(|config| {
                config.optional();
            }),
            Ref::new("StatementSegment")
        ])
        .to_matchable(),
    );

    sparksql_dialect.add([
        (
            "AddFileSegment".into(),
            NodeMatcher::new(SyntaxKind::AddFileStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("FILE"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AddJarSegment".into(),
            NodeMatcher::new(SyntaxKind::AddJarStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("JAR"),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileLiteralSegment")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "AnalyzeTableSegment".into(),
            NodeMatcher::new(SyntaxKind::AnalyzeTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ANALYZE"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("PartitionSpecGrammar").optional(),
                            Ref::keyword("COMPUTE"),
                            Ref::keyword("STATISTICS"),
                            one_of(vec_of_erased![
                                Ref::keyword("NOSCAN"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("FOR"),
                                    Ref::keyword("COLUMNS"),
                                    optionally_bracketed(vec_of_erased![Delimited::new(
                                        vec_of_erased![Ref::new("ColumnReferenceSegment")]
                                    )])
                                ])
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLES"),
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                                Ref::new("DatabaseReferenceSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            }),
                            Ref::keyword("COMPUTE"),
                            Ref::keyword("STATISTICS"),
                            Ref::keyword("NOSCAN").optional()
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CacheTableSegment".into(),
            NodeMatcher::new(SyntaxKind::CacheTable, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CACHE"),
                    Ref::keyword("LAZY").optional(),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("OptionsGrammar").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS").optional(),
                        Ref::new("SelectableGrammar")
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
            "ClearCacheSegment".into(),
            NodeMatcher::new(SyntaxKind::ClearCache, |_| {
                Sequence::new(vec_of_erased![Ref::keyword("CLEAR"), Ref::keyword("CACHE")])
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DescribeObjectGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("DATABASE"),
                        Ref::keyword("SCHEMA")
                    ]),
                    Ref::keyword("EXTENDED").optional(),
                    Ref::new("DatabaseReferenceSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FUNCTION"),
                    Ref::keyword("EXTENDED").optional(),
                    Ref::new("FunctionNameSegment")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE").optional(),
                    Ref::keyword("EXTENDED").optional(),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("PartitionSpecGrammar").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        AnyNumberOf::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("DotSegment"),
                                Ref::new("SingleIdentifierGrammar")
                            ])
                            .config(|config| {
                                config.disallow_gaps();
                            })
                        ])
                        .config(|config| {
                            config.max_times = Some(2);
                            config.disallow_gaps();
                        })
                    ])
                    .config(|config| {
                        config.optional();
                        config.disallow_gaps();
                    })
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("QUERY").optional(),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE"),
                            Ref::new("TableReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::new("TableReferenceSegment"),
                            Ref::keyword("SELECT"),
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                            Ref::new("WhereClauseSegment").optional(),
                            Ref::new("GroupByClauseSegment").optional(),
                            Ref::new("OrderByClauseSegment").optional(),
                            Ref::new("LimitClauseSegment").optional()
                        ]),
                        Ref::new("StatementSegment")
                    ])
                ])
            ])
            .config(|config| {
                config.exclude = one_of(vec_of_erased![
                    Ref::keyword("HISTORY"),
                    Ref::keyword("DETAIL")
                ])
                .to_matchable()
                .into();
            })
            .to_matchable()
            .into(),
        ),
        // A `DESCRIBE` statement.
        // This class provides coverage for databases, tables, functions, and queries.

        // NB: These are similar enough that it makes sense to include them in a
        // common class, especially since there wouldn't be any specific rules that
        // would apply to one describe vs another, but they could be broken out to
        // one class per describe statement type.

        // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-describe-database.html
        // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-describe-function.html
        // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-describe-query.html
        // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-describe-table.html
        (
            "DescribeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DescribeStatement, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("DESCRIBE"),
                        Ref::keyword("DESC")
                    ]),
                    Ref::new("DescribeObjectGrammar"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ListFileSegment".into(),
            NodeMatcher::new(SyntaxKind::ListFileStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIST"),
                    Ref::keyword("FILE"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ListJarSegment".into(),
            NodeMatcher::new(SyntaxKind::ListJarStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIST"),
                    Ref::keyword("JAR"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RefreshStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RefreshStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFRESH"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE").optional(),
                            Ref::new("TableReferenceSegment")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FUNCTION"),
                            Ref::new("FunctionNameSegment")
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ResetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ResetStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESET"),
                    Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]).config(
                        |config| {
                            config.delimiter(Ref::new("DotSegment"));
                            config.optional();
                        }
                    )
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ShowViewsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("VIEWS"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
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
            .to_matchable()
            .into(),
        ),
        (
            "SetStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("SQLConfPropertiesSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::new("PropertyListGrammar"),
                        Ref::new("PropertyNameSegment")
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
            // Common class for `SHOW` statements.
            //
            // NB: These are similar enough that it makes sense to include them in a
            // common class, especially since there wouldn't be any specific rules that
            // would apply to one show vs another, but they could be broken out to
            // one class per show statement type.
            //
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-columns.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-create-table.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-databases.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-functions.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-partitions.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-table.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-tables.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-tblproperties.html
            // https://spark.apache.org/docs/latest/sql-ref-syntax-aux-show-views.html
            "ShowStatement".into(),
            NodeMatcher::new(SyntaxKind::ShowStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SHOW"),
                    Ref::new("ShowObjectGrammar")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UncacheTableSegment".into(),
            NodeMatcher::new(SyntaxKind::UncacheTable, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNCACHE"),
                    Ref::keyword("TABLE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("TableReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec_of_erased![
                Ref::new("AlterDatabaseStatementSegment"),
                Ref::new("AlterTableStatementSegment"),
                Ref::new("AlterViewStatementSegment"),
                Ref::new("CreateHiveFormatTableStatementSegment"),
                Ref::new("MsckRepairTableStatementSegment"),
                Ref::new("UseDatabaseStatementSegment"),
                Ref::new("AddFileSegment"),
                Ref::new("AddJarSegment"),
                Ref::new("AnalyzeTableSegment"),
                Ref::new("CacheTableSegment"),
                Ref::new("ClearCacheSegment"),
                Ref::new("ListFileSegment"),
                Ref::new("ListJarSegment"),
                Ref::new("RefreshStatementSegment"),
                Ref::new("ResetStatementSegment"),
                Ref::new("SetStatementSegment"),
                Ref::new("ShowStatement"),
                Ref::new("UncacheTableSegment"),
                Ref::new("InsertOverwriteDirectorySegment"),
                Ref::new("InsertOverwriteDirectoryHiveFmtSegment"),
                Ref::new("LoadDataSegment"),
                Ref::new("ClusterByClauseSegment"),
                Ref::new("DistributeByClauseSegment"),
                Ref::new("VacuumStatementSegment"),
                Ref::new("DescribeHistoryStatementSegment"),
                Ref::new("DescribeDetailStatementSegment"),
                Ref::new("GenerateManifestFileStatementSegment"),
                Ref::new("ConvertToDeltaStatementSegment"),
                Ref::new("RestoreTableStatementSegment"),
                Ref::new("ConstraintStatementSegment"),
                Ref::new("ApplyChangesIntoStatementSegment"),
                Ref::new("CreateWidgetStatementSegment"),
                Ref::new("RemoveWidgetStatementSegment"),
                Ref::new("ReplaceTableStatementSegment"),
                Ref::new("SetVariableStatementSegment"),
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

    sparksql_dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("JoinTypeKeywords").optional(),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent(),
                Conditional::new(MetaSegment::indent()).indented_using_on(),
                one_of(vec_of_erased![
                    Ref::new("JoinOnConditionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Conditional::new(MetaSegment::indent()),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SingleIdentifierGrammar"
                        )])])
                        .config(|config| {
                            config.parse_mode(ParseMode::Greedy);
                        }),
                        Conditional::new(MetaSegment::dedent())
                    ])
                ])
                .config(|config| {
                    config.optional();
                }),
                Conditional::new(MetaSegment::dedent()).indented_using_on()
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("NaturalJoinKeywordsGrammar"),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent()
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "AliasExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("AS").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                ]),
                Ref::new("SingleIdentifierGrammar")
            ])
            .config(|config| {
                config.exclude = one_of(vec_of_erased![
                    Ref::keyword("LATERAL"),
                    Ref::new("JoinTypeKeywords"),
                    Ref::keyword("WINDOW"),
                    Ref::keyword("PIVOT"),
                    Ref::keyword("KEYS"),
                    Ref::keyword("FROM")
                ])
                .to_matchable()
                .into();
            })
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "ValuesClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("VALUES"),
            Delimited::new(vec_of_erased![
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Ref::keyword("NULL"),
                        Ref::new("ExpressionSegment")
                    ])])
                    .config(|config| {
                        config.parse_mode(ParseMode::Greedy);
                    }),
                    Ref::keyword("NULL"),
                    Ref::new("ExpressionSegment")
                ])
                .config(|config| {
                    config.exclude = one_of(vec_of_erased![Ref::keyword("VALUES")])
                        .to_matchable()
                        .into();
                })
            ]),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::keyword("LIMIT"),
                    Ref::keyword("ORDER")
                ]))
                .optional()
                .config(|config| {
                    config.exclude =
                        one_of(vec_of_erased![Ref::keyword("LIMIT"), Ref::keyword("ORDER")])
                            .to_matchable()
                            .into();
                }),
            Ref::new("OrderByClauseSegment").optional(),
            Ref::new("LimitClauseSegment").optional()
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            Ref::new("FunctionSegment"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("FileReferenceSegment"),
                    Ref::new("TableReferenceSegment")
                ]),
                one_of(vec_of_erased![
                    Ref::new("AtSignLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        MetaSegment::indent(),
                        one_of(vec_of_erased![
                            Ref::new("TimestampAsOfGrammar"),
                            Ref::new("VersionAsOfGrammar")
                        ]),
                        MetaSegment::dedent()
                    ])
                ])
                .config(|config| {
                    config.optional();
                })
            ]),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([(
        "FileReferenceSegment".into(),
        NodeMatcher::new(SyntaxKind::FileReference, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("DataSourcesV2FileTypeGrammar"),
                Ref::new("DotSegment"),
                Ref::new("BackQuotedIdentifierSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "FromExpressionElementSegment",
        Sequence::new(vec_of_erased![
            Ref::new("PreTableFunctionKeywordsGrammar").optional(),
            optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
            Ref::new("SamplingExpressionSegment").optional(),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::new("FromClauseTerminatorGrammar"),
                    Ref::new("JoinLikeClauseGrammar")
                ]))
                .optional(),
            Ref::new("SamplingExpressionSegment").optional(),
            AnyNumberOf::new(vec_of_erased![Ref::new("LateralViewClauseSegment")]),
            Ref::new("NamedWindowSegment").optional(),
            Ref::new("PostTableExpressionGrammar").optional()
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "PropertyNameSegment".into(),
            NodeMatcher::new(SyntaxKind::PropertyNameIdentifier, |_| {
                Sequence::new(vec_of_erased![one_of(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("PropertiesNakedIdentifierSegment")])
                        .config(|config| {
                            config.delimiter(Ref::new("DotSegment"));
                            config.disallow_gaps();
                        }),
                    Ref::new("SingleIdentifierGrammar")
                ])])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GeneratedColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::GeneratedColumnDefinition, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("DatatypeSegment"),
                    Bracketed::new(vec_of_erased![Anything::new()]).config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("GENERATED"),
                        Ref::keyword("ALWAYS"),
                        Ref::keyword("AS"),
                        Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::new("FunctionSegment"),
                            Ref::new("BareFunctionSegment")
                        ])])
                    ]),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("ColumnConstraintSegment").optional()
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "MergeUpdateClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("UPDATE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("WildcardIdentifierSegment")
                ]),
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::new("SetClauseListSegment"),
                    MetaSegment::dedent()
                ])
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "MergeInsertClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("INSERT"),
            one_of(vec_of_erased![
                Ref::new("WildcardIdentifierSegment"),
                Sequence::new(vec_of_erased![
                    MetaSegment::indent(),
                    Ref::new("BracketedColumnReferenceListGrammar"),
                    MetaSegment::dedent(),
                    Ref::new("ValuesClauseSegment")
                ])
            ])
        ])
        .to_matchable(),
    );

    sparksql_dialect.replace_grammar(
        "UpdateStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("UPDATE"),
            one_of(vec_of_erased![
                Ref::new("FileReferenceSegment"),
                Ref::new("TableReferenceSegment")
            ]),
            Ref::new("AliasExpressionSegment")
                .exclude(Ref::keyword("SET"))
                .optional()
                .config(|config| {
                    config.exclude = Ref::keyword("SET").to_matchable().into();
                }),
            Ref::new("SetClauseListSegment"),
            Ref::new("WhereClauseSegment")
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([(
        "IntervalLiteralSegment".into(),
        NodeMatcher::new(SyntaxKind::IntervalLiteral, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("SignedSegmentGrammar").optional(),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("SignedQuotedLiteralSegment")
                ]),
                Ref::new("DatetimeUnitSegment"),
                Ref::keyword("TO").optional(),
                Ref::new("DatetimeUnitSegment").optional()
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    sparksql_dialect.replace_grammar(
        "IntervalExpressionSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("INTERVAL"),
            one_of(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![Ref::new("IntervalLiteralSegment")]),
                Ref::new("QuotedLiteralSegment")
            ])
        ])
        .to_matchable(),
    );
    sparksql_dialect.add([
        (
            "VacuumStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::VacuumStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("VACUUM"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RETAIN"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("DatetimeUnitSegment")
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("DRY"), Ref::keyword("RUN")])
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
            "DescribeHistoryStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DescribeHistoryStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DESCRIBE"),
                    Ref::keyword("HISTORY"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Ref::new("LimitClauseSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DescribeDetailStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DescribeDetailStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DESCRIBE"),
                    Ref::keyword("DETAIL"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "GenerateManifestFileStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::GenerateManifestFileStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("GENERATE"),
                    StringParser::new("symlink_format_manifest", SyntaxKind::SymlinkFormatManifest),
                    Ref::keyword("FOR"),
                    Ref::keyword("TABLE"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ConvertToDeltaStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ConvertToDeltaStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONVERT"),
                    Ref::keyword("TO"),
                    Ref::keyword("DELTA"),
                    Ref::new("FileReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NO"),
                        Ref::keyword("STATISTICS")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Ref::new("PartitionSpecGrammar").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "RestoreTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RestoreTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESTORE"),
                    Ref::keyword("TABLE"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("FileReferenceSegment"),
                        Ref::new("TableReferenceSegment")
                    ]),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![
                        Ref::new("TimestampAsOfGrammar"),
                        Ref::new("VersionAsOfGrammar")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ConstraintStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ConstraintStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("EXPECT"),
                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ON"),
                        Ref::keyword("VIOLATION")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("FAIL"), Ref::keyword("UPDATE")]),
                        Sequence::new(vec_of_erased![Ref::keyword("DROP"), Ref::keyword("ROW")])
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
            // A statement ingest CDC data a target table.
            // https://docs.databricks.com/workflows/delta-live-tables/delta-live-tables-cdc.html#sql
            "ApplyChangesIntoStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ApplyChangesIntoStatement, |_| {
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("APPLY"),
                        Ref::keyword("CHANGES"),
                        Ref::keyword("INTO")
                    ]),
                    MetaSegment::indent(),
                    Ref::new("TableExpressionSegment"),
                    MetaSegment::dedent(),
                    Ref::new("FromClauseSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("KEYS"),
                        MetaSegment::indent(),
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        MetaSegment::dedent()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IGNORE"),
                        Ref::keyword("NULL"),
                        Ref::keyword("UPDATES")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Ref::new("WhereClauseSegment").optional(),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("APPLY"),
                        Ref::keyword("AS"),
                        one_of(vec_of_erased![
                            Ref::keyword("DELETE"),
                            Ref::keyword("TRUNCATE")
                        ]),
                        Ref::keyword("WHEN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment")
                    ])])
                    .config(|config| {
                        config.max_times = Some(2);
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SEQUENCE"),
                        Ref::keyword("BY"),
                        Ref::new("ColumnReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMNS"),
                        one_of(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                            Sequence::new(vec_of_erased![
                                Ref::new("StarSegment"),
                                Ref::keyword("EXCEPT"),
                                Ref::new("BracketedColumnReferenceListGrammar")
                            ])
                        ])
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("STORED"),
                        Ref::keyword("AS"),
                        Ref::keyword("SCD"),
                        Ref::keyword("TYPE"),
                        Ref::new("NumericLiteralSegment")
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TRACK"),
                        Ref::keyword("HISTORY"),
                        Ref::keyword("ON"),
                        one_of(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                            Sequence::new(vec_of_erased![
                                Ref::new("StarSegment"),
                                Ref::keyword("EXCEPT"),
                                Ref::new("BracketedColumnReferenceListGrammar")
                            ])
                        ])
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "WildcardExpressionSegment",
        ansi::wildcard_expression_segment().copy(
            Some(vec_of_erased![Ref::new("ExceptClauseSegment").optional()]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );

    // A reference to an object.
    // allow whitespace
    sparksql_dialect.replace_grammar(
        "ObjectReferenceSegment",
        Delimited::new(vec_of_erased![one_of(vec_of_erased![
            Ref::new("SingleIdentifierGrammar"),
            Ref::new("IdentifierClauseSegment")
        ])])
        .config(|config| {
            config.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
            config.terminators = vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
            config.disallow_gaps();
        })
        .to_matchable(),
    );

    sparksql_dialect.add([
        (
            "ExceptClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectExceptClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXCEPT"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SELECT"),
                    one_of(vec_of_erased![
                        Ref::new("TransformClauseSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("SelectClauseModifierSegment").optional(),
                            MetaSegment::indent(),
                            Delimited::new(vec_of_erased![Ref::new("SelectClauseElementSegment")])
                                .config(|config| {
                                    config.allow_trailing = true;
                                })
                        ])
                    ])
                ])
                .config(|config| {
                    config.terminators = vec_of_erased![Ref::new("SelectClauseTerminatorGrammar"),];
                    config.parse_mode(ParseMode::GreedyOnceStarted);
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "UsingClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::UsingClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Ref::new("DataSourceFormatSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DataSourceFormatSegment".into(),
            NodeMatcher::new(SyntaxKind::DataSourceFormat, |_| {
                one_of(vec_of_erased![
                    Ref::new("FileFormatGrammar"),
                    Ref::keyword("JDBC"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IcebergTransformationSegment".into(),
            NodeMatcher::new(SyntaxKind::IcebergTransformation, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("YEARS"),
                            Ref::keyword("MONTHS"),
                            Ref::keyword("DAYS"),
                            Ref::keyword("DATE"),
                            Ref::keyword("HOURS"),
                            Ref::keyword("DATE_HOUR")
                        ]),
                        Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("BUCKET"),
                            Ref::keyword("TRUNCATE")
                        ]),
                        Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("CommaSegment"),
                            Ref::new("ColumnReferenceSegment")
                        ])])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.add([
        (
            // Show Functions
            "ShowFunctionsGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("USER"),
                    Ref::keyword("SYSTEM"),
                    Ref::keyword("ALL")
                ])
                .config(|config| {
                    config.optional();
                }),
                Ref::keyword("FUNCTIONS"),
                one_of(vec_of_erased![
                    // qualified function from a database
                    Sequence::new(vec_of_erased![
                        Ref::new("DatabaseReferenceSegment"),
                        Ref::new("DotSegment"),
                        Ref::new("FunctionNameSegment")
                    ])
                    .config(|config| {
                        config.disallow_gaps();
                        config.optional();
                    }),
                    // non-qualified function
                    Ref::new("FunctionNameSegment").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LIKE"),
                        Ref::new("QuotedLiteralSegment")
                    ])
                    .config(|config| {
                        config.optional();
                    })
                ])
                .config(|config| {
                    config.optional();
                })
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ShowTablesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TABLES"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
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
            .to_matchable()
            .into(),
        ),
        (
            "ShowDatabasesSchemasGrammar".into(),
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
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ShowObjectGrammar".into(),
            one_of(vec_of_erased![Sequence::new(vec_of_erased![one_of(
                vec_of_erased![
                    Ref::new("ShowFunctionsGrammar"),
                    Ref::new("ShowViewsGrammar"),
                    Ref::new("ShowTablesGrammar"),
                    Ref::new("ShowDatabasesSchemasGrammar"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CREATE"),
                        Ref::keyword("TABLE"),
                        Ref::new("TableExpressionSegment"),
                        Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::keyword("SERDE")])
                            .config(|config| {
                                config.optional();
                            })
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMNS"),
                        Ref::keyword("IN"),
                        Ref::new("TableExpressionSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("IN"),
                            Ref::new("DatabaseReferenceSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        })
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("USER"),
                            Ref::keyword("SYSTEM"),
                            Ref::keyword("ALL")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Ref::keyword("FUNCTIONS"),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("DatabaseReferenceSegment"),
                                Ref::new("DotSegment"),
                                Ref::new("FunctionNameSegment")
                            ])
                            .config(|config| {
                                config.disallow_gaps();
                                config.optional();
                            }),
                            Ref::new("FunctionNameSegment").optional(),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("LIKE"),
                                Ref::new("QuotedLiteralSegment")
                            ])
                            .config(|config| {
                                config.optional();
                            })
                        ])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PARTITIONS"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("PartitionSpecGrammar").optional()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TABLE"),
                        Ref::keyword("EXTENDED"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("IN"), Ref::keyword("FROM")]),
                            Ref::new("DatabaseReferenceSegment")
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Ref::keyword("LIKE"),
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("PartitionSpecGrammar").optional()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TBLPROPERTIES"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("BracketedPropertyNameListGrammar").optional()
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("VIEWS"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
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
                ]
            )])])
            .to_matchable()
            .into(),
        ),
    ]);

    sparksql_dialect.replace_grammar(
        "FrameClauseSegment",
        {
            let frame_extent = one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("CURRENT"), Ref::keyword("ROW")]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::keyword("UNBOUNDED"),
                        Ref::new("IntervalExpressionSegment")
                    ]),
                    one_of(vec_of_erased![
                        Ref::keyword("PRECEDING"),
                        Ref::keyword("FOLLOWING")
                    ])
                ])
            ]);

            Sequence::new(vec_of_erased![
                Ref::new("FrameClauseUnitGrammar"),
                one_of(vec_of_erased![
                    frame_extent.clone(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BETWEEN"),
                        frame_extent.clone(),
                        Ref::keyword("AND"),
                        frame_extent
                    ])
                ])
            ])
        }
        .to_matchable(),
    );
    sparksql_dialect
}

pub fn dialect() -> Dialect {
    raw_dialect().config(|config| config.expand())
}
