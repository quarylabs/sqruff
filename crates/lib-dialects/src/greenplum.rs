//! The Greenplum dialect.
//!
//! Greenplum (https://greenplum.org/) is a massively parallel Postgres, so this
//! dialect is based on Postgres and adds the `DISTRIBUTED` clause to `CREATE TABLE`.

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, any_set_of, one_of};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Ref};
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::RegexParser;

use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(GreenplumDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: GreenplumDialectConfig = config
        .map(GreenplumDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|this| this.expand())
}

fn copy_literal_option(keyword: &'static str) -> Matchable {
    Sequence::new(vec![
        Ref::keyword(keyword).to_matchable(),
        Ref::keyword("AS").optional().to_matchable(),
        Ref::new("QuotedLiteralSegment").to_matchable(),
    ])
    .to_matchable()
}

fn copy_column_list_option(keyword: &'static str) -> Matchable {
    Sequence::new(vec![
        Ref::keyword(keyword).to_matchable(),
        Bracketed::new(vec![
            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()]).to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable()
}

pub fn raw_dialect() -> Dialect {
    let mut greenplum = super::postgres::raw_dialect();
    greenplum.name = DialectKind::Greenplum;

    // Greenplum-specific keywords from the Greenplum 6 keyword table.
    greenplum
        .sets_mut("reserved_keywords")
        .extend(["DECODE", "DISTRIBUTED", "LOG", "SCATTER"]);
    greenplum.sets_mut("unreserved_keywords").extend([
        "ACTIVE",
        "CONCURRENCY",
        "CONTAINS",
        "CPU_RATE_LIMIT",
        "CPUSET",
        "CREATEEXTTABLE",
        "CUBE",
        "DENY",
        "DXL",
        "ERRORS",
        "EVERY",
        "EXCHANGE",
        "EXPAND",
        "FIELDS",
        "FILL",
        "FORMAT",
        "FULLSCAN",
        "GROUP_ID",
        "GROUPING",
        "HASH",
        "HOST",
        "IGNORE",
        "INCLUSIVE",
        "LIST",
        "MASTER",
        "MEDIAN",
        "MEMORY_LIMIT",
        "MEMORY_SHARED_QUOTA",
        "MEMORY_SPILL_RATIO",
        "MISSING",
        "MODIFIES",
        "NEWLINE",
        "NOCREATEEXTTABLE",
        "NOOVERCOMMIT",
        "ORDERED",
        "OTHERS",
        "OVERCOMMIT",
        "PARTITIONS",
        "PERCENT",
        "PROTOCOL",
        "QUEUE",
        "RANDOMLY",
        "READABLE",
        "READS",
        "REJECT",
        "REPLICATED",
        "RESOURCE",
        "RETRIEVE",
        "ROLLUP",
        "ROOTPARTITION",
        "SEGMENT",
        "SEGMENTS",
        "SETS",
        "SPLIT",
        "SQL",
        "SUBPARTITION",
        "THRESHOLD",
        "TIES",
        "VALIDATION",
        "WEB",
        "WRITABLE",
    ]);

    // Greenplum storage option values can be literals OR bare identifiers, including
    // reserved words (e.g. `compresstype = zstd`, `orientation = column`).
    greenplum.add([(
        "GreenplumTableOptionValueGrammar".into(),
        one_of(vec![
            Ref::new("LiteralGrammar").to_matchable(),
            Ref::new("QuotedIdentifierSegment").to_matchable(),
            RegexParser::new(
                "[A-Za-z_][A-Za-z0-9_]*",
                SyntaxKind::PropertiesNakedIdentifier,
            )
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    greenplum.add([(
        "DistributedBySegment".into(),
        NodeMatcher::new(SyntaxKind::DistributedBy, |_| {
            Sequence::new(vec![
                Ref::keyword("DISTRIBUTED").to_matchable(),
                one_of(vec![
                    Ref::keyword("RANDOMLY").to_matchable(),
                    Ref::keyword("REPLICATED").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BY").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
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
        .into(),
    )]);

    // Override `CREATE TABLE` to add the Greenplum `DISTRIBUTED` clause.
    // https://docs.vmware.com/en/VMware-Tanzu-Greenplum/6/greenplum-database/GUID-ref_guide-sql_commands-CREATE_TABLE.html
    greenplum.replace_grammar(
        "CreateTableStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("GLOBAL").to_matchable(),
                            Ref::keyword("LOCAL").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::new("TemporaryGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("UNLOGGED").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                one_of(vec![
                    // Columns and comment syntax
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Sequence::new(vec![
                                        Ref::new("ColumnReferenceSegment").to_matchable(),
                                        Ref::new("DatatypeSegment").to_matchable(),
                                        AnyNumberOf::new(vec![
                                            one_of(vec![
                                                Ref::new("ColumnConstraintSegment").to_matchable(),
                                                Sequence::new(vec![
                                                    Ref::keyword("COLLATE").to_matchable(),
                                                    Ref::new("CollationReferenceSegment")
                                                        .to_matchable(),
                                                ])
                                                .to_matchable(),
                                            ])
                                            .to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("TableConstraintSegment").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("LIKE").to_matchable(),
                                        Ref::new("TableReferenceSegment").to_matchable(),
                                        AnyNumberOf::new(vec![
                                            Ref::new("LikeOptionSegment").to_matchable(),
                                        ])
                                        .config(|this| this.optional())
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INHERITS").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("TableReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // Create OF syntax
                    Sequence::new(vec![
                        Ref::keyword("OF").to_matchable(),
                        Ref::new("ParameterNameSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("WITH").to_matchable(),
                                        Ref::keyword("OPTIONS").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                    AnyNumberOf::new(vec![
                                        Ref::new("ColumnConstraintSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("TableConstraintSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // Create PARTITION OF syntax
                    Sequence::new(vec![
                        Ref::keyword("PARTITION").to_matchable(),
                        Ref::keyword("OF").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("WITH").to_matchable(),
                                        Ref::keyword("OPTIONS").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                    AnyNumberOf::new(vec![
                                        Ref::new("ColumnConstraintSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("TableConstraintSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("FOR").to_matchable(),
                                Ref::keyword("VALUES").to_matchable(),
                                Ref::new("PartitionBoundSpecSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("DEFAULT").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("PARTITION").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        one_of(vec![
                            Ref::keyword("RANGE").to_matchable(),
                            Ref::keyword("LIST").to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("SUBPARTITION").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("RANGE").to_matchable(),
                                    Ref::keyword("LIST").to_matchable(),
                                ])
                                .to_matchable(),
                                Bracketed::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("SUBPARTITION").to_matchable(),
                                    Ref::keyword("TEMPLATE").to_matchable(),
                                    Bracketed::new(vec![Anything::new().to_matchable()])
                                        .to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![Anything::new().to_matchable()]).to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Ref::new("ParameterNameSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Sequence::new(vec![
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::new("EqualsSegment").to_matchable(),
                                        Ref::new("GreenplumTableOptionValueGrammar").to_matchable(),
                                    ])
                                    .config(|this| this.optional())
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ON").to_matchable(),
                        Ref::keyword("COMMIT").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("PRESERVE").to_matchable(),
                                Ref::keyword("ROWS").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("DELETE").to_matchable(),
                                Ref::keyword("ROWS").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("DROP").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TABLESPACE").to_matchable(),
                        Ref::new("TablespaceReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DistributedBySegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    greenplum.replace_grammar(
        "AnalyzeStatementSegment",
        NodeMatcher::new(SyntaxKind::AnalyzeStatement, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("ANALYZE").to_matchable(),
                    Ref::keyword("ANALYSE").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("VERBOSE").optional().to_matchable(),
                Ref::keyword("ROOTPARTITION").optional().to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .config(|this| this.allow_trailing())
                                .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("ALL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    greenplum.add([
        (
            "FetchStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::FetchStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("FETCH").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("FIRST").to_matchable(),
                            Ref::keyword("NEXT").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("ABSOLUTE").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("RELATIVE").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
                            Ref::keyword("FORWARD").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FORWARD").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("FORWARD").to_matchable(),
                                Ref::keyword("ALL").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("FROM").to_matchable(),
                            Ref::keyword("IN").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DeclareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DECLARE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    any_set_of(vec![
                        Ref::keyword("BINARY").to_matchable(),
                        Ref::keyword("INSENSITIVE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NO").to_matchable(),
                            Ref::keyword("SCROLL").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("PARALLEL").to_matchable(),
                            Ref::keyword("RETRIEVE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("CURSOR").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Ref::keyword("WITHOUT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("HOLD").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("SelectableGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("READ").to_matchable(),
                        Ref::keyword("ONLY").to_matchable(),
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
            "CloseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CloseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CLOSE").to_matchable(),
                    one_of(vec![
                        Ref::new("TableReferenceSegment").to_matchable(),
                        Ref::keyword("ALL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    let copy_target = one_of(vec![
        Ref::new("QuotedLiteralSegment").to_matchable(),
        Sequence::new(vec![
            Ref::keyword("PROGRAM").to_matchable(),
            Ref::new("QuotedLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
    ]);
    let copy_table = Sequence::new(vec![
        Ref::new("TableReferenceSegment").to_matchable(),
        Bracketed::new(vec![
            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                .config(|this| this.allow_trailing())
                .to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable(),
    ]);
    let copy_option = any_set_of(vec![
        Sequence::new(vec![
            Ref::keyword("FORMAT").to_matchable(),
            Ref::new("SingleIdentifierGrammar").to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            Ref::keyword("ON").to_matchable(),
            Ref::keyword("SEGMENT").to_matchable(),
        ])
        .to_matchable(),
        Ref::keyword("BINARY").to_matchable(),
        Sequence::new(vec![
            Ref::keyword("OIDS").to_matchable(),
            Ref::new("BooleanLiteralGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            Ref::keyword("FREEZE").to_matchable(),
            Ref::new("BooleanLiteralGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
        copy_literal_option("DELIMITER"),
        copy_literal_option("NULL"),
        Sequence::new(vec![
            Ref::keyword("HEADER").to_matchable(),
            Ref::new("BooleanLiteralGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
        copy_literal_option("QUOTE"),
        copy_literal_option("ESCAPE"),
        copy_literal_option("NEWLINE"),
        Sequence::new(vec![
            Ref::keyword("FORCE_QUOTE").to_matchable(),
            one_of(vec![
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("StarSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
        copy_column_list_option("FORCE_NOT_NULL"),
        copy_column_list_option("FORCE_NULL"),
        Sequence::new(vec![
            Ref::keyword("ENCODING").to_matchable(),
            Ref::new("QuotedLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            Ref::keyword("FILL").to_matchable(),
            Ref::keyword("MISSING").to_matchable(),
            Ref::keyword("FIELDS").to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            Ref::keyword("LOG").to_matchable(),
            Ref::keyword("ERRORS").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SEGMENT").to_matchable(),
                Ref::keyword("REJECT").to_matchable(),
                Ref::keyword("LIMIT").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("ROWS").to_matchable(),
                    Ref::keyword("PERCENT").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
        ])
        .to_matchable(),
        Sequence::new(vec![
            Ref::keyword("CSV").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("QUOTE").to_matchable(),
                Ref::keyword("AS").optional().to_matchable(),
                Ref::new("QuotedLiteralSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("FORCE").to_matchable(),
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("FORCE").to_matchable(),
                    Ref::keyword("QUOTE").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
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
            Ref::keyword("IGNORE").to_matchable(),
            Ref::keyword("EXTERNAL").to_matchable(),
            Ref::keyword("PARTITIONS").to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable();
    let bracketed_copy_option = Bracketed::new(vec![copy_option.clone()]).to_matchable();

    let copy_grammar = Sequence::new(vec![
        Ref::keyword("COPY").to_matchable(),
        one_of(vec![
            Sequence::new(vec![
                copy_table.clone().to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                one_of(vec![
                    copy_target.clone().to_matchable(),
                    Ref::keyword("STDIN").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("WITH").optional().to_matchable(),
                one_of(vec![copy_option.clone(), bracketed_copy_option.clone()])
                    .config(|this| this.optional())
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("SEGMENT").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                one_of(vec![
                    copy_table.clone().to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("UnorderedSelectStatementSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("TO").to_matchable(),
                one_of(vec![
                    copy_target.clone().to_matchable(),
                    Ref::keyword("STDOUT").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("WITH").optional().to_matchable(),
                one_of(vec![copy_option.clone(), bracketed_copy_option.clone()])
                    .config(|this| this.optional())
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("SEGMENT").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    ])
    .to_matchable();
    let mut copy_matcher = NodeMatcher::new(SyntaxKind::CopyStatement, |_| {
        Anything::new().to_matchable()
    });
    copy_matcher.replace(copy_grammar);
    greenplum.replace_grammar("CopyStatementSegment", copy_matcher.to_matchable());

    let statement = greenplum
        .grammar("StatementSegment")
        .match_grammar(&greenplum)
        .unwrap()
        .copy(
            Some(
                [
                    "FetchStatementSegment",
                    "DeclareStatementSegment",
                    "CloseStatementSegment",
                ]
                .into_iter()
                .map(|name| Ref::new(name).to_matchable())
                .collect(),
            ),
            Some(0),
            None,
            None,
            vec![],
            false,
        );
    greenplum.replace_grammar("StatementSegment", statement);

    greenplum.replace_grammar(
        "CreateTableAsStatementSegment",
        greenplum
            .grammar("CreateTableAsStatementSegment")
            .match_grammar(&greenplum)
            .unwrap()
            .copy(
                Some(vec![
                    Ref::new("DistributedBySegment").optional().to_matchable(),
                ]),
                None,
                None,
                None,
                vec![],
                false,
            ),
    );

    for grammar_name in ["UnorderedSelectStatementSegment", "SelectStatementSegment"] {
        greenplum.replace_grammar(
            grammar_name,
            greenplum
                .grammar(grammar_name)
                .match_grammar(&greenplum)
                .unwrap()
                .copy(
                    None,
                    None,
                    None,
                    None,
                    vec![Ref::new("DistributedBySegment").to_matchable()],
                    false,
                ),
        );
    }

    greenplum.replace_grammar(
        "SelectClauseSegment",
        greenplum
            .grammar("SelectClauseSegment")
            .match_grammar(&greenplum)
            .unwrap()
            .copy(
                None,
                None,
                None,
                None,
                vec![Ref::new("DistributedBySegment").to_matchable()],
                false,
            ),
    );

    greenplum
}
