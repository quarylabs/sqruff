//! The Greenplum dialect.
//!
//! Greenplum (https://greenplum.org/) is a massively parallel Postgres, so this
//! dialect is based on Postgres and adds the `DISTRIBUTED` clause to `CREATE TABLE`.

use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
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

pub fn raw_dialect() -> Dialect {
    let mut greenplum = super::postgres::raw_dialect();
    greenplum.name = DialectKind::Greenplum;

    // Greenplum-specific reserved keywords used by the DISTRIBUTED clause.
    greenplum
        .sets_mut("reserved_keywords")
        .extend(["DISTRIBUTED", "RANDOMLY", "REPLICATED"]);

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
                            Ref::keyword("HASH").to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![
                            AnyNumberOf::new(vec![
                                Delimited::new(vec![
                                    Sequence::new(vec![
                                        one_of(vec![
                                            Ref::new("ColumnReferenceSegment").to_matchable(),
                                            Ref::new("FunctionSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        AnyNumberOf::new(vec![
                                            Sequence::new(vec![
                                                Ref::keyword("COLLATE").to_matchable(),
                                                Ref::new("CollationReferenceSegment")
                                                    .to_matchable(),
                                            ])
                                            .config(|this| this.optional())
                                            .to_matchable(),
                                            Ref::new("ParameterNameSegment")
                                                .optional()
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
                    Sequence::new(vec![
                        Ref::keyword("DISTRIBUTED").to_matchable(),
                        one_of(vec![
                            Ref::keyword("RANDOMLY").to_matchable(),
                            Ref::keyword("REPLICATED").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("BY").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("ColumnReferenceSegment").to_matchable(),
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
            .to_matchable()
        })
        .to_matchable(),
    );

    greenplum
}
