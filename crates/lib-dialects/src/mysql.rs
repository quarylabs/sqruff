use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::base::Ref;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::vec_of_erased;
use sqruff_lib_core::{parser::grammar::sequence::Sequence, parser::lexer::Matcher};

use crate::mysql_keywords::{MYSQL_RESERVED_KEYWORDS, MYSQL_UNRESERVED_KEYWORDS};

use super::ansi;

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
    ]);

    mysql
}
