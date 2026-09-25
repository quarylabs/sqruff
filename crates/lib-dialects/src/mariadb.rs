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
use sqruff_lib_core::parser::grammar::anyof::{
    AnyNumberOf, any_set_of, one_of, optionally_bracketed,
};
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
