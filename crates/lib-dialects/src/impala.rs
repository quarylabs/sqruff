//! The Apache Impala dialect.

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectConfig, DialectKind};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::StringParser;
use sqruff_lib_core::value::Value;

use crate::impala_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};

sqruff_lib_core::dialect_config!(ImpalaDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: ImpalaDialectConfig = config
        .map(ImpalaDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut impala = super::hive::raw_dialect();
    impala.name = DialectKind::Impala;

    for keyword in UNRESERVED_KEYWORDS.lines() {
        let keyword = keyword.trim();
        if !keyword.is_empty() {
            impala.sets_mut("unreserved_keywords").insert(keyword);
        }
    }
    for keyword in RESERVED_KEYWORDS.lines() {
        let keyword = keyword.trim();
        if !keyword.is_empty() {
            impala.sets_mut("reserved_keywords").insert(keyword);
        }
    }

    impala.replace_grammar(
        "DivideSegment",
        one_of(vec![
            StringParser::new("DIV", SyntaxKind::BinaryOperator).to_matchable(),
            StringParser::new("/", SyntaxKind::BinaryOperator).to_matchable(),
        ])
        .to_matchable(),
    );

    impala.add([
        (
            "PoolNameReferenceSegment".into(),
            Ref::new("ObjectReferenceSegment").to_matchable().into(),
        ),
        (
            "ComputeStatsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ComputeStatsStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("COMPUTE").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("STATS").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INCREMENTAL").to_matchable(),
                            Ref::keyword("STATS").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                            Ref::new("PartitionSpecGrammar").optional().to_matchable(),
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
    ]);

    impala.replace_grammar(
        "CreateTableStatementSegment",
        Sequence::new(vec![
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("EXTERNAL").optional().to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::new("IfNotExistsGrammar").optional().to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Bracketed::new(vec![
                Delimited::new(vec![
                    one_of(vec![
                        Ref::new("TableConstraintSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("ColumnDefinitionSegment").to_matchable(),
                            Ref::new("CommentGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| config.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("PARTITIONED").to_matchable(),
                Ref::keyword("BY").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("CommentGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| config.optional())
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SORT").to_matchable(),
                Ref::keyword("BY").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| config.optional())
            .to_matchable(),
            Ref::new("CommentGrammar").optional().to_matchable(),
            Ref::new("RowFormatClauseSegment").optional().to_matchable(),
            Ref::new("SerdePropertiesGrammar").optional().to_matchable(),
            Ref::new("StoredAsGrammar").optional().to_matchable(),
            Ref::new("LocationGrammar").optional().to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("CACHED").to_matchable(),
                    Ref::keyword("IN").to_matchable(),
                    Delimited::new(vec![Ref::new("PoolNameReferenceSegment").to_matchable()])
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("UNCACHED").to_matchable(),
            ])
            .config(|config| config.optional())
            .to_matchable(),
            Ref::new("TablePropertiesGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    let shuffle_hint = || {
        Bracketed::new(vec![
            one_of(vec![
                Ref::keyword("SHUFFLE").to_matchable(),
                Ref::keyword("NOSHUFFLE").to_matchable(),
            ])
            .to_matchable(),
        ])
        .config(|config| {
            config.bracket_type("square");
            config.optional();
        })
        .to_matchable()
    };

    impala.replace_grammar(
        "InsertStatementSegment",
        Sequence::new(vec![
            Ref::keyword("INSERT").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("OVERWRITE").to_matchable(),
                    Ref::keyword("TABLE").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("PartitionSpecGrammar").optional().to_matchable(),
                    shuffle_hint(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("INTO").to_matchable(),
                    Ref::keyword("TABLE").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::new("PartitionSpecGrammar").optional().to_matchable(),
                    shuffle_hint(),
                    one_of(vec![
                        Ref::new("SelectableGrammar").to_matchable(),
                        Ref::new("ValuesClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    let statement_segment = super::ansi::statement_segment().copy(
        Some(vec![
            Ref::new("ComputeStatsStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("AlterDatabaseStatementSegment").to_matchable(),
            Ref::new("MsckRepairTableStatementSegment").to_matchable(),
            Ref::new("MsckTableStatementSegment").to_matchable(),
            Ref::new("SetStatementSegment").to_matchable(),
        ]),
        None,
        None,
        None,
        Vec::new(),
        false,
    );
    impala.replace_grammar("StatementSegment", statement_segment);

    impala
}
