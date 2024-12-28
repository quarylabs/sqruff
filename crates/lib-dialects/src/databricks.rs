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
