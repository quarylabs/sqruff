//! The Materialize SQL dialect.
//!
//! This is based on PostgreSQL, matching Materialize's origins and SQLFluff's
//! dialect at the revision recorded in `.sqlfluff-sha`.

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::{DialectConfig, DialectKind};
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::one_of;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Ref};
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::MultiStringParser;
use sqruff_lib_core::value::Value;

use crate::materialize_keywords::{RESERVED_KEYWORDS, UNRESERVED_KEYWORDS};

sqruff_lib_core::dialect_config!(MaterializeDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config = config
        .map(MaterializeDialectConfig::from_value)
        .unwrap_or_default();
    raw_dialect().config(|dialect| dialect.expand())
}

fn kw(value: &'static str) -> Matchable {
    Ref::keyword(value).to_matchable()
}
fn r(value: &'static str) -> Matchable {
    Ref::new(value).to_matchable()
}
fn opt(value: &'static str) -> Matchable {
    Ref::new(value).optional().to_matchable()
}
fn seq(items: Vec<Matchable>) -> Matchable {
    Sequence::new(items).to_matchable()
}
fn optional_seq(items: Vec<Matchable>) -> Matchable {
    Sequence::new(items)
        .config(|this| this.optional())
        .to_matchable()
}
fn bracketed_anything() -> Matchable {
    Bracketed::new(vec![Anything::new().to_matchable()]).to_matchable()
}
fn delimited_anything() -> Matchable {
    Delimited::new(vec![Anything::new().to_matchable()]).to_matchable()
}
fn node(kind: SyntaxKind, grammar: Matchable) -> Matchable {
    let mut matcher = NodeMatcher::new(kind, |_| Anything::new().to_matchable());
    matcher.replace(grammar);
    matcher.to_matchable()
}

pub fn raw_dialect() -> Dialect {
    let mut materialize = super::postgres::raw_dialect();
    materialize.name = DialectKind::Materialize;
    materialize.sets_mut("reserved_keywords").clear();
    materialize
        .sets_mut("reserved_keywords")
        .extend(RESERVED_KEYWORDS);
    materialize
        .sets_mut("unreserved_keywords")
        .extend(UNRESERVED_KEYWORDS);
    // Referenced by inherited PostgreSQL grammars during statement matching.
    materialize
        .sets_mut("unreserved_keywords")
        .insert("CONCURRENTLY");
    materialize.sets_mut("materialize_sizes").clear();
    materialize.sets_mut("materialize_sizes").extend([
        "3xsmall", "2xsmall", "xsmall", "small", "medium", "large", "xlarge", "2xlarge", "3xlarge",
        "4xlarge", "5xlarge", "6xlarge",
    ]);

    materialize.add([
        (
            "InstanceSizes".into(),
            one_of(vec![
                MultiStringParser::new(
                    materialize
                        .sets("materialize_sizes")
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    SyntaxKind::MaterializeSize,
                )
                .to_matchable(),
                MultiStringParser::new(
                    materialize
                        .sets("materialize_sizes")
                        .into_iter()
                        .map(|size| format!("'{size}'"))
                        .collect(),
                    SyntaxKind::CompressionType,
                )
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "InCluster".into(),
            seq(vec![kw("IN"), kw("CLUSTER"), r("ObjectReferenceSegment")]).into(),
        ),
        (
            "AlterConnectionRotateKeys".into(),
            node(
                SyntaxKind::AlterConnectionRotateKeys,
                seq(vec![
                    kw("ALTER"),
                    kw("CONNECTION"),
                    opt("IfExistsGrammar"),
                    r("ObjectReferenceSegment"),
                    kw("ROTATE"),
                    kw("KEYS"),
                ]),
            )
            .into(),
        ),
        (
            "AlterRenameStatementSegment".into(),
            node(
                SyntaxKind::AlterRenameStatement,
                seq(vec![
                    kw("ALTER"),
                    one_of(vec![
                        kw("CONNECTION"),
                        kw("INDEX"),
                        kw("SOURCE"),
                        kw("SINK"),
                        kw("VIEW"),
                        seq(vec![kw("MATERIALIZED"), kw("VIEW")]),
                        kw("SECRET"),
                    ])
                    .to_matchable(),
                    r("ObjectReferenceSegment"),
                    kw("RENAME"),
                    kw("TO"),
                    r("ObjectReferenceSegment"),
                ]),
            )
            .into(),
        ),
        (
            "AlterIndexStatementSegment".into(),
            node(
                SyntaxKind::AlterIndexStatement,
                seq(vec![
                    kw("ALTER"),
                    kw("INDEX"),
                    r("ObjectReferenceSegment"),
                    kw("SET"),
                    kw("ENABLED"),
                ]),
            )
            .into(),
        ),
        (
            "AlterSecretStatementSegment".into(),
            node(
                SyntaxKind::AlterSecretStatement,
                seq(vec![
                    kw("ALTER"),
                    kw("SECRET"),
                    opt("IfExistsGrammar"),
                    r("ObjectReferenceSegment"),
                    kw("AS"),
                    Anything::new().to_matchable(),
                ]),
            )
            .into(),
        ),
        (
            "AlterSourceSinkSizeStatementSegment".into(),
            node(
                SyntaxKind::AlterSourceSinkSizeStatement,
                seq(vec![
                    kw("ALTER"),
                    one_of(vec![kw("SOURCE"), kw("SINK")]).to_matchable(),
                    opt("IfExistsGrammar"),
                    r("ObjectReferenceSegment"),
                    kw("SET"),
                    Bracketed::new(vec![kw("SIZE"), r("InstanceSizes")]).to_matchable(),
                ]),
            )
            .into(),
        ),
        (
            "CloseStatementSegment".into(),
            node(
                SyntaxKind::CloseStatement,
                seq(vec![kw("CLOSE"), r("ObjectReferenceSegment")]),
            )
            .into(),
        ),
        (
            "CopyToStatementSegment".into(),
            node(
                SyntaxKind::CopyToStatement,
                seq(vec![
                    kw("COPY"),
                    Bracketed::new(vec![
                        one_of(vec![
                            r("SelectStatementSegment"),
                            seq(vec![kw("SUBSCRIBE"), r("ObjectReferenceSegment")]),
                            seq(vec![kw("VALUES"), delimited_anything()]),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    kw("TO"),
                    kw("STDOUT"),
                    optional_seq(vec![kw("WITH"), bracketed_anything()]),
                ]),
            )
            .into(),
        ),
        (
            "CopyFromStatementSegment".into(),
            node(
                SyntaxKind::CopyFromStatement,
                seq(vec![
                    kw("COPY"),
                    r("ObjectReferenceSegment"),
                    Bracketed::new(vec![Anything::new().to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                    kw("FROM"),
                    kw("STDIN"),
                    optional_seq(vec![
                        Ref::keyword("WITH").optional().to_matchable(),
                        bracketed_anything(),
                    ]),
                ]),
            )
            .into(),
        ),
        (
            "CreateClusterStatementSegment".into(),
            node(
                SyntaxKind::CreateClusterStatement,
                seq(vec![
                    kw("CREATE"),
                    kw("CLUSTER"),
                    r("ObjectReferenceSegment"),
                    optional_seq(vec![
                        kw("REPLICAS"),
                        Bracketed::new(vec![delimited_anything()]).to_matchable(),
                    ]),
                ]),
            )
            .into(),
        ),
        (
            "CreateClusterReplicaStatementSegment".into(),
            node(
                SyntaxKind::CreateClusterReplicaStatement,
                seq(vec![
                    kw("CREATE"),
                    kw("CLUSTER"),
                    kw("REPLICA"),
                    r("ObjectReferenceSegment"),
                    optional_seq(vec![Anything::new().to_matchable()]),
                ]),
            )
            .into(),
        ),
        (
            "CreateConnectionStatementSegment".into(),
            node(
                SyntaxKind::CreateConnectionStatement,
                seq(vec![
                    kw("CREATE"),
                    kw("CONNECTION"),
                    opt("IfNotExistsGrammar"),
                    r("ObjectReferenceSegment"),
                    kw("TO"),
                    one_of(vec![
                        seq(vec![kw("AWS"), kw("PRIVATELINK")]),
                        seq(vec![kw("CONFLUENT"), kw("SCHEMA"), kw("REGISTRY")]),
                        kw("KAFKA"),
                        kw("POSTGRES"),
                        seq(vec![kw("SSH"), kw("TUNNEL")]),
                    ])
                    .to_matchable(),
                    bracketed_anything(),
                ]),
            )
            .into(),
        ),
        (
            "CreateIndexStatementSegment".into(),
            node(SyntaxKind::CreateIndexStatement, create_index()).into(),
        ),
        (
            "CreateMaterializedViewStatementSegment".into(),
            node(
                SyntaxKind::CreateMaterializedViewStatement,
                create_materialized_view(),
            )
            .into(),
        ),
        (
            "CreateSecretStatementSegment".into(),
            node(
                SyntaxKind::CreateSecretStatement,
                seq(vec![
                    kw("CREATE"),
                    kw("SECRET"),
                    opt("IfNotExistsGrammar"),
                    r("ObjectReferenceSegment"),
                    kw("AS"),
                    Anything::new().to_matchable(),
                ]),
            )
            .into(),
        ),
        (
            "CreateSinkKafkaStatementSegment".into(),
            node(SyntaxKind::CreateSinkKafkaStatement, create_sink()).into(),
        ),
        (
            "CreateSourceKafkaStatementSegment".into(),
            node(
                SyntaxKind::CreateSourceKafkaStatement,
                create_source_kafka(),
            )
            .into(),
        ),
        (
            "CreateSourceLoadGeneratorStatementSegment".into(),
            node(
                SyntaxKind::CreateSourceLoadGeneratorStatement,
                create_source_load_generator(),
            )
            .into(),
        ),
        (
            "CreateSourcePostgresStatementSegment".into(),
            node(
                SyntaxKind::CreateSourcePostgresStatement,
                create_source_postgres(),
            )
            .into(),
        ),
        (
            "CreateTypeStatementSegment".into(),
            node(SyntaxKind::CreateTypeStatement, create_type()).into(),
        ),
        (
            "CreateViewStatementSegment".into(),
            node(SyntaxKind::CreateViewStatement, create_view()).into(),
        ),
        (
            "DropStatementSegment".into(),
            node(SyntaxKind::DropStatement, drop_statement()).into(),
        ),
        (
            "ShowStatementSegment".into(),
            node(SyntaxKind::ShowStatement, show_statement()).into(),
        ),
        (
            "ShowCreateStatementSegment".into(),
            node(SyntaxKind::ShowCreateStatement, show_create()).into(),
        ),
        (
            "ShowIndexesStatementSegment".into(),
            node(SyntaxKind::ShowIndexesStatement, show_indexes()).into(),
        ),
        (
            "ShowMaterializedViewsStatementSegment".into(),
            node(
                SyntaxKind::ShowMaterializedViewsStatement,
                seq(vec![
                    kw("SHOW"),
                    kw("MATERIALIZED"),
                    kw("VIEWS"),
                    optional_seq(vec![kw("FROM"), r("ObjectReferenceSegment")]),
                    opt("InCluster"),
                ]),
            )
            .into(),
        ),
        (
            "MaterializeExplainStatementSegment".into(),
            node(SyntaxKind::ExplainStatement, explain_statement()).into(),
        ),
        (
            "FetchStatementSegment".into(),
            node(SyntaxKind::FetchStatement, fetch_statement()).into(),
        ),
        (
            "DeclareStatementSegment".into(),
            node(SyntaxKind::DeclareStatement, declare_statement()).into(),
        ),
    ]);

    let ansi = super::ansi::raw_dialect();
    materialize.replace_grammar(
        "StatementSegment",
        ansi.grammar("StatementSegment")
            .match_grammar(&ansi)
            .unwrap()
            .copy(
                Some(
                    vec![
                        "AlterConnectionRotateKeys",
                        "AlterIndexStatementSegment",
                        "AlterRenameStatementSegment",
                        "AlterSecretStatementSegment",
                        "AlterSourceSinkSizeStatementSegment",
                        "CloseStatementSegment",
                        "CopyToStatementSegment",
                        "CopyFromStatementSegment",
                        "CreateClusterStatementSegment",
                        "CreateClusterReplicaStatementSegment",
                        "CreateConnectionStatementSegment",
                        "CreateIndexStatementSegment",
                        "CreateMaterializedViewStatementSegment",
                        "CreateSecretStatementSegment",
                        "CreateSinkKafkaStatementSegment",
                        "CreateSourceKafkaStatementSegment",
                        "CreateSourceLoadGeneratorStatementSegment",
                        "CreateSourcePostgresStatementSegment",
                        "CreateTypeStatementSegment",
                        "CreateViewStatementSegment",
                        "DropStatementSegment",
                        "FetchStatementSegment",
                        "MaterializeExplainStatementSegment",
                        "ShowStatementSegment",
                        "ShowCreateStatementSegment",
                        "ShowIndexesStatementSegment",
                        "ShowMaterializedViewsStatementSegment",
                        "DeclareStatementSegment",
                    ]
                    .into_iter()
                    .map(|name| Ref::new(name).to_matchable())
                    .collect(),
                ),
                Some(0),
                None,
                None,
                vec![
                    r("CreateIndexStatementSegment"),
                    r("DropIndexStatementSegment"),
                ],
                false,
            ),
    );
    materialize
}

fn create_index() -> Matchable {
    seq(vec![
        kw("CREATE"),
        one_of(vec![
            seq(vec![
                kw("INDEX"),
                r("ObjectReferenceSegment"),
                opt("InCluster"),
                kw("ON"),
                r("ObjectReferenceSegment"),
                optional_seq(vec![kw("USING"), Anything::new().to_matchable()]),
                Bracketed::new(vec![delimited_anything()]).to_matchable(),
            ]),
            seq(vec![
                kw("DEFAULT"),
                kw("INDEX"),
                opt("InCluster"),
                kw("ON"),
                r("ObjectReferenceSegment"),
                optional_seq(vec![kw("USING"), Anything::new().to_matchable()]),
            ]),
        ])
        .to_matchable(),
    ])
}
fn create_materialized_view() -> Matchable {
    let columns = || {
        Bracketed::new(vec![
            Delimited::new(vec![r("ColumnReferenceSegment")]).to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable()
    };
    seq(vec![
        kw("CREATE"),
        one_of(vec![
            seq(vec![
                kw("MATERIALIZED"),
                kw("VIEW"),
                opt("IfNotExistsGrammar"),
                r("ObjectReferenceSegment"),
                columns(),
                opt("InCluster"),
                kw("AS"),
                Anything::new().to_matchable(),
            ]),
            seq(vec![
                r("OrReplaceGrammar"),
                kw("MATERIALIZED"),
                kw("VIEW"),
                r("ObjectReferenceSegment"),
                columns(),
                opt("InCluster"),
                kw("AS"),
                Anything::new().to_matchable(),
            ]),
        ])
        .to_matchable(),
    ])
}
fn with_anything() -> Matchable {
    optional_seq(vec![
        kw("WITH"),
        Bracketed::new(vec![delimited_anything()]).to_matchable(),
    ])
}
fn create_sink() -> Matchable {
    seq(vec![
        kw("CREATE"),
        kw("SINK"),
        opt("IfNotExistsGrammar"),
        r("ObjectReferenceSegment"),
        kw("FROM"),
        r("ObjectReferenceSegment"),
        kw("INTO"),
        Anything::new().to_matchable(),
        optional_seq(vec![
            kw("KEY"),
            Bracketed::new(vec![
                Delimited::new(vec![r("ColumnReferenceSegment")]).to_matchable(),
            ])
            .to_matchable(),
        ]),
        optional_seq(vec![kw("FORMAT"), Anything::new().to_matchable()]),
        optional_seq(vec![
            kw("ENVELOPE"),
            one_of(vec![kw("DEBEZIUM"), kw("UPSERT")]).to_matchable(),
        ]),
        with_anything(),
    ])
}
fn create_source_kafka() -> Matchable {
    seq(vec![
        kw("CREATE"),
        kw("SOURCE"),
        opt("IfNotExistsGrammar"),
        r("ObjectReferenceSegment"),
        Bracketed::new(vec![
            Delimited::new(vec![r("ColumnReferenceSegment")]).to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
        kw("FROM"),
        kw("KAFKA"),
        kw("CONNECTION"),
        r("ObjectReferenceSegment"),
        Bracketed::new(vec![delimited_anything()]).to_matchable(),
        optional_seq(vec![
            kw("KEY"),
            kw("FORMAT"),
            Anything::new().to_matchable(),
            kw("VALUE"),
            kw("FORMAT"),
            Anything::new().to_matchable(),
        ]),
        optional_seq(vec![kw("FORMAT"), Anything::new().to_matchable()]),
        optional_seq(vec![kw("INCLUDE"), delimited_anything()]),
        optional_seq(vec![
            kw("ENVELOPE"),
            one_of(vec![kw("NONE"), kw("DEBEZIUM"), kw("UPSERT")]).to_matchable(),
        ]),
        with_anything(),
    ])
}
fn table_selection() -> Matchable {
    one_of(vec![
        seq(vec![kw("FOR"), kw("ALL"), kw("TABLES")]),
        seq(vec![
            kw("FOR"),
            kw("TABLES"),
            Bracketed::new(vec![delimited_anything()]).to_matchable(),
        ]),
    ])
    .config(|this| this.optional())
    .to_matchable()
}
fn create_source_load_generator() -> Matchable {
    seq(vec![
        kw("CREATE"),
        kw("SOURCE"),
        opt("IfNotExistsGrammar"),
        r("ObjectReferenceSegment"),
        kw("FROM"),
        kw("LOAD"),
        kw("GENERATOR"),
        one_of(vec![kw("AUCTION"), kw("COUNTER"), kw("TPCH")]).to_matchable(),
        Bracketed::new(vec![delimited_anything()])
            .config(|this| this.optional())
            .to_matchable(),
        table_selection(),
        with_anything(),
    ])
}
fn create_source_postgres() -> Matchable {
    seq(vec![
        kw("CREATE"),
        kw("SOURCE"),
        opt("IfNotExistsGrammar"),
        r("ObjectReferenceSegment"),
        optional_seq(vec![
            kw("FROM"),
            kw("POSTGRES"),
            kw("CONNECTION"),
            r("ObjectReferenceSegment"),
            Bracketed::new(vec![delimited_anything()]).to_matchable(),
        ]),
        table_selection(),
        with_anything(),
    ])
}
fn create_type() -> Matchable {
    seq(vec![
        kw("CREATE"),
        kw("TYPE"),
        r("ObjectReferenceSegment"),
        one_of(vec![
            seq(vec![
                kw("AS"),
                Bracketed::new(vec![
                    Delimited::new(vec![seq(vec![
                        r("ObjectReferenceSegment"),
                        r("DatatypeSegment"),
                    ])])
                    .to_matchable(),
                ])
                .to_matchable(),
            ]),
            seq(vec![
                kw("AS"),
                one_of(vec![kw("LIST"), kw("MAP")]).to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![seq(vec![
                        r("ObjectReferenceSegment"),
                        r("EqualsSegment"),
                        Anything::new().to_matchable(),
                    ])])
                    .to_matchable(),
                ])
                .to_matchable(),
            ]),
        ])
        .to_matchable(),
    ])
}
fn create_view() -> Matchable {
    seq(vec![
        kw("CREATE"),
        one_of(vec![kw("TEMP"), kw("TEMPORARY")])
            .config(|this| this.optional())
            .to_matchable(),
        kw("VIEW"),
        opt("IfNotExistsGrammar"),
        r("ObjectReferenceSegment"),
        Bracketed::new(vec![
            Delimited::new(vec![r("ColumnReferenceSegment")]).to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
        kw("AS"),
        r("SelectableGrammar"),
    ])
}
fn drop_statement() -> Matchable {
    seq(vec![
        kw("DROP"),
        one_of(vec![
            kw("CONNECTION"),
            kw("CLUSTER"),
            seq(vec![kw("CLUSTER"), kw("REPLICA")]),
            kw("DATABASE"),
            kw("INDEX"),
            seq(vec![kw("MATERIALIZED"), kw("VIEW")]),
            kw("ROLE"),
            kw("SECRET"),
            kw("SCHEMA"),
            kw("SINK"),
            kw("SOURCE"),
            kw("TABLE"),
            kw("TYPE"),
            kw("VIEW"),
            kw("USER"),
        ])
        .to_matchable(),
        opt("IfExistsGrammar"),
        r("ObjectReferenceSegment"),
        one_of(vec![kw("CASCADE"), kw("RESTRICT")])
            .config(|this| this.optional())
            .to_matchable(),
    ])
}
fn like_where() -> Matchable {
    one_of(vec![
        seq(vec![kw("LIKE"), r("QuotedLiteralSegment")]),
        seq(vec![kw("WHERE"), r("ExpressionSegment")]),
    ])
    .config(|this| this.optional())
    .to_matchable()
}
fn show_statement() -> Matchable {
    seq(vec![
        kw("SHOW"),
        one_of(vec![
            kw("COLUMNS"),
            kw("CONNECTIONS"),
            kw("CLUSTERS"),
            seq(vec![kw("CLUSTER"), kw("REPLICAS")]),
            kw("DATABASES"),
            kw("INDEXES"),
            seq(vec![kw("MATERIALIZED"), kw("VIEWS")]),
            kw("SECRETS"),
            kw("SCHEMAS"),
            kw("SINKS"),
            kw("SOURCES"),
            kw("TABLES"),
            kw("TYPES"),
            kw("VIEWS"),
            kw("OBJECTS"),
        ])
        .to_matchable(),
        opt("ObjectReferenceSegment"),
        optional_seq(vec![kw("FROM"), r("ObjectReferenceSegment")]),
        like_where(),
    ])
}
fn show_create() -> Matchable {
    seq(vec![
        kw("SHOW"),
        kw("CREATE"),
        one_of(vec![
            kw("CONNECTION"),
            kw("INDEX"),
            seq(vec![kw("MATERIALIZED"), kw("VIEW")]),
            kw("SINK"),
            kw("SOURCE"),
            kw("TABLE"),
            kw("VIEW"),
        ])
        .config(|this| this.optional())
        .to_matchable(),
        r("ObjectReferenceSegment"),
    ])
}
fn show_indexes() -> Matchable {
    seq(vec![
        kw("SHOW"),
        kw("INDEXES"),
        optional_seq(vec![kw("ON"), r("ObjectReferenceSegment")]),
        optional_seq(vec![kw("FROM"), r("ObjectReferenceSegment")]),
        opt("InCluster"),
        like_where(),
    ])
}
fn explain_statement() -> Matchable {
    seq(vec![
        kw("EXPLAIN"),
        optional_seq(vec![
            one_of(vec![
                kw("RAW"),
                kw("DECORRELATED"),
                kw("OPTIMIZED"),
                kw("PHYSICAL"),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::keyword("PLAN").optional().to_matchable(),
        ]),
        with_anything(),
        optional_seq(vec![
            kw("AS"),
            one_of(vec![kw("TEXT"), kw("JSON")]).to_matchable(),
        ]),
        Ref::keyword("FOR").optional().to_matchable(),
        one_of(vec![
            r("SelectableGrammar"),
            seq(vec![kw("VIEW"), r("ObjectReferenceSegment")]),
            seq(vec![
                kw("MATERIALIZED"),
                kw("VIEW"),
                r("ObjectReferenceSegment"),
            ]),
            Anything::new().to_matchable(),
        ])
        .to_matchable(),
    ])
}
fn fetch_statement() -> Matchable {
    seq(vec![
        kw("FETCH"),
        Ref::keyword("FORWARD").optional().to_matchable(),
        one_of(vec![kw("ALL"), r("NumericLiteralSegment")])
            .config(|this| this.optional())
            .to_matchable(),
        Ref::keyword("FROM").optional().to_matchable(),
        r("ObjectReferenceSegment"),
        with_anything(),
    ])
}
fn declare_statement() -> Matchable {
    seq(vec![
        kw("DECLARE"),
        r("ObjectReferenceSegment"),
        kw("CURSOR"),
        optional_seq(vec![kw("WITHOUT"), kw("HOLD")]),
        kw("FOR"),
        one_of(vec![
            r("SelectableGrammar"),
            seq(vec![kw("VIEW"), r("ObjectReferenceSegment")]),
            seq(vec![
                kw("MATERIALIZED"),
                kw("VIEW"),
                r("ObjectReferenceSegment"),
            ]),
            Anything::new().to_matchable(),
        ])
        .to_matchable(),
    ])
}
