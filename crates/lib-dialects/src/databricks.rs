use std::collections::HashSet;

use crate::databricks_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::sparksql;
use sqruff_lib_core::parser::grammar::anyof::AnyNumberOf;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::Bracketed;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::{
    dialects::{base::Dialect, init::DialectKind, syntax::SyntaxKind},
    helpers::{Config, ToMatchable},
    parser::{
        grammar::{anyof::one_of, base::Ref, sequence::Sequence},
        lexer::Matcher,
    },
    vec_of_erased,
};

pub fn dialect() -> Dialect {
    let raw_sparksql = sparksql::dialect();

    let mut databricks = sparksql::dialect();
    databricks.name = DialectKind::Databricks;

    // What want to translate from Sqlfluff
    // databricks_dialect.sets("unreserved_keywords").update(UNRESERVED_KEYWORDS)
    // databricks_dialect.sets("unreserved_keywords").update(
    //     sparksql_dialect.sets("reserved_keywords")
    // )
    // databricks_dialect.sets("unreserved_keywords").difference_update(RESERVED_KEYWORDS)
    // databricks_dialect.sets("reserved_keywords").clear()
    // databricks_dialect.sets("reserved_keywords").update(RESERVED_KEYWORDS)
    // databricks_dialect.sets("date_part_function_name").update(["TIMEDIFF"])

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
        .sets_mut("data_part_function_name")
        .extend(["TIMEDIFF"]);

    println!("reserved {:?}", databricks.sets("reserved_keywords"));
    println!("unreserved {:?}", databricks.sets("unreserved_keywords"));

    // databricks.sets_mut("reserverd_keywords").clear();
    // databricks.sets_mut("reserverd_keywords").extend(RESERVED_KEYWORDS);

    // databricks.sets_mut("data_part_function_name").extend(["TIMEDIFF"]);

    // Named Function Parameters:
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-function-invocation.html#named-parameter-invocation
    databricks.insert_lexer_matchers(
        vec![Matcher::string("right_array", "=>", SyntaxKind::RightArrow)],
        "equals",
    );

    // Notebook Cell Delimiter:
    // https://learn.microsoft.com/en-us/azure/databricks/notebooks/notebook-export-import#sql-1
    // // databricks.insert_lexer_matchers(
    //     vec![Match::regex(
    //         "command",
    //         r"(\r?\n){2}-- COMMAND ----------(\r?\n)",
    //         SyntaxKind::Code,
    //     )],
    //     "newline",
    // );

    // Datbricks Notebook Start:
    // Needed to insert "so early" to avoid magic + notebook
    // start to be interpreted as inline comment
    // databricks.insert_lexer_matchers(
    //     vec![
    //         Matcher::regex(
    //             "notebook_start",
    //             r"-- Databricks notebook source(\r?\n){1}",
    //             SyntaxKind::NotebookStart,
    //         ),
    //         Matcher::regex(
    //             "magic_line",
    //             r"(-- MAGIC)( [^%]{1})([^\n]*)",
    //             SyntaxKind::MagicLine,
    //         ),
    //         Matcher::regex(
    //             "magic_start",
    //             r"(-- MAGIC %)([^\n]{2,})(\r?\n)",
    //             SyntaxKind::MagicStart,
    //         ),
    //     ],
    //     "inline_comment",
    // );

    databricks.add([
        (
            "CatalogReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        ),
        (
            //     SetOwnerGrammar=Sequence(
            //     Ref.keyword("SET", optional=True),
            //     "OWNER",
            //     "TO",
            //     Ref("PrincipalIdentifierSegment"),
            // ),
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
        (
            "PredictiveOptimizationGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ENABLE"),
                    Ref::keyword("DISABLE"),
                    Ref::keyword("INHERIT"),
                ]),
                Ref::keyword("PREDICTIVE"),
                Ref::keyword("OPTIMIZATION"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // https://docs.databricks.com/en/sql/language-manual/sql-ref-principal.html
            "PrincipalIdentifierSegment".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("BackQuotedIdentifierSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AlterCatalogStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("CATALOG"),
                Ref::new("CatalogReferenceSegment"),
                one_of(vec_of_erased![
                    Ref::new("SetOwnerGrammar"),
                    Ref::new("SetTagsGrammar"),
                    Ref::new("UnsetTagsGrammar"),
                    Ref::new("PredictiveOptimizationGrammar"),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "SetTagsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::keyword("TAGS"),
                Ref::new("BracketedPropertyListGrammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "UnsetTagsGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("UNSET"),
                Ref::keyword("TAGS"),
                Ref::new("BracketedPropertyNameListGrammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ColumnDefaultGrammar".into(),
            one_of(vec_of_erased!(
                Ref::new("LiteralGrammar"),
                Ref::new("FucntionSegmenet"),
            ))
            .to_matchable()
            .into(),
        ),
        (
            "ConstraintOptionGrammar".into(),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("ENABLE"),
                    Ref::keyword("NOVALIDATE")
                ])
                .config(|config| { config.optional() }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("ENFORCED")
                ])
                .config(|config| { config.optional() }),
                Sequence::new(vec_of_erased![Ref::keyword("DEFERRABLE")])
                    .config(|config| { config.optional() }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("INITIALLY"),
                    Ref::keyword("DEFERRED")
                ])
                .config(|config| { config.optional() }),
                one_of(vec_of_erased![Ref::keyword("NORELY"), Ref::keyword("RELY"),])
                    .config(|config| { config.optional() }),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ForeignKeyOptionGrammar".into(),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("MATCH"), Ref::keyword("FULL"),])
                    .config(|config| { config.optional() }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("UPDATE"),
                    Ref::keyword("NO"),
                    Ref::keyword("ACTION"),
                ])
                .config(|config| { config.optional() }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("DELETE"),
                    Ref::keyword("NO"),
                    Ref::keyword("ACTION"),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "DropConstraintGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("PrimaryKeyGrammar"),
                    Ref::new("IfExistsGrammar").optional(),
                    one_of(vec_of_erased![
                        Ref::keyword("RESTRICT"),
                        Ref::keyword("CASCADE"),
                    ])
                    .config(|config| config.optional()),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("ForeignKeyGrammar"),
                    Ref::new("IfExistsGrammar").optional(),
                    Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("IfExistsGrammar").optional(),
                    Ref::new("ObjectReferenceSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("RESTRICT"),
                        Ref::keyword("CASCADE"),
                    ])
                    .config(|config| config.optional()),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AlterPartitionGrammar".into(),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("SetClauseSegment"),
                ]),])
                .config(|config| config.min_times(1))
            ])])
            .to_matchable()
            .into(),
        ),
        (
            "RowFilterClauseGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("ROW"),
                Ref::keyword("FILTER"),
                Ref::new("ObjectReferenceSegment"),
                Ref::keyword("ON"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("LiteralGrammar"),
                    ]
                )])
                .config(|config| config.optional())])
            ])
            .to_matchable()
            .into(),
        ),
        // TODO Sort out the following grammar
        // (
        //     "PropertiesBackTickedIdentifierSegment".into(),
        //     Matcher::regex(
        //         "properties_naked_identifier",
        //         r"`.+`",
        //         SyntaxKind::PropertiesNakedIdentifier,
        //     ).to_matchable().into(),
        // ),
        (
            "LocationWithCredentialGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("LOCATION"),
                Ref::new("QuotedLiteralSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![
                        Ref::keyword("CREDENTIAL"),
                        Ref::new("PrincipalIdentifierSegment")
                    ]),
                ])
                .config(|config| { config.optional() }),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ShowVolumesStatement".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SHOW"),
                Ref::keyword("VOLUMES"),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("IN"),]),
                    Ref::new("DatabaseReferenceSegment"),
                ])
                .config(|config| { config.optional() }),
                Sequence::new(vec_of_erased![
                    Ref::keyword("LIKE").optional(),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|config| { config.optional() }),
                //                 "VOLUMES",
                // Sequence(
                //     OneOf("FROM", "IN"),
                //     Ref("DatabaseReferenceSegment"),
                //     optional=True,
                // ),
                // Sequence(
                //     Ref.keyword("LIKE", optional=True),
                //     Ref("QuotedLiteralSegment"),
                //     optional=True,
                // ),
            ])
            .to_matchable()
            .into(),
        ),
        // // NotebookStart=TypedParser("notebook_start", CommentSegment, type="notebook_start"),
        // // MagicLineGrammar=TypedParser("magic_line", CodeSegment, type="magic_line"),
        // // MagicStartGrammar=TypedParser("magic_start", CodeSegment, type="magic_start"),
        (
            "VariableNameIdentifierSegment".into(),
            one_of(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                Ref::new("BackQuotedIdentifierSegment"),
            ])
            .to_matchable()
            .into(),
        ), // // VariableNameIdentifierSegment=OneOf(
           // //     Ref("NakedIdentifierSegment"),
           // //     Ref("BackQuotedIdentifierSegment"),
           // // ),
    ]);

    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-show-views.html
    // Only difference between this and the SparkSQL version:
    // - `LIKE` keyword is optional
    databricks.replace_grammar(
        "ShowViewsStatement".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("SHOW"),
            Ref::keyword("VIEWS"),
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
            .config(|config| { config.optional() })
        ])
        .to_matchable()
        .into(),
    );

    let mut show_statements = sparksql::show_statements();
    show_statements.push(Ref::new("ShowVolumesStatement").to_matchable().into());
    databricks.replace_grammar(
        "ShowStatement".into(),
        one_of(show_statements).to_matchable().into(),
    );

    // An `ALTER DATABASE/SCHEMA` statement.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-alter-schema.html
    databricks.replace_grammar(
        "AlterDatabaseStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            one_of(vec_of_erased![
                Ref::keyword("DATABASE"),
                Ref::keyword("SCHEMA")
            ]),
            Ref::new("DatabaseReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("DatabasePropertiesGrammar"),
                ]),
                Ref::new("SetOwnerGrammar"),
                Ref::new("SetTagsGrammar"),
                Ref::new("UnsetTagsGrammar"),
                Ref::new("PredictiveOptimizationGrammar"),
            ]),
        ])
        .to_matchable()
        .into(),
    );

    // An `ALTER TABLE` statement.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-alter-table.html
    //     match_grammar = Sequence(
    //     "ALTER",
    //     "TABLE",
    //     Ref("TableReferenceSegment"),
    //     Indent,
    //     OneOf(
    //         Sequence(
    //             "RENAME",
    //             "TO",
    //             Ref("TableReferenceSegment"),
    //         ),
    //         Sequence(
    //             "ADD",
    //             OneOf("COLUMNS", "COLUMN"),
    //             Indent,
    //             Bracketed(
    //                 Delimited(
    //                     Sequence(
    //                         Ref("ColumnFieldDefinitionSegment"),
    //                         Ref("ColumnDefaultGrammar", optional=True),
    //                         Ref("CommentGrammar", optional=True),
    //                         Ref("FirstOrAfterGrammar", optional=True),
    //                         Ref("MaskStatementSegment", optional=True),
    //                     ),
    //                 ),
    //             ),
    //             Dedent,
    //         ),
    //         Sequence(
    //             OneOf("ALTER", "CHANGE"),
    //             Ref.keyword("COLUMN", optional=True),
    //             Ref("ColumnReferenceSegment"),
    //             OneOf(
    //                 Ref("CommentGrammar"),
    //                 Ref("FirstOrAfterGrammar"),
    //                 Sequence(
    //                     OneOf("SET", "DROP"),
    //                     "NOT",
    //                     "NULL",
    //                 ),
    //                 Sequence(
    //                     "TYPE",
    //                     Ref("DatatypeSegment"),
    //                 ),
    //                 Sequence(
    //                     "SET",
    //                     Ref("ColumnDefaultGrammar"),
    //                 ),
    //                 Sequence(
    //                     "DROP",
    //                     "DEFAULT",
    //                 ),
    //                 Sequence(
    //                     "SYNC",
    //                     "IDENTITY",
    //                 ),
    //                 Sequence(
    //                     "SET",
    //                     Ref("MaskStatementSegment"),
    //                 ),
    //                 Sequence(
    //                     "DROP",
    //                     "MASK",
    //                 ),
    //                 Ref("SetTagsGrammar"),
    //                 Ref("UnsetTagsGrammar"),
    //             ),
    //         ),
    //         Sequence(
    //             "DROP",
    //             OneOf("COLUMN", "COLUMNS", optional=True),
    //             Ref("IfExistsGrammar", optional=True),
    //             OptionallyBracketed(
    //                 Delimited(
    //                     Ref("ColumnReferenceSegment"),
    //                 ),
    //             ),
    //         ),
    //         Sequence(
    //             "RENAME",
    //             "COLUMN",
    //             Ref("ColumnReferenceSegment"),
    //             "TO",
    //             Ref("ColumnReferenceSegment"),
    //         ),
    //         Sequence(
    //             "ADD",
    //             Ref("TableConstraintSegment"),
    //         ),
    //         Ref("DropConstraintGrammar"),
    //         Sequence(
    //             "DROP",
    //             "FEATURE",
    //             Ref("ObjectReferenceSegment"),
    //             Sequence(
    //                 "TRUNCATE",
    //                 "HISTORY",
    //                 optional=True,
    //             ),
    //         ),
    //         Sequence(
    //             "ADD",
    //             Ref("IfNotExistsGrammar", optional=True),
    //             AnyNumberOf(Ref("AlterPartitionGrammar")),
    //         ),
    //         Sequence(
    //             "DROP",
    //             Ref("IfExistsGrammar", optional=True),
    //             AnyNumberOf(Ref("AlterPartitionGrammar")),
    //         ),
    //         Sequence(
    //             Ref("AlterPartitionGrammar"),
    //             "SET",
    //             Ref("LocationGrammar"),
    //         ),
    //         Sequence(
    //             Ref("AlterPartitionGrammar"),
    //             "RENAME",
    //             "TO",
    //             Ref("AlterPartitionGrammar"),
    //         ),
    //         Sequence(
    //             "RECOVER",
    //             "PARTITIONS",
    //         ),
    //         Sequence(
    //             "SET",
    //             Ref("RowFilterClauseGrammar"),
    //         ),
    //         Sequence(
    //             "DROP",
    //             "ROW",
    //             "FILTER",
    //         ),
    //         Sequence(
    //             "SET",
    //             Ref("TablePropertiesGrammar"),
    //         ),
    //         Ref("UnsetTablePropertiesGrammar"),
    //         Sequence(
    //             "SET",
    //             "SERDE",
    //             Ref("QuotedLiteralSegment"),
    //             Sequence(
    //                 "WITH",
    //                 "SERDEPROPERTIES",
    //                 Ref("BracketedPropertyListGrammar"),
    //                 optional=True,
    //             ),
    //         ),
    //         Sequence(
    //             "SET",
    //             Ref("LocationGrammar"),
    //         ),
    //         Ref("SetOwnerGrammar"),
    //         Sequence(
    //             Sequence(
    //                 "ALTER",
    //                 "COLUMN",
    //                 Ref("ColumnReferenceSegment"),
    //                 optional=True,
    //             ),
    //             Ref("SetTagsGrammar"),
    //         ),
    //         Sequence(
    //             Sequence(
    //                 "ALTER",
    //                 "COLUMN",
    //                 Ref("ColumnReferenceSegment"),
    //                 optional=True,
    //             ),
    //             Ref("UnsetTagsGrammar"),
    //         ),
    //         Ref("ClusterByClauseSegment"),
    //         Ref("PredictiveOptimizationGrammar"),
    //     ),
    //     Dedent,
    // )
    databricks.replace_grammar(
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
                    Ref::new("TableReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    one_of(vec_of_erased![
                        Ref::keyword("COLUMNS"),
                        Ref::keyword("COLUMN")
                    ]),
                    MetaSegment::indent(),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnFieldDefinitionSegment"),
                            Ref::new("ColumnDefaultGrammar").optional(),
                            Ref::new("CommentGrammar").optional(),
                            Ref::new("FirstOrAfterGrammar").optional(),
                            Ref::new("MaskStatementSegment").optional(),
                        ]),
                    ]),]),
                    MetaSegment::dedent(),
                ]),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ALTER"),
                        Ref::keyword("CHANGE")
                    ]),
                    Ref::keyword("COLUMN").optional(),
                    Ref::new("ColumnReferenceSegment"),
                    one_of(vec_of_erased![
                        Ref::new("CommentGrammar"),
                        Ref::new("FirstOrAfterGrammar"),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![Ref::keyword("SET"), Ref::keyword("DROP")]),
                            Ref::keyword("NOT"),
                            Ref::keyword("NULL"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TYPE"),
                            Ref::new("DatatypeSegment"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::new("ColumnDefaultGrammar"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DROP"),
                            Ref::keyword("DEFAULT"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SYNC"),
                            Ref::keyword("IDENTITY"),
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Ref::new("MaskStatementSegment"),
                        ]),
                        Sequence::new(vec_of_erased![Ref::keyword("DROP"), Ref::keyword("MASK"),]),
                        Ref::new("SetTagsGrammar"),
                        Ref::new("UnsetTagsGrammar"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    one_of(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::keyword("COLUMNS")
                    ])
                    .config(|config| { config.optional() }),
                    Ref::new("IfExistsGrammar").optional(),
                    one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")]),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )]),]),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("COLUMN"),
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::new("ColumnReferenceSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("TableConstraintSegment"),
                ]),
                Ref::new("DropConstraintGrammar"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("FEATURE"),
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TRUNCATE"),
                        Ref::keyword("HISTORY"),
                    ])
                    .config(|config| { config.optional() }),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ADD"),
                    Ref::new("IfNotExistsGrammar").optional(),
                    AnyNumberOf::new(vec_of_erased![Ref::new("AlterPartitionGrammar"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::new("IfExistsGrammar").optional(),
                    AnyNumberOf::new(vec_of_erased![Ref::new("AlterPartitionGrammar"),]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("AlterPartitionGrammar"),
                    Ref::keyword("SET"),
                    Ref::new("LocationGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::new("AlterPartitionGrammar"),
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("AlterPartitionGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("RECOVER"),
                    Ref::keyword("PARTITIONS"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("RowFilterClauseGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("ROW"),
                    Ref::keyword("FILTER"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("TablePropertiesGrammar"),
                ]),
                Ref::new("UnsetTablePropertiesGrammar"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("SERDE"),
                    Ref::new("QuotedLiteralSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("SERDEPROPERTIES"),
                        Ref::new("BracketedPropertyListGrammar"),
                    ])
                    .config(|config| { config.optional() }),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::new("LocationGrammar"),
                ]),
                Ref::new("SetOwnerGrammar"),
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALTER"),
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                    ])
                    .config(|config| { config.optional() }),
                    Ref::new("SetTagsGrammar"),
                ]),
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALTER"),
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                    ])
                    .config(|config| { config.optional() }),
                    Ref::new("UnsetTagsGrammar"),
                ]),
                Ref::new("ClusterByClauseSegment"),
                Ref::new("PredictiveOptimizationGrammar"),
            ]),
            MetaSegment::dedent(),
        ])
        .to_matchable()
        .into(),
    );

    // `COMMENT ON` statement.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-comment.html
    databricks.add([(
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
    )]);

    databricks.add([(
        "VolumeReferenceSegment".into(),
        Ref::new("ObjectReferenceSegment").to_matchable().into(),
    )]);

    // An `ALTER VOLUME` statement.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-alter-volume.html
    databricks.add([(
        "AlterVolumeStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            Ref::keyword("VOLUME"),
            Ref::new("VolumeReferenceSegment"),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    Ref::keyword("TO"),
                    Ref::new("VolumeReferenceSegment"),
                ]),
                Ref::new("SetOwnerGrammar"),
                Ref::new("SetTagsGrammar"),
                Ref::new("UnsetTagsGrammar"),
            ]),
        ])
        .to_matchable()
        .into(),
    )]);

    // A `CREATE CATALOG` statement.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-create-catalog.html
    databricks.add([(
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
    )]);

    // A `DROP CATALOG` statement.
    // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-drop-catalog.html
    databricks.add([(
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
    )]);

    // A `SET TIME ZONE` statement.
    // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-aux-conf-mgmt-set-timezone.html
    databricks.add([(
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
    )]);

    // A `SET VARIABLE` statement used to set session variables.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-aux-set-variable.html
    // set var v1=val, v2=val2;
    //     # set var v1=val, v2=val2;
    let kv_pair = Sequence::new(vec_of_erased![Delimited::new(vec_of_erased![
        Ref::new("VariableNameIdentifierSegment"),
        Ref::new("EqualsSegment"),
        one_of(vec_of_erased![
            Ref::keyword("DEFAULT"),
            one_of(vec_of_erased![
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                Ref::new("ExpressionSegment"),
            ]),
        ]),
    ])]);
    // set var (v1,v2) = (values(100,200))
    let bracketed_kv_pair = Sequence::new(vec_of_erased![
        Bracketed::new(vec_of_erased![Ref::new("VariableNameIdentifierSegment")]),
        Ref::new("EqualsSegment"),
        Bracketed::new(vec_of_erased![one_of(vec_of_erased![
            Ref::new("SelectStatementSegment"),
            Ref::new("ValuesClauseSegment"),
        ]),]),
    ]);
    databricks.add([(
        "SetVariableStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            one_of(vec_of_erased![
                Ref::keyword("VAR"),
                Ref::keyword("VARIABLE"),
            ]),
            one_of(vec_of_erased![kv_pair.clone(), bracketed_kv_pair.clone(),])
                .config(|config| config.allow_gaps = true),
        ])
        .to_matchable()
        .into(),
    )]);

    databricks.add([
        (
            "DatabaseReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        )
    ]);
    // A `USE DATABASE` statement.
    // https://docs.databricks.com/sql/language-manual/sql-ref-syntax-ddl-usedb.html
    databricks.replace_grammar(
        "UseDatabaseStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("USE"),
            one_of(vec_of_erased![
                Ref::keyword("DATABASE"),
                Ref::keyword("SCHEMA")
            ])
            .config(|config| {
                config.optional();
            },),
            Ref::new("DatabaseReferenceSegment"),
        ])
        .to_matchable()
        .into(),
    );

    // The parameters for a function ie. `(column type COMMENT 'comment')`.
    databricks.add([(
        "FunctionParameterListGrammarWithComments".into(),
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::new("FunctionParameterGrammar"),
                AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        Ref::new("LiteralGrammar"),
                    ])
                    .config(|config| config.optional()),
                    Ref::new("CommentClauseSegment").optional(),
                ]),
            ]),
        ])])
        .to_matchable()
        .into(),
    )]);

    // A `CREATE FUNCTION` statement.
    // https://docs.databricks.com/en/sql/language-manual/sql-ref-syntax-ddl-create-sql-function.html
    databricks.add([(
        "CreateDatabricksFunctionStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::new("OrReplaceGrammar").optional(),
            Ref::new("TemporaryGrammar").optional(),
            Ref::keyword("FUNCTION"),
            Ref::new("IfNotExistsGrammar").optional(),
            Ref::new("FunctionNameSegment"),
            Ref::new("FunctionParameterListGrammarWithComments"),
            Sequence::new(vec_of_erased![
                Ref::keyword("RETURNS"),
                one_of(vec_of_erased![
                    Ref::new("DatatypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TABLE"),
                        Sequence::new(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("ColumnReferenceSegment"),
                                    Ref::new("DatatypeSegment"),
                                    Ref::new("CommentGrammar").optional(),
                                ]),
                            ]),]),
                        ])
                        .config(|config| { config.optional() }),
                    ]),
                ])
                .config(|config| { config.optional() }),
            ])
            .config(|config| { config.optional() }),
            Ref::new("FunctionDefinitionGrammar"),

        ]).to_matchable().into(),
    )]);

    databricks.replace_grammar(
        "StatementSegment",
        raw_sparksql
            .grammar("StatementSegment")
            .match_grammar()
            .unwrap()
            .copy(
                Some(vec_of_erased![
                    Ref::new("AlterCatalogStatementSegment"),
                    Ref::new("DropCatalogStatementSegment"),
                    Ref::new("AlterVolumeStatementSegment"),
                    Ref::new("CommentOnStatementSegment"),
                    Ref::new("CreateCatalogStatementSegment"),
                    Ref::new("SetVariableStatementSegment"),
                    Ref::new("SetTimeZoneStatementSegment"),
                    Ref::new("CreateDatabricksFunctionStatementSegment"),
                    Ref::new("FunctionParameterListGrammarWithComments"),
                    ]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
    );

    databricks.expand();

    return databricks;
}
