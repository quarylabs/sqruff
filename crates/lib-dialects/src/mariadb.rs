//! The MariaDB SQL dialect.
//!
//! MariaDB is a community-developed fork of MySQL, so this dialect is based on
//! the MySQL dialect at the revision recorded in `.sqlfluff-sha`.
//!
//! https://mariadb.com/kb/en/sql-statements-structure/

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectConfig, DialectKind};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, any_set_of, one_of};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::value::Value;

use super::mysql;
use crate::mariadb_keywords::{MARIADB_RESERVED_KEYWORDS, MARIADB_UNRESERVED_KEYWORDS};

sqruff_lib_core::dialect_config!(MariaDBDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: MariaDBDialectConfig = config
        .map(MariaDBDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

fn mariadb_table_options_grammar() -> Matchable {
    optionally_delimited_table_options(vec![
        Sequence::new(vec![
            Ref::keyword("STORAGE").optional().to_matchable(),
            Ref::keyword("ENGINE").to_matchable(),
            Ref::new("EqualsSegment").optional().to_matchable(),
            quoted_or_identifier(),
        ])
        .to_matchable(),
        numeric_table_option("AUTO_INCREMENT"),
        numeric_table_option("AVG_ROW_LENGTH"),
        charset_table_option(),
        numeric_table_option("CHECKSUM"),
        collate_table_option(),
        quoted_table_option("COMMENT"),
        quoted_table_option("CONNECTION"),
        directory_table_option("DATA"),
        numeric_table_option("DELAY_KEY_WRITE"),
        keyword_table_option("ENCRYPTED", &["YES", "NO"]),
        numeric_table_option("ENCRYPTION_KEY_ID"),
        keyword_table_option("IETF_QUOTES", &["YES", "NO"]),
        directory_table_option("INDEX"),
        keyword_table_option("INSERT_METHOD", &["NO", "FIRST", "LAST"]),
        numeric_table_option("KEY_BLOCK_SIZE"),
        numeric_table_option("MAX_ROWS"),
        numeric_table_option("MIN_ROWS"),
        number_or_default_table_option("PACK_KEYS"),
        numeric_table_option("PAGE_CHECKSUM"),
        numeric_table_option("PAGE_COMPRESSED"),
        numeric_table_option("PAGE_COMPRESSION_LEVEL"),
        quoted_table_option("PASSWORD"),
        keyword_table_option(
            "ROW_FORMAT",
            &[
                "DEFAULT",
                "DYNAMIC",
                "FIXED",
                "COMPRESSED",
                "REDUNDANT",
                "COMPACT",
                "PAGE",
            ],
        ),
        numeric_table_option("SEQUENCE"),
        number_or_default_table_option("STATS_AUTO_RECALC"),
        number_or_default_table_option("STATS_PERSISTENT"),
        number_or_default_table_option("STATS_SAMPLE_PAGES"),
        Sequence::new(vec![
            Ref::keyword("TABLESPACE").to_matchable(),
            Ref::new("NakedIdentifierSegment").to_matchable(),
        ])
        .to_matchable(),
        numeric_table_option("TRANSACTIONAL"),
        union_table_option(),
        Sequence::new(vec![
            Ref::keyword("WITH").to_matchable(),
            Ref::keyword("SYSTEM").to_matchable(),
            Ref::keyword("VERSIONING").to_matchable(),
        ])
        .to_matchable(),
    ])
}

/// The MariaDB-only partitioning clause for system-versioned table history.
fn system_time_partition_grammar() -> Matchable {
    Sequence::new(vec![
        Ref::keyword("PARTITION").to_matchable(),
        Ref::keyword("BY").to_matchable(),
        Ref::keyword("SYSTEM_TIME").to_matchable(),
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("INTERVAL").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
                Ref::new("DatetimeUnitSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("STARTS").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("LIMIT").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
        Ref::keyword("AUTO").optional().to_matchable(),
        Sequence::new(vec![
            Ref::keyword("PARTITIONS").to_matchable(),
            Ref::new("NumericLiteralSegment").to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
        Sequence::new(vec![
            Ref::keyword("SUBPARTITION").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            Ref::keyword("LINEAR").optional().to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("HASH").to_matchable(),
                    Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("KEY").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SUBPARTITIONS").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
        Bracketed::new(vec![
            Delimited::new(vec![
                Sequence::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    one_of(vec![
                        Ref::keyword("HISTORY").to_matchable(),
                        Ref::keyword("CURRENT").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
    ])
    .to_matchable()
}

fn optionally_delimited_table_options(options: Vec<Matchable>) -> Matchable {
    Delimited::new(vec![one_of(options).to_matchable()])
        .config(|this| this.optional_delimiter())
        .to_matchable()
}

fn equals_table_option(keyword: &'static str, value: Matchable) -> Matchable {
    Sequence::new(vec![
        Ref::keyword(keyword).to_matchable(),
        Ref::new("EqualsSegment").optional().to_matchable(),
        value,
    ])
    .to_matchable()
}

fn numeric_table_option(keyword: &'static str) -> Matchable {
    equals_table_option(keyword, Ref::new("NumericLiteralSegment").to_matchable())
}

fn quoted_table_option(keyword: &'static str) -> Matchable {
    equals_table_option(keyword, Ref::new("QuotedLiteralSegment").to_matchable())
}

fn keyword_table_option(keyword: &'static str, values: &[&'static str]) -> Matchable {
    equals_table_option(
        keyword,
        one_of(
            values
                .iter()
                .map(|value| Ref::keyword(*value).to_matchable())
                .collect(),
        )
        .to_matchable(),
    )
}

fn number_or_default_table_option(keyword: &'static str) -> Matchable {
    equals_table_option(
        keyword,
        one_of(vec![
            Ref::keyword("DEFAULT").to_matchable(),
            Ref::new("NumericLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
    )
}

fn quoted_or_identifier() -> Matchable {
    one_of(vec![
        Ref::new("QuotedLiteralSegment").to_matchable(),
        Ref::new("NakedIdentifierSegment").to_matchable(),
    ])
    .to_matchable()
}

/// MariaDB's ALTER TABLE ADD constraints allow IF NOT EXISTS, unlike CREATE TABLE.
fn alter_table_constraint_grammar() -> Matchable {
    one_of(vec![
        Sequence::new(vec![
            Sequence::new(vec![
                Ref::keyword("CONSTRAINT").to_matchable(),
                Ref::new("ObjectReferenceSegment").optional().to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("UNIQUE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("INDEX").to_matchable(),
                        Ref::keyword("KEY").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("IndexReferenceSegment").optional().to_matchable(),
                    Ref::new("IndexTypeGrammar").optional().to_matchable(),
                    Ref::new("BracketedKeyPartListGrammar").to_matchable(),
                    Ref::new("IndexOptionsSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("PrimaryKeyGrammar").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("IndexTypeGrammar").optional().to_matchable(),
                    Ref::new("BracketedKeyPartListGrammar").to_matchable(),
                    Ref::new("IndexOptionsSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("ForeignKeyGrammar").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("IndexReferenceSegment").optional().to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                    Ref::keyword("REFERENCES").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar").to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            one_of(vec![
                                Ref::keyword("DELETE").to_matchable(),
                                Ref::keyword("UPDATE").to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("RESTRICT").to_matchable(),
                                Ref::keyword("CASCADE").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("SET").to_matchable(),
                                    Ref::keyword("NULL").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("NO").to_matchable(),
                                    Ref::keyword("ACTION").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("SET").to_matchable(),
                                    Ref::keyword("DEFAULT").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CHECK").to_matchable(),
                    Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ENFORCED").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("ENFORCED").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("INDEX").to_matchable(),
                Ref::keyword("KEY").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("IndexReferenceSegment").optional().to_matchable(),
            Ref::new("IndexTypeGrammar").optional().to_matchable(),
            Ref::new("BracketedKeyPartListGrammar").to_matchable(),
            Ref::new("IndexOptionsSegment").optional().to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("FULLTEXT").to_matchable(),
                Ref::keyword("SPATIAL").to_matchable(),
            ])
            .to_matchable(),
            one_of(vec![
                Ref::keyword("INDEX").to_matchable(),
                Ref::keyword("KEY").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("IndexReferenceSegment").optional().to_matchable(),
            Ref::new("BracketedKeyPartListGrammar").to_matchable(),
            Ref::new("IndexOptionsSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable()
}

fn quoted_or_identifier_or_default() -> Matchable {
    one_of(vec![
        Ref::new("QuotedLiteralSegment").to_matchable(),
        Ref::new("NakedIdentifierSegment").to_matchable(),
        Ref::keyword("DEFAULT").to_matchable(),
    ])
    .to_matchable()
}

fn charset_grammar() -> Matchable {
    one_of(vec![
        Sequence::new(vec![
            Ref::keyword("CHARACTER").to_matchable(),
            Ref::keyword("SET").to_matchable(),
        ])
        .to_matchable(),
        Ref::keyword("CHARSET").to_matchable(),
    ])
    .to_matchable()
}

fn charset_table_option() -> Matchable {
    Sequence::new(vec![
        Ref::keyword("DEFAULT").optional().to_matchable(),
        charset_grammar(),
        Ref::new("EqualsSegment").optional().to_matchable(),
        one_of(vec![
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Ref::new("CharacterSetSegment").to_matchable(),
            Ref::keyword("BINARY").to_matchable(),
            Ref::keyword("DEFAULT").to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable()
}

fn collate_table_option() -> Matchable {
    Sequence::new(vec![
        Ref::keyword("DEFAULT").optional().to_matchable(),
        Ref::keyword("COLLATE").to_matchable(),
        Ref::new("EqualsSegment").optional().to_matchable(),
        quoted_or_identifier_or_default(),
    ])
    .to_matchable()
}

fn directory_table_option(kind: &'static str) -> Matchable {
    Sequence::new(vec![
        Ref::keyword(kind).to_matchable(),
        Ref::keyword("DIRECTORY").to_matchable(),
        Ref::new("EqualsSegment").optional().to_matchable(),
        Ref::new("QuotedLiteralSegment").to_matchable(),
    ])
    .to_matchable()
}

fn union_table_option() -> Matchable {
    Sequence::new(vec![
        Ref::keyword("UNION").to_matchable(),
        Ref::new("EqualsSegment").optional().to_matchable(),
        Bracketed::new(vec![
            Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()]).to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable()
}

pub fn raw_dialect() -> Dialect {
    let mut mariadb = mysql::raw_dialect();
    mariadb.name = DialectKind::Mariadb;

    // MariaDB has its own reserved/unreserved keyword sets.
    for kw in MARIADB_UNRESERVED_KEYWORDS.lines() {
        let kw = kw.trim();
        if !kw.is_empty() {
            mariadb.sets_mut("unreserved_keywords").insert(kw);
        }
    }
    mariadb.sets_mut("reserved_keywords").clear();
    for kw in MARIADB_RESERVED_KEYWORDS.lines() {
        let kw = kw.trim();
        if !kw.is_empty() {
            mariadb.sets_mut("reserved_keywords").insert(kw);
        }
    }

    mariadb.add([(
        "SystemTimePartitionSegment".into(),
        NodeMatcher::new(SyntaxKind::SystemTimePartition, |_| {
            system_time_partition_grammar()
        })
        .to_matchable()
        .into(),
    )]);

    // Preserve the inherited MySQL CREATE TABLE grammar, inserting this
    // MariaDB-only clause into its trailing table-option alternatives.
    let create_table = mariadb
        .grammar("CreateTableStatementSegment")
        .as_node_matcher_ref()
        .expect("MySQL CREATE TABLE is a node matcher")
        .match_grammar(&mariadb);
    let mut create_table_elements = create_table.elements().to_vec();
    let table_options = create_table_elements
        .pop()
        .expect("CREATE TABLE ends with table options");
    create_table_elements.push(table_options.copy(
        Some(vec![Ref::new("SystemTimePartitionSegment").to_matchable()]),
        Some(0),
        None,
        None,
        Vec::new(),
        false,
    ));
    mariadb.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(create_table_elements).to_matchable(),
    );

    mariadb.replace_grammar(
        "AddDropSystemVersioningGrammar",
        one_of(vec![
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("ADD").to_matchable(),
                    Ref::keyword("DROP").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("SYSTEM").to_matchable(),
                Ref::keyword("VERSIONING").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ADD").to_matchable(),
                Ref::keyword("PERIOD").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::keyword("FOR").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("PERIOD").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::keyword("FOR").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    mariadb.replace_grammar(
        "TriggerOrReplaceGrammar",
        Sequence::new(vec![
            Ref::keyword("OR").to_matchable(),
            Ref::keyword("REPLACE").to_matchable(),
        ])
        .to_matchable(),
    );

    // Application-time UNIQUE and PRIMARY KEY definitions may include WITHOUT OVERLAPS.
    mariadb.replace_grammar(
        "BracketedKeyPartListGrammar",
        Bracketed::new(vec![
            Delimited::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("IndexColumnPrefixLengthSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ASC").to_matchable(),
                        Ref::keyword("DESC").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITHOUT").to_matchable(),
                        Ref::keyword("OVERLAPS").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    mariadb.add([
        (
            "TemporalQuerySegment".into(),
            NodeMatcher::new(SyntaxKind::TemporalQuery, |_| {
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("SYSTEM_TIME").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ALL").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("AS").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("BETWEEN").to_matchable(),
                            Ref::new("Expression_B_Grammar").to_matchable(),
                            Ref::keyword("AND").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FROM").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::keyword("TO").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
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
            "ForPortionOfSegment".into(),
            NodeMatcher::new(SyntaxKind::ForPortionOfClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("PORTION").to_matchable(),
                    Ref::keyword("OF").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PeriodSegment".into(),
            NodeMatcher::new(SyntaxKind::PeriodSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("PERIOD").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::keyword("SYSTEM_TIME").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    let table_constraint = mariadb.grammar("TableConstraintSegment");
    let mysql_table_constraint = table_constraint
        .as_node_matcher_ref()
        .expect("MySQL table constraint is a node matcher")
        .match_grammar(&mariadb);
    mariadb.replace_grammar(
        "TableConstraintSegment",
        one_of(vec![
            Ref::new("PeriodSegment").to_matchable(),
            mysql_table_constraint,
        ])
        .to_matchable(),
    );

    mariadb.replace_grammar("TableOptionsSegment", mariadb_table_options_grammar());

    // MariaDB additionally supports PERSISTENT generated columns.
    // https://mariadb.com/kb/en/generated-columns/
    mariadb.replace_grammar(
        "ColumnConstraintSegment",
        mysql::column_constraint_grammar(true, true),
    );

    // MariaDB's INSERT, single-table DELETE, and REPLACE statements support a
    // trailing RETURNING clause.
    // https://mariadb.com/kb/en/insertreturning/
    // https://mariadb.com/kb/en/deletereturning/
    // https://mariadb.com/kb/en/replacereturning/
    mariadb.add([(
        "ReturningClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::ReturningClause, |_| {
            Sequence::new(vec![
                Ref::keyword("RETURNING").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Delimited::new(vec![Ref::new("SelectClauseElementSegment").to_matchable()])
                    .config(|this| this.allow_trailing())
                    .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .terminators(vec![
                Ref::new("SelectClauseTerminatorGrammar").to_matchable(),
            ])
            .config(|this| this.parse_mode(ParseMode::GreedyOnceStarted))
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    mariadb.replace_grammar(
        "DeleteStatementSegment",
        one_of(vec![
            // System-versioned history purges are distinct from ordinary DELETE:
            // LOW_PRIORITY, QUICK, and IGNORE do not apply to this form.
            Sequence::new(vec![
                Ref::keyword("DELETE").to_matchable(),
                Ref::keyword("HISTORY").to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("SelectPartitionClauseSegment")
                    .optional()
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("BEFORE").to_matchable(),
                    Ref::keyword("SYSTEM_TIME").to_matchable(),
                    one_of(vec![
                        Ref::keyword("TIMESTAMP").to_matchable(),
                        Ref::keyword("TRANSACTION").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            mysql::delete_statement_grammar(true, true),
        ])
        .to_matchable(),
    );
    mariadb.replace_grammar(
        "UpdateStatementSegment",
        mysql::update_statement_grammar(true),
    );
    mariadb.replace_grammar(
        "InsertStatementSegment",
        mysql::insert_statement_grammar(false, true),
    );
    mariadb.replace_grammar(
        "ReplaceSegment",
        mysql::replace_statement_grammar(false, true),
    );
    mariadb.replace_grammar(
        "SelectStatementSegment",
        mysql::select_statement_grammar(true),
    );

    // MariaDB permits ASC or DESC after each GROUP BY expression and an
    // optional WITH ROLLUP clause after the expression list.
    // https://mariadb.com/kb/en/select-with-rollup/
    mariadb.add([(
        "WithRollupClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::WithRollupClause, |_| {
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("ROLLUP").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    mariadb.replace_grammar(
        "GroupByClauseSegment",
        Sequence::new(vec![
            Ref::keyword("GROUP").to_matchable(),
            Ref::keyword("BY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            Delimited::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ASC").to_matchable(),
                        Ref::keyword("DESC").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![Ref::new("GroupByClauseTerminatorGrammar").to_matchable()];
            })
            .to_matchable(),
            Ref::new("WithRollupClauseSegment")
                .optional()
                .to_matchable(),
            MetaSegment::dedent().to_matchable(),
        ])
        .to_matchable(),
    );

    // `CREATE [OR REPLACE] USER`.
    // https://mariadb.com/kb/en/create-user/
    mariadb.replace_grammar(
        "CreateUserStatementSegment",
        mysql::create_user_grammar(true),
    );

    // MariaDB permits OR REPLACE before PROCEDURE and FUNCTION.
    // https://mariadb.com/docs/server/server-usage/stored-routines/stored-procedures/create-procedure
    // https://mariadb.com/docs/server/reference/sql-statements/data-definition/create/create-function
    mariadb.replace_grammar(
        "CreateProcedureStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("DefinerSegment").optional().to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            Ref::keyword("PROCEDURE").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("FunctionNameSegment").to_matchable(),
            Ref::new("ProcedureParameterListGrammar")
                .optional()
                .to_matchable(),
            Ref::new("CommentClauseSegment").optional().to_matchable(),
            Ref::new("CharacteristicStatement")
                .optional()
                .to_matchable(),
            Ref::new("FunctionDefinitionGrammar").to_matchable(),
        ])
        .to_matchable(),
    );
    mariadb.replace_grammar(
        "CreateFunctionStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("DefinerSegment").optional().to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            Ref::keyword("FUNCTION").to_matchable(),
            Ref::new("FunctionNameSegment").to_matchable(),
            Ref::new("FunctionParameterListGrammar")
                .optional()
                .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("RETURNS").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("CommentClauseSegment").optional().to_matchable(),
            Ref::new("CharacteristicStatement").to_matchable(),
            Ref::new("FunctionDefinitionGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    // MariaDB permits IF NOT EXISTS immediately after VIEW; MySQL does not.
    // https://mariadb.com/kb/en/create-view/
    mariadb.replace_grammar(
        "CreateViewStatementSegment",
        mysql::create_view_grammar(true),
    );

    // MariaDB CREATE INDEX supports OR REPLACE, IF NOT EXISTS, RTREE,
    // MariaDB-specific index options, WAIT/NOWAIT, ALGORITHM, and LOCK.
    // https://mariadb.com/kb/en/create-index/
    mariadb.add([
        (
            "IndexTypeSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexType, |_| {
                Sequence::new(vec![
                    Ref::keyword("USING").to_matchable(),
                    one_of(vec![
                        Ref::keyword("BTREE").to_matchable(),
                        Ref::keyword("HASH").to_matchable(),
                        Ref::keyword("RTREE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "IndexOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexOption, |_| {
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("KEY_BLOCK_SIZE").to_matchable(),
                        Ref::new("EqualsSegment").optional().to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("IndexTypeSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("PARSER").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("COMMENT").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CLUSTERING").to_matchable(),
                        Ref::new("EqualsSegment").optional().to_matchable(),
                        one_of(vec![
                            Ref::keyword("YES").to_matchable(),
                            Ref::keyword("NO").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("IGNORED").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOT").to_matchable(),
                            Ref::keyword("IGNORED").to_matchable(),
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
            "AlgorithmOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::AlgorithmOption, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALGORITHM").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("INPLACE").to_matchable(),
                        Ref::keyword("COPY").to_matchable(),
                        Ref::keyword("NOCOPY").to_matchable(),
                        Ref::keyword("INSTANT").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LockOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::LockOption, |_| {
                Sequence::new(vec![
                    Ref::keyword("LOCK").to_matchable(),
                    Ref::new("EqualsSegment").optional().to_matchable(),
                    one_of(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("NONE").to_matchable(),
                        Ref::keyword("SHARED").to_matchable(),
                        Ref::keyword("EXCLUSIVE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "WaitOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::WaitOption, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("WAIT").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("NOWAIT").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);
    mariadb.replace_grammar(
        "CreateIndexStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::new("OrReplaceGrammar").optional().to_matchable(),
            one_of(vec![
                Ref::keyword("UNIQUE").to_matchable(),
                Ref::keyword("FULLTEXT").to_matchable(),
                Ref::keyword("SPATIAL").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::keyword("INDEX").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("IndexReferenceSegment").to_matchable(),
            Ref::new("IndexTypeSegment").optional().to_matchable(),
            Ref::keyword("ON").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("BracketedKeyPartListGrammar").to_matchable(),
            Ref::new("WaitOptionSegment").optional().to_matchable(),
            Ref::new("IndexOptionSegment").optional().to_matchable(),
            any_set_of(vec![
                Ref::new("AlgorithmOptionSegment").to_matchable(),
                Ref::new("LockOptionSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // MariaDB alone accepts IF [NOT] EXISTS on ALTER TABLE index and
    // constraint actions. Keep CREATE TABLE's shared constraint unchanged.
    // https://mariadb.com/docs/server/reference/sql-statements/data-definition/alter/alter-table
    mariadb.replace_grammar(
        "AlterTableConstraintSegment",
        NodeMatcher::new(SyntaxKind::TableConstraint, |_| {
            alter_table_constraint_grammar()
        })
        .to_matchable(),
    );
    mariadb.replace_grammar(
        "AlterTableIfExistsGrammar",
        Ref::new("IfExistsGrammar").to_matchable(),
    );
    mariadb.replace_grammar(
        "AlterTableIndexKeywordGrammar",
        one_of(vec![
            Ref::keyword("INDEX").to_matchable(),
            Ref::keyword("KEY").to_matchable(),
        ])
        .to_matchable(),
    );
    mariadb.replace_grammar(
        "AlterTableIndexStateGrammar",
        one_of(vec![
            Ref::keyword("VISIBLE").to_matchable(),
            Ref::keyword("INVISIBLE").to_matchable(),
            Ref::keyword("IGNORED").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("NOT").to_matchable(),
                Ref::keyword("IGNORED").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // MariaDB allows IF EXISTS after ALTER TABLE and after DROP PARTITION.
    mariadb.replace_grammar(
        "AlterTablePartitionActionGrammar",
        mysql::alter_table_partition_action_grammar(true),
    );
    let alter_table_grammar = mariadb
        .grammar("AlterTableStatementSegment")
        .match_grammar(&mariadb)
        .expect("MySQL ALTER TABLE has a grammar");
    mariadb.replace_grammar(
        "AlterTableStatementSegment",
        alter_table_grammar.copy(
            Some(vec![Ref::new("IfExistsGrammar").optional().to_matchable()]),
            None,
            Some(Ref::new("TableReferenceSegment").to_matchable()),
            None,
            vec![],
            false,
        ),
    );

    // `FLUSH` statement.
    // https://mariadb.com/kb/en/flush/
    mariadb.replace_grammar(
        "FlushStatementSegment",
        Sequence::new(vec![
            Ref::keyword("FLUSH").to_matchable(),
            one_of(vec![
                Ref::keyword("NO_WRITE_TO_BINLOG").to_matchable(),
                Ref::keyword("LOCAL").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            one_of(vec![
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("BINARY").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ENGINE").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ERROR").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("GENERAL").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("QUERY").to_matchable(),
                        Ref::keyword("CACHE").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SLOW").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RESET").optional().to_matchable(),
                        Ref::keyword("MASTER").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("GLOBAL").to_matchable(),
                            Ref::keyword("SESSION").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("STATUS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RELAY").to_matchable(),
                        Ref::keyword("LOGS").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("FOR").to_matchable(),
                            Ref::keyword("CHANNEL").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("HOSTS").to_matchable(),
                    Ref::keyword("LOGS").to_matchable(),
                    Ref::keyword("PRIVILEGES").to_matchable(),
                    Ref::keyword("CHANGED_PAGE_BITMAPS").to_matchable(),
                    Ref::keyword("CLIENT_STATISTICS").to_matchable(),
                    Ref::keyword("DES_KEY_FILE").to_matchable(),
                    Ref::keyword("INDEX_STATISTICS").to_matchable(),
                    Ref::keyword("QUERY_RESPONSE_TIME").to_matchable(),
                    Ref::keyword("SLAVE").to_matchable(),
                    Ref::keyword("SSL").to_matchable(),
                    Ref::keyword("TABLE_STATISTICS").to_matchable(),
                    Ref::keyword("USER_STATISTICS").to_matchable(),
                    Ref::keyword("USER_VARIABLES").to_matchable(),
                    Ref::keyword("USER_RESOURCES").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("TABLES").to_matchable(),
                    Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                        .config(|this| {
                            this.optional();
                            this.base.terminators = vec![Ref::keyword("WITH").to_matchable()];
                        })
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("READ").to_matchable(),
                        Ref::keyword("LOCK").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("AND").to_matchable(),
                            Ref::keyword("DISABLE").to_matchable(),
                            Ref::keyword("CHECKPOINT").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("TABLES").to_matchable(),
                    Delimited::new(vec![Ref::new("TableReferenceSegment").to_matchable()])
                        .config(|this| {
                            this.base.terminators = vec![Ref::keyword("FOR").to_matchable()];
                        })
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("EXPORT").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    mariadb
}
