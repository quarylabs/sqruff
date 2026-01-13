use crate::databricks_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::sparksql;
use sqruff_lib_core::helpers::Config;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::Bracketed;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::{
    dialects::{Dialect, init::DialectKind},
    helpers::ToMatchable,
    parser::grammar::{Ref, sequence::Sequence},
};

pub fn dialect() -> Dialect {
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

    databricks.add([
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
                Ref::new("SetOwnerGrammar").to_matchable(),
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
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
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
            // Drop Volume Statement.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-drop-volume.html
            "DropVolumeStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("VOLUME").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("VolumeReferenceSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "VolumeReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        ),
        (
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-describe-volume.html
            "DescribeObjectGrammar".into(),
            sparksql::dialect()
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
        sparksql::dialect()
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

    // Override statement segment
    databricks.replace_grammar(
        "StatementSegment",
        raw_sparksql
            .grammar("StatementSegment")
            .match_grammar(&databricks)
            .unwrap()
            .copy(
                Some(vec![
                    Ref::new("AlterCatalogStatementSegment").to_matchable(),
                    Ref::new("CreateCatalogStatementSegment").to_matchable(),
                    Ref::new("DropCatalogStatementSegment").to_matchable(),
                    Ref::new("UseCatalogStatementSegment").to_matchable(),
                    Ref::new("DropVolumeStatementSegment").to_matchable(),
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
