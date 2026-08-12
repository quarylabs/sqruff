//! Teradata dialect, ported from SQLFluff at the revision in `.sqlfluff-sha`.

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectConfig, DialectKind};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::{DialectElementType, ParseMode};
use sqruff_lib_core::value::Value;

use crate::ansi;

sqruff_lib_core::dialect_config!(TeradataDialectConfig {});

fn kw(keyword: &'static str) -> Matchable {
    Ref::keyword(keyword).to_matchable()
}

fn optional_sequence(elements: Vec<Matchable>) -> Matchable {
    Sequence::new(elements)
        .config(|this| this.optional())
        .to_matchable()
}

trait OptionalMatchable {
    fn optional(self) -> Matchable;
}

impl OptionalMatchable for Matchable {
    fn optional(self) -> Matchable {
        Sequence::new(vec![self])
            .config(|this| this.optional())
            .to_matchable()
    }
}

impl OptionalMatchable for Anything {
    fn optional(self) -> Matchable {
        Sequence::new(vec![self.to_matchable()])
            .config(|this| this.optional())
            .to_matchable()
    }
}

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: TeradataDialectConfig = config
        .map(TeradataDialectConfig::from_value)
        .unwrap_or_default();
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut dialect = ansi::raw_dialect();
    dialect.name = DialectKind::Teradata;

    dialect.patch_lexer_matchers(vec![Matcher::regex(
        "numeric_literal",
        r"([0-9]+(\.[0-9]*)?)",
        SyntaxKind::NumericLiteral,
    )]);

    dialect.sets_mut("unreserved_keywords").remove("UNION");
    dialect.sets_mut("unreserved_keywords").remove("TIMESTAMP");
    dialect.sets_mut("unreserved_keywords").extend([
        "AUTOINCREMENT",
        "ACTIVITYCOUNT",
        "CASESPECIFIC",
        "CS",
        "DAYS",
        "DEL",
        "DUAL",
        "EQ",
        "ERRORCODE",
        "EXPORT",
        "FALLBACK",
        "FORMAT",
        "GE",
        "GT",
        "HASH",
        "IMPORT",
        "JOURNAL",
        "LABEL",
        "LE",
        "LT",
        "LOGON",
        "LOGOFF",
        "MACRO",
        "MAXINTERVALS",
        "MAXVALUELENGTH",
        "MEETS",
        "MERGEBLOCKRATIO",
        "NONE",
        "NE",
        "PERCENT",
        "PROFILE",
        "PROTECTION",
        "QUERY_BAND",
        "QUIT",
        "RUN",
        "SAMPLE",
        "SEL",
        "SS",
        "STAT",
        "STATS",
        "STATISTICS",
        "SUMMARY",
        "THRESHOLD",
        "UC",
        "UPPERCASE",
    ]);
    dialect
        .sets_mut("reserved_keywords")
        .extend(["UNION", "TIMESTAMP"]);
    dialect.sets_mut("bare_functions").insert("DATE");

    add_operators(&mut dialect);
    add_segments(&mut dialect);
    replace_core_grammars(&mut dialect);
    dialect
}

fn add_operators(dialect: &mut Dialect) {
    for (name, keyword) in [
        ("EqualsSegment_a", "EQ"),
        ("GreaterThanSegment_a", "GT"),
        ("LessThanSegment_a", "LT"),
        ("GreaterThanOrEqualToSegment_a", "GE"),
        ("LessThanOrEqualToSegment_a", "LE"),
        ("NotEqualToSegment_a", "NE"),
    ] {
        dialect.add([(name.into(), kw(keyword).into())]);
    }
    dialect.add([
        (
            "NotEqualToSegment_b".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec![kw("NOT"), Ref::new("RawEqualsSegment").to_matchable()])
                    .config(|this| this.disallow_gaps())
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NotEqualToSegment_c".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec![
                    Ref::new("BitwiseXorSegment").to_matchable(),
                    Ref::new("RawEqualsSegment").to_matchable(),
                ])
                .config(|this| this.disallow_gaps())
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);
    dialect.replace_grammar(
        "ComparisonOperatorGrammar",
        one_of(vec![
            Ref::new("EqualsSegment").to_matchable(),
            Ref::new("EqualsSegment_a").to_matchable(),
            Ref::new("GreaterThanSegment").to_matchable(),
            Ref::new("GreaterThanSegment_a").to_matchable(),
            Ref::new("LessThanSegment").to_matchable(),
            Ref::new("LessThanSegment_a").to_matchable(),
            Ref::new("GreaterThanOrEqualToSegment").to_matchable(),
            Ref::new("GreaterThanOrEqualToSegment_a").to_matchable(),
            Ref::new("LessThanOrEqualToSegment").to_matchable(),
            Ref::new("LessThanOrEqualToSegment_a").to_matchable(),
            Ref::new("NotEqualToSegment").to_matchable(),
            Ref::new("NotEqualToSegment_a").to_matchable(),
            Ref::new("NotEqualToSegment_b").to_matchable(),
            Ref::new("NotEqualToSegment_c").to_matchable(),
            Ref::new("LikeOperatorSegment").to_matchable(),
            Sequence::new(vec![kw("IS"), kw("DISTINCT"), kw("FROM")]).to_matchable(),
            Sequence::new(vec![kw("IS"), kw("NOT"), kw("DISTINCT"), kw("FROM")]).to_matchable(),
        ])
        .to_matchable(),
    );
}

fn add_segments(dialect: &mut Dialect) {
    dialect.add([
        (
            "BteqKeyWordSegment".into(),
            NodeMatcher::new(SyntaxKind::BteqKeyWordSegment, |_| {
                Sequence::new(vec![
                    Ref::new("DotSegment").optional().to_matchable(),
                    one_of(
                        vec![
                            "IF",
                            "THEN",
                            "LOGON",
                            "ACTIVITYCOUNT",
                            "ERRORCODE",
                            "DATABASE",
                            "LABEL",
                            "GOTO",
                            "LOGOFF",
                            "IMPORT",
                            "EXPORT",
                            "RUN",
                            "QUIT",
                        ]
                        .into_iter()
                        .map(kw)
                        .collect(),
                    )
                    .to_matchable(),
                    Ref::new("LiteralGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BteqStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::BteqStatement, |_| {
                Sequence::new(vec![
                    Ref::new("DotSegment").to_matchable(),
                    Ref::new("BteqKeyWordSegment").to_matchable(),
                    AnyNumberOf::new(vec![
                        Ref::new("BteqKeyWordSegment").to_matchable(),
                        optional_sequence(vec![
                            Ref::new("ComparisonOperatorGrammar").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
                        ]),
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
            "TdCollectStatUsingOptionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::CollectStatUsingOptionClause, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            kw("SAMPLE"),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            kw("PERCENT"),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            kw("SYSTEM"),
                            kw("THRESHOLD"),
                            one_of(vec![kw("PERCENT"), kw("DAYS")])
                                .config(|this| this.optional())
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![kw("SYSTEM"), kw("SAMPLE")]).to_matchable(),
                        Sequence::new(vec![
                            kw("THRESHOLD"),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            one_of(vec![kw("PERCENT"), kw("DAYS")]).to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            kw("NO"),
                            kw("THRESHOLD"),
                            one_of(vec![kw("PERCENT"), kw("DAYS")])
                                .config(|this| this.optional())
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![kw("NO"), kw("SAMPLE")]).to_matchable(),
                        Sequence::new(vec![
                            kw("MAXINTERVALS"),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![kw("SYSTEM"), kw("MAXINTERVALS")]).to_matchable(),
                        Sequence::new(vec![
                            kw("MAXVALUELENGTH"),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![kw("SYSTEM"), kw("MAXVALUELENGTH")]).to_matchable(),
                        kw("SAMPLE"),
                    ])
                    .to_matchable(),
                    optional_sequence(vec![kw("FOR"), kw("CURRENT")]),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TdOrderByStatClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OrderbyClause, |_| {
                Sequence::new(vec![
                    kw("ORDER"),
                    kw("BY"),
                    one_of(vec![kw("VALUES"), kw("HASH")]).to_matchable(),
                    Bracketed::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TdCollectStatisticsStatementSegment".into(),
            collect_statistics_segment(),
        ),
        ("TdCommentStatementSegment".into(), comment_segment()),
        (
            "TdRenameStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RenameTableStatement, |_| {
                Sequence::new(vec![
                    kw("RENAME"),
                    kw("TABLE"),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    one_of(vec![kw("TO"), kw("AS")]).to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TeradataCastSegment".into(),
            NodeMatcher::new(SyntaxKind::CastExpression, |_| {
                Bracketed::new(vec![Ref::new("DatatypeSegment").to_matchable()]).to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TdColumnConstraintSegment".into(),
            column_constraint_segment(),
        ),
        ("TdCreateTableOptions".into(), create_table_options()),
        ("TdTablePartitioningLevel".into(), partitioning_level()),
        ("TdTableConstraints".into(), table_constraints()),
        (
            "FromUpdateClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::FromInUpdateClause, |_| {
                Sequence::new(vec![
                    kw("FROM"),
                    Ref::new("FromExpressionSegment").to_matchable(),
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
                    kw("QUALIFY"),
                    MetaSegment::indent().to_matchable(),
                    optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseStatement, |_| {
                Sequence::new(vec![
                    kw("DATABASE"),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "SetSessionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetSessionStatement, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![kw("SET"), kw("SESSION")]).to_matchable(),
                        kw("SS"),
                    ])
                    .to_matchable(),
                    Ref::new("DatabaseStatementSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        ("SetQueryBandStatementSegment".into(), query_band_segment()),
    ]);
}

fn collect_statistics_segment() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::CollectStatisticsStatement, |_| {
        Sequence::new(vec![
            kw("COLLECT"),
            kw("SUMMARY").optional(),
            one_of(vec![kw("STAT"), kw("STATS"), kw("STATISTICS")]).to_matchable(),
            optional_sequence(vec![
                kw("USING"),
                Delimited::new(vec![
                    Ref::new("TdCollectStatUsingOptionClauseSegment").to_matchable(),
                ])
                .config(|this| this.delimiter(Ref::keyword("AND")))
                .to_matchable(),
            ]),
            Delimited::new(vec![
                one_of(vec![
                    Sequence::new(vec![
                        kw("UNIQUE").optional(),
                        kw("INDEX"),
                        Ref::new("IndexReferenceSegment").optional().to_matchable(),
                        kw("ALL").optional(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("TdOrderByStatClauseSegment")
                            .optional()
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        kw("UNIQUE").optional(),
                        kw("INDEX"),
                        Ref::new("IndexReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        kw("COLUMN"),
                        optionally_bracketed(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    kw("PARTITION"),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        optional_sequence(vec![
                            kw("AS").optional(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ]),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            kw("ON"),
            kw("TEMPORARY").optional(),
            Ref::new("TableReferenceSegment").to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn comment_segment() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::CommentClause, |_| {
        Sequence::new(vec![
            kw("COMMENT"),
            kw("ON").optional(),
            one_of(vec![
                Sequence::new(vec![
                    kw("COLUMN"),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("FUNCTION"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("MACRO"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("MAP"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("METHOD"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("PROCEDURE"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("PROFILE"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("ROLE"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("TRIGGER"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("TYPE"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("VIEW"),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("DATABASE"),
                    Ref::new("DatabaseReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("FILE"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("TABLE"),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    kw("USER"),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            optional_sequence(vec![
                one_of(vec![kw("AS"), kw("IS")])
                    .config(|this| this.optional())
                    .to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ]),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn column_constraint_segment() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::TdColumnAttributeConstraint, |_| {
        one_of(vec![
            Sequence::new(vec![
                kw("CHARACTER"),
                kw("SET"),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                kw("NOT").optional(),
                one_of(vec![kw("CASESPECIFIC"), kw("CS")]).to_matchable(),
            ])
            .to_matchable(),
            one_of(vec![kw("UPPERCASE"), kw("UC")]).to_matchable(),
            Sequence::new(vec![
                kw("COMPRESS"),
                one_of(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("LiteralGrammar").to_matchable(),
                    kw("NULL"),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn create_table_options() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::CreateTableOptionsStatement, |_| {
        Sequence::new(vec![
            Ref::new("CommaSegment").to_matchable(),
            Delimited::new(vec![
                one_of(vec![
                    Sequence::new(vec![
                        kw("NO").optional(),
                        kw("FALLBACK"),
                        kw("PROTECTION").optional(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            kw("NO"),
                            kw("DUAL"),
                            kw("LOCAL"),
                            Sequence::new(vec![kw("NOT"), kw("LOCAL")]).to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        one_of(vec![kw("BEFORE"), kw("AFTER")])
                            .config(|this| this.optional())
                            .to_matchable(),
                        kw("JOURNAL"),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        kw("CHECKSUM"),
                        Ref::new("EqualsSegment").to_matchable(),
                        one_of(vec![kw("ON"), kw("OFF"), kw("DEFAULT")]).to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![kw("DEFAULT"), kw("NO")]).to_matchable(),
                        kw("MERGEBLOCKRATIO"),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        kw("MERGEBLOCKRATIO"),
                        Ref::new("EqualsSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        kw("PERCENT").optional(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn partitioning_level() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::TdPartitioningLevel, |_| {
        one_of(vec![
            Sequence::new(vec![
                Ref::new("FunctionNameSegment").to_matchable(),
                Bracketed::new(vec![Anything::new().optional()]).to_matchable(),
            ])
            .to_matchable(),
            Bracketed::new(vec![
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::new("FunctionNameSegment").to_matchable(),
                        Bracketed::new(vec![Anything::new().optional()]).to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn table_constraints() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::TdTableConstraint, |_| {
        AnyNumberOf::new(vec![
            one_of(vec![
                Sequence::new(vec![
                    kw("UNIQUE").optional(),
                    kw("PRIMARY"),
                    kw("INDEX"),
                    Ref::new("ObjectReferenceSegment").optional().to_matchable(),
                    one_of(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![kw("NO"), kw("PRIMARY"), kw("INDEX")]).to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                kw("PARTITION"),
                kw("BY"),
                Ref::new("TdTablePartitioningLevel").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                kw("UNIQUE").optional(),
                kw("INDEX"),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                kw("ALL").optional(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![kw("WITH"), kw("NO").optional(), kw("DATA")]).to_matchable(),
            optional_sequence(vec![
                kw("AND"),
                kw("NO").optional(),
                one_of(vec![kw("STAT"), kw("STATS"), kw("STATISTICS")]).to_matchable(),
            ]),
            Sequence::new(vec![
                kw("ON"),
                kw("COMMIT"),
                one_of(vec![kw("PRESERVE"), kw("DELETE")]).to_matchable(),
                kw("ROWS"),
            ])
            .to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn query_band_segment() -> DialectElementType {
    NodeMatcher::new(SyntaxKind::SetQueryBandStatement, |_| {
        Sequence::new(vec![
            kw("SET"),
            kw("QUERY_BAND"),
            Ref::new("EqualsSegment").to_matchable(),
            one_of(vec![
                Ref::new("QuotedLiteralSegment").to_matchable(),
                kw("NONE"),
            ])
            .to_matchable(),
            kw("UPDATE").optional(),
            kw("FOR"),
            one_of(vec![
                Sequence::new(vec![kw("SESSION"), kw("VOLATILE").optional()]).to_matchable(),
                kw("TRANSACTION"),
            ])
            .to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
    .into()
}

fn replace_core_grammars(dialect: &mut Dialect) {
    let from_clause_terminator = dialect.grammar("FromClauseTerminatorGrammar");
    dialect.replace_grammar(
        "FromClauseTerminatorGrammar",
        from_clause_terminator.copy(
            Some(vec![kw("QUALIFY")]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );
    dialect.replace_grammar(
        "DatatypeSegment",
        NodeMatcher::new(SyntaxKind::DataType, |_| {
            Sequence::new(vec![
                Ref::new("DatatypeIdentifierSegment").to_matchable(),
                Ref::new("BracketedArguments").optional().to_matchable(),
                Bracketed::new(vec![
                    one_of(vec![
                        Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                optional_sequence(vec![
                    kw("FORMAT"),
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                ]),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );
    dialect.replace_grammar(
        "ExpressionSegment",
        NodeMatcher::new(SyntaxKind::Expression, |_| {
            Sequence::new(vec![
                Ref::new("Expression_A_Grammar").to_matchable(),
                Ref::new("TeradataCastSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );
    dialect.replace_grammar(
        "ColumnDefinitionSegment",
        NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
            Sequence::new(vec![
                Ref::new("ColumnReferenceSegment").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
                Bracketed::new(vec![Anything::new().to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::new("ColumnConstraintSegment")
                        .optional()
                        .to_matchable(),
                    Ref::new("TdColumnConstraintSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );
    dialect.replace_grammar("CreateTableStatementSegment", create_table_statement());
    dialect.replace_grammar(
        "DeleteStatementSegment",
        Sequence::new(vec![
            one_of(vec![kw("DELETE"), kw("DEL")]).to_matchable(),
            Ref::new("FromClauseSegment").to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );
    dialect.replace_grammar("UpdateStatementSegment", update_statement());
    dialect.replace_grammar(
        "SelectClauseSegment",
        NodeMatcher::new(SyntaxKind::SelectClause, |_| ansi::select_clause_segment())
            .to_matchable(),
    );
    dialect.replace_grammar(
        "SelectClauseSegment",
        Sequence::new(vec![
            one_of(vec![kw("SELECT"), kw("SEL")]).to_matchable(),
            Ref::new("SelectClauseModifierSegment")
                .optional()
                .to_matchable(),
            MetaSegment::indent().to_matchable(),
            Delimited::new(vec![Ref::new("SelectClauseElementSegment").to_matchable()])
                .config(|this| this.allow_trailing())
                .to_matchable(),
        ])
        .terminators(vec![
            kw("FROM"),
            kw("WHERE"),
            Sequence::new(vec![kw("ORDER"), kw("BY")]).to_matchable(),
            kw("LIMIT"),
            Ref::new("SetOperatorSegment").to_matchable(),
        ])
        .config(|this| this.parse_mode(ParseMode::GreedyOnceStarted))
        .to_matchable(),
    );
    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        NodeMatcher::new(SyntaxKind::SelectClauseModifier, |_| select_modifier()).to_matchable(),
    );
    dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::select_statement().copy(
            Some(vec![
                Ref::new("QualifyClauseSegment").optional().to_matchable(),
            ]),
            None,
            Some(Ref::new("OrderByClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        ),
    );
    dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec![
                Ref::new("QualifyClauseSegment").optional().to_matchable(),
            ]),
            None,
            Some(Ref::new("OverlapsClauseSegment").optional().to_matchable()),
            None,
            Vec::new(),
            false,
        ),
    );
    dialect.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec![
                Ref::new("TdCollectStatisticsStatementSegment").to_matchable(),
                Ref::new("BteqStatementSegment").to_matchable(),
                Ref::new("TdRenameStatementSegment").to_matchable(),
                Ref::new("QualifyClauseSegment").to_matchable(),
                Ref::new("TdCommentStatementSegment").to_matchable(),
                Ref::new("DatabaseStatementSegment").to_matchable(),
                Ref::new("SetSessionStatementSegment").to_matchable(),
                Ref::new("SetQueryBandStatementSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ),
    );
}

fn create_table_statement() -> Matchable {
    NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
        Sequence::new(vec![
            kw("CREATE"),
            optional_sequence(vec![kw("OR"), kw("REPLACE")]),
            one_of(vec![kw("SET"), kw("MULTISET")])
                .config(|this| this.optional())
                .to_matchable(),
            one_of(vec![
                Sequence::new(vec![kw("GLOBAL"), kw("TEMPORARY")]).to_matchable(),
                kw("VOLATILE"),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            kw("TABLE"),
            optional_sequence(vec![kw("IF"), kw("NOT"), kw("EXISTS")]),
            Ref::new("TableReferenceSegment").to_matchable(),
            Ref::new("TdCreateTableOptions").optional().to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                                Ref::new("TableConstraintSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("CommentClauseSegment").optional().to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![kw("AS"), Ref::new("SelectableGrammar").to_matchable()])
                    .to_matchable(),
                Sequence::new(vec![
                    kw("LIKE"),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("TdTableConstraints").optional().to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
}

fn update_statement() -> Matchable {
    NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
        Sequence::new(vec![
            kw("UPDATE"),
            one_of(vec![
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("FromUpdateClauseSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("FromUpdateClauseSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("SetClauseListSegment").to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
        ])
        .to_matchable()
    })
    .to_matchable()
}

fn select_modifier() -> Matchable {
    one_of(vec![
        kw("DISTINCT"),
        kw("ALL"),
        Sequence::new(vec![
            kw("TOP"),
            Ref::new("ExpressionSegment").to_matchable(),
            kw("PERCENT").optional(),
            optional_sequence(vec![kw("WITH"), kw("TIES")]),
        ])
        .to_matchable(),
        Sequence::new(vec![
            kw("NORMALIZE"),
            one_of(vec![
                Sequence::new(vec![kw("ON"), kw("MEETS"), kw("OR"), kw("OVERLAPS")]).to_matchable(),
                Sequence::new(vec![kw("ON"), kw("OVERLAPS")]).to_matchable(),
                Sequence::new(vec![kw("ON"), kw("OVERLAPS"), kw("OR"), kw("MEETS")]).to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable()
}
