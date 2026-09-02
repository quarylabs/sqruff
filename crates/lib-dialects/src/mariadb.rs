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
use sqruff_lib_core::parser::grammar::anyof::{one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
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

    // MariaDB additionally supports PERSISTENT generated columns.
    // https://mariadb.com/kb/en/generated-columns/
    mariadb.replace_grammar(
        "ColumnConstraintSegment",
        mysql::column_constraint_grammar(true),
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
        mysql::delete_statement_grammar(true),
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

    // `CREATE [OR REPLACE] USER`.
    // https://mariadb.com/kb/en/create-user/
    mariadb.replace_grammar(
        "CreateUserStatementSegment",
        mysql::create_user_grammar(true),
    );

    // `CREATE [OR REPLACE] [TEMPORARY] TABLE`, additionally allowing the
    // `CREATE ... [AS] SELECT` form without a bracketed column list.
    // https://mariadb.com/kb/en/create-table/
    mariadb.replace_grammar(
        "CreateTableStatementSegment",
        mysql::create_table_grammar(
            one_of(vec![
                // Columns and comment syntax, optionally followed by AS SELECT:
                Sequence::new(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("TableConstraintSegment").to_matchable(),
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("CommentClauseSegment").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AS").optional().to_matchable(),
                        optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                // Create AS syntax (AS optional):
                Sequence::new(vec![
                    Ref::keyword("AS").optional().to_matchable(),
                    optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                // Create like syntax:
                Sequence::new(vec![
                    Ref::keyword("LIKE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
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
