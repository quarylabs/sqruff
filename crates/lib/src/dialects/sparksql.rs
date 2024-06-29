use crate::core::dialects::base::Dialect;
use crate::core::parser::lexer::Matcher;
use crate::core::parser::segments::base::{
    CodeSegment, CodeSegmentNewArgs, CommentSegment, CommentSegmentNewArgs,
};

pub fn sparksql_dialect() -> Dialect {
    let mut sparksql_dialect = super::ansi::raw_dialect();
    sparksql_dialect.name = "sparksql";

    let _hive_dialect = super::hive::raw_dialect();

    sparksql_dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"(--)[^\n]*", |slice, marker| {
            CommentSegment::create(
                slice,
                marker.into(),
                CommentSegmentNewArgs { r#type: "inline_comment", trim_start: Some(vec!["--"]) },
            )
        }),
        Matcher::regex("back_quote", r"`([^`]|``)*`", |slice, marker| {
            CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "back_quote", ..<_>::default() },
            )
        }),
        Matcher::regex("numeric_literal", r#"(?>(?>\d+\.\d+|\d+\.|\.\d+)([eE][+-]?\d+)?([dDfF]|BD|bd)?|\d+[eE][+-]?\d+([dDfF]|BD|bd)?|\d+([dDfFlLsSyY]|BD|bd)?)((?<=\.)|(?=\b))"#
, |slice, marker| {
    CodeSegment::create(
                slice,
                marker.into(),
                CodeSegmentNewArgs { code_type: "inline_comment", ..<_>::default() },
            )
        }),
    ]);

    sparksql_dialect.sets_mut("bare_functions").clear();
    sparksql_dialect.sets_mut("bare_functions").extend([
        "CURRENT_DATE",
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
    ]);

    sparksql_dialect.expand();
    sparksql_dialect
}
