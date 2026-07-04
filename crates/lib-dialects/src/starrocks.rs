use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectConfig, DialectKind};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::StringParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::value::Value;

use super::mysql;
use crate::starrocks_keywords::{STARROCKS_RESERVED_KEYWORDS, STARROCKS_UNRESERVED_KEYWORDS};

sqruff_lib_core::dialect_config!(StarRocksDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: StarRocksDialectConfig = config
        .map(StarRocksDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut starrocks = mysql::raw_dialect();
    starrocks.name = DialectKind::Starrocks;

    for kw in STARROCKS_UNRESERVED_KEYWORDS.lines() {
        let kw = kw.trim();
        if !kw.is_empty() {
            starrocks.sets_mut("unreserved_keywords").insert(kw);
        }
    }

    starrocks.sets_mut("reserved_keywords").clear();
    for kw in STARROCKS_RESERVED_KEYWORDS.lines() {
        let kw = kw.trim();
        if !kw.is_empty() {
            starrocks.sets_mut("reserved_keywords").insert(kw);
        }
    }

    starrocks.sets_mut("engine_types").extend([
        "OLAP",
        "MYSQL",
        "ELASTICSEARCH",
        "HIVE",
        "HUDI",
        "ICEBERG",
        "JDBC",
    ]);

    starrocks.add([
        (
            "EngineTypeSegment".into(),
            one_of(vec![
                StringParser::new("OLAP", SyntaxKind::EngineType).to_matchable(),
                StringParser::new("MYSQL", SyntaxKind::EngineType).to_matchable(),
                StringParser::new("ELASTICSEARCH", SyntaxKind::EngineType).to_matchable(),
                StringParser::new("HIVE", SyntaxKind::EngineType).to_matchable(),
                StringParser::new("HUDI", SyntaxKind::EngineType).to_matchable(),
                StringParser::new("ICEBERG", SyntaxKind::EngineType).to_matchable(),
                StringParser::new("JDBC", SyntaxKind::EngineType).to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "IndexDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexDefinition, |_| {
                Sequence::new(vec![
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::new("IndexReferenceSegment").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Ref::keyword("BITMAP").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("COMMENT").to_matchable(),
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PartitionSegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("RANGE").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Sequence::new(vec![
                                            Ref::keyword("PARTITION").to_matchable(),
                                            Ref::new("ObjectReferenceSegment").to_matchable(),
                                            Ref::keyword("VALUES").to_matchable(),
                                            one_of(vec![
                                                Sequence::new(vec![
                                                    Ref::keyword("LESS").to_matchable(),
                                                    Ref::keyword("THAN").to_matchable(),
                                                    one_of(vec![
                                                        Ref::keyword("MAXVALUE").to_matchable(),
                                                        Bracketed::new(vec![
                                                            Delimited::new(vec![
                                                                Ref::new("LiteralGrammar")
                                                                    .to_matchable(),
                                                            ])
                                                            .to_matchable(),
                                                        ])
                                                        .to_matchable(),
                                                    ])
                                                    .to_matchable(),
                                                ])
                                                .to_matchable(),
                                                Bracketed::new(vec![
                                                    Bracketed::new(vec![
                                                        Delimited::new(vec![
                                                            Ref::new("LiteralGrammar")
                                                                .to_matchable(),
                                                        ])
                                                        .to_matchable(),
                                                    ])
                                                    .to_matchable(),
                                                    Ref::new("CommaSegment").to_matchable(),
                                                    Bracketed::new(vec![
                                                        Delimited::new(vec![
                                                            Ref::new("LiteralGrammar")
                                                                .to_matchable(),
                                                        ])
                                                        .to_matchable(),
                                                    ])
                                                    .to_matchable(),
                                                ])
                                                .to_matchable(),
                                            ])
                                            .to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Bracketed::new(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("START").to_matchable(),
                                        Bracketed::new(vec![
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::keyword("END").to_matchable(),
                                        Bracketed::new(vec![
                                            Ref::new("QuotedLiteralSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::keyword("EVERY").to_matchable(),
                                        Bracketed::new(vec![
                                            one_of(vec![
                                                Ref::new("QuotedLiteralSegment").to_matchable(),
                                                Sequence::new(vec![
                                                    Ref::keyword("INTERVAL").to_matchable(),
                                                    Ref::new("NumericLiteralSegment")
                                                        .to_matchable(),
                                                    one_of(vec![
                                                        Ref::keyword("YEAR").to_matchable(),
                                                        Ref::keyword("MONTH").to_matchable(),
                                                        Ref::keyword("DAY").to_matchable(),
                                                        Ref::keyword("HOUR").to_matchable(),
                                                    ])
                                                    .to_matchable(),
                                                ])
                                                .to_matchable(),
                                            ])
                                            .to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("FunctionSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
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
            "DistributionSegment".into(),
            NodeMatcher::new(SyntaxKind::DistributionSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DISTRIBUTED").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("HASH").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("BUCKETS").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("RANDOM").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("BUCKETS").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .config(|this| this.optional())
                            .to_matchable(),
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
            "QualifyClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::QualifyClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("QUALIFY").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Sequence::new(vec![
                        Ref::new("FunctionSegment").to_matchable(),
                        Ref::new("ComparisonOperatorGrammar").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateRoutineLoadPropertiesSegment".into(),
            NodeMatcher::new(SyntaxKind::RoutineLoadProperties, |_| {
                Sequence::new(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "CreateRoutineLoadDataSourcePropertiesSegment".into(),
            NodeMatcher::new(SyntaxKind::RoutineLoadDataSourceProperties, |_| {
                Sequence::new(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    starrocks.replace_grammar(
        "ColumnConstraintSegment",
        Sequence::new(vec![
            Sequence::new(vec![
                Ref::keyword("CONSTRAINT").to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("NOT").optional().to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CHECK").to_matchable(),
                    Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("DEFAULT").to_matchable(),
                    Ref::new("ColumnConstraintDefaultGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("PrimaryKeyGrammar").to_matchable(),
                Ref::new("UniqueKeyGrammar").to_matchable(),
                Ref::new("AutoIncrementGrammar").to_matchable(),
                Ref::new("ReferenceDefinitionGrammar").to_matchable(),
                Ref::new("CommentClauseSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COLLATE").to_matchable(),
                    Ref::new("CollationReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CHARACTER").to_matchable(),
                    Ref::keyword("SET").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("COLLATE").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    starrocks.replace_grammar(
        "CreateTableStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::new("OrReplaceGrammar").optional().to_matchable(),
                Ref::keyword("EXTERNAL").optional().to_matchable(),
                Ref::keyword("TEMPORARY").optional().to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("TableConstraintSegment").to_matchable(),
                                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                                    Ref::new("IndexDefinitionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ENGINE").to_matchable(),
                            Ref::new("EqualsSegment").to_matchable(),
                            Ref::new("EngineTypeSegment").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Sequence::new(vec![
                                    Ref::keyword("AGGREGATE").to_matchable(),
                                    Ref::keyword("KEY").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("UNIQUE").to_matchable(),
                                    Ref::keyword("KEY").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("PRIMARY").to_matchable(),
                                    Ref::keyword("KEY").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("DUPLICATE").to_matchable(),
                                    Ref::keyword("KEY").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("CommentClauseSegment").optional().to_matchable(),
                        Ref::new("PartitionSegment").optional().to_matchable(),
                        Ref::new("DistributionSegment").optional().to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ORDER").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("PROPERTIES").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Sequence::new(vec![
                                        Ref::new("QuotedLiteralSegment").to_matchable(),
                                        Ref::new("EqualsSegment").to_matchable(),
                                        Ref::new("QuotedLiteralSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("LIKE").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("AS").optional().to_matchable(),
                        optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    starrocks.add([
        (
            "CreateRoutineLoadStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateRoutineLoadStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("ROUTINE").to_matchable(),
                    Ref::keyword("LOAD").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("COLUMNS").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("QuotedIdentifierSegment").to_matchable(),
                                    Ref::new("NakedIdentifierSegment").to_matchable(),
                                    Sequence::new(vec![
                                        one_of(vec![
                                            Ref::new("QuotedIdentifierSegment").to_matchable(),
                                            Ref::new("NakedIdentifierSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::new("EqualsSegment").to_matchable(),
                                        Ref::new("ExpressionSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PROPERTIES").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("CreateRoutineLoadPropertiesSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("FROM").to_matchable(),
                    Ref::keyword("KAFKA").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Ref::new("CreateRoutineLoadDataSourcePropertiesSegment").to_matchable(),
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
            "StopRoutineLoadStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::StopRoutineLoadStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("STOP").to_matchable(),
                    Ref::keyword("ROUTINE").to_matchable(),
                    Ref::keyword("LOAD").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PauseRoutineLoadStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PauseRoutineLoadStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("PAUSE").to_matchable(),
                    Ref::keyword("ROUTINE").to_matchable(),
                    Ref::keyword("LOAD").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ResumeRoutineLoadStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ResumeRoutineLoadStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("RESUME").to_matchable(),
                    Ref::keyword("ROUTINE").to_matchable(),
                    Ref::keyword("LOAD").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "InsertOverwriteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertOverwriteStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("INSERT").to_matchable(),
                    Ref::keyword("OVERWRITE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PARTITION").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("ExpressionSegment").to_matchable(),
                                    Ref::new("NakedIdentifierSegment").to_matchable(),
                                    Ref::new("ObjectReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    starrocks.replace_grammar(
        "StatementSegment",
        starrocks
            .grammar("StatementSegment")
            .match_grammar(&starrocks)
            .unwrap()
            .copy(
                Some(vec![
                    Ref::new("CreateRoutineLoadStatementSegment").to_matchable(),
                    Ref::new("StopRoutineLoadStatementSegment").to_matchable(),
                    Ref::new("PauseRoutineLoadStatementSegment").to_matchable(),
                    Ref::new("ResumeRoutineLoadStatementSegment").to_matchable(),
                    Ref::new("InsertOverwriteStatementSegment").to_matchable(),
                ]),
                Some(0),
                None,
                None,
                Vec::new(),
                false,
            ),
    );

    starrocks.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::new("IntoClauseSegment").optional().to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("SelectPartitionClauseSegment")
                .optional()
                .to_matchable(),
            Ref::new("IndexHintClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("OverlapsClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
            Ref::new("QualifyClauseSegment").optional().to_matchable(),
            Ref::new("ForClauseSegment").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::new("OrderByClauseSegment").to_matchable(),
            Ref::new("LimitClauseSegment").to_matchable(),
            Ref::new("IntoClauseSegment").to_matchable(),
            Ref::new("ForClauseSegment").to_matchable(),
            Ref::new("IndexHintClauseSegment").to_matchable(),
            Ref::new("SelectPartitionClauseSegment").to_matchable(),
            Ref::new("UpsertClauseListSegment").to_matchable(),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    starrocks.replace_grammar(
        "SelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::new("IntoClauseSegment").optional().to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("SelectPartitionClauseSegment")
                .optional()
                .to_matchable(),
            Ref::new("IndexHintClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("OverlapsClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
            Ref::new("QualifyClauseSegment").optional().to_matchable(),
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
            Ref::new("IntoClauseSegment").optional().to_matchable(),
            Ref::new("ForClauseSegment").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::new("UpsertClauseListSegment").to_matchable(),
            Ref::new("WithCheckOptionSegment").to_matchable(),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    starrocks
}
