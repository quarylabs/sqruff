use std::collections::HashSet;

use crate::databricks_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};
use crate::sparksql;
use sqruff_lib_core::parser::grammar::anyof::AnyNumberOf;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::Bracketed;
use sqruff_lib_core::parser::matchable::MatchableTrait;
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
    databricks.replace_grammar("AlterDatabaseStatementSegment", 
    Sequence::new(vec_of_erased![
        Ref::keyword("ALTER"),
        one_of(vec_of_erased![Ref::keyword("DATABASE"), Ref::keyword("SCHEMA")]),
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
    ]).to_matchable().into());



    databricks.replace_grammar(
        "StatementSegment",
        raw_sparksql
            .grammar("StatementSegment")
            .match_grammar()
            .unwrap()
            .copy(
                Some(vec_of_erased![Ref::new("AlterCatalogStatementSegment"),]),
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
