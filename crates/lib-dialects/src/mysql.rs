use super::ansi::{self};
use crate::mysql_keywords::{MYSQL_RESERVED_KEYWORDS, MYSQL_UNRESERVED_KEYWORDS};
use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::base::{Anything, Ref};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::Bracketed;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::vec_of_erased;
use sqruff_lib_core::{parser::grammar::sequence::Sequence, parser::lexer::Matcher};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut mysql = ansi::raw_dialect();
    mysql.name = DialectKind::Mysql;

    mysql.patch_lexer_matchers(vec![Matcher::regex(
        "inline_comment",
        r"(^--|-- |#)[^\n]*",
        SyntaxKind::InlineComment,
    )]);

    // # Set Keywords
    // Do not clear inherited unreserved ansi keywords. Too many are needed to parse well.
    // Just add MySQL unreserved keywords.
    mysql.update_keywords_set_from_multiline_string(
        "unreserved_keywords",
        MYSQL_UNRESERVED_KEYWORDS,
    );
    mysql.sets("reserved_keywords").clear();
    mysql.update_keywords_set_from_multiline_string("reserved_keywords", MYSQL_RESERVED_KEYWORDS);

    // Set the datetime units
    mysql.sets_mut("datetime_units").clear();
    mysql.sets_mut("datetime_units").extend(vec![
        // https://github.com/mysql/mysql-server/blob/1bfe02bdad6604d54913c62614bde57a055c8332/sql/sql_yacc.yy#L12321-L12345
        // interval:
        "DAY_HOUR",
        "DAY_MICROSECOND",
        "DAY_MINUTE",
        "DAY_SECOND",
        "HOUR_MICROSECOND",
        "HOUR_MINUTE",
        "HOUR_SECOND",
        "MINUTE_MICROSECOND",
        "MINUTE_SECOND",
        "SECOND_MICROSECOND",
        "YEAR_MONTH",
        // interval_time_stamp
        "DAY",
        "WEEK",
        "HOUR",
        "MINUTE",
        "MONTH",
        "QUARTER",
        "SECOND",
        "MICROSECOND",
        "YEAR",
    ]);

    mysql.sets_mut("date_part_function_name").clear();
    mysql.sets_mut("date_part_function_name").extend(vec![
        "EXTRACT",
        "TIMESTAMPADD",
        "TIMESTAMPDIFF",
    ]);

    mysql.add([(
        // MySQL allows the usage of a double quoted identifier for an alias.
        "DoubleQuotedIdentifierSegment".into(),
        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::Identifier)
            .to_matchable()
            .into(),
    )]);

    mysql.add([
        (
            // A reference to an object with an `AS` clause.
            // The optional AS keyword allows both implicit and explicit aliasing.
            "AliasExpressionSegment".into(),
            Sequence::new(vec_of_erased![
                MetaSegment::indent(),
                Ref::keyword("AS").optional(),
                one_of(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("SingleQuotedIdentifierSegment"),
                    Ref::new("DoubleQuotedIdentifierSegment"),
                ]),
                MetaSegment::dedent(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A column definition, e.g. for CREATE TABLE or ALTER TABLE.
            "ColumnDefinitionSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"), // Column name
                one_of(vec_of_erased![
                    // DATETIME and TIMESTAMP take special logic
                    Ref::new("DatatypeSegment").exclude(one_of(vec_of_erased![
                        Ref::keyword("DATETIME"),
                        Ref::keyword("TIMESTAMP"),
                    ])),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("DATETIME"),
                            Ref::keyword("TIMESTAMP"),
                        ]),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment"),])
                            .config(|bracketed| bracketed.optional()), // Precision
                        AnyNumberOf::new(vec_of_erased![
                            // Allow NULL/NOT NULL, DEFAULT, and ON UPDATE in any order
                            Sequence::new(vec_of_erased![
                                Sequence::new(vec_of_erased![Ref::keyword("NOT"),])
                                    .config(|sequence| sequence.optional()),
                                Ref::keyword("NULL"),
                            ])
                            .config(|sequence| sequence.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                one_of(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        one_of(vec_of_erased![
                                            Ref::keyword("CURRENT_TIMESTAMP"),
                                            Ref::keyword("NOW"),
                                        ]),
                                        Bracketed::new(vec_of_erased![
                                            Ref::new("NumericLiteralSegment").optional()
                                        ])
                                        .config(|bracketed| bracketed.optional()),
                                    ]),
                                    Ref::new("NumericLiteralSegment"),
                                    Ref::new("QuotedLiteralSegment"),
                                    Ref::keyword("NULL"),
                                ]),
                            ])
                            .config(|sequence| sequence.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ON"),
                                Ref::keyword("UPDATE"),
                                one_of(vec_of_erased![
                                    Ref::keyword("CURRENT_TIMESTAMP"),
                                    Ref::keyword("NOW"),
                                ]),
                                Bracketed::new(vec_of_erased![
                                    Ref::new("NumericLiteralSegment").optional()
                                ])
                                .config(|bracketed| bracketed.optional()),
                            ])
                            .config(|sequence| sequence.optional()),
                        ])
                        .config(|any_number| any_number.optional()),
                    ]),
                ]),
                Bracketed::new(vec_of_erased![Anything::new(),])
                    .config(|bracketed| bracketed.optional()), // For types like VARCHAR(100)
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("ColumnConstraintSegment").optional(),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is a `CREATE USER` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/create-user.html
            "CreateUserStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("USER"),
                Ref::new("IfNotExistsGrammar").optional(),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("RoleReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::keyword("IDENTIFIED"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("BY"),
                                    one_of(vec_of_erased![
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("RANDOM"),
                                            Ref::keyword("PASSWORD"),
                                        ]),
                                        Ref::new("QuotedLiteralSegment"),
                                    ]),
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("WITH"),
                                    Ref::new("ObjectReferenceSegment"),
                                    Sequence::new(vec_of_erased![one_of(vec_of_erased![
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("BY"),
                                            one_of(vec_of_erased![
                                                Sequence::new(vec_of_erased![
                                                    Ref::keyword("RANDOM"),
                                                    Ref::keyword("PASSWORD"),
                                                ]),
                                                Ref::new("QuotedLiteralSegment"),
                                            ]),
                                        ]),
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("AS"),
                                            Ref::new("QuotedLiteralSegment"),
                                        ]),
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("INITIAL"),
                                            Ref::keyword("AUTHENTICATION"),
                                            Ref::keyword("IDENTIFIED"),
                                            one_of(vec_of_erased![
                                                Sequence::new(vec_of_erased![
                                                    Ref::keyword("BY"),
                                                    one_of(vec_of_erased![
                                                        Sequence::new(vec_of_erased![
                                                            Ref::keyword("RANDOM"),
                                                            Ref::keyword("PASSWORD"),
                                                        ]),
                                                        Ref::new("QuotedLiteralSegment"),
                                                    ]),
                                                ]),
                                                Sequence::new(vec_of_erased![
                                                    Ref::keyword("WITH"),
                                                    Ref::new("ObjectReferenceSegment"),
                                                    Ref::keyword("AS"),
                                                    Ref::new("QuotedLiteralSegment"),
                                                ]),
                                            ]),
                                        ]),
                                    ]),])
                                    .config(|sequence| sequence.optional()),
                                ]),
                            ]),
                        ]),])
                        .config(|delimited| delimited.delimiter(Ref::keyword("AND"))),
                    ])
                    .config(|sequence| sequence.optional()),
                ]),]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DEFAULT"),
                    Ref::keyword("ROLE"),
                    Delimited::new(vec_of_erased![Ref::new("RoleReferenceSegment")]),
                ])
                .config(|sequence| sequence.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REQUIRE"),
                    one_of(vec_of_erased![
                        Ref::keyword("NONE"),
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::keyword("SSL"),
                            Ref::keyword("X509"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CIPHER"),
                                Ref::new("QuotedLiteralSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ISSUER"),
                                Ref::new("QuotedLiteralSegment"),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SUBJECT"),
                                Ref::new("QuotedLiteralSegment"),
                            ]),
                        ]),])
                        .config(|delimited| delimited.delimiter(Ref::keyword("AND"))),
                    ]),
                ])
                .config(|sequence| sequence.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("MAX_QUERIES_PER_HOUR"),
                            Ref::keyword("MAX_UPDATES_PER_HOUR"),
                            Ref::keyword("MAX_CONNECTIONS_PER_HOUR"),
                            Ref::keyword("MAX_USER_CONNECTIONS"),
                        ]),
                        Ref::new("NumericLiteralSegment"),
                    ]),]),
                ])
                .config(|sequence| sequence.optional()),
                Sequence::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PASSWORD"),
                        Ref::keyword("EXPIRE"),
                        Sequence::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::keyword("NEVER"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("INTERVAL"),
                                Ref::new("NumericLiteralSegment"),
                                Ref::keyword("DAY"),
                            ]),
                        ]),])
                        .config(|sequence| sequence.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PASSWORD"),
                        Ref::keyword("HISTORY"),
                        one_of(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::new("NumericLiteralSegment"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PASSWORD"),
                        Ref::keyword("REUSE"),
                        Ref::keyword("INTERVAL"),
                        one_of(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Sequence::new(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::keyword("DAY"),
                            ]),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PASSWORD"),
                        Ref::keyword("REQUIRE"),
                        Ref::keyword("CURRENT"),
                        Sequence::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::keyword("OPTIONAL"),
                        ]),])
                        .config(|sequence| sequence.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FAILED_LOGIN_ATTEMPTS"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PASSWORD_LOCK_TIME"),
                        one_of(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::keyword("UNBOUNDED"),
                        ]),
                    ]),
                ]),])
                .config(|sequence| sequence.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ACCOUNT"),
                    one_of(vec_of_erased![Ref::keyword("UNLOCK"), Ref::keyword("LOCK")]),
                ])
                .config(|sequence| sequence.optional()),
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("COMMENT"),
                        Ref::keyword("ATTRIBUTE")
                    ]),
                    Ref::new("QuotedLiteralSegment"),
                ])
                .config(|sequence| sequence.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is a CLOSE or Open statement.
            // https://dev.mysql.com/doc/refman/8.0/en/close.html
            // https://dev.mysql.com/doc/refman/8.0/en/open.html
            "CursorOpenCloseSegment".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("CLOSE"), Ref::keyword("OPEN"),]),
                one_of(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("QuotedIdentifierSegment"),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `ITERATE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/iterate.html
            "IterateStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("ITERATE"),
                Ref::new("SingleIdentifierGrammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is the body of a `EXECUTE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/execute.html
            "ExecuteSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("EXECUTE"),
                Ref::new("NakedIdentifierSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("USING"),
                    Delimited::new(vec_of_erased![Ref::new("SessionVariableNameSegment")]),
                ])
                .config(|delimited| delimited.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `REPEAT-UNTIL` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/repeat.html
            "RepeatStatementSegment".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("ColonSegment"),
                    ])
                    .config(|sequence| sequence.optional()),
                    Ref::keyword("REPEAT"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("StatementSegment")]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNTIL"),
                    Ref::new("ExpressionSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("END"),
                        Ref::keyword("REPEAT"),
                        Ref::new("SingleIdentifierGrammar").optional(),
                    ]),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is the body of a `DEALLOCATE/DROP` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/deallocate-prepare.html
            "DeallocateSegment".into(),
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("DEALLOCATE"),
                        Ref::keyword("DROP"),
                    ]),
                    Ref::keyword("PREPARE"),
                ]),
                Ref::new("NakedIdentifierSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is the body of a `RESIGNAL` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/resignal.html
            "ResignalSegment".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("SIGNAL"),
                    Ref::keyword("RESIGNAL"),
                ]),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SQLSTATE"),
                        Ref::keyword("VALUE").optional(),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Ref::new("NakedIdentifierSegment"),
                ])
                .config(|one_of| one_of.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("CLASS_ORIGIN"),
                            Ref::keyword("SUBCLASS_ORIGIN"),
                            Ref::keyword("RETURNED_SQLSTATE"),
                            Ref::keyword("MESSAGE_TEXT"),
                            Ref::keyword("MYSQL_ERRNO"),
                            Ref::keyword("CONSTRAINT_CATALOG"),
                            Ref::keyword("CONSTRAINT_SCHEMA"),
                            Ref::keyword("CONSTRAINT_NAME"),
                            Ref::keyword("CATALOG_NAME"),
                            Ref::keyword("SCHEMA_NAME"),
                            Ref::keyword("TABLE_NAME"),
                            Ref::keyword("COLUMN_NAME"),
                            Ref::keyword("CURSOR_NAME"),
                        ]),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::new("SessionVariableNameSegment"),
                            Ref::new("LocalVariableNameSegment"),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                    ]),]),
                ])
                .config(|sequence| sequence.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is a FETCH statement.
            // https://dev.mysql.com/doc/refman/8.0/en/fetch.html
            "CursorFetchSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("FETCH"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NEXT").optional(),
                    Ref::keyword("FROM"),
                ])
                .config(|sequence| sequence.optional()),
                Ref::new("NakedIdentifierSegment"),
                Ref::keyword("INTO"),
                Delimited::new(vec_of_erased![
                    Ref::new("SessionVariableNameSegment"),
                    Ref::new("LocalVariableNameSegment"),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // This is a `DROP INDEX` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/drop-index.html
            "DropIndexStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("INDEX"),
                Ref::new("IndexReferenceSegment"),
                Ref::keyword("ON"),
                Ref::new("TableReferenceSegment"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALGORITHM"),
                        Ref::new("EqualsSegment").optional(),
                        one_of(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::keyword("INPLACE"),
                            Ref::keyword("COPY"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LOCK"),
                        Ref::new("EqualsSegment").optional(),
                        one_of(vec_of_erased![
                            Ref::keyword("DEFAULT"),
                            Ref::keyword("NONE"),
                            Ref::keyword("SHARED"),
                            Ref::keyword("EXCLUSIVE"),
                        ]),
                    ]),
                ])
                .config(|delimited| delimited.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `DROP` statement that addresses stored procedures and functions..
            // https://dev.mysql.com/doc/refman/8.0/en/drop-procedure.html
            "DropProcedureStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                one_of(vec_of_erased![
                    Ref::keyword("PROCEDURE"),
                    Ref::keyword("FUNCTION"),
                ]),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("ObjectReferenceSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `DROP` statement that addresses loadable functions.
            // https://dev.mysql.com/doc/refman/8.0/en/drop-function-loadable.html
            "DropFunctionStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("FUNCTION"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("FunctionNameSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `RENAME TABLE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/rename-table.html
            "RenameTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("RENAME"),
                Ref::keyword("TABLE"),
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::keyword("TO"),
                    Ref::new("TableReferenceSegment"),
                ]),]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `RESET MASTER` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/reset-master.html
            "ResetMasterStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("RESET"),
                Ref::keyword("MASTER"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("TO"),
                    Ref::new("NumericLiteralSegment"),
                ])
                .config(|sequence| sequence.optional()),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `PURGE BINARY LOGS` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/purge-binary-logs.html
            "PurgeBinaryLogsStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("PURGE"),
                one_of(vec_of_erased![
                    Ref::keyword("BINARY"),
                    Ref::keyword("MASTER"),
                ]),
                Ref::keyword("LOGS"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TO"),
                        Ref::new("QuotedLiteralSegment"),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BEFORE"),
                        Ref::new("ExpressionSegment"),
                    ]),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `HELP` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/help.html
            "HelpStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("HELP"),
                Ref::new("QuotedLiteralSegment"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `CHECK TABLE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/check-table.html
            "CheckTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECK"),
                Ref::keyword("TABLE"),
                Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment"),]),
                AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("FOR"), Ref::keyword("UPGRADE"),]),
                    Ref::keyword("QUICK"),
                    Ref::keyword("FAST"),
                    Ref::keyword("MEDIUM"),
                    Ref::keyword("EXTENDED"),
                    Ref::keyword("CHANGED"),
                ])
                .config(|any_number| any_number.min_times(1)),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `CHECKSUM TABLE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/checksum-table.html
            "ChecksumTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECKSUM"),
                Ref::keyword("TABLE"),
                Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment"),]),
                one_of(vec_of_erased![
                    Ref::keyword("QUICK"),
                    Ref::keyword("EXTENDED"),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // An `ANALYZE TABLE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/analyze-table.html
            "AnalyzeTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("ANALYZE"),
                one_of(vec_of_erased![
                    Ref::keyword("NO_WRITE_TO_BINLOG"),
                    Ref::keyword("LOCAL"),
                ])
                .config(|one| one.optional()),
                Ref::keyword("TABLE"),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "TableReferenceSegment"
                    ),]),]),
                    Sequence::new(vec_of_erased![
                        Ref::new("TableReferenceSegment"),
                        Ref::keyword("UPDATE"),
                        Ref::keyword("HISTOGRAM"),
                        Ref::keyword("ON"),
                        Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::keyword("BUCKETS"),
                        ])
                        .config(|seq| seq.optional()),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("TableReferenceSegment"),
                        Ref::keyword("DROP"),
                        Ref::keyword("HISTOGRAM"),
                        Ref::keyword("ON"),
                        Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment"),]),
                    ]),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `REPAIR TABLE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/repair-table.html
            "RepairTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("REPAIR"),
                one_of(vec_of_erased![
                    Ref::keyword("NO_WRITE_TO_BINLOG"),
                    Ref::keyword("LOCAL"),
                ])
                .config(|one| one.optional()),
                Ref::keyword("TABLE"),
                Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment"),]),
                AnyNumberOf::new(vec_of_erased![
                    Ref::keyword("QUICK"),
                    Ref::keyword("EXTENDED"),
                    Ref::keyword("USE_FRM"),
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            // An `OPTIMIZE TABLE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/optimize-table.html
            "OptimizeTableStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("OPTIMIZE"),
                one_of(vec_of_erased![
                    Ref::keyword("NO_WRITE_TO_BINLOG"),
                    Ref::keyword("LOCAL"),
                ])
                .config(|one| one.optional()),
                Ref::keyword("TABLE"),
                Delimited::new(vec_of_erased![Ref::new("TableReferenceSegment"),]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // An `UPDATE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/update.html
            "UpdateStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("UPDATE"),
                Ref::keyword("LOW_PRIORITY").optional(),
                Ref::keyword("IGNORE").optional(),
                MetaSegment::indent(),
                Delimited::new(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::new("FromExpressionSegment"),
                ]),
                MetaSegment::dedent(),
                Ref::new("SetClauseListSegment"),
                Ref::new("WhereClauseSegment").optional(),
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("LimitClauseSegment").optional(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `DELIMITER` statement.
            "DelimiterStatement".into(),
            Sequence::new(vec_of_erased![Ref::keyword("DELIMITER"),])
                .to_matchable()
                .into(),
        ),
        (
            // A `DECLARE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/declare-local-variable.html
            // https://dev.mysql.com/doc/refman/8.0/en/declare-handler.html
            // https://dev.mysql.com/doc/refman/8.0/en/declare-condition.html
            // https://dev.mysql.com/doc/refman/8.0/en/declare-cursor.html
            "DeclareStatement".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("DECLARE"),
                    Ref::new("NakedIdentifierSegment"),
                    Ref::keyword("CURSOR"),
                    Ref::keyword("FOR"),
                    Ref::new("StatementSegment"),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DECLARE"),
                    one_of(vec_of_erased![
                        Ref::keyword("CONTINUE"),
                        Ref::keyword("EXIT"),
                        Ref::keyword("UNDO"),
                    ]),
                    Ref::keyword("HANDLER"),
                    Ref::keyword("FOR"),
                    one_of(vec_of_erased![
                        Ref::keyword("SQLEXCEPTION"),
                        Ref::keyword("SQLWARNING"),
                        Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("FOUND"),]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SQLSTATE"),
                            Ref::keyword("VALUE").optional(),
                            Ref::new("QuotedLiteralSegment"),
                        ]),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("NakedIdentifierSegment"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![Ref::new("StatementSegment")]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DECLARE"),
                    Ref::new("NakedIdentifierSegment"),
                    Ref::keyword("CONDITION"),
                    Ref::keyword("FOR"),
                    one_of(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"),
                        Ref::new("NumericLiteralSegment"),
                    ]),
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("DECLARE"),
                    Ref::new("LocalVariableNameSegment"),
                    Ref::new("DatatypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("FunctionSegment"),
                        ]),
                    ])
                    .config(|seq| seq.optional()),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `CREATE PROCEDURE` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/create-procedure.html
            "CreateProcedureStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::new("DefinerSegment").optional(),
                Ref::keyword("PROCEDURE"),
                Ref::new("FunctionNameSegment"),
                Ref::new("ProcedureParameterListGrammar").optional(),
                Ref::new("CommentClauseSegment").optional(),
                Ref::new("CharacteristicStatement").optional(),
                Ref::new("FunctionDefinitionGrammar"),
            ])
            .to_matchable()
            .into(),
        ),
        (
            // A `SET TRANSACTION` statement.
            // https://dev.mysql.com/doc/refman/8.0/en/set-transaction.html
            "SetTransactionStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                one_of(vec_of_erased![
                    Ref::keyword("GLOBAL"),
                    Ref::keyword("SESSION"),
                ])
                .config(|this| this.optional()),
                Ref::keyword("TRANSACTION"),
                Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ISOLATION"),
                        Ref::keyword("LEVEL"),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("READ"),
                                one_of(vec_of_erased![
                                    Ref::keyword("COMMITTED"),
                                    Ref::keyword("UNCOMMITTED"),
                                ]),
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("REPEATABLE"),
                                Ref::keyword("READ"),
                            ]),
                            Ref::keyword("SERIALIZABLE"),
                        ]),
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("READ"),
                        one_of(vec_of_erased![Ref::keyword("WRITE"), Ref::keyword("ONLY"),]),
                    ]),
                ]),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    mysql.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec_of_erased![
                Ref::new("DelimiterStatement"),
                Ref::new("CreateProcedureStatementSegment"),
                Ref::new("DeclareStatement"),
                Ref::new("SetTransactionStatementSegment"),
                Ref::new("SetAssignmentStatementSegment"),
                Ref::new("IfExpressionStatement"),
                Ref::new("WhileStatementSegment"),
                Ref::new("IterateStatementSegment"),
                Ref::new("RepeatStatementSegment"),
                Ref::new("LoopStatementSegment"),
                Ref::new("CallStoredProcedureSegment"),
                Ref::new("PrepareSegment"),
                Ref::new("ExecuteSegment"),
                Ref::new("DeallocateSegment"),
                Ref::new("GetDiagnosticsSegment"),
                Ref::new("ResignalSegment"),
                Ref::new("CursorOpenCloseSegment"),
                Ref::new("CursorFetchSegment"),
                Ref::new("DropProcedureStatementSegment"),
                Ref::new("AlterTableStatementSegment"),
                Ref::new("AlterViewStatementSegment"),
                Ref::new("CreateViewStatementSegment"),
                Ref::new("RenameTableStatementSegment"),
                Ref::new("ResetMasterStatementSegment"),
                Ref::new("PurgeBinaryLogsStatementSegment"),
                Ref::new("HelpStatementSegment"),
                Ref::new("CheckTableStatementSegment"),
                Ref::new("ChecksumTableStatementSegment"),
                Ref::new("AnalyzeTableStatementSegment"),
                Ref::new("RepairTableStatementSegment"),
                Ref::new("OptimizeTableStatementSegment"),
                Ref::new("UpsertClauseListSegment"),
                Ref::new("InsertRowAliasSegment"),
                Ref::new("FlushStatementSegment"),
                Ref::new("LoadDataSegment"),
                Ref::new("ReplaceSegment"),
                Ref::new("AlterDatabaseStatementSegment"),
                Ref::new("ReturnStatementSegment"),
                Ref::new("SetNamesStatementSegment"),
                Ref::new("CreateEventStatementSegment"),
                Ref::new("AlterEventStatementSegment"),
                Ref::new("DropEventStatementSegment"),
            ]),
            None,
            None,
            Some(vec_of_erased![
                // handle CREATE SCHEMA in CreateDatabaseStatementSegment
                Ref::new("CreateSchemaStatementSegment"),
            ]),
            Vec::new(),
            false,
        ),
    );

    mysql
}
