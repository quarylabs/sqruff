use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::base::Ref;
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

    mysql.add([(
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
    )]);

    mysql
}
