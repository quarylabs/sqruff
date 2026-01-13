use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;

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
            one_of(vec![
                Ref::new("PlusSegment").to_matchable(),
                Ref::new("MinusSegment").to_matchable(),
                Ref::new("DivideSegment").to_matchable(),
                Ref::new("MultiplySegment").to_matchable(),
                Ref::new("ModuloSegment").to_matchable(),
                Ref::new("BitwiseAndSegment").to_matchable(),
                Ref::new("BitwiseOrSegment").to_matchable(),
                Ref::new("BitwiseXorSegment").to_matchable(),
                Ref::new("BitwiseLShiftSegment").to_matchable(),
                Ref::new("BitwiseRShiftSegment").to_matchable(),
                Ref::new("DivBinaryOperatorSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    mysql
}
