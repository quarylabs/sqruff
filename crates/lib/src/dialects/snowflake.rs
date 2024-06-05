use std::sync::Arc;

use ahash::AHashSet;
use itertools::Itertools;

use super::ansi::{self, ansi_raw_dialect, Node, NodeTrait};
use super::snowflake_keywords::{SNOWFLAKE_RESERVED_KEYWORDS, SNOWFLAKE_UNRESERVED_KEYWORDS};
use crate::core::dialects::base::Dialect;
use crate::core::parser::grammar::anyof::{any_set_of, one_of, optionally_bracketed, AnyNumberOf};
use crate::core::parser::grammar::base::{Nothing, Ref};
use crate::core::parser::grammar::delimited::Delimited;
use crate::core::parser::grammar::sequence::{Bracketed, Sequence};
use crate::core::parser::lexer::Matcher;
use crate::core::parser::matchable::Matchable;
use crate::core::parser::parsers::{MultiStringParser, RegexParser, StringParser, TypedParser};
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs, IdentifierSegment,
    Segment, SymbolSegment, SymbolSegmentNewArgs,
};
use crate::core::parser::segments::generator::SegmentGenerator;
use crate::core::parser::segments::meta::MetaSegment;
use crate::core::parser::types::ParseMode;
use crate::helpers::{Config, ToMatchable};
use crate::vec_of_erased;

trait Boxed {
    fn boxed(self) -> Arc<Self>;
}

impl<T> Boxed for T {
    fn boxed(self) -> Arc<Self>
    where
        Self: Sized,
    {
        Arc::new(self)
    }
}

pub fn snowflake_dialect() -> Dialect {
    let mut snowflake_dialect = ansi_raw_dialect();

    snowflake_dialect.node_mut::<ansi::SelectClauseElementSegment>().match_grammar =
        ansi::SelectClauseElementSegment::match_grammar()
            .copy(
                Some(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("SystemFunctionName"),
                    Bracketed::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                ])]),
                None,
                Some(Ref::new("WildcardExpressionSegment").to_matchable()),
                None,
                Vec::new(),
                false,
            )
            .into();

    snowflake_dialect.node_mut::<ansi::FromExpressionElementSegment>().match_grammar =
        Sequence::new(vec_of_erased![
            Ref::new("PreTableFunctionKeywordsGrammar").optional(),
            optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::new("FromClauseTerminatorGrammar"),
                    Ref::new("SamplingExpressionSegment"),
                    Ref::new("ChangesClauseSegment"),
                    Ref::new("JoinLikeClauseGrammar"),
                    Ref::keyword("CROSS"),
                ]))
                .optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("OFFSET"),
                Ref::new("AliasExpressionSegment"),
            ])
            .config(|this| this.optional()),
            Ref::new("SamplingExpressionSegment").optional(),
            Ref::new("PostTableExpressionGrammar").optional(),
        ])
        .to_matchable()
        .into();

    snowflake_dialect.patch_lexer_matchers(vec![
        Matcher::regex("single_quote", r"'([^'\\]|\\.|'')*'", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "single_quote", ..Default::default() },
            )
        }),
        Matcher::regex("inline_comment", r"(--|#|//)[^\n]*", |slice, marker| {
            CommentSegment::create(
                slice,
                marker.into(),
                CommentSegmentNewArgs {
                    r#type: "inline_comment",
                    trim_start: Some(vec!["--", "#", "//"]),
                },
            )
        }),
    ]);

    snowflake_dialect.insert_lexer_matchers(
        vec![
            Matcher::string("parameter_assigner", "=>", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "parameter_assigner", ..Default::default() },
                )
            }),
            Matcher::string("function_assigner", "->", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "function_assigner", ..Default::default() },
                )
            }),
            Matcher::regex("stage_path", r"(?:@[^\s;)]+|'@[^']+')", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "stage_path", ..Default::default() },
                )
            }),
            Matcher::regex("column_selector", r"\$[0-9]+", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "column_selector", ..Default::default() },
                )
            }),
            Matcher::regex("dollar_quote", r"\$\$.*\$\$", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "dollar_quote", ..Default::default() },
                )
            }),
            Matcher::regex("dollar_literal", r"[$][a-zA-Z0-9_.]*", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "dollar_literal", ..Default::default() },
                )
            }),
            Matcher::regex(
                "inline_dollar_sign",
                r"[a-zA-Z_][a-zA-Z0-9_$]*\$[a-zA-Z0-9_$]*",
                |slice, marker| {
                    CodeSegment::create(
                        slice,
                        marker.into(),
                        CodeSegmentNewArgs { code_type: "raw", ..Default::default() },
                    )
                },
            ),
            Matcher::regex(
                "unquoted_file_path",
                r"file://(?:[a-zA-Z]+:|/)+(?:[0-9a-zA-Z\\/_*?-]+)(?:\.[0-9a-zA-Z]+)?",
                |slice, marker| {
                    CodeSegment::create(
                        slice,
                        marker.into(),
                        CodeSegmentNewArgs {
                            code_type: "unquoted_file_path",
                            ..Default::default()
                        },
                    )
                },
            ),
            Matcher::string("question_mark", "?", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "question_mark", ..Default::default() },
                )
            }),
            Matcher::string("exclude_bracket_open", "{-", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "exclude_bracket_open", ..Default::default() },
                )
            }),
            Matcher::string("exclude_bracket_close", "-}", |slice, marker| {
                CodeSegment::create(
                    slice,
                    marker.into(),
                    CodeSegmentNewArgs { code_type: "exclude_bracket_close", ..Default::default() },
                )
            }),
        ],
        "like_operator",
    );

    snowflake_dialect.insert_lexer_matchers(
        vec![Matcher::string("walrus_operator", ":=", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "walrus_operator", ..Default::default() },
            )
        })],
        "equals",
    );

    snowflake_dialect.bracket_sets_mut("bracket_pairs").insert((
        "exclude",
        "StartExcludeBracketSegment",
        "EndExcludeBracketSegment",
        true,
    ));

    snowflake_dialect.sets_mut("bare_functions").clear();
    snowflake_dialect.sets_mut("bare_functions").extend([
        "CURRENT_DATE",
        "CURRENT_TIME",
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
        "LOCALTIME",
        "LOCALTIMESTAMP",
    ]);

    snowflake_dialect.sets_mut("compression_types").clear();
    snowflake_dialect.sets_mut("compression_types").extend([
        "AUTO",
        "AUTO_DETECT",
        "GZIP",
        "BZ2",
        "BROTLI",
        "ZSTD",
        "DEFLATE",
        "RAW_DEFLATE",
        "LZO",
        "NONE",
        "SNAPPY",
    ]);

    snowflake_dialect.sets_mut("files_types").clear();
    snowflake_dialect
        .sets_mut("files_types")
        .extend(["CSV", "JSON", "AVRO", "ORC", "PARQUET", "XML"]);

    snowflake_dialect.sets_mut("warehouse_types").clear();
    snowflake_dialect.sets_mut("warehouse_types").extend(["STANDARD", "SNOWPARK-OPTIMIZED"]);

    snowflake_dialect.sets_mut("warehouse_sizes").clear();
    snowflake_dialect.sets_mut("warehouse_sizes").extend([
        "XSMALL", "SMALL", "MEDIUM", "LARGE", "XLARGE", "XXLARGE", "X2LARGE", "XXXLARGE",
        "X3LARGE", "X4LARGE", "X5LARGE", "X6LARGE", "X-SMALL", "X-LARGE", "2X-LARGE", "3X-LARGE",
        "4X-LARGE", "5X-LARGE", "6X-LARGE",
    ]);

    snowflake_dialect.sets_mut("warehouse_scaling_policies").clear();
    snowflake_dialect.sets_mut("warehouse_scaling_policies").extend(["STANDARD", "ECONOMY"]);

    snowflake_dialect.add([
        (
            "ParameterAssignerSegment".into(),
            StringParser::new(
                "=>",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "parameter_assigner" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "FunctionAssignerSegment".into(),
            StringParser::new(
                "->",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "function_assigner" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "WalrusOperatorSegment".into(),
            StringParser::new(
                ":=",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "assignment_operator" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "QuotedStarSegment".into(),
            StringParser::new(
                "'*'",
                |segment: &dyn Segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "quoted_star", ..Default::default() },
                    )
                },
                None,
                false,
                vec!['\''].into(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "NakedSemiStructuredElementSegment".into(),
            RegexParser::new(
                "[A-Z0-9_]*",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "semi_structured_element",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "QuotedSemiStructuredElementSegment".into(),
            TypedParser::new(
                "double_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "semi_structured_element" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ColumnIndexIdentifierSegment".into(),
            RegexParser::new(
                r"\$[0-9]+",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "column_index_identifier_segment",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "LocalVariableNameSegment".into(),
            RegexParser::new(
                r"[a-zA-Z0-9_]*",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "variable", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "ReferencedVariableNameSegment".into(),
            RegexParser::new(
                r"\$[A-Z_][A-Z0-9_]*",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "variable", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "WarehouseType".into(),
            one_of(vec_of_erased![
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("warehouse_types")
                        .into_iter()
                        .filter(|it| !it.contains('-'))
                        .map(Into::into)
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "warehouse_size",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("warehouse_types")
                        .into_iter()
                        .map(|it| format!("'{it}'"))
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "warehouse_size",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WarehouseSize".into(),
            one_of(vec_of_erased![
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("warehouse_sizes")
                        .into_iter()
                        .filter(|it| !it.contains('-'))
                        .map(Into::into)
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "warehouse_size",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("warehouse_sizes")
                        .into_iter()
                        .map(|it| format!("'{it}'"))
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "warehouse_size",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "CompressionType".into(),
            one_of(vec_of_erased![
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("compression_types")
                        .into_iter()
                        .map(Into::into)
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "compression_type",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("compression_types")
                        .into_iter()
                        .map(|it| format!("'{it}'"))
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "compression_type",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ScalingPolicy".into(),
            one_of(vec_of_erased![
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("warehouse_scaling_policies")
                        .into_iter()
                        .filter(|it| !it.contains('-'))
                        .map(Into::into)
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "scaling_policy",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("warehouse_scaling_policies")
                        .into_iter()
                        .map(|it| format!("'{it}'"))
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "scaling_policy",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ValidationModeOptionSegment".into(),
            RegexParser::new(
                r"'?RETURN_(?:\d+_ROWS|ERRORS|ALL_ERRORS)'?",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "validation_mode_option",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "CopyOptionOnErrorSegment".into(),
            RegexParser::new(
                r"'?CONTINUE'?|'?SKIP_FILE(?:_[0-9]+%?)?'?|'?ABORT_STATEMENT'?",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "copy_on_error_option",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DoubleQuotedUDFBody".into(),
            TypedParser::new(
                "double_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "udf_body" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SingleQuotedUDFBody".into(),
            TypedParser::new(
                "single_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "udf_body" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DollarQuotedUDFBody".into(),
            TypedParser::new(
                "dollar_quote",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "udf_body" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "StagePath".into(),
            RegexParser::new(
                r"(?:@[^\s;)]+|'@[^']+')",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "stage_path", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "S3Path".into(),
            RegexParser::new(
                r"'s3://[a-z0-9][a-z0-9\.-]{1,61}[a-z0-9](?:/.*)?'",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "bucket_path", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "GCSPath".into(),
            RegexParser::new(
                r"'gcs://[a-z0-9][\w\.-]{1,61}[a-z0-9](?:/.+)?'",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "bucket_path", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "AzureBlobStoragePath".into(),
            RegexParser::new(
                r"'azure://[a-z0-9][a-z0-9-]{1,61}[a-z0-9]\.blob\.core\.windows\.net/[a-z0-9][a-z0-9\.-]{1,61}[a-z0-9](?:/.+)?'",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs { code_type: "bucket_path", ..Default::default() },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "UnquotedFilePath".into(),
            TypedParser::new(
                "unquoted_file_path",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs { r#type: "unquoted_file_path" },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SnowflakeEncryptionOption".into(),
            MultiStringParser::new(
                vec!["'SNOWFLAKE_FULL'".into(), "'SNOWFLAKE_SSE'".into()],
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "stage_encryption_option",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
            ).to_matchable().into(),
        ),
        (
            "S3EncryptionOption".into(),
            MultiStringParser::new(
                vec!["'AWS_CSE'".into(), "'AWS_SSE_S3'".into(), "'AWS_SSE_KMS'".into()],
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "stage_encryption_option",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
            ).to_matchable().into(),
        ),
        (
            "GCSEncryptionOption".into(),
            MultiStringParser::new(
                vec!["'GCS_SSE_KMS'".into()],
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "stage_encryption_option",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
            ).to_matchable().into(),
        ),
        (
            "AzureBlobStorageEncryptionOption".into(),
            MultiStringParser::new(
                vec!["'AZURE_CSE'".into()],
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "stage_encryption_option",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
            ).to_matchable().into(),
        ),
        (
            "FileType".into(),
            one_of(vec_of_erased![
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("file_types")
                        .into_iter()
                        .filter(|it| !it.contains('-'))
                        .map(Into::into)
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "file_type",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                ),
                MultiStringParser::new(
                    snowflake_dialect
                        .sets("file_types")
                        .into_iter()
                        .map(|it| format!("'{it}'"))
                        .collect_vec(),
                    |segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "file_type",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None,
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IntegerSegment".into(),
            RegexParser::new(
                r"[0-9]+",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "integer_literal",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "SystemFunctionName".into(),
            RegexParser::new(
                r"SYSTEM\$([A-Za-z0-9_]*)",
                |segment| {
                    CodeSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        CodeSegmentNewArgs {
                            code_type: "system_function_name",
                            ..Default::default()
                        },
                    )
                },
                None,
                false,
                None,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "GroupByContentsGrammar".into(),
            Delimited::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    // Can `GROUP BY 1`
                    Ref::new("NumericLiteralSegment"),
                    // Can `GROUP BY coalesce(col, 1)`
                    Ref::new("ExpressionSegment"),
                ])
            ]).config(|this|this.terminators = vec_of_erased![
                Ref::keyword("ORDER"),
                Ref::keyword("LIMIT"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
            ]).to_matchable().into()
        ),
        (
            "LimitLiteralGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Ref::keyword("NULL"),
                Ref::new("QuotedLiteralSegment")
            ]).to_matchable().into()
        ),
        (
            "StartExcludeBracketSegment".into(),
            StringParser::new(
                "{-",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs {
                            r#type: "start_exclude_bracket",
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "EndExcludeBracketSegment".into(),
            StringParser::new(
                "-}",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs {
                            r#type: "end_exclude_bracket",
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "QuestionMarkSegment".into(),
            StringParser::new(
                "?",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs {
                            r#type: "question_mark",
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "CaretSegment".into(),
            StringParser::new(
                "^",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs {
                            r#type: "caret",
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "DollarSegment".into(),
            StringParser::new(
                "$",
                |segment: &dyn Segment| {
                    SymbolSegment::create(
                        &segment.raw(),
                        segment.get_position_marker(),
                        SymbolSegmentNewArgs {
                            r#type: "dollar",
                        },
                    )
                },
                None,
                false,
                None,
            )
            .to_matchable()
            .into(),
        ),
        (
            "PatternQuantifierGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("PositiveSegment"),
                    Ref::new("StarSegment"),
                    Ref::new("QuestionMarkSegment"),
                    Bracketed::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::new("CommaSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::new("CommaSegment"),
                                Ref::new("NumericLiteralSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::new("CommaSegment"),
                                Ref::new("NumericLiteralSegment"),
                            ]),
                        ])
                    ]).config(|this| {
                        this.bracket_type = "curly";
                        this.bracket_pairs_set = "bracket_pairs";
                    })
                ]),
                Ref::new("QuestionMarkSegment").optional()
            ]).config(|this| {
                this.allow_gaps = false;
            }).to_matchable().into()
        ),
        (
            "PatternSymbolGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("PatternQuantifierGrammar").optional()
            ]).config(|this| {
                this.allow_gaps = false;
            }).to_matchable().into()
        ),
        (
            "PatternOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("PatternSymbolGrammar"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Bracketed::new(vec_of_erased![
                            one_of(vec_of_erased![
                                AnyNumberOf::new(vec_of_erased![Ref::new("PatternOperatorGrammar")]),
                                Delimited::new(
                                    vec_of_erased![
                                        Ref::new("PatternOperatorGrammar"),
                                    ]
                                )
                                .config(|this|this.delimiter(Ref::new("BitwiseOrSegment"))),
                            ])
                        ]).config(|this| {
                            this.bracket_type = "exclude";
                            this.bracket_pairs_set = "bracket_pairs";
                        }),
                        Bracketed::new(vec_of_erased![
                            one_of(vec_of_erased![
                                AnyNumberOf::new(vec_of_erased![Ref::new("PatternOperatorGrammar")]),
                                Delimited::new(
                                    vec_of_erased![
                                        Ref::new("PatternOperatorGrammar"),
                                    ]
                                )
                                .config(|this|this.delimiter(Ref::new("BitwiseOrSegment"))),
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PERMUTE"),
                            Bracketed::new(vec_of_erased![Delimited::new(
                                vec_of_erased![
                                    Ref::new("PatternSymbolGrammar"),
                                ]
                            )]),
                        ]),
                    ]),
                    Ref::new("PatternQuantifierGrammar").optional()
                ]).config(|this| {
                    this.allow_gaps = false;
                }),
            ]).to_matchable().into()
        ),
        (
            "ContextHeadersGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("CURRENT_ACCOUNT"),
                Ref::keyword("CURRENT_CLIENT"),
                Ref::keyword("CURRENT_DATABASE"),
                Ref::keyword("CURRENT_DATE"),
                Ref::keyword("CURRENT_IP_ADDRESS"),
                Ref::keyword("CURRENT_REGION"),
                Ref::keyword("CURRENT_ROLE"),
                Ref::keyword("CURRENT_SCHEMA"),
                Ref::keyword("CURRENT_SCHEMAS"),
                Ref::keyword("CURRENT_SESSION"),
                Ref::keyword("CURRENT_STATEMENT"),
                Ref::keyword("CURRENT_TIME"),
                Ref::keyword("CURRENT_TIMESTAMP"),
                Ref::keyword("CURRENT_TRANSACTION"),
                Ref::keyword("CURRENT_USER"),
                Ref::keyword("CURRENT_VERSION"),
                Ref::keyword("CURRENT_WAREHOUSE"),
                Ref::keyword("LAST_QUERY_ID"),
                Ref::keyword("LAST_TRANSACTION"),
                Ref::keyword("LOCALTIME"),
                Ref::keyword("LOCALTIMESTAMP"),
            ]).to_matchable().into()
        )
    ]);

    snowflake_dialect.add([
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                // Generate the anti template from the set of reserved keywords
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({})$", pattern);

                RegexParser::new(
                    "[a-zA-Z_][a-zA-Z0-9_$]*",
                    |segment| {
                        IdentifierSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "naked_identifier",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    anti_template.into(),
                    None,
                )
                .boxed()
            })
            .into(),
        ),
        (
            "LiteralGrammar".into(),
            snowflake_dialect
                .grammar("LiteralGrammar")
                .copy(
                    Some(vec_of_erased![Ref::new("ReferencedVariableNameSegment")]),
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
            AnyNumberOf::new(vec_of_erased![
                Ref::new("ArrayAccessorSegment"),
                Ref::new("SemiStructuredAccessorSegment")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PreTableFunctionKeywordsGrammar".into(),
            one_of(vec_of_erased![Ref::keyword("LATERAL")]).to_matchable().into(),
        ),
        (
            "FunctionContentsExpressionGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("DatetimeUnitSegment"),
                Ref::new("NamedParameterExpressionSegment"),
                Ref::new("ReferencedVariableNameSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("IGNORE"), Ref::keyword("RESPECT"),]),
                        Ref::keyword("NULLS")
                    ])
                    .config(|this| this.optional())
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "JoinLikeClauseGrammar".into(),
            Sequence::new(vec_of_erased![
                any_set_of(vec_of_erased![
                    Ref::new("MatchRecognizeClauseSegment"),
                    Ref::new("ChangesClauseSegment"),
                    Ref::new("ConnectByClauseSegment"),
                    Ref::new("FromBeforeExpressionSegment"),
                    Ref::new("FromPivotExpressionSegment"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("FromUnpivotExpressionSegment")]),
                    Ref::new("SamplingExpressionSegment")
                ])
                .config(|this| this.min_times = 1),
                Ref::new("AliasExpressionSegment").optional()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SingleIdentifierGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                Ref::new("ColumnIndexIdentifierSegment"),
                Ref::new("ReferencedVariableNameSegment"),
                Ref::new("StagePath"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IDENTIFIER"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("SingleQuotedIdentifierSegment"),
                        Ref::new("ReferencedVariableNameSegment"),
                    ])]),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PostFunctionGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("WithinGroupClauseSegment").optional(),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("IGNORE"), Ref::keyword("RESPECT")]),
                    Ref::keyword("NULLS"),
                ])
                .config(|this| this.optional()),
                Ref::new("OverClauseSegment").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("LOCAL"), Ref::keyword("GLOBAL")])
                    .config(|this| this.optional()),
                one_of(vec_of_erased![Ref::keyword("TEMP"), Ref::keyword("TEMPORARY")])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![Ref::keyword("VOLATILE")])
                    .config(|this| this.optional()),
            ])
            .config(|this| this.optional())
            .to_matchable()
            .into(),
        ),
        (
            "TemporaryTransientGrammar".into(),
            one_of(vec_of_erased![Ref::new("TemporaryGrammar"), Ref::keyword("TRANSIENT")])
                .to_matchable()
                .into(),
        ),
        (
            "BaseExpressionElementGrammar".into(),
            snowflake_dialect
                .grammar("BaseExpressionElementGrammar")
                .copy(
                    Some(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("CONNECT_BY_ROOT"),
                        Ref::new("ColumnReferenceSegment")
                    ])]),
                    None,
                    Some(Ref::new("LiteralGrammar").to_matchable()),
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            "QuotedLiteralSegment".into(),
            one_of(vec_of_erased![
                TypedParser::new(
                    "single_quote",
                    |segment: &dyn Segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "quoted_literal",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None
                ),
                TypedParser::new(
                    "dollar_quote",
                    |segment: &dyn Segment| {
                        CodeSegment::create(
                            &segment.raw(),
                            segment.get_position_marker(),
                            CodeSegmentNewArgs {
                                code_type: "quoted_literal",
                                ..Default::default()
                            },
                        )
                    },
                    None,
                    false,
                    None
                )
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LikeGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE"),
                    one_of(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("ANY"),])
                        .config(|this| this.optional())
                ]),
                Ref::keyword("RLIKE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ILIKE"),
                    Ref::keyword("ANY").optional(),
                ]),
                Ref::keyword("REGEXP"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SelectClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("FROM"),
                Ref::keyword("WHERE"),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Ref::keyword("LIMIT"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Ref::new("SetOperatorSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FromClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("WHERE"),
                Ref::keyword("LIMIT"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY"),]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY"),]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::new("SetOperatorSegment"),
                Ref::new("WithNoSchemaBindingClauseSegment"),
                Ref::new("WithDataClauseSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "WhereClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY"),]),
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY"),]),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::keyword("OVERLAPS"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "OrderByClauseTerminators".into(),
            one_of(vec_of_erased![
                Ref::keyword("LIMIT"),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::new("FrameClauseUnitGrammar"),
                Ref::keyword("SEPARATOR"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Ref::keyword("MEASURES"),
            ])
            .to_matchable()
            .into(),
        ),
        ("TrimParametersGrammar".into(), Nothing::new().to_matchable().into()),
        (
            "GroupByClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("ORDER"),
                Ref::keyword("LIMIT"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Ref::keyword("HAVING"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "HavingClauseTerminatorGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY"),]),
                Ref::keyword("LIMIT"),
                Ref::keyword("QUALIFY"),
                Ref::keyword("WINDOW"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    snowflake_dialect.sets_mut("unreserved_keywords").clear();
    snowflake_dialect.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        SNOWFLAKE_UNRESERVED_KEYWORDS,
    );

    snowflake_dialect.sets_mut("reserved_keywords").clear();
    snowflake_dialect.update_keywords_set_from_multiline_string(
        "reserved_keywords",
        SNOWFLAKE_RESERVED_KEYWORDS,
    );

    snowflake_dialect.sets_mut("datetime_units").clear();
    snowflake_dialect.sets_mut("datetime_units").extend([
        "YEAR",
        "Y",
        "YY",
        "YYY",
        "YYYY",
        "YR",
        "YEARS",
        "YRS",
        "MONTH",
        "MM",
        "MON",
        "MONS",
        "MONTHS",
        "DAY",
        "D",
        "DD",
        "DAYS",
        "DAYOFMONTH",
        "DAYOFWEEK",
        "WEEKDAY",
        "DOW",
        "DW",
        "DAYOFWEEKISO",
        "WEEKDAY_ISO",
        "DOW_ISO",
        "DW_ISO",
        "DAYOFYEAR",
        "YEARDAY",
        "DOY",
        "DY",
        "WEEK",
        "W",
        "WK",
        "WEEKOFYEAR",
        "WOY",
        "WY",
        "WEEKISO",
        "WEEK_ISO",
        "WEEKOFYEARISO",
        "WEEKOFYEAR_ISO",
        "QUARTER",
        "Q",
        "QTR",
        "QTRS",
        "QUARTERS",
        "YEAROFWEEK",
        "YEAROFWEEKISO",
        "HOUR",
        "H",
        "HH",
        "HR",
        "HOURS",
        "HRS",
        "MINUTE",
        "M",
        "MI",
        "MIN",
        "MINUTES",
        "MINS",
        "SECOND",
        "S",
        "SEC",
        "SECONDS",
        "SECS",
        "MILLISECOND",
        "MS",
        "MSEC",
        "MILLISECONDS",
        "MICROSECOND",
        "US",
        "USEC",
        "MICROSECONDS",
        "NANOSECOND",
        "NS",
        "NSEC",
        "NANOSEC",
        "NSECOND",
        "NANOSECONDS",
        "NANOSECS",
        "NSECONDS",
        "EPOCH_SECOND",
        "EPOCH",
        "EPOCH_SECONDS",
        "EPOCH_MILLISECOND",
        "EPOCH_MILLISECONDS",
        "EPOCH_MICROSECOND",
        "EPOCH_MICROSECONDS",
        "EPOCH_NANOSECOND",
        "EPOCH_NANOSECONDS",
        "TIMEZONE_HOUR",
        "TZH",
        "TIMEZONE_MINUTE",
        "TZM",
    ]);

    macro_rules! add_segments {
        ($dialect:ident, $( $segment:ident ),*) => {
            $(
                $dialect.add([(
                    stringify!($segment).into(),
                    Node::<$segment>::new().to_matchable().into(),
                )]);
            )*
        }
    }

    add_segments!(
        snowflake_dialect,
        FunctionNameSegment,
        ConnectByClauseSegment,
        GroupByClauseSegment,
        ValuesClauseSegment,
        InsertStatementSegment,
        FunctionDefinitionGrammar,
        StatementSegment,
        SetAssignmentStatementSegment,
        CallStoredProcedureSegment,
        WithinGroupClauseSegment,
        PatternSegment,
        MatchRecognizeClauseSegment,
        ChangesClauseSegment,
        FromAtExpressionSegment,
        FromBeforeExpressionSegment,
        FromPivotExpressionSegment,
        FromUnpivotExpressionSegment,
        SamplingExpressionSegment,
        NamedParameterExpressionSegment,
        SemiStructuredAccessorSegment,
        QualifyClauseSegment,
        SelectStatementSegment,
        WildcardExpressionSegment,
        ExcludeClauseSegment,
        RenameClauseSegment,
        ReplaceClauseSegment,
        SelectClauseModifierSegment,
        AlterTableStatementSegment,
        AlterTableTableColumnActionSegment,
        AlterTableClusteringActionSegment,
        AlterTableConstraintActionSegment,
        AlterWarehouseStatementSegment,
        AlterShareStatementSegment,
        AlterStorageIntegrationSegment,
        AlterExternalTableStatementSegment,
        CommentEqualsClauseSegment,
        TagBracketedEqualsSegment,
        TagEqualsSegment,
        UnorderedSelectStatementSegment,
        AccessStatementSegment,
        CreateCloneStatementSegment,
        CreateDatabaseFromShareStatementSegment,
        CreateProcedureStatementSegment,
        ReturnStatementSegment,
        ScriptingBlockStatementSegment,
        ScriptingLetStatementSegment,
        CreateFunctionStatementSegment,
        AlterFunctionStatementSegment,
        CreateExternalFunctionStatementSegment,
        WarehouseObjectPropertiesSegment,
        WarehouseObjectParamsSegment,
        ConstraintPropertiesSegment,
        ColumnConstraintSegment,
        CopyOptionsSegment,
        CreateSchemaStatementSegment,
        AlterRoleStatementSegment,
        AlterSchemaStatementSegment,
        SchemaObjectParamsSegment,
        CreateTableStatementSegment,
        CreateTaskSegment,
        TaskExpressionSegment,
        CreateStatementSegment,
        CreateUserSegment,
        CreateViewStatementSegment,
        AlterViewStatementSegment,
        AlterMaterializedViewStatementSegment,
        CreateFileFormatSegment,
        AlterFileFormatSegment,
        CsvFileFormatTypeParameters,
        JsonFileFormatTypeParameters,
        AvroFileFormatTypeParameters,
        OrcFileFormatTypeParameters,
        ParquetFileFormatTypeParameters,
        XmlFileFormatTypeParameters,
        AlterPipeSegment,
        FileFormatSegment,
        FormatTypeOptions,
        OrcFileFormatTypeParameters,
        ParquetFileFormatTypeParameters,
        XmlFileFormatTypeParameters,
        AlterPipeSegment,
        FileFormatSegment,
        FormatTypeOptions,
        CreateExternalTableSegment,
        TableExpressionSegment,
        PartitionBySegment,
        CopyIntoLocationStatementSegment,
        CopyIntoTableStatementSegment,
        StorageLocation,
        InternalStageParameters,
        S3ExternalStageParameters,
        GCSExternalStageParameters,
        AzureBlobStorageExternalStageParameters,
        CreateStageSegment,
        AlterStageSegment,
        CreateStreamStatementSegment,
        AlterStreamStatementSegment,
        ShowStatementSegment,
        CreateStageSegment,
        AlterStageSegment,
        CreateStreamStatementSegment,
        AlterStreamStatementSegment,
        ShowStatementSegment,
        AlterUserStatementSegment,
        CreateRoleStatementSegment,
        ExplainStatementSegment,
        AlterSessionStatementSegment,
        AlterSessionSetClauseSegment,
        AlterSessionUnsetClauseSegment,
        AlterTaskStatementSegment,
        AlterTaskSpecialSetClauseSegment,
        AlterTaskSetClauseSegment,
        AlterTaskUnsetClauseSegment,
        ExecuteTaskClauseSegment,
        MergeUpdateClauseSegment,
        MergeDeleteClauseSegment,
        MergeInsertClauseSegment,
        DeleteStatementSegment,
        DescribeStatementSegment,
        TransactionStatementSegment,
        TruncateStatementSegment,
        UnsetStatementSegment,
        UndropStatementSegment,
        CommentStatementSegment,
        UseStatementSegment,
        CallStatementSegment,
        LimitClauseSegment,
        SelectClauseSegment,
        OrderByClauseSegment,
        FrameClauseSegment,
        DropProcedureStatementSegment,
        DropExternalTableStatementSegment,
        DropFunctionStatementSegment,
        DropMaterializedViewStatementSegment,
        DropObjectStatementSegment,
        ListStatementSegment,
        GetStatementSegment,
        PutStatementSegment,
        RemoveStatementSegment,
        SetOperatorSegment,
        ShorthandCastSegment
    );

    snowflake_dialect.expand();
    snowflake_dialect
}

pub struct FunctionNameSegment;

impl NodeTrait for FunctionNameSegment {
    const TYPE: &'static str = "function_name";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            // Project name, schema identifier, etc.
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("DotSegment"),
            ]),])
            .config(|this| {
                this.terminators = vec_of_erased![Ref::new("BracketedSegment")];
            }),
            // Base function name
            one_of(vec_of_erased![
                Ref::new("FunctionNameIdentifierSegment"),
                Ref::new("QuotedIdentifierSegment"),
                // Snowflake's IDENTIFIER pseudo-function
                Sequence::new(vec_of_erased![
                    Ref::keyword("IDENTIFIER"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("SingleQuotedIdentifierSegment"),
                        Ref::new("ReferencedVariableNameSegment"),
                    ])]),
                ]),
            ]),
        ])
        .config(|this| {
            this.allow_gaps = false;
        })
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["function_name"].into()
    }
}

pub struct ConnectByClauseSegment;

impl NodeTrait for ConnectByClauseSegment {
    const TYPE: &'static str = "connectby_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("START"),
            Ref::keyword("WITH"),
            Ref::new("ExpressionSegment"),
            Ref::keyword("CONNECT"),
            Ref::keyword("BY"),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("PRIOR").optional(),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("EqualsSegment"),
                Ref::keyword("PRIOR").optional(),
                Ref::new("ColumnReferenceSegment"),
            ])])
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["connectby_clause"].into()
    }
}

pub struct GroupByClauseSegment;

impl NodeTrait for GroupByClauseSegment {
    const TYPE: &'static str = "groupby_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("GROUP"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CUBE"),
                        Ref::keyword("ROLLUP"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("GROUPING"),
                            Ref::keyword("SETS"),
                        ]),
                    ]),
                    Bracketed::new(vec_of_erased![Ref::new("GroupByContentsGrammar"),]),
                ]),
                Ref::keyword("ALL"),
                Ref::new("GroupByContentsGrammar"),
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["groupby_clause"].into()
    }
}

pub struct ValuesClauseSegment;

impl NodeTrait for ValuesClauseSegment {
    const TYPE: &'static str = "values_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("VALUES"),
            Delimited::new(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    Ref::keyword("NULL"),
                    Ref::new("ExpressionSegment"),
                ])])
                .config(|this| {
                    this.parse_mode = ParseMode::Greedy;
                }),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["values_clause"].into()
    }
}

pub struct InsertStatementSegment;

impl NodeTrait for InsertStatementSegment {
    const TYPE: &'static str = "insert_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INSERT"),
            Ref::keyword("OVERWRITE").optional(),
            one_of(vec_of_erased![
                // Single table INSERT INTO
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTO"),
                    Ref::new("TableReferenceSegment"),
                    Ref::new("BracketedColumnReferenceListGrammar").optional(),
                    Ref::new("SelectableGrammar"),
                ]),
                // Unconditional multi-table INSERT INTO
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALL"),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("INTO"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("BracketedColumnReferenceListGrammar").optional(),
                        Ref::new("ValuesClauseSegment").optional(),
                    ]),])
                    .config(|this| {
                        this.min_times = 1;
                    }),
                    Ref::new("SelectStatementSegment"),
                ]),
                // Conditional multi-table INSERT INTO
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("ALL")]),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("WHEN"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("THEN"),
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::keyword("INTO"),
                            Ref::new("TableReferenceSegment"),
                            Ref::new("BracketedColumnReferenceListGrammar").optional(),
                            Ref::new("ValuesClauseSegment").optional(),
                        ]),])
                        .config(|this| {
                            this.min_times = 1;
                        }),
                    ]),])
                    .config(|this| {
                        this.min_times = 1;
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ELSE"),
                        Ref::keyword("INTO"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("BracketedColumnReferenceListGrammar").optional(),
                        Ref::new("ValuesClauseSegment").optional(),
                    ])
                    .config(|this| this.optional()),
                    Ref::new("SelectStatementSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["insert_statement"].into()
    }
}

pub struct FunctionDefinitionGrammar;

impl NodeTrait for FunctionDefinitionGrammar {
    const TYPE: &'static str = "function_definition";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("AS"),
            Ref::new("QuotedLiteralSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("LANGUAGE"),
                Ref::new("NakedIdentifierSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["function_definition"].into()
    }
}

pub struct StatementSegment;

impl NodeTrait for StatementSegment {
    const TYPE: &'static str = "statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::StatementSegment::match_grammar().copy(
            Some(vec_of_erased![
                Ref::new("AccessStatementSegment"),
                Ref::new("CreateStatementSegment"),
                Ref::new("CreateTaskSegment"),
                Ref::new("CreateUserSegment"),
                Ref::new("CreateCloneStatementSegment"),
                Ref::new("CreateProcedureStatementSegment"),
                Ref::new("ScriptingBlockStatementSegment"),
                Ref::new("ScriptingLetStatementSegment"),
                Ref::new("ReturnStatementSegment"),
                Ref::new("ShowStatementSegment"),
                Ref::new("AlterUserStatementSegment"),
                Ref::new("AlterSessionStatementSegment"),
                Ref::new("AlterTaskStatementSegment"),
                Ref::new("SetAssignmentStatementSegment"),
                Ref::new("CallStoredProcedureSegment"),
                Ref::new("MergeStatementSegment"),
                Ref::new("CopyIntoTableStatementSegment"),
                Ref::new("CopyIntoLocationStatementSegment"),
                Ref::new("FormatTypeOptions"),
                Ref::new("AlterWarehouseStatementSegment"),
                Ref::new("AlterShareStatementSegment"),
                Ref::new("CreateExternalTableSegment"),
                Ref::new("AlterExternalTableStatementSegment"),
                Ref::new("CreateSchemaStatementSegment"),
                Ref::new("AlterSchemaStatementSegment"),
                Ref::new("CreateFunctionStatementSegment"),
                Ref::new("AlterFunctionStatementSegment"),
                Ref::new("CreateExternalFunctionStatementSegment"),
                Ref::new("CreateStageSegment"),
                Ref::new("AlterStageSegment"),
                Ref::new("CreateStreamStatementSegment"),
                Ref::new("AlterStreamStatementSegment"),
                Ref::new("UnsetStatementSegment"),
                Ref::new("UndropStatementSegment"),
                Ref::new("CommentStatementSegment"),
                Ref::new("CallStatementSegment"),
                Ref::new("AlterViewStatementSegment"),
                Ref::new("AlterMaterializedViewStatementSegment"),
                Ref::new("DropProcedureStatementSegment"),
                Ref::new("DropExternalTableStatementSegment"),
                Ref::new("DropMaterializedViewStatementSegment"),
                Ref::new("DropObjectStatementSegment"),
                Ref::new("CreateFileFormatSegment"),
                Ref::new("AlterFileFormatSegment"),
                Ref::new("AlterPipeSegment"),
                Ref::new("ListStatementSegment"),
                Ref::new("GetStatementSegment"),
                Ref::new("PutStatementSegment"),
                Ref::new("RemoveStatementSegment"),
                Ref::new("CreateDatabaseFromShareStatementSegment"),
                Ref::new("AlterRoleStatementSegment"),
                Ref::new("AlterStorageIntegrationSegment"),
                Ref::new("ExecuteTaskClauseSegment"),
            ]),
            None,
            None,
            Some(vec_of_erased![
                Ref::new("CreateIndexStatementSegment"),
                Ref::new("DropIndexStatementSegment"),
            ]),
            Vec::new(),
            false,
        )
    }
}

pub struct SetAssignmentStatementSegment;

impl NodeTrait for SetAssignmentStatementSegment {
    const TYPE: &'static str = "set_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::new("LocalVariableNameSegment"),
                Ref::new("EqualsSegment"),
                Ref::new("ExpressionSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "LocalVariableNameSegment"
                ),]),]),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ExpressionSegment"
                ),]),]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["set_statement"].into()
    }
}

pub struct CallStoredProcedureSegment;

impl NodeTrait for CallStoredProcedureSegment {
    const TYPE: &'static str = "call_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("CALL"), Ref::new("FunctionSegment"),])
            .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["call_segment"].into()
    }
}

pub struct WithinGroupClauseSegment;

impl NodeTrait for WithinGroupClauseSegment {
    const TYPE: &'static str = "withingroup_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("WITHIN"),
            Ref::keyword("GROUP"),
            Bracketed::new(vec_of_erased![Ref::new("OrderByClauseSegment").optional(),]).config(
                |this| {
                    this.parse_mode = ParseMode::Greedy;
                }
            ),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["withingroup_clause"].into()
    }
}

pub struct PatternSegment;

impl NodeTrait for PatternSegment {
    const TYPE: &'static str = "pattern_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("CaretSegment").optional(),
            one_of(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![Ref::new("PatternOperatorGrammar")]),
                Delimited::new(vec_of_erased![Ref::new("PatternOperatorGrammar")])
                    .config(|this| this.delimiter(Ref::new("BitwiseOrSegment"))),
            ]),
            Ref::new("DollarSegment").optional(),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["pattern_expression"].into()
    }
}

pub struct MatchRecognizeClauseSegment;

impl NodeTrait for MatchRecognizeClauseSegment {
    const TYPE: &'static str = "match_recognize_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("MATCH_RECOGNIZE"),
            Bracketed::new(vec_of_erased![
                Ref::new("PartitionClauseSegment").optional(),
                Ref::new("OrderByClauseSegment").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MEASURES"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("FINAL"), Ref::keyword("RUNNING"),])
                            .config(|this| this.optional()),
                        Ref::new("ExpressionSegment"),
                        Ref::new("AliasExpressionSegment"),
                    ]),]),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ONE"),
                        Ref::keyword("ROW"),
                        Ref::keyword("PER"),
                        Ref::keyword("MATCH"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALL"),
                        Ref::keyword("ROWS"),
                        Ref::keyword("PER"),
                        Ref::keyword("MATCH"),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SHOW"),
                                Ref::keyword("EMPTY"),
                                Ref::keyword("MATCHES"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("OMIT"),
                                Ref::keyword("EMPTY"),
                                Ref::keyword("MATCHES"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Ref::keyword("UNMATCHED"),
                                Ref::keyword("ROWS"),
                            ]),
                        ])
                        .config(|this| this.optional()),
                    ]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AFTER"),
                    Ref::keyword("MATCH"),
                    Ref::keyword("SKIP"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PAST"),
                            Ref::keyword("LAST"),
                            Ref::keyword("ROW"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TO"),
                            Ref::keyword("NEXT"),
                            Ref::keyword("ROW"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TO"),
                            one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST"),])
                                .config(|this| this.optional()),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                    ]),
                ])
                .config(|this| this.optional()),
                Ref::keyword("PATTERN"),
                Bracketed::new(vec_of_erased![Ref::new("PatternSegment"),]),
                Ref::keyword("DEFINE"),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("AS"),
                    Ref::new("ExpressionSegment"),
                ]),]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["match_recognize_clause"].into()
    }
}

pub struct ChangesClauseSegment;

impl NodeTrait for ChangesClauseSegment {
    const TYPE: &'static str = "changes_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CHANGES"),
            Bracketed::new(vec_of_erased![
                Ref::keyword("INFORMATION"),
                Ref::new("ParameterAssignerSegment"),
                one_of(vec_of_erased![Ref::keyword("DEFAULT"), Ref::keyword("APPEND_ONLY"),]),
            ]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("AT"),
                    Bracketed::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("TIMESTAMP"),
                            Ref::keyword("OFFSET"),
                            Ref::keyword("STATEMENT"),
                        ]),
                        Ref::new("ParameterAssignerSegment"),
                        Ref::new("ExpressionSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BEFORE"),
                    Bracketed::new(vec_of_erased![
                        Ref::keyword("STATEMENT"),
                        Ref::new("ParameterAssignerSegment"),
                        Ref::new("ExpressionSegment"),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("END"),
                Bracketed::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("TIMESTAMP"),
                        Ref::keyword("OFFSET"),
                        Ref::keyword("STATEMENT"),
                    ]),
                    Ref::new("ParameterAssignerSegment"),
                    Ref::new("ExpressionSegment"),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["changes_clause"].into()
    }
}

pub struct FromAtExpressionSegment;

impl NodeTrait for FromAtExpressionSegment {
    const TYPE: &'static str = "from_at_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("AT"),
            Bracketed::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("TIMESTAMP"),
                    Ref::keyword("OFFSET"),
                    Ref::keyword("STATEMENT"),
                ]),
                Ref::new("ParameterAssignerSegment"),
                Ref::new("ExpressionSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["from_at_expression"].into()
    }
}

pub struct FromBeforeExpressionSegment;

impl NodeTrait for FromBeforeExpressionSegment {
    const TYPE: &'static str = "from_before_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("BEFORE"),
            Bracketed::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("TIMESTAMP"),
                    Ref::keyword("OFFSET"),
                    Ref::keyword("STATEMENT"),
                ]),
                Ref::new("ParameterAssignerSegment"),
                Ref::new("ExpressionSegment"),
            ])
            .config(|this| {
                this.parse_mode = ParseMode::Greedy;
            }),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["from_before_expression"].into()
    }
}

pub struct FromPivotExpressionSegment;

impl NodeTrait for FromPivotExpressionSegment {
    const TYPE: &'static str = "from_pivot_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("PIVOT"),
            Bracketed::new(vec_of_erased![
                Ref::new("FunctionSegment"),
                Ref::keyword("FOR"),
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("IN"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "LiteralGrammar"
                ),]),]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["from_pivot_expression"].into()
    }
}

pub struct FromUnpivotExpressionSegment;

impl NodeTrait for FromUnpivotExpressionSegment {
    const TYPE: &'static str = "from_unpivot_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNPIVOT"),
            Bracketed::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("FOR"),
                Ref::new("SingleIdentifierGrammar"),
                Ref::keyword("IN"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "SingleIdentifierGrammar"
                ),]),]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["from_unpivot_expression"].into()
    }
}

pub struct SamplingExpressionSegment;

impl NodeTrait for SamplingExpressionSegment {
    const TYPE: &'static str = "sampling_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("SAMPLE"), Ref::keyword("TABLESAMPLE"),]),
            one_of(vec_of_erased![
                Ref::keyword("BERNOULLI"),
                Ref::keyword("ROW"),
                Ref::keyword("SYSTEM"),
                Ref::keyword("BLOCK"),
            ])
            .config(|this| this.optional()),
            Bracketed::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ReferencedVariableNameSegment"),
                ]),
                Ref::keyword("ROWS").optional(),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("REPEATABLE"), Ref::keyword("SEED"),]),
                Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment"),]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct NamedParameterExpressionSegment;

impl NodeTrait for NamedParameterExpressionSegment {
    const TYPE: &'static str = "snowflake_keyword_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::new("ParameterNameSegment"),
            Ref::new("ParameterAssignerSegment"),
            one_of(vec_of_erased![
                Ref::new("LiteralGrammar"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("ExpressionSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["snowflake_keyword_expression"].into()
    }
}

pub struct SemiStructuredAccessorSegment;

impl NodeTrait for SemiStructuredAccessorSegment {
    const TYPE: &'static str = "semi_structured_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::new("DotSegment"), Ref::new("ColonSegment"),]),
            one_of(vec_of_erased![
                Ref::new("NakedSemiStructuredElementSegment"),
                Ref::new("QuotedSemiStructuredElementSegment"),
            ]),
            Ref::new("ArrayAccessorSegment").optional(),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::new("DotSegment"), Ref::new("ColonSegment"),]),
                    one_of(vec_of_erased![
                        Ref::new("NakedSemiStructuredElementSegment"),
                        Ref::new("QuotedSemiStructuredElementSegment"),
                    ]),
                ])
                .config(|this| {
                    this.allow_gaps = true;
                }),
                Ref::new("ArrayAccessorSegment").optional(),
            ])
            .config(|this| {
                this.allow_gaps = true;
            }),
        ])
        .config(|this| {
            this.allow_gaps = true;
        })
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["semi_structured_expression"].into()
    }
}

pub struct QualifyClauseSegment;

impl NodeTrait for QualifyClauseSegment {
    const TYPE: &'static str = "qualify_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("QUALIFY"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                Ref::new("ExpressionSegment"),
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["qualify_clause"].into()
    }
}

pub struct SelectStatementSegment;

impl NodeTrait for SelectStatementSegment {
    const TYPE: &'static str = "select_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::SelectStatementSegment::match_grammar().copy(
            Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
            None,
            Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        )
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_statement"].into()
    }
}

pub struct WildcardExpressionSegment;

impl NodeTrait for WildcardExpressionSegment {
    const TYPE: &'static str = "wildcard_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::WildcardExpressionSegment::match_grammar().copy(
            Some(vec_of_erased![
                Ref::new("ExcludeClauseSegment").optional(),
                Ref::new("ReplaceClauseSegment").optional(),
                Ref::new("RenameClauseSegment").optional(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        )
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_statement"].into()
    }
}

pub struct ExcludeClauseSegment;

impl NodeTrait for ExcludeClauseSegment {
    const TYPE: &'static str = "select_exclude_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("EXCLUDE"),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "SingleIdentifierGrammar"
                ),]),]),
                Ref::new("SingleIdentifierGrammar"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_exclude_clause"].into()
    }
}

pub struct RenameClauseSegment;

impl NodeTrait for RenameClauseSegment {
    const TYPE: &'static str = "select_rename_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("RENAME"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("AS"),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::keyword("AS"),
                        Ref::new("SingleIdentifierGrammar"),
                    ]
                ),]),]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_rename_clause"].into()
    }
}

pub struct ReplaceClauseSegment;

impl NodeTrait for ReplaceClauseSegment {
    const TYPE: &'static str = "select_replace_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("REPLACE"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Ref::keyword("AS"),
                    Ref::new("SingleIdentifierGrammar"),
                ]
            ),]),]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_replace_clause"].into()
    }
}

pub struct SelectClauseModifierSegment;

impl NodeTrait for SelectClauseModifierSegment {
    const TYPE: &'static str = "select_clause_modifier";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("TOP"), Ref::new("NumericLiteralSegment"),])
                .config(|this| this.optional()),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_clause_modifier"].into()
    }
}

pub struct AlterTableStatementSegment;

impl NodeTrait for AlterTableStatementSegment {
    const TYPE: &'static str = "alter_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                // Rename
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment"),
                ]),
                // Swap With
                Sequence::new(vec_of_erased![
                    Ref::keyword("SWAP"),
                    Ref::keyword("WITH"),
                    Ref::new("TableReferenceSegment"),
                ]),
                // searchOptimizationAction
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("DROP"),]),
                    Ref::keyword("SEARCH"),
                    Ref::keyword("OPTIMIZATION"),
                ]),
                Ref::new("AlterTableClusteringActionSegment"),
                Ref::new("AlterTableConstraintActionSegment"),
                // SET Table options
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Ref::keyword("COMMENT"),
                    ]),
                    Ref::new("EqualsSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Ref::new("NakedIdentifierSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                // Drop primary key
                Sequence::new(vec_of_erased![Ref::keyword("DROP"), Ref::new("PrimaryKeyGrammar"),]),
                // Add primary key
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("PrimaryKeyGrammar"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),])
                            .config(|this| this.optional()),
                    ]),
                ]),
                Ref::new("AlterTableTableColumnActionSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_table_statement"].into()
    }
}

pub struct AlterTableTableColumnActionSegment;

impl NodeTrait for AlterTableTableColumnActionSegment {
    const TYPE: &'static str = "alter_table_table_column_action";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            // Add Column
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::keyword("COLUMN").optional(),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("DatatypeSegment"),
                    one_of(vec_of_erased![
                        // Default
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::new("ExpressionSegment"),
                        ]),
                        // Auto-increment/identity column
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("AUTOINCREMENT"),
                                Ref::keyword("IDENTITY"),
                            ]),
                            one_of(vec_of_erased![
                                // ( <start_num>, <step_num> )
                                Bracketed::new(vec_of_erased![
                                    Ref::new("NumericLiteralSegment"),
                                    Ref::new("CommaSegment"),
                                    Ref::new("NumericLiteralSegment"),
                                ]),
                                // START <num> INCREMENT <num>
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("START"),
                                    Ref::new("NumericLiteralSegment"),
                                    Ref::keyword("INCREMENT"),
                                    Ref::new("NumericLiteralSegment"),
                                ]),
                            ])
                            .config(|this| this.optional()),
                        ])
                        .config(|this| this.optional()),
                    ])
                    .config(|this| this.optional()),
                    // Masking Policy
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH").optional(),
                        Ref::keyword("MASKING"),
                        Ref::keyword("POLICY"),
                        Ref::new("FunctionNameSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("USING"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
                                    Ref::new("ColumnReferenceSegment"),
                                    Ref::new("ExpressionSegment"),
                                ]
                            ),]),]),
                        ])
                        .config(|this| this.optional()),
                    ])
                    .config(|this| this.optional()),
                    Ref::new("CommentClauseSegment").optional(),
                ]),]),
            ]),
            // Rename column
            Sequence::new(vec_of_erased![
                Ref::keyword("RENAME"),
                Ref::keyword("COLUMN"),
                Ref::new("ColumnReferenceSegment"),
                Ref::keyword("TO"),
                Ref::new("ColumnReferenceSegment"),
            ]),
            // Alter/Modify column(s)
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("ALTER"), Ref::keyword("MODIFY"),]),
                optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![
                    // Add things
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN").optional(),
                        Ref::new("ColumnReferenceSegment"),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DROP"),
                                Ref::keyword("DEFAULT"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SET"),
                                Ref::keyword("DEFAULT"),
                                Ref::new("NakedIdentifierSegment"),
                                Ref::new("DotSegment"),
                                Ref::keyword("NEXTVAL"),
                            ]),
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DROP"),])
                                    .config(|this| this.optional()),
                                Ref::keyword("NOT"),
                                Ref::keyword("NULL"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("SET"),
                                        Ref::keyword("DATA").optional(),
                                    ]),
                                    Ref::keyword("TYPE").optional(),
                                ]),
                                Ref::new("DatatypeSegment"),
                            ]),
                            Ref::new("CommentClauseSegment"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("SET"),
                        Ref::keyword("MASKING"),
                        Ref::keyword("POLICY"),
                        Ref::new("FunctionNameSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("USING"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
                                    Ref::new("ColumnReferenceSegment"),
                                    Ref::new("ExpressionSegment"),
                                ]
                            ),]),]),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("UNSET"),
                        Ref::keyword("MASKING"),
                        Ref::keyword("POLICY"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("SET"),
                        Ref::keyword("TAG"),
                        Ref::new("TagReferenceSegment"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("UNSET"),
                        Ref::keyword("TAG"),
                        Ref::new("TagReferenceSegment"),
                    ]),
                ]),]),
            ]),
            // Drop column
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("COLUMN").optional(),
                Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),]),
            ]),
            // Add or Modify column
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("MODIFY"),]),
                Ref::keyword("COLUMN").optional(),
                Ref::new("ColumnDefinitionSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("AFTER"),]),
                        Ref::new("ColumnReferenceSegment"),
                    ]),
                    // Bracketed Version of the same
                    Ref::new("BracketedColumnReferenceListGrammar"),
                ])
                .config(|this| this.optional()),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_table_table_column_action"].into()
    }
}

pub struct AlterTableClusteringActionSegment;

impl NodeTrait for AlterTableClusteringActionSegment {
    const TYPE: &'static str = "alter_table_clustering_action";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CLUSTER"),
                Ref::keyword("BY"),
                one_of(vec_of_erased![
                    Ref::new("FunctionSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    ),]),]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("RECLUSTER"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAX_SIZE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Ref::new("WhereClauseSegment").optional(),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("SUSPEND"), Ref::keyword("RESUME"),]),
                Ref::keyword("RECLUSTER"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("CLUSTERING"),
                Ref::keyword("KEY"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_table_clustering_action"].into()
    }
}

pub struct AlterTableConstraintActionSegment;

impl NodeTrait for AlterTableConstraintActionSegment {
    const TYPE: &'static str = "alter_table_constraint_action";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            // Add Column
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    one_of(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"),
                        Ref::new("QuotedIdentifierSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("PrimaryKeyGrammar"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        ),]),]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ForeignKeyGrammar"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                            ]),]),
                        ]),
                        Ref::keyword("REFERENCES"),
                        Ref::new("TableReferenceSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        ),]),])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("UNIQUE"),
                        Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),])
                            .config(|this| this.optional()),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("NakedIdentifierSegment"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Ref::new("PrimaryKeyGrammar"),
                    Ref::new("ForeignKeyGrammar"),
                    Ref::keyword("UNIQUE"),
                ]),
                Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("RENAME"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("NakedIdentifierSegment"),
                Ref::keyword("TO"),
                Ref::new("NakedIdentifierSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_table_constraint_action"].into()
    }
}
pub struct AlterWarehouseStatementSegment;

impl NodeTrait for AlterWarehouseStatementSegment {
    const TYPE: &'static str = "alter_warehouse_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("WAREHOUSE"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("ObjectReferenceSegment").optional(),
                    one_of(vec_of_erased![
                        Ref::keyword("SUSPEND"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESUME"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("IF"),
                                Ref::keyword("SUSPENDED"),
                            ])
                            .config(|this| this.optional()),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("ObjectReferenceSegment").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ABORT"),
                        Ref::keyword("ALL"),
                        Ref::keyword("QUERIES"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("ObjectReferenceSegment").optional(),
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        AnyNumberOf::new(vec_of_erased![
                            Ref::new("WarehouseObjectPropertiesSegment"),
                            Ref::new("CommentEqualsClauseSegment"),
                            Ref::new("WarehouseObjectParamsSegment"),
                        ]),
                        Ref::new("TagEqualsSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("NakedIdentifierSegment"),]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TAG"),
                            Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                        ]),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_warehouse_statement"].into()
    }
}

pub struct AlterShareStatementSegment;

impl NodeTrait for AlterShareStatementSegment {
    const TYPE: &'static str = "alter_share_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("SHARE"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("NakedIdentifierSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("REMOVE"),]),
                    Ref::keyword("ACCOUNTS"),
                    Ref::new("EqualsSegment"),
                    Delimited::new(vec_of_erased![Ref::new("NakedIdentifierSegment"),]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SHARE_RESTRICTIONS"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("ACCOUNTS"),
                    Ref::new("EqualsSegment"),
                    Delimited::new(vec_of_erased![Ref::new("NakedIdentifierSegment"),]),
                    Ref::new("CommentEqualsClauseSegment").optional(),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::new("TagEqualsSegment"),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    Ref::keyword("TAG"),
                    Ref::new("TagReferenceSegment"),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("CommaSegment"),
                        Ref::new("TagReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("UNSET"), Ref::keyword("COMMENT"),]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_share_statement"].into()
    }
}

pub struct AlterStorageIntegrationSegment;

impl NodeTrait for AlterStorageIntegrationSegment {
    const TYPE: &'static str = "alter_storage_integration_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("STORAGE").optional(),
            Ref::keyword("INTEGRATION"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("TagEqualsSegment").optional(),
                        any_set_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COMMENT"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ENABLED"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ]),
                            one_of(vec_of_erased![
                                any_set_of(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("STORAGE_AWS_ROLE_ARN"),
                                        Ref::new("EqualsSegment"),
                                        Ref::new("QuotedLiteralSegment"),
                                    ]),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("STORAGE_AWS_OBJECT_ACL"),
                                        Ref::new("EqualsSegment"),
                                        Ref::new("QuotedLiteralSegment"),
                                    ]),
                                ]),
                                any_set_of(vec_of_erased![Sequence::new(vec_of_erased![
                                    Ref::keyword("AZURE_TENANT_ID"),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("QuotedLiteralSegment"),
                                ]),]),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("STORAGE_ALLOWED_LOCATIONS"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                        one_of(vec_of_erased![
                                            Ref::new("S3Path"),
                                            Ref::new("GCSPath"),
                                            Ref::new("AzureBlobStoragePath"),
                                        ]),
                                    ]),]),
                                    Bracketed::new(vec_of_erased![Ref::new("QuotedStarSegment"),]),
                                ]),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("STORAGE_BLOCKED_LOCATIONS"),
                                Ref::new("EqualsSegment"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::new("S3Path"),
                                        Ref::new("GCSPath"),
                                        Ref::new("AzureBlobStoragePath"),
                                    ]),
                                ]),]),
                            ]),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TAG"),
                            Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                        ])
                        .config(|this| this.optional()),
                        Ref::keyword("COMMENT"),
                        Ref::keyword("ENABLED"),
                        Ref::keyword("STORAGE_BLOCKED_LOCATIONS"),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_storage_integration_statement"].into()
    }
}

pub struct AlterExternalTableStatementSegment;

impl NodeTrait for AlterExternalTableStatementSegment {
    const TYPE: &'static str = "alter_external_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("EXTERNAL"),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFRESH"),
                    Ref::new("QuotedLiteralSegment").optional(),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ADD"), Ref::keyword("REMOVE"),]),
                    Ref::keyword("FILES"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AUTO_REFRESH"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ])
                    .config(|this| this.optional()),
                    Ref::new("TagEqualsSegment").optional(),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("UNSET"), Ref::new("TagEqualsSegment"),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("PARTITION"),
                    Ref::keyword("LOCATION"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("PARTITION"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                        vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ]
                    ),]),]),
                    Ref::keyword("LOCATION"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_external_table_statement"].into()
    }
}

pub struct CommentEqualsClauseSegment;

impl NodeTrait for CommentEqualsClauseSegment {
    const TYPE: &'static str = "comment_equals_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("COMMENT"),
            Ref::new("EqualsSegment"),
            Ref::new("QuotedLiteralSegment"),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["comment_equals_clause"].into()
    }
}

pub struct TagBracketedEqualsSegment;

impl NodeTrait for TagBracketedEqualsSegment {
    const TYPE: &'static str = "tag_bracketed_equals";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("WITH"),]).config(|this| this.optional()),
            Ref::keyword("TAG"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                vec_of_erased![
                    Ref::new("TagReferenceSegment"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]
            )])]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["tag_bracketed_equals"].into()
    }
}

pub struct TagEqualsSegment;

impl NodeTrait for TagEqualsSegment {
    const TYPE: &'static str = "tag_equals";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("TAG"),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("TagReferenceSegment"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
            ])])
        ])
        .to_matchable()
    }
}

pub struct UnorderedSelectStatementSegment;

impl NodeTrait for UnorderedSelectStatementSegment {
    const TYPE: &'static str = "select_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::UnorderedSelectStatementSegment::match_grammar().copy(
            Some(vec_of_erased![Ref::new("QualifyClauseSegment").optional()]),
            None,
            Some(Ref::new("OverlapsClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        )
    }

    fn class_types() -> AHashSet<&'static str> {
        ["select_statement"].into()
    }
}

pub struct AccessStatementSegment;

impl NodeTrait for AccessStatementSegment {
    const TYPE: &'static str = "access_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        let global_permissions = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                one_of(vec_of_erased![
                    Ref::keyword("ACCOUNT"),
                    Ref::keyword("ROLE"),
                    Ref::keyword("USER"),
                    Ref::keyword("WAREHOUSE"),
                    Ref::keyword("DATABASE"),
                    Ref::keyword("INTEGRATION"),
                    Ref::keyword("SHARE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DATA"),
                        Ref::keyword("EXCHANGE"),
                        Ref::keyword("LISTING"),
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("NETWORK"), Ref::keyword("POLICY"),]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("APPLY"),
                Ref::keyword("MASKING"),
                Ref::keyword("POLICY"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("APPLY"),
                Ref::keyword("ROW"),
                Ref::keyword("ACCESS"),
                Ref::keyword("POLICY"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("APPLY"),
                Ref::keyword("SESSION"),
                Ref::keyword("POLICY"),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("APPLY"), Ref::keyword("TAG"),]),
            Sequence::new(vec_of_erased![Ref::keyword("ATTACH"), Ref::keyword("POLICY"),]),
            Sequence::new(vec_of_erased![Ref::keyword("EXECUTE"), Ref::keyword("TASK"),]),
            Sequence::new(vec_of_erased![Ref::keyword("IMPORT"), Ref::keyword("SHARE"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("MANAGE"),
                one_of(vec_of_erased![
                    Ref::keyword("GRANTS"),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("ACCOUNT"),
                            Ref::keyword("ORGANIZATION"),
                            Ref::keyword("USER"),
                        ]),
                        Ref::keyword("SUPPORT"),
                        Ref::keyword("CASES"),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("MONITOR"),
                one_of(vec_of_erased![Ref::keyword("EXECUTION"), Ref::keyword("USAGE"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("OVERRIDE"),
                Ref::keyword("SHARE"),
                Ref::keyword("RESTRICTIONS"),
            ]),
        ]);

        let schema_object_names = [
            "TABLE",
            "VIEW",
            "STAGE",
            "FUNCTION",
            "PROCEDURE",
            "ROUTINE",
            "SEQUENCE",
            "STREAM",
            "TASK",
            "PIPE",
        ];

        let schema_object_names_keywrods: Vec<Arc<dyn Matchable>> =
            schema_object_names.iter().map(|name| Ref::keyword(name).to_matchable()).collect();

        let mut schema_object_types = schema_object_names_keywrods.clone();
        schema_object_types.append(&mut vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("MATERIALIZED"), Ref::keyword("VIEW")]),
            Sequence::new(vec_of_erased![Ref::keyword("EXTERNAL"), Ref::keyword("TABLE")]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("TEMP"), Ref::keyword("TEMPORARY")]),
                Ref::keyword("TABLE"),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMAT")]),
            Sequence::new(vec_of_erased![Ref::keyword("SESSION"), Ref::keyword("POLICY")]),
            Sequence::new(vec_of_erased![Ref::keyword("MASKING"), Ref::keyword("POLICY")]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ROW"),
                Ref::keyword("ACCESS"),
                Ref::keyword("POLICY"),
            ]),
        ]);

        let schema_object_types = one_of(schema_object_types);

        let schema_object_types_plural = one_of(
            schema_object_names
                .iter()
                .map(|name| Ref::keyword(&format!("{}S", name)).to_matchable())
                .collect(),
        );

        let permissions = Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    one_of(vec_of_erased![Ref::keyword("SCHEMA"), schema_object_types.clone(),]),
                ]),
                Sequence::new(
                    vec_of_erased![Ref::keyword("IMPORTED"), Ref::keyword("PRIVILEGES"),]
                ),
                Ref::keyword("APPLY"),
                Ref::keyword("CONNECT"),
                Ref::keyword("CREATE"),
                Ref::keyword("DELETE"),
                Ref::keyword("EXECUTE"),
                Ref::keyword("INSERT"),
                Ref::keyword("MODIFY"),
                Ref::keyword("MONITOR"),
                Ref::keyword("OPERATE"),
                Ref::keyword("OWNERSHIP"),
                Ref::keyword("READ"),
                Ref::keyword("REFERENCE_USAGE"),
                Ref::keyword("REFERENCES"),
                Ref::keyword("SELECT"),
                Ref::keyword("TEMP"),
                Ref::keyword("TEMPORARY"),
                Ref::keyword("TRIGGER"),
                Ref::keyword("TRUNCATE"),
                Ref::keyword("UPDATE"),
                Ref::keyword("USAGE"),
                Ref::keyword("USE_ANY_ROLE"),
                Ref::keyword("WRITE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALL"),
                    Ref::keyword("PRIVILEGES").optional(),
                ]),
            ]),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
        ]);

        let objects = one_of(vec_of_erased![
            Ref::keyword("ACCOUNT"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESOURCE"),
                        Ref::keyword("MONITOR"),
                    ]),
                    Ref::keyword("WAREHOUSE"),
                    Ref::keyword("DATABASE"),
                    Ref::keyword("DOMAIN"),
                    Ref::keyword("INTEGRATION"),
                    Ref::keyword("SCHEMA"),
                    Ref::keyword("ROLE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALL"),
                        Ref::keyword("SCHEMAS"),
                        Ref::keyword("IN"),
                        Ref::keyword("DATABASE"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FUTURE"),
                        Ref::keyword("SCHEMAS"),
                        Ref::keyword("IN"),
                        Ref::keyword("DATABASE"),
                    ]),
                    schema_object_types.clone(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALL"),
                        one_of(vec_of_erased![
                            schema_object_types_plural.clone(),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MATERIALIZED"),
                                Ref::keyword("VIEWS"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("EXTERNAL"),
                                Ref::keyword("TABLES"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILE"),
                                Ref::keyword("FORMATS"),
                            ]),
                        ]),
                        Ref::keyword("IN"),
                        one_of(vec_of_erased![Ref::keyword("SCHEMA"), Ref::keyword("DATABASE"),]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FUTURE"),
                        one_of(vec_of_erased![
                            schema_object_types_plural.clone(),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MATERIALIZED"),
                                Ref::keyword("VIEWS"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("EXTERNAL"),
                                Ref::keyword("TABLES"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILE"),
                                Ref::keyword("FORMATS"),
                            ]),
                        ]),
                        Ref::keyword("IN"),
                        one_of(vec_of_erased![Ref::keyword("DATABASE"), Ref::keyword("SCHEMA"),]),
                    ]),
                ])
                .config(|this| this.optional()),
                Delimited::new(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("FunctionNameSegment"),
                        Ref::new("FunctionParameterListGrammar").optional(),
                    ]),
                ])
                .config(|this| this.terminators =
                    vec_of_erased![Ref::keyword("TO"), Ref::keyword("FROM")]),
            ]),
        ]);

        one_of(vec_of_erased![
            // Grant statement
            Sequence::new(vec_of_erased![
                Ref::keyword("GRANT"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            global_permissions.clone(),
                            permissions.clone()
                        ]),])
                        .config(|this| this.terminators = vec_of_erased![Ref::keyword("ON")]),
                        Ref::keyword("ON"),
                        objects.clone(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ROLE"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OWNERSHIP"),
                        Ref::keyword("ON"),
                        Ref::keyword("USER"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Ref::keyword("TO"),
                Ref::keyword("USER").optional(),
                Ref::keyword("ROLE").optional(),
                Ref::keyword("SHARE").optional(),
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("RoleReferenceSegment"),
                    Ref::new("FunctionSegment"),
                    Ref::keyword("PUBLIC"),
                ]),]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("GRANT"),
                        Ref::keyword("OPTION"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("ADMIN"),
                        Ref::keyword("OPTION"),
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("REVOKE"), Ref::keyword("COPY"),]),
                        Ref::keyword("CURRENT"),
                        Ref::keyword("GRANTS"),
                    ])
                    .config(|this| this.optional()),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("GRANTED"),
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Ref::keyword("CURRENT_USER"),
                        Ref::keyword("SESSION_USER"),
                        Ref::new("ObjectReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ])
                .config(|this| this.optional()),
            ]),
            // Revoke statement
            Sequence::new(vec_of_erased![
                Ref::keyword("REVOKE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("GRANT"),
                    Ref::keyword("OPTION"),
                    Ref::keyword("FOR"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            global_permissions.clone(),
                            permissions.clone()
                        ]),])
                        .config(|this| {
                            this.terminators = vec_of_erased![Ref::keyword("ON")];
                        }),
                        Ref::keyword("ON"),
                        objects.clone(),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ROLE"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OWNERSHIP"),
                        Ref::keyword("ON"),
                        Ref::keyword("USER"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                ]),
                Ref::keyword("FROM"),
                Ref::keyword("USER").optional(),
                Ref::keyword("ROLE").optional(),
                Ref::keyword("SHARE").optional(),
                Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment"),]),
                Ref::new("DropBehaviorGrammar").optional(),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["access_statement"].into()
    }
}

pub struct CreateCloneStatementSegment;

impl NodeTrait for CreateCloneStatementSegment {
    const TYPE: &'static str = "create_clone_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            one_of(vec_of_erased![
                Ref::keyword("DATABASE"),
                Ref::keyword("SCHEMA"),
                Ref::keyword("TABLE"),
                Ref::keyword("SEQUENCE"),
                Sequence::new(vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMAT"),]),
                Ref::keyword("STAGE"),
                Ref::keyword("STREAM"),
                Ref::keyword("TASK"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS"),
            ])
            .config(|this| this.optional()),
            Ref::new("ObjectReferenceSegment"),
            Ref::keyword("CLONE"),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Ref::new("FromAtExpressionSegment"),
                Ref::new("FromBeforeExpressionSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["create_clone_statement"].into()
    }
}

pub struct CreateDatabaseFromShareStatementSegment;

impl NodeTrait for CreateDatabaseFromShareStatementSegment {
    const TYPE: &'static str = "create_database_from_share_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("DATABASE"),
            Ref::new("ObjectReferenceSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("SHARE"),]),
            Ref::new("ObjectReferenceSegment"),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["create_database_from_share_statement"].into()
    }
}

pub struct CreateProcedureStatementSegment;

impl NodeTrait for CreateProcedureStatementSegment {
    const TYPE: &'static str = "create_procedure_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("SECURE"),]).config(|this| this.optional()),
            Ref::keyword("PROCEDURE"),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
            Sequence::new(vec_of_erased![Ref::keyword("COPY"), Ref::keyword("GRANTS"),])
                .config(|this| this.optional()),
            Ref::keyword("RETURNS"),
            one_of(vec_of_erased![
                Ref::new("DatatypeSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("ColumnDefinitionSegment"),])
                            .config(|this| this.optional()),
                    ]),
                ]),
            ]),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("NULL"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LANGUAGE"),
                    one_of(vec_of_erased![
                        Ref::keyword("JAVA"),
                        Ref::keyword("JAVASCRIPT"),
                        Ref::keyword("PYTHON"),
                        Ref::keyword("SCALA"),
                        Ref::keyword("SQL"),
                    ])
                    .config(|this| this.optional()),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CALLED"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETURNS"),
                        Ref::keyword("NULL"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Ref::keyword("STRICT"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![Ref::keyword("VOLATILE"), Ref::keyword("IMMUTABLE"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RUNTIME_VERSION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Ref::new("CommentEqualsClauseSegment").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IMPORTS"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PACKAGES"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("HANDLER"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TARGET_PATH"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXECUTE"),
                    Ref::keyword("AS"),
                    one_of(vec_of_erased![Ref::keyword("CALLER"), Ref::keyword("OWNER"),]),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
            Ref::keyword("AS"),
            one_of(vec_of_erased![
                Ref::new("DoubleQuotedUDFBody"),
                Ref::new("SingleQuotedUDFBody"),
                Ref::new("DollarQuotedUDFBody"),
                Ref::new("ScriptingBlockStatementSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["create_procedure_statement"].into()
    }
}

pub struct ReturnStatementSegment;

impl NodeTrait for ReturnStatementSegment {
    const TYPE: &'static str = "return_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![Ref::keyword("RETURN"), Ref::new("ExpressionSegment"),])
            .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["return_statement"].into()
    }
}

pub struct ScriptingBlockStatementSegment;

impl NodeTrait for ScriptingBlockStatementSegment {
    const TYPE: &'static str = "scripting_block_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                Delimited::new(vec_of_erased![Ref::new("StatementSegment"),]),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("END")]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["scripting_block_statement"].into()
    }
}

pub struct ScriptingLetStatementSegment;

impl NodeTrait for ScriptingLetStatementSegment {
    const TYPE: &'static str = "scripting_let_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            // Initial declaration and assignment
            Sequence::new(vec_of_erased![
                Ref::keyword("LET"),
                Ref::new("LocalVariableNameSegment"),
                one_of(vec_of_erased![
                    // Variable assignment
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            one_of(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("WalrusOperatorSegment"),
                            ]),
                            Ref::new("ExpressionSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("WalrusOperatorSegment"),
                            ]),
                            Ref::new("ExpressionSegment"),
                        ]),
                    ]),
                    // Cursor assignment
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CURSOR"),
                        Ref::keyword("FOR"),
                        one_of(vec_of_erased![
                            Ref::new("LocalVariableNameSegment"),
                            Ref::new("SelectableGrammar"),
                        ]),
                    ]),
                    // Resultset assignment
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESULTSET"),
                        Ref::new("WalrusOperatorSegment"),
                        Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar"),]),
                    ]),
                ]),
            ]),
            // Subsequent assignment
            Sequence::new(vec_of_erased![
                Ref::new("LocalVariableNameSegment"),
                Ref::new("WalrusOperatorSegment"),
                one_of(vec_of_erased![
                    // Variable reassignment
                    Ref::new("ExpressionSegment"),
                    // Resultset reassignment
                    Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar"),]),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["scripting_let_statement"].into()
    }
}

pub struct CreateFunctionStatementSegment;

impl NodeTrait for CreateFunctionStatementSegment {
    const TYPE: &'static str = "create_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("SECURE"),]).config(|this| this.optional()),
            Ref::keyword("FUNCTION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
            Ref::keyword("RETURNS"),
            one_of(vec_of_erased![
                Ref::new("DatatypeSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnDefinitionSegment"
                    ),]),]),
                ]),
            ]),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("NULL"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LANGUAGE"),
                    one_of(vec_of_erased![
                        Ref::keyword("JAVASCRIPT"),
                        Ref::keyword("SQL"),
                        Ref::keyword("PYTHON"),
                        Ref::keyword("JAVA"),
                        Ref::keyword("SCALA"),
                    ])
                    .config(|this| this.optional()),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CALLED"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETURNS"),
                        Ref::keyword("NULL"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT"),
                    ]),
                    Ref::keyword("STRICT"),
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![Ref::keyword("VOLATILE"), Ref::keyword("IMMUTABLE"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RUNTIME_VERSION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Ref::new("CommentEqualsClauseSegment").optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("IMPORTS"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PACKAGES"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("HANDLER"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TARGET_PATH"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    // Either a foreign programming language UDF...
                    Ref::new("DoubleQuotedUDFBody"),
                    Ref::new("SingleQuotedUDFBody"),
                    Ref::new("DollarQuotedUDFBody"),
                    // ...or a SQL UDF
                    Ref::new("ScriptingBlockStatementSegment"),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["create_function_statement"].into()
    }
}

pub struct AlterFunctionStatementSegment;

impl NodeTrait for AlterFunctionStatementSegment {
    const TYPE: &'static str = "alter_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("FUNCTION"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("FunctionNameSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("CommentEqualsClauseSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("API_INTEGRATION"),
                            Ref::new("EqualsSegment"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("HEADERS"),
                            Ref::new("EqualsSegment"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("SingleQuotedIdentifierSegment"),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("SingleQuotedIdentifierSegment"),
                                ]),
                            ]),]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CONTEXT_HEADERS"),
                            Ref::new("EqualsSegment"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ContextHeadersGrammar"),
                            ]),]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MAX_BATCH_ROWS"),
                            Ref::new("EqualsSegment"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COMPRESSION"),
                            Ref::new("EqualsSegment"),
                            Ref::new("CompressionType"),
                        ]),
                        Ref::keyword("SECURE"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("REQUEST_TRANSLATOR"),
                                Ref::keyword("RESPONSE_TRANSLATOR"),
                            ]),
                            Ref::new("EqualsSegment"),
                            Ref::new("FunctionNameSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Ref::keyword("COMMENT"),
                        Ref::keyword("HEADERS"),
                        Ref::keyword("CONTEXT_HEADERS"),
                        Ref::keyword("MAX_BATCH_ROWS"),
                        Ref::keyword("COMPRESSION"),
                        Ref::keyword("SECURE"),
                        Ref::keyword("REQUEST_TRANSLATOR"),
                        Ref::keyword("RESPONSE_TRANSLATOR"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_function_statement"].into()
    }
}

pub struct CreateExternalFunctionStatementSegment;

impl NodeTrait for CreateExternalFunctionStatementSegment {
    const TYPE: &'static str = "create_external_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![Ref::keyword("SECURE"),]).config(|this| this.optional()),
            Ref::keyword("EXTERNAL"),
            Ref::keyword("FUNCTION"),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
            Ref::keyword("RETURNS"),
            Ref::new("DatatypeSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("NOT").optional(), Ref::keyword("NULL"),])
                .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CALLED"),
                    Ref::keyword("ON"),
                    Ref::keyword("NULL"),
                    Ref::keyword("INPUT"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RETURNS"),
                    Ref::keyword("NULL"),
                    Ref::keyword("ON"),
                    Ref::keyword("NULL"),
                    Ref::keyword("INPUT"),
                ]),
                Ref::keyword("STRICT"),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![Ref::keyword("VOLATILE"), Ref::keyword("IMMUTABLE"),])
                .config(|this| this.optional()),
            Ref::new("CommentEqualsClauseSegment").optional(),
            Ref::keyword("API_INTEGRATION"),
            Ref::new("EqualsSegment"),
            Ref::new("SingleIdentifierGrammar"),
            Sequence::new(vec_of_erased![
                Ref::keyword("HEADERS"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("SingleQuotedIdentifierSegment"),
                        Ref::new("EqualsSegment"),
                        Ref::new("SingleQuotedIdentifierSegment"),
                    ]
                ),]),]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("CONTEXT_HEADERS"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ContextHeadersGrammar"
                ),]),]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("MAX_BATCH_ROWS"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("CompressionType"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("REQUEST_TRANSLATOR"),
                Ref::new("EqualsSegment"),
                Ref::new("FunctionNameSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("RESPONSE_TRANSLATOR"),
                Ref::new("EqualsSegment"),
                Ref::new("FunctionNameSegment"),
            ])
            .config(|this| this.optional()),
            Ref::keyword("AS"),
            Ref::new("SingleQuotedIdentifierSegment"),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["create_external_function_statement"].into()
    }
}

pub struct WarehouseObjectPropertiesSegment;

impl NodeTrait for WarehouseObjectPropertiesSegment {
    const TYPE: &'static str = "warehouse_object_properties";

    fn match_grammar() -> Arc<dyn Matchable> {
        any_set_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("WAREHOUSE_TYPE"),
                Ref::new("EqualsSegment"),
                Ref::new("WarehouseType"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("WAREHOUSE_SIZE"),
                Ref::new("EqualsSegment"),
                Ref::new("WarehouseSize"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("WAIT_FOR_COMPLETION"),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("MAX_CLUSTER_COUNT"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("MIN_CLUSTER_COUNT"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SCALING_POLICY"),
                Ref::new("EqualsSegment"),
                Ref::new("ScalingPolicy"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("AUTO_SUSPEND"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::new("NumericLiteralSegment"), Ref::keyword("NULL"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("AUTO_RESUME"),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("INITIALLY_SUSPENDED"),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("RESOURCE_MONITOR"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["warehouse_object_properties"].into()
    }
}

pub struct WarehouseObjectParamsSegment;

impl NodeTrait for WarehouseObjectParamsSegment {
    const TYPE: &'static str = "warehouse_object_properties";

    fn match_grammar() -> Arc<dyn Matchable> {
        any_set_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("MAX_CONCURRENCY_LEVEL"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("STATEMENT_QUEUED_TIMEOUT_IN_SECONDS"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("STATEMENT_TIMEOUT_IN_SECONDS"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["warehouse_object_properties"].into()
    }
}

pub struct ConstraintPropertiesSegment;

impl NodeTrait for ConstraintPropertiesSegment {
    const TYPE: &'static str = "constraint_properties_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("CONSTRAINT"),
                Ref::new("QuotedLiteralSegment"),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNIQUE"),
                    Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("PrimaryKeyGrammar"),
                    Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),])
                        .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("ForeignKeyGrammar"),
                        Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),])
                            .config(|this| this.optional()),
                    ]),
                    Ref::keyword("REFERENCES"),
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),]),
                ]),
            ]),
            any_set_of(vec_of_erased![one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT").optional(),
                    Ref::keyword("ENFORCED"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT").optional(),
                    Ref::keyword("DEFERRABLE"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INITIALLY"),
                    one_of(vec_of_erased![Ref::keyword("DEFERRED"), Ref::keyword("IMMEDIATE"),]),
                ]),
            ]),]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["constraint_properties_segment"].into()
    }
}

pub struct ColumnConstraintSegment;

impl NodeTrait for ColumnConstraintSegment {
    const TYPE: &'static str = "column_constraint_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        any_set_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("COLLATE"),
                Ref::new("CollationReferenceSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CURRENT_TIMESTAMP"),
                        Bracketed::new(vec_of_erased![
                            Ref::new("NumericLiteralSegment").optional(),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SYSDATE"),
                        Bracketed::new(vec_of_erased![]),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("AUTOINCREMENT"), Ref::keyword("IDENTITY"),]),
                one_of(vec_of_erased![
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "NumericLiteralSegment"
                    ),]),]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("START"),
                        Ref::new("NumericLiteralSegment"),
                        Ref::keyword("INCREMENT"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                ])
                .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("NOT").optional(), Ref::keyword("NULL"),]),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH").optional(),
                Ref::keyword("MASKING"),
                Ref::keyword("POLICY"),
                Ref::new("FunctionNameSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            Ref::new("ExpressionSegment"),
                        ]
                    ),]),]),
                ])
                .config(|this| this.optional()),
            ]),
            Ref::new("TagBracketedEqualsSegment").optional(),
            Ref::new("ConstraintPropertiesSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT"),
                Ref::new("QuotedLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECK"),
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT"),
                one_of(vec_of_erased![Ref::new("LiteralGrammar"), Ref::new("FunctionSegment"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("REFERENCES"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("BracketedColumnReferenceListGrammar").optional(),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CopyOptionsSegment;

impl NodeTrait for CopyOptionsSegment {
    const TYPE: &'static str = "copy_options";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON_ERROR"),
                    Ref::new("EqualsSegment"),
                    Ref::new("CopyOptionOnErrorSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SIZE_LIMIT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PURGE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RETURN_FAILED_ONLY"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MATCH_BY_COLUMN_NAME"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("CASE_SENSITIVE"),
                        Ref::keyword("CASE_INSENSITIVE"),
                        Ref::keyword("NONE"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENFORCE_LENGTH"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TRUNCATECOLUMNS"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FORCE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("OVERWRITE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SINGLE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAX_FILE_SIZE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INCLUDE_QUERY_ID"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DETAILED_OUTPUT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["copy_options"].into()
    }
}

pub struct CreateSchemaStatementSegment;

impl NodeTrait for CreateSchemaStatementSegment {
    const TYPE: &'static str = "create_schema_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::new("TemporaryTransientGrammar").optional(),
            Ref::keyword("SCHEMA"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("SchemaReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("MANAGED"),
                Ref::keyword("ACCESS"),
            ])
            .config(|this| this.optional()),
            Ref::new("SchemaObjectParamsSegment").optional(),
            Ref::new("TagBracketedEqualsSegment").optional(),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["create_schema_statement"].into()
    }
}

pub struct AlterRoleStatementSegment;

impl NodeTrait for AlterRoleStatementSegment {
    const TYPE: &'static str = "alter_role_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("ROLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("RoleReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("RoleReferenceSegment"),
                        Ref::new("TagEqualsSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COMMENT"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Ref::new("RoleReferenceSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TAG"),
                            Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("COMMENT"),]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    one_of(vec_of_erased![Ref::new("RoleReferenceSegment"),]),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_role_statement"].into()
    }
}

pub struct AlterSchemaStatementSegment;

impl NodeTrait for AlterSchemaStatementSegment {
    const TYPE: &'static str = "alter_schema_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("SCHEMA"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("SchemaReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SWAP"),
                    Ref::keyword("WITH"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("SchemaObjectParamsSegment"),
                        Ref::new("TagEqualsSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            Ref::keyword("DATA_RETENTION_TIME_IN_DAYS"),
                            Ref::keyword("MAX_DATA_EXTENSION_TIME_IN_DAYS"),
                            Ref::keyword("DEFAULT_DDL_COLLATION"),
                            Ref::keyword("COMMENT"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TAG"),
                            Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ENABLE"), Ref::keyword("DISABLE"),]),
                    Sequence::new(vec_of_erased![Ref::keyword("MANAGED"), Ref::keyword("ACCESS"),]),
                ]),
            ]),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["alter_schema_statement"].into()
    }
}

pub struct SchemaObjectParamsSegment;

impl NodeTrait for SchemaObjectParamsSegment {
    const TYPE: &'static str = "schema_object_properties";

    fn match_grammar() -> Arc<dyn Matchable> {
        any_set_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("DATA_RETENTION_TIME_IN_DAYS"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("MAX_DATA_EXTENSION_TIME_IN_DAYS"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT_DDL_COLLATION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
            ]),
            Ref::new("CommentEqualsClauseSegment"),
        ])
        .to_matchable()
    }

    fn class_types() -> AHashSet<&'static str> {
        ["schema_object_properties"].into()
    }
}

pub struct CreateTableStatementSegment;

impl NodeTrait for CreateTableStatementSegment {
    const TYPE: &'static str = "create_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::new("TemporaryTransientGrammar").optional(),
            Ref::keyword("TABLE"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![Delimited::new(
                    vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("TableConstraintSegment"),
                            Ref::new("ColumnDefinitionSegment"),
                            Ref::new("SingleIdentifierGrammar"),
                        ]),
                        Ref::new("CommentClauseSegment").optional(),
                    ]),]
                ),]),])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTER"),
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Ref::new("FunctionSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ExpressionSegment"
                        ),]),]),
                    ]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STAGE_FILE_FORMAT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("FileFormatSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STAGE_COPY_OPTIONS"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("CopyOptionsSegment"),]),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DATA_RETENTION_TIME_IN_DAYS"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAX_DATA_EXTENSION_TIME_IN_DAYS"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CHANGE_TRACKING"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT_DDL_COLLATION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralGrammar"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![Ref::keyword("COPY"), Ref::keyword("GRANTS"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH").optional(),
                    Ref::keyword("ROW"),
                    Ref::keyword("ACCESS"),
                    Ref::keyword("POLICY"),
                    Ref::new("NakedIdentifierSegment"),
                    Ref::keyword("ON"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    ),]),]),
                ])
                .config(|this| this.optional()),
                Ref::new("TagBracketedEqualsSegment").optional(),
                Ref::new("CommentEqualsClauseSegment").optional(),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar"),]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LIKE"),
                        Ref::new("TableReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CLONE"),
                        Ref::new("TableReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USING"),
                        Ref::keyword("TEMPLATE"),
                        Ref::new("SelectableGrammar"),
                    ]),
                ])
                .config(|this| this.optional()),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateTaskSegment;

impl NodeTrait for CreateTaskSegment {
    const TYPE: &'static str = "create_task_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Ref::keyword("TASK"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS"),
            ])
            .config(|this| this.optional()),
            Ref::new("ObjectReferenceSegment"),
            MetaSegment::indent(),
            AnyNumberOf::new(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WAREHOUSE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USER_TASK_MANAGED_INITIAL_WAREHOUSE_SIZE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("WarehouseSize"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEDULE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALLOW_OVERLAPPING_EXECUTION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USER_TASK_TIMEOUT_MS"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("BooleanLiteralGrammar"),
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                ]),]),
                Sequence::new(vec_of_erased![Ref::keyword("COPY"), Ref::keyword("GRANTS"),]),
                Ref::new("CommentEqualsClauseSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("AFTER"),
                Ref::new("ObjectReferenceSegment"),
            ])
            .config(|this| this.optional()),
            MetaSegment::dedent(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WHEN"),
                MetaSegment::indent(),
                Ref::new("TaskExpressionSegment"),
                MetaSegment::dedent(),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                MetaSegment::indent(),
                Ref::new("StatementSegment"),
                MetaSegment::dedent(),
            ]),
        ])
        .to_matchable()
    }
}

pub struct TaskExpressionSegment;

impl NodeTrait for TaskExpressionSegment {
    const TYPE: &'static str = "snowflake_task_expression_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("ExpressionSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("SystemFunctionName"),
                    Bracketed::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),]),
                ]),
            ]),],)
            .config(|this| this
                .delimiter(one_of(vec_of_erased![Ref::new("BooleanBinaryOperatorGrammar"),]))),
        ])
        .to_matchable()
    }
}

pub struct CreateStatementSegment;

impl NodeTrait for CreateStatementSegment {
    const TYPE: &'static str = "create_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("NETWORK"), Ref::keyword("POLICY"),]),
                Sequence::new(vec_of_erased![Ref::keyword("RESOURCE"), Ref::keyword("MONITOR"),]),
                Ref::keyword("SHARE"),
                Ref::keyword("ROLE"),
                Ref::keyword("USER"),
                Ref::keyword("TAG"),
                Ref::keyword("WAREHOUSE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOTIFICATION"),
                    Ref::keyword("INTEGRATION"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SECURITY"),
                    Ref::keyword("INTEGRATION"),
                ]),
                Sequence::new(
                    vec_of_erased![Ref::keyword("STORAGE"), Ref::keyword("INTEGRATION"),]
                ),
                Sequence::new(vec_of_erased![Ref::keyword("MATERIALIZED"), Ref::keyword("VIEW"),]),
                Sequence::new(vec_of_erased![Ref::keyword("MASKING"), Ref::keyword("POLICY"),]),
                Ref::keyword("PIPE"),
                Sequence::new(vec_of_erased![Ref::keyword("EXTERNAL"), Ref::keyword("FUNCTION"),]),
                Ref::keyword("DATABASE"),
                Ref::keyword("SEQUENCE"),
            ]),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("TYPE"),
                    Ref::new("EqualsSegment"),
                    Ref::keyword("QUEUE"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENABLED"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOTIFICATION_PROVIDER"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("AWS_SNS"),
                        Ref::keyword("AZURE_EVENT_GRID"),
                        Ref::keyword("GCP_PUBSUB"),
                        Ref::keyword("AZURE_STORAGE_QUEUE"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AWS_SNS_TOPIC_ARN"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AWS_SNS_ROLE_ARN"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AZURE_TENANT_ID"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AZURE_STORAGE_QUEUE_PRIMARY_URI"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AZURE_EVENT_GRID_TOPIC_ENDPOINT"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("GCP_PUBSUB_SUBSCRIPTION_NAME"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("GCP_PUBSUB_TOPIC_NAME"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DIRECTION"),
                    Ref::new("EqualsSegment"),
                    Ref::keyword("OUTBOUND"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALLOWED_VALUES"),
                    Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALLOWED_IP_LIST"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BLOCKED_IP_LIST"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    ),]),]),
                ]),
            ]),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("TYPE"),
                    Ref::new("EqualsSegment"),
                    Ref::keyword("EXTERNAL_STAGE"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENABLED"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_PROVIDER"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("S3"),
                        Ref::keyword("AZURE"),
                        Ref::keyword("GCS"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AZURE_TENANT_ID"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_AWS_ROLE_ARN"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_AWS_OBJECT_ACL"),
                    Ref::new("EqualsSegment"),
                    StringParser::new(
                        "'bucket-owner-full-control'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker(),
                                SymbolSegmentNewArgs { r#type: "literal" },
                            )
                        },
                        None,
                        false,
                        None,
                    )
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_ALLOWED_LOCATIONS"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                            vec_of_erased![
                                Ref::new("S3Path"),
                                Ref::new("GCSPath"),
                                Ref::new("AzureBlobStoragePath"),
                            ]
                        ),]),]),
                        Bracketed::new(vec_of_erased![Ref::new("QuotedStarSegment"),]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_BLOCKED_LOCATIONS"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("S3Path"),
                            Ref::new("GCSPath"),
                            Ref::new("AzureBlobStoragePath"),
                        ]
                    ),]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("AUTO_INGEST"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ERROR_INTEGRATION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AWS_SNS_TOPIC"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTEGRATION"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("WITH"),]).config(|this| this.optional()),
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("WarehouseObjectPropertiesSegment"),
                    Ref::new("CommentEqualsClauseSegment"),
                    Ref::new("WarehouseObjectParamsSegment"),
                ]),
                Ref::new("TagBracketedEqualsSegment").optional(),
            ])
            .config(|this| this.optional()),
            Ref::new("CommentEqualsClauseSegment").optional(),
            Ref::keyword("AS").optional(),
            one_of(vec_of_erased![
                Ref::new("SelectStatementSegment"),
                Sequence::new(vec_of_erased![
                    Bracketed::new(vec_of_erased![Ref::new("FunctionContentsGrammar"),])
                        .config(|this| this.optional()),
                    Ref::keyword("RETURNS"),
                    Ref::new("DatatypeSegment"),
                    Ref::new("FunctionAssignerSegment"),
                    Ref::new("ExpressionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COMMENT"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ])
                    .config(|this| this.optional()),
                ])
                .config(|this| this.optional()),
                Ref::new("CopyIntoTableStatementSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct CreateUserSegment;

impl NodeTrait for CreateUserSegment {
    const TYPE: &'static str = "create_user_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Ref::keyword("USER"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS"),
            ])
            .config(|this| this.optional()),
            Ref::new("ObjectReferenceSegment"),
            MetaSegment::indent(),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("PASSWORD"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LOGIN_NAME"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISPLAY_NAME"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FIRST_NAME"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MIDDLE_NAME"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LAST_NAME"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EMAIL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MUST_CHANGE_PASSWORD"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DISABLED"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DAYS_TO_EXPIRY"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MINS_TO_UNLOCK"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT_WAREHOUSE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT_NAMESPACE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT_ROLE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT_SECONDARY_ROLES"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MINS_TO_BYPASS_MFA"),
                    Ref::new("EqualsSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RSA_PUBLIC_KEY"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RSA_PUBLIC_KEY_2"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Ref::new("CommentEqualsClauseSegment"),
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct CreateViewStatementSegment;

impl NodeTrait for CreateViewStatementSegment {
    const TYPE: &'static str = "create_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            any_set_of(vec_of_erased![Ref::keyword("SECURE"), Ref::keyword("RECURSIVE"),]),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("VIEW"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            any_set_of(vec_of_erased![
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                    vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("CommentClauseSegment").optional(),
                    ]
                ),]),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH").optional(),
                    Ref::keyword("ROW"),
                    Ref::keyword("ACCESS"),
                    Ref::keyword("POLICY"),
                    Ref::new("NakedIdentifierSegment"),
                    Ref::keyword("ON"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    ),]),]),
                ]),
                Ref::new("TagBracketedEqualsSegment"),
                Sequence::new(vec_of_erased![Ref::keyword("COPY"), Ref::keyword("GRANTS"),]),
                Ref::new("CommentEqualsClauseSegment"),
            ]),
            Ref::keyword("AS"),
            optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar"),]),
        ])
        .to_matchable()
    }
}

pub struct AlterViewStatementSegment;

impl NodeTrait for AlterViewStatementSegment {
    const TYPE: &'static str = "alter_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMMENT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("UNSET"), Ref::keyword("COMMENT"),]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("UNSET"),]),
                    Ref::keyword("SECURE"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::new("TagEqualsSegment"),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    Ref::keyword("TAG"),
                    Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                ]),
                Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ADD"),
                        Ref::keyword("ROW"),
                        Ref::keyword("ACCESS"),
                        Ref::keyword("POLICY"),
                        Ref::new("FunctionNameSegment"),
                        Ref::keyword("ON"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        ),]),]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DROP"),
                        Ref::keyword("ROW"),
                        Ref::keyword("ACCESS"),
                        Ref::keyword("POLICY"),
                        Ref::new("FunctionNameSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("ALTER"), Ref::keyword("MODIFY"),]),
                    one_of(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMN").optional(),
                            Ref::new("ColumnReferenceSegment"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("SET"),
                                    Ref::keyword("MASKING"),
                                    Ref::keyword("POLICY"),
                                    Ref::new("FunctionNameSegment"),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("USING"),
                                        Bracketed::new(vec_of_erased![Delimited::new(
                                            vec_of_erased![Ref::new("ColumnReferenceSegment"),]
                                        ),])
                                        .config(|this| this.optional()),
                                    ])
                                    .config(|this| this.optional()),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("UNSET"),
                                    Ref::keyword("MASKING"),
                                    Ref::keyword("POLICY"),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("SET"),
                                    Ref::new("TagEqualsSegment"),
                                ]),
                            ]),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("COLUMN"),
                            Ref::new("ColumnReferenceSegment"),
                            Ref::keyword("UNSET"),
                            Ref::keyword("TAG"),
                            Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                        ]),
                    ]),]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterMaterializedViewStatementSegment;

impl NodeTrait for AlterMaterializedViewStatementSegment {
    const TYPE: &'static str = "alter_materialized_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            Ref::new("TableReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTER"),
                    Ref::keyword("BY"),
                    Delimited::new(vec_of_erased![Ref::new("ExpressionSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("CLUSTERING"),
                    Ref::keyword("KEY"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("SUSPEND"), Ref::keyword("RECLUSTER"),]),
                Sequence::new(vec_of_erased![Ref::keyword("RESUME"), Ref::keyword("RECLUSTER"),]),
                Ref::keyword("SUSPEND"),
                Ref::keyword("RESUME"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("UNSET"),]),
                    one_of(vec_of_erased![
                        Ref::keyword("SECURE"),
                        Ref::new("CommentEqualsClauseSegment"),
                        Ref::new("TagEqualsSegment"),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateFileFormatSegment;

impl NodeTrait for CreateFileFormatSegment {
    const TYPE: &'static str = "create_file_format_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Sequence::new(vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMAT"),]),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Ref::new("CsvFileFormatTypeParameters"),
                Ref::new("JsonFileFormatTypeParameters"),
                Ref::new("AvroFileFormatTypeParameters"),
                Ref::new("OrcFileFormatTypeParameters"),
                Ref::new("ParquetFileFormatTypeParameters"),
                Ref::new("XmlFileFormatTypeParameters"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("CommaSegment").optional(),
                Ref::new("CommentEqualsClauseSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterFileFormatSegment;

impl NodeTrait for AlterFileFormatSegment {
    const TYPE: &'static str = "alter_file_format_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Sequence::new(vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMAT"),]),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    one_of(vec_of_erased![
                        Ref::new("CsvFileFormatTypeParameters"),
                        Ref::new("JsonFileFormatTypeParameters"),
                        Ref::new("AvroFileFormatTypeParameters"),
                        Ref::new("OrcFileFormatTypeParameters"),
                        Ref::new("ParquetFileFormatTypeParameters"),
                        Ref::new("XmlFileFormatTypeParameters"),
                    ]),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::new("CommaSegment").optional(),
                Ref::new("CommentEqualsClauseSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct CsvFileFormatTypeParameters;

impl NodeTrait for CsvFileFormatTypeParameters {
    const TYPE: &'static str = "csv_file_format_type_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        let file_format_type_parameter = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    StringParser::new(
                        "'CSV'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "CSV",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("CompressionType"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILE_EXTENSION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("SKIP_HEADER"),
                Ref::new("EqualsSegment"),
                Ref::new("IntegerSegment"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATE_FORMAT"),
                    Ref::keyword("TIME_FORMAT"),
                    Ref::keyword("TIMESTAMP_FORMAT"),
                ]),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("AUTO"), Ref::new("QuotedLiteralSegment"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("BINARY_FORMAT"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("HEX"),
                    Ref::keyword("BASE64"),
                    Ref::keyword("UTF8"),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("RECORD_DELIMITER"),
                    Ref::keyword("FIELD_DELIMITER"),
                    Ref::keyword("ESCAPE"),
                    Ref::keyword("ESCAPE_UNENCLOSED_FIELD"),
                    Ref::keyword("FIELD_OPTIONALLY_ENCLOSED_BY"),
                ]),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULL_IF"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),])
                        .config(|this| this.optional()),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("SKIP_BLANK_LINES"),
                    Ref::keyword("ERROR_ON_COLUMN_COUNT_MISMATCH"),
                    Ref::keyword("REPLACE_INVALID_CHARACTERS"),
                    Ref::keyword("VALIDATE_UTF8"),
                    Ref::keyword("EMPTY_FIELD_AS_NULL"),
                    Ref::keyword("SKIP_BYTE_ORDER_MARK"),
                    Ref::keyword("TRIM_SPACE"),
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ENCODING"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("UTF8"), Ref::new("QuotedLiteralSegment"),]),
            ]),
        ]);

        one_of(vec_of_erased![
            Delimited::new(vec_of_erased![file_format_type_parameter.clone()]),
            AnyNumberOf::new(vec_of_erased![file_format_type_parameter]),
        ])
        .to_matchable()
    }
}

pub struct JsonFileFormatTypeParameters;

impl NodeTrait for JsonFileFormatTypeParameters {
    const TYPE: &'static str = "json_file_format_type_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        let file_format_type_parameter = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    StringParser::new(
                        "'JSON'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "JSON",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("CompressionType"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATE_FORMAT"),
                    Ref::keyword("TIME_FORMAT"),
                    Ref::keyword("TIMESTAMP_FORMAT"),
                ]),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::new("QuotedLiteralSegment"), Ref::keyword("AUTO"),]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("BINARY_FORMAT"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("HEX"),
                    Ref::keyword("BASE64"),
                    Ref::keyword("UTF8"),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULL_IF"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),])
                        .config(|this| this.optional()),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILE_EXTENSION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("TRIM_SPACE"),
                    Ref::keyword("ENABLE_OCTAL"),
                    Ref::keyword("ALLOW_DUPLICATE"),
                    Ref::keyword("STRIP_OUTER_ARRAY"),
                    Ref::keyword("STRIP_NULL_VALUES"),
                    Ref::keyword("REPLACE_INVALID_CHARACTERS"),
                    Ref::keyword("IGNORE_UTF8_ERRORS"),
                    Ref::keyword("SKIP_BYTE_ORDER_MARK"),
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
        ]);

        one_of(vec_of_erased![
            Delimited::new(vec_of_erased![file_format_type_parameter.clone()]),
            AnyNumberOf::new(vec_of_erased![file_format_type_parameter]),
        ])
        .to_matchable()
    }
}

pub struct AvroFileFormatTypeParameters;

impl NodeTrait for AvroFileFormatTypeParameters {
    const TYPE: &'static str = "avro_file_format_type_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        let file_format_type_parameter = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    StringParser::new(
                        "'AVRO'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "AVRO",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("CompressionType"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("TRIM_SPACE"),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULL_IF"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),])
                        .config(|this| this.optional()),
                ]),
            ]),
        ]);

        one_of(vec_of_erased![
            Delimited::new(vec_of_erased![file_format_type_parameter.clone()]),
            AnyNumberOf::new(vec_of_erased![file_format_type_parameter]),
        ])
        .to_matchable()
    }
}

pub struct OrcFileFormatTypeParameters;

impl NodeTrait for OrcFileFormatTypeParameters {
    const TYPE: &'static str = "orc_file_format_type_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        let file_format_type_parameter = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    StringParser::new(
                        "'ORC'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "ORC",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("TRIM_SPACE"),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULL_IF"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),])
                        .config(|this| this.optional()),
                ]),
            ]),
        ]);

        one_of(vec_of_erased![
            Delimited::new(vec_of_erased![file_format_type_parameter.clone()]),
            AnyNumberOf::new(vec_of_erased![file_format_type_parameter]),
        ])
        .to_matchable()
    }
}

pub struct ParquetFileFormatTypeParameters;

impl NodeTrait for ParquetFileFormatTypeParameters {
    const TYPE: &'static str = "parquet_file_format_type_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        let file_format_type_parameter = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    StringParser::new(
                        "'PARQUET'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "PARQUET",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("CompressionType"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("SNAPPY_COMPRESSION"),
                    Ref::keyword("BINARY_AS_TEXT"),
                    Ref::keyword("TRIM_SPACE"),
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("NULL_IF"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),])
                        .config(|this| this.optional()),
                ]),
            ]),
        ]);

        one_of(vec_of_erased![
            Delimited::new(vec_of_erased![file_format_type_parameter.clone()]),
            AnyNumberOf::new(vec_of_erased![file_format_type_parameter]),
        ])
        .to_matchable()
    }
}

pub struct XmlFileFormatTypeParameters;

impl NodeTrait for XmlFileFormatTypeParameters {
    const TYPE: &'static str = "xml_file_format_type_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        let file_format_type_parameter = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    StringParser::new(
                        "'XML'",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                    StringParser::new(
                        "XML",
                        |segment: &dyn Segment| {
                            SymbolSegment::create(
                                &segment.raw(),
                                segment.get_position_marker().unwrap().into(),
                                SymbolSegmentNewArgs { r#type: "file_type" },
                            )
                        },
                        None,
                        false,
                        None,
                    ),
                ]),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("CompressionType"),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("IGNORE_UTF8_ERRORS"),
                    Ref::keyword("PRESERVE_SPACE"),
                    Ref::keyword("STRIP_OUTER_ELEMENT"),
                    Ref::keyword("DISABLE_SNOWFLAKE_DATA"),
                    Ref::keyword("DISABLE_AUTO_CONVERT"),
                    Ref::keyword("SKIP_BYTE_ORDER_MARK"),
                ]),
                Ref::new("EqualsSegment"),
                Ref::new("BooleanLiteralGrammar"),
            ]),
        ]);

        one_of(vec_of_erased![
            Delimited::new(vec_of_erased![file_format_type_parameter.clone()]),
            AnyNumberOf::new(vec_of_erased![file_format_type_parameter]),
        ])
        .to_matchable()
    }
}

pub struct AlterPipeSegment;

impl NodeTrait for AlterPipeSegment {
    const TYPE: &'static str = "alter_pipe_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("PIPE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    AnyNumberOf::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PIPE_EXECUTION_PAUSED"),
                            Ref::new("EqualsSegment"),
                            Ref::new("BooleanLiteralGrammar"),
                        ]),
                        Ref::new("CommentEqualsClauseSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Ref::keyword("PIPE_EXECUTION_PAUSED"),
                        Ref::keyword("COMMENT"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("SET"), Ref::new("TagEqualsSegment"),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TAG"),
                        Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment"),]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFRESH"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PREFIX"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MODIFIED_AFTER"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
            ]),
            Ref::new("CommaSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct FileFormatSegment;

impl NodeTrait for FileFormatSegment {
    const TYPE: &'static str = "file_format_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("QuotedLiteralSegment"),
                Ref::new("ObjectReferenceSegment"),
            ]),
            Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FORMAT_NAME"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("ObjectReferenceSegment"),
                        ]),
                    ]),
                    one_of(vec_of_erased![
                        Ref::new("CsvFileFormatTypeParameters"),
                        Ref::new("JsonFileFormatTypeParameters"),
                        Ref::new("AvroFileFormatTypeParameters"),
                        Ref::new("OrcFileFormatTypeParameters"),
                        Ref::new("ParquetFileFormatTypeParameters"),
                        Ref::new("XmlFileFormatTypeParameters"),
                    ]),
                ]),
                Ref::new("FormatTypeOptions").optional(),
            ]),]),
        ])
        .to_matchable()
    }
}

pub struct FormatTypeOptions;

impl NodeTrait for FormatTypeOptions {
    const TYPE: &'static str = "format_type_options";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            // COPY INTO <location>, open for extension
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("COMPRESSION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("CompressionType"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RECORD_DELIMITER"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FIELD_DELIMITER"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ESCAPE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ESCAPE_UNENCLOSED_FIELD"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DATA_FORMAT"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("AUTO"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TIME_FORMAT"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TIMESTAMP_FORMAT"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BINARY_FORMAT"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("HEX"),
                        Ref::keyword("BASE64"),
                        Ref::keyword("UTF8"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FIELD_OPTIONALITY_ENCLOSED_BY"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![Ref::keyword("NONE"), Ref::new("QuotedLiteralSegment"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NULL_IF"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("QuotedLiteralSegment"),])
                            .config(|this| this.optional()),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EMPTY_FIELD_AS_NULL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SNAPPY_COMPRESSION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
            // COPY INTO <table>, open for extension
            any_set_of(vec_of_erased![]),
        ])
        .to_matchable()
    }
}

pub struct CreateExternalTableSegment;

impl NodeTrait for CreateExternalTableSegment {
    const TYPE: &'static str = "create_external_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Ref::keyword("EXTERNAL"),
            Ref::keyword("TABLE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS"),
            ])
            .config(|this| this.optional()),
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Sequence::new(
                vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("DatatypeSegment"),
                    Ref::keyword("AS"),
                    optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ExpressionSegment"),
                        Ref::new("TableConstraintSegment").optional(),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NOT").optional(),
                            Ref::keyword("NULL").optional(),
                        ])
                        .config(|this| this.optional()),
                    ])]),
                ]
            )])])
            .config(|this| this.optional()),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTEGRATION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARTITION"),
                    Ref::keyword("BY"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SingleIdentifierGrammar"
                    ),]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("WITH")])
                        .config(|this| this.optional()),
                    Ref::keyword("LOCATION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("StagePath"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFRESH_ON_CREATE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AUTO_REFRESH"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PATTERN"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FILE_FORMAT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("FileFormatSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AWS_SNS_TOPIC"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("COPY"), Ref::keyword("GRANTS"),]),
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("WITH")])
                        .config(|this| this.optional()),
                    Ref::keyword("ROW"),
                    Ref::keyword("ACCESS"),
                    Ref::keyword("POLICY"),
                    Ref::new("NakedIdentifierSegment"),
                ]),
                Ref::new("TagBracketedEqualsSegment"),
                Ref::new("CommentEqualsClauseSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct TableExpressionSegment;

impl NodeTrait for TableExpressionSegment {
    const TYPE: &'static str = "table_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::new("BareFunctionSegment"),
            Ref::new("FunctionSegment"),
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
            Ref::new("ValuesClauseSegment"),
            Sequence::new(vec_of_erased![
                Ref::new("StagePath"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FILE_FORMAT"),
                        Ref::new("ParameterAssignerSegment"),
                        Ref::new("FileFormatSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PATTERN"),
                        Ref::new("ParameterAssignerSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                ]),])
                .config(|this| this.optional()),
            ]),
        ])
        .to_matchable()
    }
}

pub struct PartitionBySegment;

impl NodeTrait for PartitionBySegment {
    const TYPE: &'static str = "partition_by_segment";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("PARTITION"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            optionally_bracketed(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ExpressionSegment"
            )]),]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct CopyIntoLocationStatementSegment;

impl NodeTrait for CopyIntoLocationStatementSegment {
    const TYPE: &'static str = "copy_into_location_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("COPY"),
            Ref::keyword("INTO"),
            Ref::new("StorageLocation"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ColumnReferenceSegment"
            )]),])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("FROM"),
                one_of(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectStatementSegment")]),
                ]),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Ref::new("S3ExternalStageParameters"),
                Ref::new("AzureBlobStorageExternalStageParameters"),
            ])
            .config(|this| this.optional()),
            Ref::new("InternalStageParameters").optional(),
            any_set_of(vec_of_erased![
                Ref::new("PartitionBySegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FILE_FORMAT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("FileFormatSegment"),
                ]),
                Ref::new("CopyOptionsSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("VALIDATION_MODE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ValidationModeOptionSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("HEADER"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CopyIntoTableStatementSegment;

impl NodeTrait for CopyIntoTableStatementSegment {
    const TYPE: &'static str = "copy_into_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("COPY"),
            Ref::keyword("INTO"),
            Ref::new("TableReferenceSegment"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ColumnReferenceSegment"
            )]),])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("FROM"),
                one_of(vec_of_erased![
                    Ref::new("StorageLocation"),
                    Bracketed::new(vec_of_erased![Ref::new("SelectStatementSegment")]),
                ]),
            ])
            .config(|this| this.optional()),
            one_of(vec_of_erased![
                Ref::new("S3ExternalStageParameters"),
                Ref::new("AzureBlobStorageExternalStageParameters"),
            ])
            .config(|this| this.optional()),
            Ref::new("InternalStageParameters").optional(),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("FILES"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "QuotedLiteralSegment"
                    )]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PATTERN"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("ReferencedVariableNameSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FILE_FORMAT"),
                    Ref::new("EqualsSegment"),
                    Ref::new("FileFormatSegment"),
                ]),
                Ref::new("CopyOptionsSegment"),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("VALIDATION_MODE"),
                Ref::new("EqualsSegment"),
                Ref::new("ValidationModeOptionSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct StorageLocation;

impl NodeTrait for StorageLocation {
    const TYPE: &'static str = "storage_location";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Ref::new("StagePath"),
            Ref::new("S3Path"),
            Ref::new("GCSPath"),
            Ref::new("AzureBlobStoragePath"),
        ])
        .to_matchable()
    }
}

pub struct InternalStageParameters;

impl NodeTrait for InternalStageParameters {
    const TYPE: &'static str = "stage_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("ENCRYPTION"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![
                    Ref::keyword("TYPE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("SnowflakeEncryptionOption"),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct S3ExternalStageParameters;

impl NodeTrait for S3ExternalStageParameters {
    const TYPE: &'static str = "s3_external_stage_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_INTEGRATION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREDENTIALS"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AWS_KEY_ID"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                            Ref::keyword("AWS_SECRET_KEY"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AWS_TOKEN"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment"),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AWS_ROLE"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ]),]),
                ]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("ENCRYPTION"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TYPE"),
                            Ref::new("EqualsSegment"),
                            Ref::new("S3EncryptionOption"),
                        ])
                        .config(|this| this.optional()),
                        Ref::keyword("MASTER_KEY"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TYPE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("S3EncryptionOption"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TYPE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("S3EncryptionOption"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("KMS_KEY_ID"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ])
                        .config(|this| this.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TYPE"),
                        Ref::new("EqualsSegment"),
                        Ref::keyword("NONE"),
                    ]),
                ]),]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct GCSExternalStageParameters;

impl NodeTrait for GCSExternalStageParameters {
    const TYPE: &'static str = "gcs_external_stage_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("STORAGE_INTEGRATION"),
                Ref::new("EqualsSegment"),
                Ref::new("ObjectReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("ENCRYPTION"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("TYPE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("GCSEncryptionOption"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("KMS_KEY_ID"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment"),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        Ref::keyword("NONE"),
                    ]),
                ]),]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AzureBlobStorageExternalStageParameters;

impl NodeTrait for AzureBlobStorageExternalStageParameters {
    const TYPE: &'static str = "azure_blob_storage_external_stage_parameters";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("STORAGE_INTEGRATION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREDENTIALS"),
                    Ref::new("EqualsSegment"),
                    Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("AZURE_SAS_TOKEN"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),]),
                ]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("ENCRYPTION"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("TYPE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("AzureBlobStorageEncryptionOption"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MASTER_KEY"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment"),
                            ])
                            .config(|this| this.optional()),
                        ]),
                        Ref::keyword("NONE"),
                    ]),
                ]),]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct CreateStageSegment;

impl NodeTrait for CreateStageSegment {
    const TYPE: &'static str = "create_stage_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Ref::keyword("TEMPORARY").optional(),
            Ref::keyword("STAGE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS"),
            ])
            .config(|this| this.optional()),
            Ref::new("ObjectReferenceSegment"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                // Internal stages
                Sequence::new(vec_of_erased![
                    Ref::new("InternalStageParameters").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DIRECTORY"),
                        Ref::new("EqualsSegment"),
                        Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::keyword("ENABLE"),
                            Ref::new("EqualsSegment"),
                            Ref::new("BooleanLiteralGrammar"),
                        ]),]),
                    ])
                    .config(|this| this.optional()),
                ]),
                // External S3 stage
                Sequence::new(vec_of_erased![
                    Ref::keyword("URL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("S3Path"),
                    Ref::new("S3ExternalStageParameters").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DIRECTORY"),
                        Ref::new("EqualsSegment"),
                        Bracketed::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ENABLE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AUTO_REFRESH"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ])
                            .config(|this| this.optional()),
                        ]),
                    ])
                    .config(|this| this.optional()),
                ]),
                // External GCS stage
                Sequence::new(vec_of_erased![
                    Ref::keyword("URL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("GCSPath"),
                    Ref::new("GCSExternalStageParameters").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DIRECTORY"),
                        Ref::new("EqualsSegment"),
                        Bracketed::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ENABLE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AUTO_REFRESH"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("NOTIFICATION_INTEGRATION"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::new("NakedIdentifierSegment"),
                                    Ref::new("QuotedLiteralSegment"),
                                ]),
                            ])
                            .config(|this| this.optional()),
                        ]),
                    ])
                    .config(|this| this.optional()),
                ]),
                // External Azure Blob Storage stage
                Sequence::new(vec_of_erased![
                    Ref::keyword("URL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("AzureBlobStoragePath"),
                    Ref::new("AzureBlobStorageExternalStageParameters").optional(),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DIRECTORY"),
                        Ref::new("EqualsSegment"),
                        Bracketed::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ENABLE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AUTO_REFRESH"),
                                Ref::new("EqualsSegment"),
                                Ref::new("BooleanLiteralGrammar"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("NOTIFICATION_INTEGRATION"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::new("NakedIdentifierSegment"),
                                    Ref::new("QuotedLiteralSegment"),
                                ]),
                            ])
                            .config(|this| this.optional()),
                        ]),
                    ])
                    .config(|this| this.optional()),
                ]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILE_FORMAT"),
                Ref::new("EqualsSegment"),
                Ref::new("FileFormatSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("COPY_OPTIONS"),
                Ref::new("EqualsSegment"),
                Bracketed::new(vec_of_erased![Ref::new("CopyOptionsSegment")]),
            ])
            .config(|this| this.optional()),
            Ref::new("TagBracketedEqualsSegment").optional(),
            Ref::new("CommentEqualsClauseSegment").optional(),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct AlterStageSegment;

impl NodeTrait for AlterStageSegment {
    const TYPE: &'static str = "alter_stage_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("STAGE"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    MetaSegment::indent(),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("InternalStageParameters"),
                                Sequence::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("URL"),
                                        Ref::new("EqualsSegment"),
                                        Ref::new("S3Path"),
                                    ])
                                    .config(|this| this.optional()),
                                    Ref::new("S3ExternalStageParameters").optional(),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("URL"),
                                        Ref::new("EqualsSegment"),
                                        Ref::new("GCSPath"),
                                    ])
                                    .config(|this| this.optional()),
                                    Ref::new("GCSExternalStageParameters").optional(),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("URL"),
                                        Ref::new("EqualsSegment"),
                                        Ref::new("AzureBlobStoragePath"),
                                    ])
                                    .config(|this| this.optional()),
                                    Ref::new("AzureBlobStorageExternalStageParameters").optional(),
                                ]),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILE_FORMAT"),
                                Ref::new("EqualsSegment"),
                                Ref::new("FileFormatSegment"),
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COPY_OPTIONS"),
                                Ref::new("EqualsSegment"),
                                Bracketed::new(vec_of_erased![Ref::new("CopyOptionsSegment")]),
                            ])
                            .config(|this| this.optional()),
                            Ref::new("CommentEqualsClauseSegment").optional(),
                        ]),
                        Ref::new("TagEqualsSegment"),
                    ]),
                    MetaSegment::dedent(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFRESH"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SUBPATH"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateStreamStatementSegment;

impl NodeTrait for CreateStreamStatementSegment {
    const TYPE: &'static str = "create_stream_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::keyword("STREAM"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            Sequence::new(vec_of_erased![Ref::keyword("COPY"), Ref::keyword("GRANTS"),])
                .config(|this| this.optional()),
            Ref::keyword("ON"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("TABLE"), Ref::keyword("VIEW"),]),
                    Ref::new("ObjectReferenceSegment"),
                    one_of(vec_of_erased![
                        Ref::new("FromAtExpressionSegment"),
                        Ref::new("FromBeforeExpressionSegment"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("APPEND_ONLY"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SHOW_INITIAL_ROWS"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXTERNAL"),
                    Ref::keyword("TABLE"),
                    Ref::new("ObjectReferenceSegment"),
                    one_of(vec_of_erased![
                        Ref::new("FromAtExpressionSegment"),
                        Ref::new("FromBeforeExpressionSegment"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INSERT_ONLY"),
                        Ref::new("EqualsSegment"),
                        Ref::new("TrueSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STAGE"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
            ]),
            Ref::new("CommentEqualsClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct AlterStreamStatementSegment;

impl NodeTrait for AlterStreamStatementSegment {
    const TYPE: &'static str = "alter_stream_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("STREAM"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("APPEND_ONLY"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BooleanLiteralGrammar"),
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INSERT_ONLY"),
                        Ref::new("EqualsSegment"),
                        Ref::new("TrueSegment"),
                    ])
                    .config(|this| this.optional()),
                    Ref::new("TagEqualsSegment").optional(),
                    Ref::new("CommentEqualsClauseSegment").optional(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TAG"),
                            Delimited::new(vec_of_erased![Ref::new("TagReferenceSegment")]),
                        ]),
                        Ref::keyword("COMMENT"),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ShowStatementSegment;

impl NodeTrait for ShowStatementSegment {
    const TYPE: &'static str = "show_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        let object_types_plural = one_of(vec_of_erased![
            Ref::keyword("PARAMETERS"),
            Sequence::new(vec_of_erased![Ref::keyword("GLOBAL"), Ref::keyword("ACCOUNTS"),]),
            Ref::keyword("REGIONS"),
            Sequence::new(vec_of_erased![Ref::keyword("REPLICATION"), Ref::keyword("ACCOUNTS"),]),
            Sequence::new(vec_of_erased![Ref::keyword("REPLICATION"), Ref::keyword("DATABASES"),]),
            Ref::keyword("PARAMETERS"),
            Ref::keyword("VARIABLES"),
            Ref::keyword("TRANSACTIONS"),
            Ref::keyword("LOCKS"),
            Ref::keyword("PARAMETERS"),
            Ref::keyword("FUNCTIONS"),
            Sequence::new(vec_of_erased![Ref::keyword("NETWORK"), Ref::keyword("POLICIES"),]),
            Ref::keyword("SHARES"),
            Ref::keyword("ROLES"),
            Ref::keyword("GRANTS"),
            Ref::keyword("USERS"),
            Ref::keyword("WAREHOUSES"),
            Ref::keyword("DATABASES"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("API"),
                    Ref::keyword("NOTIFICATION"),
                    Ref::keyword("SECURITY"),
                    Ref::keyword("STORAGE"),
                ])
                .config(|this| this.optional()),
                Ref::keyword("INTEGRATIONS"),
            ]),
            Ref::keyword("SCHEMAS"),
            Ref::keyword("OBJECTS"),
            Ref::keyword("TABLES"),
            Sequence::new(vec_of_erased![Ref::keyword("EXTERNAL"), Ref::keyword("TABLES"),]),
            Ref::keyword("VIEWS"),
            Sequence::new(vec_of_erased![Ref::keyword("MATERIALIZED"), Ref::keyword("VIEWS"),]),
            Sequence::new(vec_of_erased![Ref::keyword("MASKING"), Ref::keyword("POLICIES"),]),
            Ref::keyword("COLUMNS"),
            Sequence::new(vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMATS"),]),
            Ref::keyword("SEQUENCES"),
            Ref::keyword("STAGES"),
            Ref::keyword("PIPES"),
            Ref::keyword("STREAMS"),
            Ref::keyword("TASKS"),
            Sequence::new(vec_of_erased![Ref::keyword("USER"), Ref::keyword("FUNCTIONS"),]),
            Sequence::new(vec_of_erased![Ref::keyword("EXTERNAL"), Ref::keyword("FUNCTIONS"),]),
            Ref::keyword("PROCEDURES"),
            Sequence::new(vec_of_erased![Ref::keyword("FUTURE"), Ref::keyword("GRANTS"),]),
        ]);

        let object_scope_types = one_of(vec_of_erased![
            Ref::keyword("ACCOUNT"),
            Ref::keyword("SESSION"),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATABASE"),
                    Ref::keyword("SCHEMA"),
                    Ref::keyword("SHARE"),
                    Ref::keyword("ROLE"),
                    Ref::keyword("TABLE"),
                    Ref::keyword("TASK"),
                    Ref::keyword("USER"),
                    Ref::keyword("WAREHOUSE"),
                    Ref::keyword("VIEW"),
                ]),
                Ref::new("ObjectReferenceSegment").optional(),
            ]),
        ]);

        Sequence::new(vec_of_erased![
            Ref::keyword("SHOW"),
            Ref::keyword("TERSE").optional(),
            object_types_plural,
            Ref::keyword("HISTORY").optional(),
            Sequence::new(vec_of_erased![Ref::keyword("LIKE"), Ref::new("QuotedLiteralSegment"),])
                .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("TO"),
                    Ref::keyword("OF"),
                    Ref::keyword("IN"),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![object_scope_types]),
                    Ref::new("ObjectReferenceSegment"),
                ]),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("STARTS"),
                Ref::keyword("WITH"),
                Ref::new("QuotedLiteralSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("PRIMARY"),
                Ref::new("ObjectReferenceSegment"),
            ])
            .config(|this| this.optional()),
            Sequence::new(vec_of_erased![
                Ref::new("LimitClauseSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FROM"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct AlterUserStatementSegment;

impl NodeTrait for AlterUserStatementSegment {
    const TYPE: &'static str = "alter_user_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("USER"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("RoleReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("RESET"), Ref::keyword("PASSWORD"),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ABORT"),
                    Ref::keyword("ALL"),
                    Ref::keyword("QUERIES"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("DELEGATED"),
                    Ref::keyword("AUTHORIZATION"),
                    Ref::keyword("OF"),
                    Ref::keyword("ROLE"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::keyword("SECURITY"),
                    Ref::keyword("INTEGRATION"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REMOVE"),
                    Ref::keyword("DELEGATED"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AUTHORIZATION"),
                            Ref::keyword("OF"),
                            Ref::keyword("ROLE"),
                            Ref::new("ObjectReferenceSegment"),
                        ]),
                        Ref::keyword("AUTHORIZATIONS"),
                    ]),
                    Ref::keyword("FROM"),
                    Ref::keyword("SECURITY"),
                    Ref::keyword("INTEGRATION"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::new("LiteralGrammar"),
                            Ref::new("ObjectReferenceSegment"),
                        ]),
                    ]),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNSET"),
                    Delimited::new(vec_of_erased![Ref::new("ParameterNameSegment")]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CreateRoleStatementSegment;

impl NodeTrait for CreateRoleStatementSegment {
    const TYPE: &'static str = "create_role_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("REPLACE"),])
                .config(|this| this.optional()),
            Ref::keyword("ROLE"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::keyword("NOT"),
                Ref::keyword("EXISTS"),
            ])
            .config(|this| this.optional()),
            Ref::new("RoleReferenceSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMMENT"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct ExplainStatementSegment;

impl NodeTrait for ExplainStatementSegment {
    const TYPE: &'static str = "explain_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("EXPLAIN"),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                one_of(vec_of_erased![
                    Ref::keyword("TABULAR"),
                    Ref::keyword("JSON"),
                    Ref::keyword("TEXT"),
                ]),
            ])
            .config(|this| this.optional()),
            ansi::ExplainStatementSegment::explainable_stmt(),
        ])
        .to_matchable()
    }
}

pub struct AlterSessionStatementSegment;

impl NodeTrait for AlterSessionStatementSegment {
    const TYPE: &'static str = "alter_session_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("SESSION"),
            one_of(vec_of_erased![
                Ref::new("AlterSessionSetClauseSegment"),
                Ref::new("AlterSessionUnsetClauseSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterSessionSetClauseSegment;

impl NodeTrait for AlterSessionSetClauseSegment {
    const TYPE: &'static str = "alter_session_set_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            Ref::new("ParameterNameSegment"),
            Ref::new("EqualsSegment"),
            one_of(vec_of_erased![
                Ref::new("BooleanLiteralGrammar"),
                Ref::new("QuotedLiteralSegment"),
                Ref::new("NumericLiteralSegment"),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterSessionUnsetClauseSegment;

impl NodeTrait for AlterSessionUnsetClauseSegment {
    const TYPE: &'static str = "alter_session_unset_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNSET"),
            Delimited::new(vec_of_erased![Ref::new("ParameterNameSegment")]),
        ])
        .to_matchable()
    }
}

pub struct AlterTaskStatementSegment;

impl NodeTrait for AlterTaskStatementSegment {
    const TYPE: &'static str = "alter_task_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("TASK"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("ObjectReferenceSegment"),
            one_of(vec_of_erased![
                Ref::keyword("RESUME"),
                Ref::keyword("SUSPEND"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REMOVE"),
                    Ref::keyword("AFTER"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::keyword("AFTER"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Ref::new("AlterTaskSpecialSetClauseSegment"),
                Ref::new("AlterTaskSetClauseSegment"),
                Ref::new("AlterTaskUnsetClauseSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MODIFY"),
                    Ref::keyword("AS"),
                    ansi::ExplainStatementSegment::explainable_stmt(),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MODIFY"),
                    Ref::keyword("WHEN"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct AlterTaskSpecialSetClauseSegment;

impl NodeTrait for AlterTaskSpecialSetClauseSegment {
    const TYPE: &'static str = "alter_task_special_set_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("WAREHOUSE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEDULE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALLOW_OVERLAPPING_EXECUTION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ])
                .config(|this| this.optional()),
            ])
            .config(|this| this.min_times(1)),
        ])
        .to_matchable()
    }
}

pub struct AlterTaskSetClauseSegment;

impl NodeTrait for AlterTaskSetClauseSegment {
    const TYPE: &'static str = "alter_task_set_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("ParameterNameSegment"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::new("BooleanLiteralGrammar"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("NumericLiteralSegment"),
                ]),
            ]),]),
        ])
        .to_matchable()
    }
}

pub struct AlterTaskUnsetClauseSegment;

impl NodeTrait for AlterTaskUnsetClauseSegment {
    const TYPE: &'static str = "alter_task_unset_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNSET"),
            Delimited::new(vec_of_erased![Ref::new("ParameterNameSegment")]),
        ])
        .to_matchable()
    }
}

pub struct ExecuteTaskClauseSegment;

impl NodeTrait for ExecuteTaskClauseSegment {
    const TYPE: &'static str = "execute_task_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("EXECUTE"),
            Ref::keyword("TASK"),
            Ref::new("ObjectReferenceSegment"),
        ])
        .to_matchable()
    }
}

pub struct MergeUpdateClauseSegment;

impl NodeTrait for MergeUpdateClauseSegment {
    const TYPE: &'static str = "merge_update_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UPDATE"),
            Ref::new("SetClauseListSegment"),
            Ref::new("WhereClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct MergeDeleteClauseSegment;

impl NodeTrait for MergeDeleteClauseSegment {
    const TYPE: &'static str = "merge_delete_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DELETE"),
            Ref::new("WhereClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct MergeInsertClauseSegment;

impl NodeTrait for MergeInsertClauseSegment {
    const TYPE: &'static str = "merge_insert_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("INSERT"),
            MetaSegment::indent(),
            Ref::new("BracketedColumnReferenceListGrammar").optional(),
            MetaSegment::dedent(),
            Ref::new("ValuesClauseSegment").optional(),
            Ref::new("WhereClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct DeleteStatementSegment;

impl NodeTrait for DeleteStatementSegment {
    const TYPE: &'static str = "delete_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DELETE"),
            Ref::keyword("FROM"),
            Ref::new("TableReferenceSegment"),
            Ref::new("AliasExpressionSegment").optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USING"),
                MetaSegment::indent(),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("TableExpressionSegment"),
                    Ref::new("AliasExpressionSegment").optional(),
                ]),]),
                MetaSegment::dedent(),
            ])
            .config(|this| this.optional()),
            Ref::new("WhereClauseSegment").optional(),
        ])
        .to_matchable()
    }
}

pub struct DescribeStatementSegment;

impl NodeTrait for DescribeStatementSegment {
    const TYPE: &'static str = "describe_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("DESCRIBE"), Ref::keyword("DESC"),]),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RESULT"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LAST_QUERY_ID"),
                            Bracketed::new(vec_of_erased![]),
                        ]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NETWORK"),
                    Ref::keyword("POLICY"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SHARE"),
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("ObjectReferenceSegment"),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USER"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WAREHOUSE"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DATABASE"),
                    Ref::new("DatabaseReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("API"),
                        Ref::keyword("NOTIFICATION"),
                        Ref::keyword("SECURITY"),
                        Ref::keyword("STORAGE"),
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("INTEGRATION"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SESSION"),
                    Ref::keyword("POLICY"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TYPE"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![Ref::keyword("COLUMNS"), Ref::keyword("STAGE"),]),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXTERNAL"),
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TYPE"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![Ref::keyword("COLUMNS"), Ref::keyword("STAGE"),]),
                    ])
                    .config(|this| this.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("VIEW"),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MATERIALIZED"),
                    Ref::keyword("VIEW"),
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SEQUENCE"),
                    Ref::new("SequenceReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("MASKING"),
                    Ref::keyword("POLICY"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROW"),
                    Ref::keyword("ACCESS"),
                    Ref::keyword("POLICY"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FILE"),
                    Ref::keyword("FORMAT"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STAGE"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PIPE"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("STREAM"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TASK"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("FUNCTION"),
                    Ref::new("FunctionNameSegment"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("DatatypeSegment"),])
                            .config(|this| this.optional()),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PROCEDURE"),
                    Ref::new("FunctionNameSegment"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("DatatypeSegment"),])
                            .config(|this| this.optional()),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct TransactionStatementSegment;

impl NodeTrait for TransactionStatementSegment {
    const TYPE: &'static str = "transaction_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                one_of(vec_of_erased![Ref::keyword("WORK"), Ref::keyword("TRANSACTION"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NAME"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("START"),
                Ref::keyword("TRANSACTION"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NAME"),
                    Ref::new("ObjectReferenceSegment"),
                ])
                .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("COMMIT"), Ref::keyword("WORK").optional(),]),
            Ref::keyword("ROLLBACK"),
        ])
        .to_matchable()
    }
}

pub struct TruncateStatementSegment;

impl NodeTrait for TruncateStatementSegment {
    const TYPE: &'static str = "truncate_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("TRUNCATE"),
            Ref::keyword("TABLE").optional(),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::new("TableReferenceSegment"),
        ])
        .to_matchable()
    }
}

pub struct UnsetStatementSegment;

impl NodeTrait for UnsetStatementSegment {
    const TYPE: &'static str = "unset_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNSET"),
            one_of(vec_of_erased![
                Ref::new("LocalVariableNameSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "LocalVariableNameSegment"
                ),]),]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct UndropStatementSegment;

impl NodeTrait for UndropStatementSegment {
    const TYPE: &'static str = "undrop_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("UNDROP"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("DATABASE"),
                    Ref::new("DatabaseReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEMA"),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLE"),
                    Ref::new("TableReferenceSegment"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CommentStatementSegment;

impl NodeTrait for CommentStatementSegment {
    const TYPE: &'static str = "comment_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("COMMENT"),
            Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS"),])
                .config(|this| this.optional()),
            Ref::keyword("ON"),
            one_of(vec_of_erased![
                Ref::keyword("COLUMN"),
                Ref::keyword("TABLE"),
                Ref::keyword("VIEW"),
                Ref::keyword("SCHEMA"),
                Ref::keyword("DATABASE"),
                Ref::keyword("WAREHOUSE"),
                Ref::keyword("USER"),
                Ref::keyword("STAGE"),
                Ref::keyword("FUNCTION"),
                Ref::keyword("PROCEDURE"),
                Ref::keyword("SEQUENCE"),
                Ref::keyword("SHARE"),
                Ref::keyword("PIPE"),
                Ref::keyword("STREAM"),
                Ref::keyword("TASK"),
                Sequence::new(vec_of_erased![Ref::keyword("NETWORK"), Ref::keyword("POLICY"),]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("API"),
                        Ref::keyword("NOTIFICATION"),
                        Ref::keyword("SECURITY"),
                        Ref::keyword("STORAGE"),
                    ]),
                    Ref::keyword("INTEGRATION"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("SESSION"), Ref::keyword("POLICY"),]),
                Sequence::new(vec_of_erased![Ref::keyword("EXTERNAL"), Ref::keyword("TABLE"),]),
                Sequence::new(vec_of_erased![Ref::keyword("MATERIALIZED"), Ref::keyword("VIEW"),]),
                Sequence::new(vec_of_erased![Ref::keyword("MASKING"), Ref::keyword("POLICY"),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROW"),
                    Ref::keyword("ACCESS"),
                    Ref::keyword("POLICY"),
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMAT"),]),
            ]),
            Ref::new("ObjectReferenceSegment"),
            Ref::keyword("IS"),
            Ref::new("QuotedLiteralSegment"),
        ])
        .to_matchable()
    }
}

pub struct UseStatementSegment;

impl NodeTrait for UseStatementSegment {
    const TYPE: &'static str = "use_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("USE"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("ROLE"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WAREHOUSE"),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DATABASE").optional(),
                    Ref::new("DatabaseReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEMA").optional(),
                    Ref::new("SchemaReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SECONDARY"),
                    Ref::keyword("ROLES"),
                    one_of(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("NONE"),]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct CallStatementSegment;

impl NodeTrait for CallStatementSegment {
    const TYPE: &'static str = "call_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("CALL"),
            Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Bracketed::new(vec_of_erased![Ref::new("FunctionContentsGrammar").optional(),])
                    .config(|this| this.parse_mode(ParseMode::Greedy)),
            ]),
        ])
        .to_matchable()
    }
}

pub struct LimitClauseSegment;

impl NodeTrait for LimitClauseSegment {
    const TYPE: &'static str = "limit_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("LIMIT"),
                MetaSegment::indent(),
                Ref::new("LimitLiteralGrammar"),
                MetaSegment::dedent(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OFFSET"),
                    MetaSegment::indent(),
                    Ref::new("LimitLiteralGrammar"),
                    MetaSegment::dedent(),
                ])
                .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("OFFSET"),
                    MetaSegment::indent(),
                    Ref::new("LimitLiteralGrammar"),
                    one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("ROWS"),])
                        .config(|this| this.optional()),
                    MetaSegment::dedent(),
                ])
                .config(|this| this.optional()),
                Ref::keyword("FETCH"),
                MetaSegment::indent(),
                one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("NEXT"),])
                    .config(|this| this.optional()),
                Ref::new("LimitLiteralGrammar"),
                one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("ROWS"),])
                    .config(|this| this.optional()),
                Ref::keyword("ONLY").optional(),
                MetaSegment::dedent(),
            ]),
        ])
        .to_matchable()
    }
}

pub struct SelectClauseSegment;

impl NodeTrait for SelectClauseSegment {
    const TYPE: &'static str = "select_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        ansi::SelectClauseSegment::match_grammar().copy(
            None,
            None,
            None,
            None,
            vec_of_erased![Ref::keyword("FETCH"), Ref::keyword("OFFSET")],
            false,
        )
    }
}

pub struct OrderByClauseSegment;

impl NodeTrait for OrderByClauseSegment {
    const TYPE: &'static str = "order_by_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("ORDER"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment"),
                ]),
                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC"),])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NULLS"),
                    one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("LAST"),]),
                ])
                .config(|this| this.optional()),
            ]),])
            .config(|this| this.terminators = vec_of_erased![
                Ref::keyword("LIMIT"),
                Ref::keyword("FETCH"),
                Ref::keyword("OFFSET"),
                Ref::new("FrameClauseUnitGrammar")
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
    }
}

pub struct FrameClauseSegment;

impl NodeTrait for FrameClauseSegment {
    const TYPE: &'static str = "frame_clause";

    fn match_grammar() -> Arc<dyn Matchable> {
        let frame_extent = one_of(vec_of_erased![
            Sequence::new(vec_of_erased![Ref::keyword("CURRENT"), Ref::keyword("ROW"),]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ReferencedVariableNameSegment"),
                    Ref::keyword("UNBOUNDED"),
                ]),
                one_of(vec_of_erased![Ref::keyword("PRECEDING"), Ref::keyword("FOLLOWING"),]),
            ]),
        ]);

        Sequence::new(vec_of_erased![
            Ref::new("FrameClauseUnitGrammar"),
            one_of(vec_of_erased![
                frame_extent.clone(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("BETWEEN"),
                    frame_extent.clone(),
                    Ref::keyword("AND"),
                    frame_extent,
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct DropProcedureStatementSegment;

impl NodeTrait for DropProcedureStatementSegment {
    const TYPE: &'static str = "drop_procedure_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("PROCEDURE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
        ])
        .to_matchable()
    }
}

pub struct DropExternalTableStatementSegment;

impl NodeTrait for DropExternalTableStatementSegment {
    const TYPE: &'static str = "drop_external_table_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("EXTERNAL"),
            Ref::keyword("TABLE"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
            Ref::new("DropBehaviorGrammar").optional(),
        ])
        .to_matchable()
    }
}

pub struct DropFunctionStatementSegment;

impl NodeTrait for DropFunctionStatementSegment {
    const TYPE: &'static str = "drop_function_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("EXTERNAL").optional(),
            Ref::keyword("FUNCTION"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammar"),
        ])
        .to_matchable()
    }
}

pub struct DropMaterializedViewStatementSegment;

impl NodeTrait for DropMaterializedViewStatementSegment {
    const TYPE: &'static str = "drop_materialized_view_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("MATERIALIZED"),
            Ref::keyword("VIEW"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TableReferenceSegment"),
        ])
        .to_matchable()
    }
}

pub struct DropObjectStatementSegment;

impl NodeTrait for DropObjectStatementSegment {
    const TYPE: &'static str = "drop_object_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CONNECTION"),
                        Sequence::new(
                            vec_of_erased![Ref::keyword("FILE"), Ref::keyword("FORMAT"),]
                        ),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("API"),
                                Ref::keyword("NOTIFICATION"),
                                Ref::keyword("SECURITY"),
                                Ref::keyword("STORAGE"),
                            ])
                            .config(|this| this.optional()),
                            Ref::keyword("INTEGRATION"),
                        ]),
                        Ref::keyword("PIPE"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ROW"),
                            Ref::keyword("ACCESS"),
                            Ref::keyword("POLICY"),
                        ]),
                        Ref::keyword("STAGE"),
                        Ref::keyword("STREAM"),
                        Ref::keyword("TAG"),
                        Ref::keyword("TASK"),
                    ]),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESOURCE"),
                            Ref::keyword("MONITOR"),
                        ]),
                        Ref::keyword("SHARE"),
                    ]),
                    Ref::new("ObjectReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MANAGED"),
                            Ref::keyword("ACCOUNT"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MASKING"),
                            Ref::keyword("POLICY"),
                        ]),
                    ]),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("NETWORK"),
                        Ref::keyword("POLICY"),
                    ]),]),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("WAREHOUSE"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SESSION"),
                            Ref::keyword("POLICY"),
                        ]),
                    ]),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("SingleIdentifierGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SEQUENCE"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::new("DropBehaviorGrammar").optional(),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct ListStatementSegment;

impl NodeTrait for ListStatementSegment {
    const TYPE: &'static str = "list_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("LIST"), Ref::keyword("LS"),]),
            Ref::new("StagePath"),
            Sequence::new(vec_of_erased![
                Ref::keyword("PATTERN"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct GetStatementSegment;

impl NodeTrait for GetStatementSegment {
    const TYPE: &'static str = "get_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("GET"),
            Ref::new("StagePath"),
            one_of(vec_of_erased![Ref::new("UnquotedFilePath"), Ref::new("QuotedLiteralSegment"),]),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARALLEL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("IntegerSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("PATTERN"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("ReferencedVariableNameSegment"),
                    ]),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct PutStatementSegment;

impl NodeTrait for PutStatementSegment {
    const TYPE: &'static str = "put_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            Ref::keyword("PUT"),
            one_of(vec_of_erased![Ref::new("UnquotedFilePath"), Ref::new("QuotedLiteralSegment"),]),
            Ref::new("StagePath"),
            any_set_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("PARALLEL"),
                    Ref::new("EqualsSegment"),
                    Ref::new("IntegerSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AUTO_COMPRESS"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SOURCE_COMPRESSION"),
                    Ref::new("EqualsSegment"),
                    Ref::new("CompressionType"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OVERWRITE"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BooleanLiteralGrammar"),
                ]),
            ]),
        ])
        .to_matchable()
    }
}

pub struct RemoveStatementSegment;

impl NodeTrait for RemoveStatementSegment {
    const TYPE: &'static str = "remove_statement";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![Ref::keyword("REMOVE"), Ref::keyword("RM"),]),
            Ref::new("StagePath"),
            Sequence::new(vec_of_erased![
                Ref::keyword("PATTERN"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("ReferencedVariableNameSegment"),
                ]),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
    }
}

pub struct SetOperatorSegment;

impl NodeTrait for SetOperatorSegment {
    const TYPE: &'static str = "set_operator";

    fn match_grammar() -> Arc<dyn Matchable> {
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("UNION"),
                one_of(vec_of_erased![Ref::keyword("DISTINCT"), Ref::keyword("ALL"),])
                    .config(|this| this.optional()),
            ]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("INTERSECT"), Ref::keyword("EXCEPT"),]),
                Ref::keyword("ALL").optional(),
            ]),
            Ref::keyword("MINUS"),
        ])
        .to_matchable()
    }
}

pub struct ShorthandCastSegment;

impl NodeTrait for ShorthandCastSegment {
    const TYPE: &'static str = "cast_expression";

    fn match_grammar() -> Arc<dyn Matchable> {
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("Expression_D_Grammar"),
                Ref::new("CaseExpressionSegment"),
            ]),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("CastOperatorSegment"),
                Ref::new("DatatypeSegment"),
                one_of(vec_of_erased![
                    Ref::new("TimeZoneGrammar"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("ArrayAccessorSegment"),]),
                ])
                .config(|this| this.optional()),
            ]),])
            .config(|this| this.min_times(1)),
        ])
        .to_matchable()
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect_file;
    use itertools::Itertools;
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    use crate::core::config::{FluffConfig, Value};
    use crate::core::linter::linter::Linter;
    use crate::core::parser::segments::base::ErasedSegment;
    use crate::helpers;

    fn parse_sql(linter: &Linter, sql: &str) -> ErasedSegment {
        let parsed = linter.parse_string(sql.into(), None, None, None).unwrap();
        parsed.tree.unwrap()
    }

    #[test]
    fn base_parse_struct() {
        let linter = Linter::new(
            FluffConfig::new(
                [(
                    "core".into(),
                    Value::Map([("dialect".into(), Value::String("snowflake".into()))].into()),
                )]
                .into(),
                None,
                None,
            ),
            None,
            None,
        );

        let files =
            glob::glob("test/fixtures/dialects/snowflake/*.sql").unwrap().flatten().collect_vec();

        files.par_iter().for_each(|file| {
            let _panic = helpers::enter_panic(file.display().to_string());

            let yaml = file.with_extension("yml");
            let yaml = std::path::absolute(yaml).unwrap();

            let actual = {
                let sql = std::fs::read_to_string(file).unwrap();
                let tree = parse_sql(&linter, &sql);
                let tree = tree.to_serialised(true, true, false);

                serde_yaml::to_string(&tree).unwrap()
            };

            expect_file![yaml].assert_eq(&actual);
        });
    }
}
