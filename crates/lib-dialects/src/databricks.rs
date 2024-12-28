use crate::databricks_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::sparksql;
use sqruff_lib_core::helpers::Config;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::Bracketed;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::{
    dialects::{base::Dialect, init::DialectKind},
    helpers::ToMatchable,
    parser::grammar::{base::Ref, sequence::Sequence},
    vec_of_erased,
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
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("BackQuotedIdentifierSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SetOwnerGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET").optional(),
                Ref::keyword("OWNER"),
                Ref::keyword("TO"),
                Ref::new("PrincipalIdentifierSegment"),
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
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("CATALOG"),
                one_of(vec_of_erased![Ref::new("SetOwnerGrammar")]),
            ])
            .to_matchable()
            .into(),
        ),
        // A `CREATE CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-create-catalog.html
        (
            "CreateCatalogStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("CATALOG"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("CatalogReferenceSegment"),
                Ref::new("CommentGrammar").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `DROP CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-drop-catalog.html
        (
            "DropCatalogStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("CATALOG"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("CatalogReferenceSegment"),
                Ref::new("DropBehaviorGrammar").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        // A `USE CATALOG` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-use-catalog.html
        (
            "UseCatalogStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USE"),
                Ref::keyword("CATALOG"),
                Ref::new("CatalogReferenceSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        // A `USE DATABASE` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-usedb.html
        (
            "UseDatabaseStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("USE"),
                one_of(vec_of_erased![
                    Ref::keyword("DATABASE"),
                    Ref::keyword("SCHEMA"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Ref::new("DatabaseReferenceSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        // A `SET TIME ZONE` statement.
        // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-aux-conf-mgmt-set-timezone.html
        (
            "SetTimeZoneStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::keyword("TIME"),
                Ref::keyword("ZONE"),
                one_of(vec_of_erased![
                    Ref::keyword("LOCAL"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("IntervalExpressionSegment")
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        // An `OPTIMIZE` statement.
        // https://docs.databricks.com/en/sql/language-manual/delta-optimize.html
        (
            "OptimizeTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("OPTIMIZE"),
                Ref::new("TableReferenceSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHERE"),
                    Ref::new("ExpressionSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ZORDER"),
                    Ref::keyword("BY"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])]),
                ])
                .config(|config| {
                    config.optional();
                }),
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
            Sequence::new(vec_of_erased![
                Ref::keyword("IDENTIFIER"),
                Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // Drop Volume Statement.
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-drop-volume.html
            "DropVolumeStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("VOLUME"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("VolumeReferenceSegment"),
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
                    Some(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("VOLUME"),
                        Ref::new("VolumeReferenceSegment"),
                    ])]),
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
            Sequence::new(vec_of_erased![
                Ref::keyword("DECLARE"),
                Ref::new("OrReplaceGrammar").optional(),
                Ref::keyword("VARIABLE").optional(),
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("DatatypeSegment").optional(),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        Ref::new("EqualsSegment")
                    ]),
                    Ref::new("ExpressionSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
            ])
            .to_matchable()
            .into(),
        ),
        // `COMMENT ON` statement.
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-comment.html
        (
            "CommentOnStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("COMMENT"),
                Ref::keyword("ON"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CATALOG"),
                        Ref::new("CatalogReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("DATABASE"),
                            Ref::keyword("SCHEMA")
                        ]),
                        Ref::new("DatabaseReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TABLE"),
                        Ref::new("TableReferenceSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("VOLUME"),
                        Ref::new("VolumeReferenceSegment"),
                    ]),
                    // TODO Split out individual items if they have references
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("CONNECTION"),
                            Ref::keyword("PROVIDER"),
                            Ref::keyword("RECIPIENT"),
                            Ref::keyword("SHARE"),
                        ]),
                        Ref::new("ObjectReferenceSegment"),
                    ]),
                ]),
                Ref::keyword("IS"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::keyword("NULL"),
                ]),
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
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATABASES"),
                    Ref::keyword("SCHEMAS"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN"),]),
                    Ref::new("DatabaseReferenceSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE").optional(),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
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
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("DATABASES"),
                    Ref::keyword("SCHEMAS"),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN"),]),
                    Ref::new("DatabaseReferenceSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE").optional(),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
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
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("USER"),
                    Ref::keyword("SYSTEM"),
                    Ref::keyword("ALL"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Ref::keyword("FUNCTIONS"),
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                        Ref::new("DatabaseReferenceSegment"),
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LIKE").optional(),
                        one_of(vec_of_erased![
                            // qualified function from a database
                            Sequence::new(vec_of_erased![
                                Ref::new("DatabaseReferenceSegment"),
                                Ref::new("DotSegment"),
                                Ref::new("FunctionNameSegment"),
                            ])
                            .config(|config| {
                                config.disallow_gaps();
                            }),
                            // non-qualified function
                            Ref::new("FunctionNameSegment"),
                            // Regex/like string
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ])
                    .config(|config| {
                        config.optional();
                    }),
                ])
                .config(|config| {
                    config.optional();
                }),
            ])
            .to_matchable()
            .into(),
        ),
        //     # https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-tables.html
        //     # Differences between this and the SparkSQL version:
        //     # - `LIKE` keyword is optional
        (
            "ShowTablesGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TABLES"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                    Ref::new("DatabaseReferenceSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE").optional(),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
            ])
            .to_matchable()
            .into(),
        ),
        // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-views.html
        // Only difference between this and the SparkSQL version:
        // - `LIKE` keyword is optional
        (
            "ShowViewsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("VIEWS"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                    Ref::new("DatabaseReferenceSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE").optional(),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|config| {
                    config.optional();
                }),
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
                    Some(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("VOLUMES"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN")]),
                            Ref::new("DatabaseReferenceSegment"),
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("LIKE").optional(),
                            Ref::new("QuotedLiteralSegment"),
                        ])
                        .config(|config| {
                            config.optional();
                        }),
                    ])]),
                    None,
                    None,
                    None,
                    Vec::new(),
                    false,
                )
                .into(),
        ),
    ]);

    // A reference to an object.
    databricks.replace_grammar(
        "ObjectReferenceSegment",
        Delimited::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                Ref::new("IdentifierClauseSegment"),
            ]),
            Ref::new("ObjectReferenceDelimiterGrammar"),
        ])
        .config(|config| {
            config.delimiter(Ref::new("ObjectReferenceDelimiterGrammar"));
            config.terminators = vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
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
            .match_grammar()
            .unwrap()
            .copy(
                Some(vec_of_erased![Ref::new("IdentifierClauseSegment")]),
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
            .match_grammar()
            .unwrap()
            .copy(
                Some(vec_of_erased![
                    Ref::new("AlterCatalogStatementSegment"),
                    Ref::new("CreateCatalogStatementSegment"),
                    Ref::new("DropCatalogStatementSegment"),
                    Ref::new("UseCatalogStatementSegment"),
                    Ref::new("DropVolumeStatementSegment"),
                    Ref::new("SetTimeZoneStatementSegment"),
                    Ref::new("OptimizeTableStatementSegment"),
                    Ref::new("CommentOnStatementSegment"),
                    Ref::new("DeclareOrReplaceVariableStatementSegment"),
                ]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
    );

    databricks.expand();
    databricks
}
