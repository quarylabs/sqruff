use sqruff_parser_core::dialects::Dialect;
use sqruff_parser_core::dialects::DialectKind;
use sqruff_parser_core::dialects::SyntaxKind;
use sqruff_parser_core::helpers::{Config, ToMatchable};
use sqruff_parser_core::parser::grammar::Ref;
use sqruff_parser_core::parser::grammar::anyof::one_of;
use sqruff_parser_core::parser::lexer::Matcher;
use sqruff_parser_core::parser::node_matcher::NodeMatcher;
use sqruff_parser_core::vec_of_erased;

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

    mysql.add([
        (
            "DivBinaryOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::BinaryOperator, |_| {
                Ref::keyword("DIV").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ArithmeticBinaryOperatorGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("PlusSegment"),
                Ref::new("MinusSegment"),
                Ref::new("DivideSegment"),
                Ref::new("MultiplySegment"),
                Ref::new("ModuloSegment"),
                Ref::new("BitwiseAndSegment"),
                Ref::new("BitwiseOrSegment"),
                Ref::new("BitwiseXorSegment"),
                Ref::new("BitwiseLShiftSegment"),
                Ref::new("BitwiseRShiftSegment"),
                Ref::new("DivBinaryOperatorSegment"),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    mysql
}
