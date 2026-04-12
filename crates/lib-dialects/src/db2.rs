use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::RegexParser;
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;

use crate::db2_keywords::UNRESERVED_KEYWORDS;

use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(Db2DialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: Db2DialectConfig =
        config.map(Db2DialectConfig::from_value).unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut db2_dialect = super::ansi::dialect(None);
    db2_dialect.name = DialectKind::Db2;

    for kw in UNRESERVED_KEYWORDS {
        db2_dialect.add_keyword_to_set("unreserved_keywords", kw);
    }

    // DB2 allows # in field names, and doesn't use it as a comment.
    db2_dialect.patch_lexer_matchers(vec![
        // Remove hash comments — DB2 only uses -- for inline comments.
        Matcher::regex("inline_comment", r"(--)[^\n]*", SyntaxKind::InlineComment),
        // Allow # in word tokens for identifiers.
        Matcher::regex("word", r"[0-9a-zA-Z_#]+", SyntaxKind::Word),
    ]);

    db2_dialect.add([
        // DB2 allows # in naked identifiers.
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({pattern})$");

                RegexParser::new("[A-Z0-9_#]*[A-Z#][A-Z0-9_#]*", SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
            .into(),
        ),
        // DB2 PostFunctionGrammar: OVER or WITHIN GROUP (no FILTER).
        (
            "PostFunctionGrammar".into(),
            one_of(vec![
                Ref::new("OverClauseSegment").to_matchable(),
                Ref::new("WithinGroupClauseSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // DB2 Expression_C_Grammar: adds duration expressions (e.g. 1 DAYS, 1 DAY).
        (
            "Expression_C_Grammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("EXISTS").to_matchable(),
                    Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    one_of(vec![
                        Ref::new("Expression_D_Grammar").to_matchable(),
                        Ref::new("CaseExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("TimeZoneGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("ShorthandCastSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("DAYS").to_matchable(),
                        Ref::keyword("DAY").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.terminators = vec![Ref::new("CommaSegment").to_matchable()])
            .to_matchable()
            .into(),
        ),
        // WithinGroupClauseSegment for DB2 window functions.
        (
            "WithinGroupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::WithingroupClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITHIN").to_matchable(),
                    Ref::keyword("GROUP").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("OrderByClauseSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    db2_dialect
}
