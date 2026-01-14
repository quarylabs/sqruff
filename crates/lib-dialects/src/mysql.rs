use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{one_of, optionally_bracketed};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::vec_of_erased;

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

    // MySQL 8.0+ supports CTEs with DML statements (INSERT, UPDATE, DELETE)
    // We add these to NonWithSelectableGrammar so WithCompoundStatementSegment can use them
    mysql.add([(
        "NonWithSelectableGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("SetExpressionSegment"),
            optionally_bracketed(vec_of_erased![Ref::new("SelectStatementSegment")]),
            Ref::new("NonSetSelectableGrammar"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("InsertStatementSegment"),
            Ref::new("DeleteStatementSegment"),
        ])
        .to_matchable()
        .into(),
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
