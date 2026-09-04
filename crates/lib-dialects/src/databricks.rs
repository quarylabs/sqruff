use crate::databricks_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::sparksql;
use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::Config;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::Bracketed;
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::{
    dialects::{Dialect, init::DialectKind},
    helpers::ToMatchable,
    parser::grammar::{Anything, Ref, sequence::Sequence},
    value::Value,
};

sqruff_lib_core::dialect_config!(DatabricksDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    // Parse and validate dialect configuration, falling back to defaults on failure
    let _dialect_config: DatabricksDialectConfig = config
        .map(DatabricksDialectConfig::from_value)
        .unwrap_or_default();
    let raw_sparksql = sparksql::raw_dialect();

    let mut databricks = sparksql::raw_dialect();
    databricks.name = DialectKind::Databricks;

    databricks
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);
    databricks
        .sets_mut("unreserved_keywords")
        .extend(raw_sparksql.sets("reserved_keywords"));
    databricks
        .sets_mut("unreserved_keywords")
        .retain(|x| !RESERVED_KEYWORDS.contains(x));
    databricks.sets_mut("reserved_keywords").clear();
    databricks
        .sets_mut("reserved_keywords")
        .extend(RESERVED_KEYWORDS);

    databricks
        .sets_mut("date_part_function_name")
        .extend(["TIMEDIFF"]);

    databricks.insert_lexer_matchers(
        vec![Matcher::string("right_arrow", "=>", SyntaxKind::RightArrow)],
        "equals",
    );

    databricks.add([
        (
            "DoubleQuotedUDFBody".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::UdfBody)
                .to_matchable()
                .into(),
        ),
        (
            "SingleQuotedUDFBody".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::UdfBody)
                .to_matchable()
                .into(),
        ),
        (
            "DollarQuotedUDFBody".into(),
            TypedParser::new(SyntaxKind::DollarQuote, SyntaxKind::UdfBody)
                .to_matchable()
                .into(),
        ),
        (
            "RightArrowSegment".into(),
            StringParser::new("=>", SyntaxKind::RightArrow)
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
            "FunctionParameterListGrammarWithComments".into(),
            NodeMatcher::new(SyntaxKind::FunctionParameterListWithComments, |_| {
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("FunctionParameterGrammar").to_matchable(),
                            AnyNumberOf::new(vec![
                                Sequence::new(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Ref::new("LiteralGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("CommentClauseSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatabricksFunctionDefinitionGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionDefinition, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("LANGUAGE").to_matchable(),
                            one_of(vec![
                                Ref::keyword("SQL").to_matchable(),
                                Ref::keyword("PYTHON").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("DETERMINISTIC").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NOT").to_matchable(),
                                Ref::keyword("DETERMINISTIC").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("CommentClauseSegment").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("CONTAINS").to_matchable(),
                                Ref::keyword("SQL").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("READS").to_matchable(),
                                Ref::keyword("SQL").to_matchable(),
                                Ref::keyword("DATA").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("AS").to_matchable(),
                            one_of(vec![
                                Ref::new("DoubleQuotedUDFBody").to_matchable(),
                                Ref::new("SingleQuotedUDFBody").to_matchable(),
                                Ref::new("DollarQuotedUDFBody").to_matchable(),
                                Bracketed::new(vec![
                                    one_of(vec![
                                        Ref::new("SelectStatementSegment").to_matchable(),
                                        Ref::new("ExpressionSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("RETURN").to_matchable(),
                            one_of(vec![
                                Ref::new("WithCompoundStatementSegment").to_matchable(),
                                Ref::new("SelectStatementSegment").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
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
            "CreateDatabricksFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::new("TemporaryGrammar").optional().to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    Ref::new("FunctionParameterListGrammarWithComments").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RETURNS").to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("DatabricksFunctionDefinitionGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PrincipalIdentifierSegment".into(),
            one_of(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("BackQuotedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SetOwnerGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("SET").optional().to_matchable(),
                Ref::keyword("OWNER").to_matchable(),
                Ref::keyword("TO").to_matchable(),
                Ref::new("PrincipalIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PredictiveOptimizationGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("ENABLE").to_matchable(),
                    Ref::keyword("DISABLE").to_matchable(),
                    Ref::keyword("INHERIT").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("PREDICTIVE").to_matchable(),
                Ref::keyword("OPTIMIZATION").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SetTagsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Ref::keyword("TAGS").to_matchable(),
                Ref::new("BracketedPropertyListGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "UnsetTagsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("UNSET").to_matchable(),
                Ref::keyword("TAGS").to_matchable(),
                Ref::new("BracketedPropertyNameListGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ColumnDefaultGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("DEFAULT").to_matchable(),
                Ref::new("LiteralGrammar").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ConstraintOptionGrammar".into(),
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::keyword("ENABLE").to_matchable(),
                    Ref::keyword("NOVALIDATE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("ENFORCED").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Ref::keyword("DEFERRABLE").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("INITIALLY").to_matchable(),
                    Ref::keyword("DEFERRED").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("NORELY").to_matchable(),
                    Ref::keyword("RELY").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ForeignKeyOptionGrammar".into(),
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::keyword("MATCH").to_matchable(),
                    Ref::keyword("FULL").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("UPDATE").to_matchable(),
                    Ref::keyword("NO").to_matchable(),
                    Ref::keyword("ACTION").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("DELETE").to_matchable(),
                    Ref::keyword("NO").to_matchable(),
                    Ref::keyword("ACTION").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DropConstraintGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("PrimaryKeyGrammar").to_matchable(),
                        Ref::new("IfExistsGrammar").optional().to_matchable(),
                        one_of(vec![
                            Ref::keyword("RESTRICT").to_matchable(),
                            Ref::keyword("CASCADE").to_matchable(),
                        ])
                        .config(|config| config.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("ForeignKeyGrammar").to_matchable(),
                        Ref::new("IfExistsGrammar").optional().to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("IfExistsGrammar").optional().to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("RESTRICT").to_matchable(),
                            Ref::keyword("CASCADE").to_matchable(),
                        ])
                        .config(|config| config.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AlterPartitionGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("PARTITION").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        AnyNumberOf::new(vec![
                            one_of(vec![
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                                Ref::new("SetClauseSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| config.min_times = 1)
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "RowFilterClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("ROW").to_matchable(),
                Ref::keyword("FILTER").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "PropertiesBackTickedIdentifierSegment".into(),
            RegexParser::new("`.+`", SyntaxKind::PropertiesNakedIdentifier)
                .to_matchable()
                .into(),
        ),
        (
            "MaskStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::MaskStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("MASK").to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Ref::keyword("COLUMNS").to_matchable(),
                        Bracketed::new(vec![
                            AnyNumberOf::new(vec![
                                one_of(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // A reference to a catalog.
        // https://docs.databricks.com/data-governance/unity-catalog/create-catalogs.html
        (
            "CatalogReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        ),
        // An `ALTER CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-alter-catalog.html
        (
            "AlterCatalogStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("CATALOG").to_matchable(),
                Ref::new("CatalogReferenceSegment").to_matchable(),
                one_of(vec![
                    Ref::new("SetOwnerGrammar").to_matchable(),
                    Ref::new("SetTagsGrammar").to_matchable(),
                    Ref::new("UnsetTagsGrammar").to_matchable(),
                    Ref::new("PredictiveOptimizationGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `CREATE CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-create-catalog.html
        (
            "CreateCatalogStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("CATALOG").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("CatalogReferenceSegment").to_matchable(),
                Ref::new("CommentGrammar").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `DROP CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-drop-catalog.html
        (
            "DropCatalogStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("CATALOG").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("CatalogReferenceSegment").to_matchable(),
                Ref::new("DropBehaviorGrammar").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `USE CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-use-catalog.html
        (
            "UseCatalogStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("USE").to_matchable(),
                Ref::keyword("CATALOG").to_matchable(),
                Ref::new("CatalogReferenceSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `USE DATABASE` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-usedb.html
        (
            "UseDatabaseStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("USE").to_matchable(),
                one_of(vec![
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Ref::new("DatabaseReferenceSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `SET TIME ZONE` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-aux-conf-mgmt-set-timezone.html
        (
            "SetTimeZoneStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                Ref::keyword("TIME").to_matchable(),
                Ref::keyword("ZONE").to_matchable(),
                one_of(vec![
                    Ref::keyword("LOCAL").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("IntervalExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // An `OPTIMIZE` statement.
        // https://docs.databricks.com/en/sql/language-manual/delta-optimize.html
        (
            "OptimizeTableStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("OPTIMIZE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WHERE").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ZORDER").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A reference to a database.
            "DatabaseReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        ),
        (
            // A reference to an table, CTE, subquery or alias.
            "TableReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |_| {
                Ref::new("ObjectReferenceSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // A reference to a schema.
            "SchemaReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        ),
        (
            "IdentifierClauseSegment".into(),
            Sequence::new(vec![
                Ref::keyword("IDENTIFIER").to_matchable(),
                Bracketed::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // Alter Volume Statement.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-alter-volume.html
            "AlterVolumeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterVolumeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("VOLUME").to_matchable(),
                    Ref::new("VolumeReferenceSegment").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("RENAME").to_matchable(),
                            Ref::keyword("TO").to_matchable(),
                            Ref::new("VolumeReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SetOwnerGrammar").to_matchable(),
                        Ref::new("SetTagsGrammar").to_matchable(),
                        Ref::new("UnsetTagsGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // Create Volume Statement.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-create-volume.html
            "CreateVolumeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateVolumeStatement, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("VOLUME").to_matchable(),
                        Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                        Ref::new("VolumeReferenceSegment").to_matchable(),
                        Ref::new("CommentGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("EXTERNAL").to_matchable(),
                        Ref::keyword("VOLUME").to_matchable(),
                        Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                        Ref::new("VolumeReferenceSegment").to_matchable(),
                        Ref::new("LocationGrammar").to_matchable(),
                        Ref::new("CommentGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // Drop Volume Statement.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-drop-volume.html
            "DropVolumeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropVolumeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("VOLUME").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Ref::new("VolumeReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "VolumeReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::VolumeReference, |_| {
                Ref::new("ObjectReferenceSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-describe-volume.html
            "DescribeObjectGrammar".into(),
            sparksql::dialect(None)
                .grammar("DescribeObjectGrammar")
                .copy(
                    Some(vec![
                        Sequence::new(vec![
                            Ref::keyword("VOLUME").to_matchable(),
                            Ref::new("VolumeReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ]),
                    Some(0),
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
        (
            // A `DECLARE [OR REPLACE] VARIABLE` statement.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-declare-variable.html
            "DeclareOrReplaceVariableStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("DECLARE").to_matchable(),
                Ref::new("OrReplaceGrammar").optional().to_matchable(),
                Ref::keyword("VARIABLE").optional().to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("DatatypeSegment").optional().to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // `COMMENT ON` statement.
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-comment.html
        (
            "CommentOnStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("COMMENT").to_matchable(),
                Ref::keyword("ON").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CATALOG").to_matchable(),
                        Ref::new("CatalogReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("DATABASE").to_matchable(),
                            Ref::keyword("SCHEMA").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DatabaseReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TABLE").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("VOLUME").to_matchable(),
                        Ref::new("VolumeReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // TODO Split out individual items if they have references
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("CONNECTION").to_matchable(),
                            Ref::keyword("PROVIDER").to_matchable(),
                            Ref::keyword("RECIPIENT").to_matchable(),
                            Ref::keyword("SHARE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("IS").to_matchable(),
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-schemas.html
        // Differences between this and the SparkSQL version:
        // - Support for `FROM`|`IN` at the catalog level
        // - `LIKE` keyword is optional
        (
            "ShowDatabasesSchemasGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("DATABASES").to_matchable(),
                    Ref::keyword("SCHEMAS").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FROM").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LIKE").optional().to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // Show Functions Statement
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-functions.html
        //
        // Represents the grammar part after the show
        //
        // Differences between this and the SparkSQL version:
        // - Support for `FROM`|`IN` at the schema level
        // - `LIKE` keyword is optional
        (
            "ShowFunctionsGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("USER").to_matchable(),
                    Ref::keyword("SYSTEM").to_matchable(),
                    Ref::keyword("ALL").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Ref::keyword("FUNCTIONS").to_matchable(),
                Sequence::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("FROM").to_matchable(),
                            Ref::keyword("IN").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DatabaseReferenceSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("LIKE").optional().to_matchable(),
                        one_of(vec![
                            // qualified function from a database
                            Sequence::new(vec![
                                Ref::new("DatabaseReferenceSegment").to_matchable(),
                                Ref::new("DotSegment").to_matchable(),
                                Ref::new("FunctionNameSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.disallow_gaps();
                            })
                            .to_matchable(),
                            // non-qualified function
                            Ref::new("FunctionNameSegment").to_matchable(),
                            // Regex/like string
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
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
            ])
            .to_matchable()
            .into(),
        ),
        //     # https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-tables.html
        //     # Differences between this and the SparkSQL version:
        //     # - `LIKE` keyword is optional
        (
            "ShowTablesGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("TABLES").to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FROM").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LIKE").optional().to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-views.html
        // Only difference between this and the SparkSQL version:
        // - `LIKE` keyword is optional
        (
            "ShowViewsGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("VIEWS").to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FROM").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("LIKE").optional().to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-volumes.html
        (
            "ShowObjectGrammar".into(),
            sparksql::raw_dialect()
                .grammar("ShowObjectGrammar")
                .copy(
                    Some(vec![
                        Sequence::new(vec![
                            Ref::keyword("VOLUMES").to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("FROM").to_matchable(),
                                    Ref::keyword("IN").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("DatabaseReferenceSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("LIKE").optional().to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
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
        // https://docs.databricks.com/aws/en/sql/language-manual/sql-ref-syntax-dml-insert-into#insert-using-the-by-name-clause
        (
            "InsertBracketedColumnReferenceListGrammar".into(),
            one_of(vec![
                Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("BY").to_matchable(),
                    Ref::keyword("NAME").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-create-table-using.html
    databricks.replace_grammar(
        "GeneratedColumnDefinitionSegment",
        Sequence::new(vec![
            Ref::new("SingleIdentifierGrammar").to_matchable(),
            Ref::new("DatatypeSegment").to_matchable(),
            Bracketed::new(vec![Anything::new().to_matchable()])
                .config(|config| config.optional())
                .to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("GENERATED").to_matchable(),
                    Ref::keyword("ALWAYS").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::new("FunctionSegment").to_matchable(),
                            Ref::new("BareFunctionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("GENERATED").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ALWAYS").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BY").to_matchable(),
                            Ref::keyword("DEFAULT").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::keyword("IDENTITY").to_matchable(),
                    Bracketed::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("START").to_matchable(),
                            Ref::keyword("WITH").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .config(|config| config.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INCREMENT").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .config(|config| config.optional())
                        .to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            AnyNumberOf::new(vec![
                Ref::new("ColumnConstraintSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.replace_grammar(
        "FunctionContentsExpressionGrammar",
        one_of(vec![
            Ref::new("ExpressionSegment").to_matchable(),
            Ref::new("NamedArgumentSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.replace_grammar(
        "PropertiesNakedIdentifierSegment",
        RegexParser::new("[A-Z_][A-Z0-9_]*", SyntaxKind::PropertiesNakedIdentifier).to_matchable(),
    );

    databricks.replace_grammar(
        "PropertyNameSegment",
        Sequence::new(vec![
            one_of(vec![
                Delimited::new(vec![
                    one_of(vec![
                        Ref::new("PropertiesNakedIdentifierSegment").to_matchable(),
                        Ref::new("PropertiesBackTickedIdentifierSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.delimiter(Ref::new("DotSegment"));
                    config.disallow_gaps();
                })
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.replace_grammar(
        "TableConstraintSegment",
        Sequence::new(vec![
            Ref::keyword("CONSTRAINT").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("ObjectReferenceSegment").optional().to_matchable(),
                    Ref::new("PrimaryKeyGrammar").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::keyword("TIMESERIES").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ConstraintOptionGrammar")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("ObjectReferenceSegment").optional().to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("ForeignKeyGrammar").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("REFERENCES").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    one_of(vec![
                        Ref::new("ForeignKeyOptionGrammar").to_matchable(),
                        Ref::new("ConstraintOptionGrammar").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("CHECK").to_matchable(),
                    Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    Ref::keyword("ENFORCED").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.replace_grammar(
        "AlterDatabaseStatementSegment",
        Sequence::new(vec![
            Ref::keyword("ALTER").to_matchable(),
            one_of(vec![
                Ref::keyword("DATABASE").to_matchable(),
                Ref::keyword("SCHEMA").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("DatabaseReferenceSegment").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("DatabasePropertiesGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SetOwnerGrammar").to_matchable(),
                Ref::new("SetTagsGrammar").to_matchable(),
                Ref::new("UnsetTagsGrammar").to_matchable(),
                Ref::new("PredictiveOptimizationGrammar").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // Databricks CREATE SCHEMA permits MANAGED LOCATION in addition to the
    // ordinary LOCATION clause inherited from SparkSQL.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-create-schema.html
    databricks.replace_grammar(
        "CreateDatabaseStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            one_of(vec![
                Ref::keyword("DATABASE").to_matchable(),
                Ref::keyword("SCHEMA").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("DatabaseReferenceSegment").to_matchable(),
            Ref::new("CommentGrammar").optional().to_matchable(),
            Sequence::new(vec![
                Ref::keyword("MANAGED").optional().to_matchable(),
                Ref::keyword("LOCATION").to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .config(|config| config.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("DBPROPERTIES").to_matchable(),
                Ref::new("BracketedPropertyListGrammar").to_matchable(),
            ])
            .config(|config| config.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.replace_grammar(
        "AlterTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            MetaSegment::indent().to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    one_of(vec![
                        Ref::keyword("COLUMNS").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("ColumnFieldDefinitionSegment").to_matchable(),
                                Ref::new("ColumnDefaultGrammar").optional().to_matchable(),
                                Ref::new("CommentGrammar").optional().to_matchable(),
                                Ref::new("FirstOrAfterGrammar").optional().to_matchable(),
                                Ref::new("MaskStatementSegment").optional().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("ALTER").to_matchable(),
                        Ref::keyword("CHANGE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("COLUMN").optional().to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("CommentGrammar").to_matchable(),
                        Ref::new("FirstOrAfterGrammar").to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("SET").to_matchable(),
                                Ref::keyword("DROP").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("TYPE").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SET").to_matchable(),
                            Ref::new("ColumnDefaultGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DROP").to_matchable(),
                            Ref::keyword("DEFAULT").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SYNC").to_matchable(),
                            Ref::keyword("IDENTITY").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SET").to_matchable(),
                            Ref::new("MaskStatementSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DROP").to_matchable(),
                            Ref::keyword("MASK").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SetTagsGrammar").to_matchable(),
                        Ref::new("UnsetTagsGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    one_of(vec![
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::keyword("COLUMNS").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::new("TableConstraintSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DropConstraintGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("FEATURE").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TRUNCATE").to_matchable(),
                        Ref::keyword("HISTORY").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("AlterPartitionGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("AlterPartitionGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("AlterPartitionGrammar").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("LocationGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("AlterPartitionGrammar").to_matchable(),
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("AlterPartitionGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("RECOVER").to_matchable(),
                    Ref::keyword("PARTITIONS").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("RowFilterClauseGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("ROW").to_matchable(),
                    Ref::keyword("FILTER").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("TablePropertiesGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("UnsetTablePropertiesGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::keyword("SERDE").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("SERDEPROPERTIES").to_matchable(),
                        Ref::new("BracketedPropertyListGrammar").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("LocationGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SetOwnerGrammar").to_matchable(),
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("ALTER").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::new("SetTagsGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("ALTER").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::new("UnsetTagsGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("ClusterByClauseSegment").to_matchable(),
                Ref::new("PredictiveOptimizationGrammar").to_matchable(),
            ])
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.replace_grammar(
        "AlterViewStatementSegment",
        Sequence::new(vec![
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("MATERIALIZED").optional().to_matchable(),
            Ref::keyword("VIEW").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("TablePropertiesGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("UnsetTablePropertiesGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("SelectStatementSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("SCHEMA").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BINDING").to_matchable(),
                        Ref::keyword("COMPENSATION").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("TYPE").optional().to_matchable(),
                            Ref::keyword("EVOLUTION").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SetOwnerGrammar").to_matchable(),
                Ref::new("SetTagsGrammar").to_matchable(),
                Ref::new("UnsetTagsGrammar").to_matchable(),
                Sequence::new(vec![
                    MetaSegment::indent().to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("ADD").to_matchable(),
                                Ref::keyword("ALTER").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("SCHEDULE").to_matchable(),
                            Ref::keyword("REFRESH").optional().to_matchable(),
                            Ref::keyword("CRON").to_matchable(),
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("AT").to_matchable(),
                                Ref::keyword("TIME").to_matchable(),
                                Ref::keyword("ZONE").to_matchable(),
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                            ])
                            .config(|config| config.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("DROP").to_matchable(),
                            Ref::keyword("SCHEDULE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // A reference to an object.
    databricks.replace_grammar(
        "ObjectReferenceSegment",
        Delimited::new(vec![
            one_of(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("IdentifierClauseSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("ObjectReferenceDelimiterGrammar").to_matchable(),
        ])
        .config(|config| {
            config.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
            config.terminators = vec![Ref::new("ObjectReferenceTerminatorGrammar").to_matchable()];
            config.disallow_gaps();
        })
        .to_matchable(),
    );

    // The main table expression e.g. within a FROM clause.
    // Enhance to allow for additional clauses allowed in Spark and Delta Lake.
    databricks.replace_grammar(
        "TableExpressionSegment",
        sparksql::dialect(None)
            .grammar("TableExpressionSegment")
            .match_grammar(&databricks)
            .unwrap()
            .copy(
                Some(vec![Ref::new("IdentifierClauseSegment").to_matchable()]),
                None,
                Some(Ref::new("ValuesClauseSegment").to_matchable()),
                None,
                Vec::new(),
                false,
            ),
    );

    // Databricks allows PIVOT aggregates without aliases, so FOR must not be
    // consumed as an implicit alias before the pivot's FOR clause.
    databricks.replace_grammar(
        "AliasExpressionSegment",
        Sequence::new(vec![
            Ref::keyword("AS").optional().to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .to_matchable(),
                    Bracketed::new(vec![Ref::new("SingleIdentifierListSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .config(|config| {
                config.exclude = one_of(vec![
                    Ref::keyword("LATERAL").to_matchable(),
                    Ref::new("JoinTypeKeywords").to_matchable(),
                    Ref::keyword("WINDOW").to_matchable(),
                    Ref::keyword("PIVOT").to_matchable(),
                    Ref::keyword("KEYS").to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                ])
                .to_matchable()
                .into();
            })
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // Override statement segment
    databricks.replace_grammar(
        "StatementSegment",
        raw_sparksql
            .grammar("StatementSegment")
            .match_grammar(&databricks)
            .unwrap()
            .copy(
                Some(vec![
                    Ref::new("CreateDatabricksFunctionStatementSegment").to_matchable(),
                    Ref::new("AlterCatalogStatementSegment").to_matchable(),
                    Ref::new("CreateCatalogStatementSegment").to_matchable(),
                    Ref::new("DropCatalogStatementSegment").to_matchable(),
                    Ref::new("UseCatalogStatementSegment").to_matchable(),
                    Ref::new("AlterVolumeStatementSegment").to_matchable(),
                    Ref::new("CreateVolumeStatementSegment").to_matchable(),
                    Ref::new("DropVolumeStatementSegment").to_matchable(),
                    Ref::new("CreateDatabaseStatementSegment").to_matchable(),
                    Ref::new("SetTimeZoneStatementSegment").to_matchable(),
                    Ref::new("OptimizeTableStatementSegment").to_matchable(),
                    Ref::new("CommentOnStatementSegment").to_matchable(),
                    Ref::new("DeclareOrReplaceVariableStatementSegment").to_matchable(),
                ]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
    );

    // Enhance `GROUP BY` clause like in `SELECT` for `CUBE`, `ROLLUP`, and `ALL`.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-qry-select-groupby.html
    databricks.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("GROUP").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            one_of(vec![
                Ref::keyword("ALL").to_matchable(),
                Delimited::new(vec![
                    Ref::new("CubeRollupClauseSegment").to_matchable(),
                    Ref::new("GroupingSetsClauseSegment").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    // Can `GROUP BY 1`
                    Ref::new("NumericLiteralSegment").optional().to_matchable(),
                    // Can `GROUP BY coalesce(col, 1)`
                    Ref::new("ExpressionSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Delimited::new(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        // Can `GROUP BY 1`
                        Ref::new("NumericLiteralSegment").optional().to_matchable(),
                        // Can `GROUP BY coalesce(col, 1)`
                        Ref::new("ExpressionSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("WithCubeRollupClauseSegment").to_matchable(),
                        Ref::new("GroupingSetsClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    databricks.expand();
    databricks
}
