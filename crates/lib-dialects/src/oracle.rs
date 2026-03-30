use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Nothing, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::ansi;
use sqruff_lib_core::dialects::init::DialectConfig;
use sqruff_lib_core::value::Value;

sqruff_lib_core::dialect_config!(OracleDialectConfig {});

pub fn dialect(config: Option<&Value>) -> Dialect {
    let _dialect_config: OracleDialectConfig = config
        .map(OracleDialectConfig::from_value)
        .unwrap_or_default();

    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    let mut oracle = ansi::raw_dialect();
    oracle.name = DialectKind::Oracle;

    // ---- Keywords ----
    oracle.sets_mut("reserved_keywords").extend([
        "ACCESS",
        "ADD",
        "AUDIT",
        "CLUSTER",
        "COLUMN_VALUE",
        "COMMENT",
        "COMPRESS",
        "CONNECT",
        "CONNECT_BY_ROOT",
        "DEFINITION",
        "DELETING",
        "DISABLE",
        "ENABLE",
        "EXCLUSIVE",
        "EXECUTE",
        "FILE",
        "FORCE",
        "IDENTIFIED",
        "IMMEDIATE",
        "INCREMENT",
        "INDEXTYPE",
        "INITIAL",
        "INSERTING",
        "INVISIBLE",
        "LEVEL",
        "LOCK",
        "LOGGING",
        "LONG",
        "LOOP",
        "MAXEXTENTS",
        "MINUS",
        "MLSLABEL",
        "MODE",
        "MODIFY",
        "MONITORING",
        "NESTED_TABLE_ID",
        "NOAUDIT",
        "NOCOMPRESS",
        "NOLOGGING",
        "NOMONITORING",
        "NOREVERSE",
        "NOWAIT",
        "NUMBER",
        "OFFLINE",
        "ONLINE",
        "OPTION",
        "OVERFLOW",
        "PARAMETERS",
        "PCTFREE",
        "PIVOT",
        "PRIOR",
        "PRIVATE",
        "PROMPT",
        "PUBLIC",
        "RAW",
        "REBUILD",
        "RENAME",
        "RESOURCE",
        "REVERSE",
        "ROWID",
        "ROWNUM",
        "SESSION",
        "SHARE",
        "SIBLINGS",
        "SIZE",
        "SMALLINT",
        "START",
        "SUCCESSFUL",
        "SYNONYM",
        "SYSDATE",
        "UID",
        "UNPIVOT",
        "UNUSABLE",
        "UPDATING",
        "VALIDATE",
        "VARCHAR2",
        "VISIBLE",
        "WHENEVER",
    ]);

    oracle.sets_mut("unreserved_keywords").extend([
        "ABSENT",
        "ACCESSIBLE",
        "ACTIVE",
        "ADMINISTER",
        "ADVISE",
        "ADVISOR",
        "ANALYTIC",
        "ARCHIVE",
        "ARCHIVAL",
        "AUTHENTICATED",
        "AUTHID",
        "BECOME",
        "BODY",
        "BULK",
        "COMMITTED",
        "CONSTRAINTS",
        "BULK_EXCEPTIONS",
        "BULK_ROWCOUNT",
        "BYTE",
        "COLLECT",
        "COMPILE",
        "COMPOUND",
        "CONSTANT",
        "CONTAINER",
        "CONTEXT",
        "CROSSEDITION",
        "CURSOR",
        "DBA_RECYCLEBIN",
        "DBTIMEZONE",
        "DDL",
        "DEBUG",
        "DEFERRED",
        "DELEGATE",
        "DIGEST",
        "DIMENSION",
        "DIRECTIVE",
        "DIRECTORIES",
        "DIRECTORY",
        "DML",
        "EDITION",
        "EDITIONABLE",
        "EDITIONING",
        "EDITIONS",
        "ELSIF",
        "EMPTY",
        "ERROR",
        "ERRORS",
        "EXEMPT",
        "EXPIRE",
        "EXTERNALLY",
        "FINE",
        "FLASHBACK",
        "FOLLOWS",
        "FORALL",
        "GLOBALLY",
        "GUARD",
        "HIERARCHY",
        "HTTP",
        "INDICES",
        "INHERITANY",
        "ISOLATION_LEVEL",
        "ISOPEN",
        "JAVA",
        "JOB",
        "KEEP",
        "LIBRARY",
        "LINK",
        "LOCKDOWN",
        "LOG",
        "LOGMINING",
        "MEASURE",
        "MINING",
        "MUTABLE",
        "NESTED",
        "NEXTVAL",
        "NOCOPY",
        "NOMAXVALUE",
        "NOMINVALUE",
        "NONEDITIONABLE",
        "NOTHING",
        "NOTFOUND",
        "OID",
        "OUTLINE",
        "PACKAGE",
        "PAIRS",
        "PARALLEL",
        "PARALLEL_ENABLE",
        "PARENT",
        "PERSISTABLE",
        "PIPELINED",
        "PLUGGABLE",
        "POLYMORPHIC",
        "PRAGMA",
        "PRECEDES",
        "PRIVILEGE",
        "PROFILE",
        "PROGRAM",
        "PROPERTY",
        "QUERY",
        "QUOTA",
        "RAISE",
        "RECORD",
        "REDACTION",
        "REDEFINE",
        "REFRESH",
        "REJECT",
        "RELIES_ON",
        "REMOTE",
        "RESTRICTED",
        "RESULT_CACHE",
        "RESUMABLE",
        "RETURNING",
        "REUSE",
        "REWRITE",
        "ROWTYPE",
        "SCHEDULER",
        "SERIALIZABLE",
        "SERVICE",
        "SHARD",
        "SHARD_ENABLE",
        "SYNC",
        "SHARED",
        "SHARING",
        "SIGN",
        "SPECIFICATION",
        "SQL_MACRO",
        "SYSGUID",
        "TIME_ZONE",
        "TIMEOUT",
        "UNLIMITED",
        "VARRAY",
        "VISIBILITY",
    ]);

    oracle.sets_mut("bare_functions").clear();
    oracle.sets_mut("bare_functions").extend([
        "COLUMN_VALUE",
        "CURRENT_DATE",
        "CURRENT_TIMESTAMP",
        "DBTIMEZONE",
        "LOCALTIMESTAMP",
        "SESSIONTIMESTAMP",
        "SYSDATE",
        "SYSTIMESTAMP",
    ]);

    // ---- Lexer ----
    // SQLFluff: RegexLexer("word", r"[\p{L}][\p{L}\p{N}_$#]*", WordSegment)
    // sqruff doesn't support Unicode categories, so we use ASCII approximation
    // SQLFluff: numeric_literal regex prevents 1. from consuming dot when followed by another dot
    // This is critical for FOR i IN 1..5 LOOP syntax
    oracle.patch_lexer_matchers(vec![
        Matcher::regex("word", r"[a-zA-Z_][a-zA-Z0-9_$#]*", SyntaxKind::Word),
        Matcher::legacy(
            "numeric_literal",
            |s| s.starts_with(|ch: char| ch.is_ascii_digit() || ch == '.'),
            r"(?>\d+\.\d+|\d+\.(?![\.\w])|\d+)(\.?[eE][+-]?\d+)?((?<!\.)|(?=\b))",
            SyntaxKind::NumericLiteral,
        ),
        Matcher::regex(
            "single_quote",
            r"'([^'\\]|\\|\\.|'')*'",
            SyntaxKind::SingleQuote,
        ),
        Matcher::regex("double_quote", r#""([^"]|"")*""#, SyntaxKind::DoubleQuote),
    ]);

    oracle.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "prompt_command",
                r"PROMPT[^\r\n]*",
                SyntaxKind::InlineComment,
            ),
            Matcher::string("at_sign", "@", SyntaxKind::AtSign),
        ],
        "word",
    );

    oracle.insert_lexer_matchers(
        vec![
            Matcher::string("right_arrow", "=>", SyntaxKind::RightArrow),
            Matcher::string(
                "assignment_operator",
                ":=",
                SyntaxKind::OracleAssignmentOperator,
            ),
        ],
        "equals",
    );

    oracle.insert_lexer_matchers(
        vec![Matcher::string(
            "power_operator",
            "**",
            SyntaxKind::OraclePowerOperator,
        )],
        "star",
    );

    // ---- NakedIdentifierSegment override ----
    // SQLFluff: r"[\p{L}\p{N}_]*[\p{L}][\p{L}\p{N}_#$]*" with reserved keywords anti_template
    // Allows # and $ in identifiers for Oracle
    use itertools::Itertools;
    oracle.add([(
        "NakedIdentifierSegment".into(),
        SegmentGenerator::new(|dialect| {
            let reserved_keywords = dialect.sets("reserved_keywords");
            let pattern = reserved_keywords.iter().join("|");
            let anti_template = format!("^({pattern})$");

            RegexParser::new(r"[A-Z0-9_]*[A-Z][A-Z0-9_#$]*", SyntaxKind::NakedIdentifier)
                .anti_template(&anti_template)
                .to_matchable()
        })
        .into(),
    )]);

    // ---- Grammar additions ----
    oracle.add([
        // AtSignSegment
        (
            "AtSignSegment".into(),
            StringParser::new("@", SyntaxKind::AtSign)
                .to_matchable()
                .into(),
        ),
        // RightArrowSegment
        (
            "RightArrowSegment".into(),
            StringParser::new("=>", SyntaxKind::RightArrow)
                .to_matchable()
                .into(),
        ),
        // AssignmentOperatorSegment
        (
            "AssignmentOperatorSegment".into(),
            StringParser::new(":=", SyntaxKind::OracleAssignmentOperator)
                .to_matchable()
                .into(),
        ),
        // PowerOperatorSegment
        (
            "PowerOperatorSegment".into(),
            StringParser::new("**", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        // ModOperatorSegment
        (
            "ModOperatorSegment".into(),
            StringParser::new("MOD", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into(),
        ),
        // SequenceNextValGrammar
        (
            "SequenceNextValGrammar".into(),
            Sequence::new(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                Ref::new("DotSegment").to_matchable(),
                Ref::keyword("NEXTVAL").to_matchable(),
            ])
            .config(|config| {
                config.allow_gaps = false;
            })
            .to_matchable()
            .into(),
        ),
        // OnCommitGrammar
        (
            "OnCommitGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("COMMIT").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("DROP").to_matchable(),
                            Ref::keyword("PRESERVE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("DEFINITION").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("DELETE").to_matchable(),
                            Ref::keyword("PRESERVE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("ROWS").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // ConnectByRootGrammar
        (
            "ConnectByRootGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("CONNECT_BY_ROOT").to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // IntervalUnitsGrammar
        (
            "IntervalUnitsGrammar".into(),
            one_of(vec![
                Ref::keyword("YEAR").to_matchable(),
                Ref::keyword("MONTH").to_matchable(),
                Ref::keyword("DAY").to_matchable(),
                Ref::keyword("HOUR").to_matchable(),
                Ref::keyword("MINUTE").to_matchable(),
                Ref::keyword("SECOND").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // PivotForInGrammar
        (
            "PivotForInGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                optionally_bracketed(vec![
                    Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("IN").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("Expression_D_Grammar").to_matchable(),
                            Ref::new("AliasExpressionSegment").optional().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // UnpivotNullsGrammar
        (
            "UnpivotNullsGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("INCLUDE").to_matchable(),
                    Ref::keyword("EXCLUDE").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("NULLS").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // StatementAndDelimiterGrammar
        (
            "StatementAndDelimiterGrammar".into(),
            Sequence::new(vec![
                Ref::new("StatementSegment").to_matchable(),
                Ref::new("DelimiterGrammar").optional().to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // OneOrMoreStatementsGrammar
        (
            "OneOrMoreStatementsGrammar".into(),
            AnyNumberOf::new(vec![
                Ref::new("StatementAndDelimiterGrammar").to_matchable(),
            ])
            .config(|config| {
                config.min_times = 1;
            })
            .to_matchable()
            .into(),
        ),
        // TimingPointGrammar
        (
            "TimingPointGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("BEFORE").to_matchable(),
                    Ref::keyword("AFTER").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("INSTEAD").to_matchable(),
                        Ref::keyword("OF").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("STATEMENT").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EACH").to_matchable(),
                        Ref::keyword("ROW").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // SharingClauseGrammar
        (
            "SharingClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("SHARING").to_matchable(),
                one_of(vec![
                    Ref::keyword("METADATA").to_matchable(),
                    Ref::keyword("NONE").to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable()
            .into(),
        ),
        // DefaultCollationClauseGrammar
        (
            "DefaultCollationClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("DEFAULT").to_matchable(),
                Ref::keyword("COLLATION").to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(),
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable()
            .into(),
        ),
        // InvokerRightsClauseGrammar
        (
            "InvokerRightsClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("AUTHID").to_matchable(),
                one_of(vec![
                    Ref::keyword("CURRENT_USER").to_matchable(),
                    Ref::keyword("DEFINER").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // AccessibleByClauseGrammar
        (
            "AccessibleByClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("ACCESSIBLE").to_matchable(),
                Ref::keyword("BY").to_matchable(),
                Delimited::new(vec![
                    Bracketed::new(vec![
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("FUNCTION").to_matchable(),
                                Ref::keyword("PROCEDURE").to_matchable(),
                                Ref::keyword("PACKAGE").to_matchable(),
                                Ref::keyword("TRIGGER").to_matchable(),
                                Ref::keyword("TYPE").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Ref::new("FunctionNameSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // DmlGrammar
        (
            "DmlGrammar".into(),
            one_of(vec![
                Ref::keyword("DELETE").to_matchable(),
                Ref::keyword("INSERT").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("UPDATE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OF").to_matchable(),
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // IterationBoundsGrammar
        (
            "IterationBoundsGrammar".into(),
            one_of(vec![
                Ref::new("NumericLiteralSegment").to_matchable(),
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("DotSegment").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // IterationSteppedControlGrammar
        (
            "IterationSteppedControlGrammar".into(),
            Sequence::new(vec![
                Ref::new("IterationBoundsGrammar").to_matchable(),
                Ref::new("DotSegment").to_matchable(),
                Ref::new("DotSegment").to_matchable(),
                Ref::new("IterationBoundsGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("BY").to_matchable(),
                    Ref::keyword("STEP").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // ParallelEnableClauseGrammar
        (
            "ParallelEnableClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("PARALLEL_ENABLE").to_matchable(),
                Bracketed::new(vec![
                    Ref::keyword("PARTITION").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ANY").to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("HASH").to_matchable(),
                                Ref::keyword("RANGE").to_matchable(),
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
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("VALUE").to_matchable(),
                            Bracketed::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // ResultCacheClauseGrammar
        (
            "ResultCacheClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("RESULT_CACHE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("RELIES_ON").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // PipelinedClauseGrammar
        (
            "PipelinedClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("PIPELINED").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ROW").to_matchable(),
                            Ref::keyword("TABLE").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("POLYMORPHIC").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("USING").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // CompileClauseGrammar
        (
            "CompileClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("COMPILE").to_matchable(),
                Ref::keyword("DEBUG").optional().to_matchable(),
                one_of(vec![
                    Ref::keyword("PACKAGE").to_matchable(),
                    Ref::keyword("SPECIFICATION").to_matchable(),
                    Ref::keyword("BODY").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("REUSE").to_matchable(),
                    Ref::keyword("SETTINGS").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // IdentityClauseGrammar
        (
            "IdentityClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("GENERATED").to_matchable(),
                one_of(vec![
                    Ref::keyword("ALWAYS").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BY").to_matchable(),
                        Ref::keyword("DEFAULT").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("NULL").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::keyword("IDENTITY").to_matchable(),
                Bracketed::new(vec![Ref::new("IdentityOptionsGrammar").to_matchable()])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // IdentityOptionsGrammar
        (
            "IdentityOptionsGrammar".into(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("START").to_matchable(),
                            Ref::keyword("WITH").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INCREMENT").to_matchable(),
                            Ref::keyword("BY").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("MAXVALUE").to_matchable(),
                        Ref::keyword("MINVALUE").to_matchable(),
                        Ref::keyword("CACHE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("NOMAXVALUE").to_matchable(),
                Ref::keyword("NOMINVALUE").to_matchable(),
                Ref::keyword("CYCLE").to_matchable(),
                Ref::keyword("NOCYCLE").to_matchable(),
                Ref::keyword("NOCACHE").to_matchable(),
                Ref::keyword("ORDER").to_matchable(),
                Ref::keyword("NOORDER").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // TriggerPredicatesGrammar
        (
            "TriggerPredicatesGrammar".into(),
            one_of(vec![
                Ref::keyword("INSERTING").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("UPDATING").to_matchable(),
                    Bracketed::new(vec![Ref::new("QuotedLiteralSegment").to_matchable()])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("DELETING").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // ForUpdateGrammar
        (
            "ForUpdateGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                Ref::keyword("UPDATE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OF").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // ImplicitCursorAttributesGrammar
        (
            "ImplicitCursorAttributesGrammar".into(),
            Sequence::new(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                Ref::new("ModuloSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("ISOPEN").to_matchable(),
                    Ref::keyword("FOUND").to_matchable(),
                    Ref::keyword("NOTFOUND").to_matchable(),
                    Ref::keyword("ROWCOUNT").to_matchable(),
                    Ref::keyword("BULK_ROWCOUNT").to_matchable(),
                    Ref::keyword("BULK_EXCEPTIONS").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // BatchDelimiterGrammar
        (
            "BatchDelimiterGrammar".into(),
            Ref::new("SlashBufferExecutorSegment").to_matchable().into(),
        ),
        // ElementSpecificationGrammar
        (
            "ElementSpecificationGrammar".into(),
            Sequence::new(vec![
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        one_of(vec![
                            Ref::keyword("OVERRIDING").to_matchable(),
                            Ref::keyword("FINAL").to_matchable(),
                            Ref::keyword("INSTANTIABLE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("MEMBER").to_matchable(),
                            Ref::keyword("STATIC").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::new("CreateFunctionStatementSegment").to_matchable(),
                            Ref::new("CreateProcedureStatementSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // ObjectTypeAndSubtypeDefGrammar
        (
            "ObjectTypeAndSubtypeDefGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("OBJECT").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("UNDER").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                                Ref::new("DatatypeSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("ElementSpecificationGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("NOT").optional().to_matchable(),
                        one_of(vec![
                            Ref::keyword("FINAL").to_matchable(),
                            Ref::keyword("INSTANTIABLE").to_matchable(),
                            Ref::keyword("PERSISTABLE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // VarrayAndNestedTypeSpecGrammar
        (
            "VarrayAndNestedTypeSpecGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("VARRAY").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("VARYING").optional().to_matchable(),
                                Ref::keyword("ARRAY").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("OF").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("NOT").to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        // DBLinkAuthenticationGrammar
        (
            "DBLinkAuthenticationGrammar".into(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("AUTHENTICATED").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::new("RoleReferenceSegment").to_matchable(),
                    Ref::keyword("IDENTIFIED").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    Ref::keyword("CREDENTIAL").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // ---- New Segments ----
    oracle.add([
        // TriggerCorrelationNameSegment
        (
            "TriggerCorrelationNameSegment".into(),
            NodeMatcher::new(SyntaxKind::TriggerCorrelationName, |_| {
                one_of(vec![
                    Ref::keyword("OLD").to_matchable(),
                    Ref::keyword("NEW").to_matchable(),
                    Ref::keyword("PARENT").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // TriggerCorrelationReferenceSegment
        (
            "TriggerCorrelationReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleBindVariable, |_| {
                Sequence::new(vec![
                    Ref::new("ColonDelimiterSegment").to_matchable(),
                    Ref::new("TriggerCorrelationNameSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("DotSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .config(|config| {
                    config.allow_gaps = false;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // SqlplusVariableGrammar
        (
            "SqlplusVariableGrammar".into(),
            NodeMatcher::new(SyntaxKind::OracleSqlplusVariable, |_| {
                optionally_bracketed(vec![
                    Ref::new("ColonSegment").to_matchable(),
                    Ref::new("ParameterNameSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("DotSegment").to_matchable(),
                        Ref::new("ParameterNameSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // SqlplusSubstitutionVariableSegment
        (
            "SqlplusSubstitutionVariableSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleSqlplusVariable, |_| {
                Sequence::new(vec![
                    Ref::new("AmpersandSegment").to_matchable(),
                    Ref::new("AmpersandSegment").optional().to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // SlashBufferExecutorSegment
        (
            "SlashBufferExecutorSegment".into(),
            NodeMatcher::new(SyntaxKind::SlashBufferExecutor, |_| {
                Ref::new("SlashSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ConnectByClauseSegment
        (
            "ConnectByClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ConnectbyClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("CONNECT").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::keyword("NOCYCLE").optional().to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // StartWithClauseSegment
        (
            "StartWithClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::StartwithClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("START").to_matchable(),
                    Ref::keyword("WITH").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // HierarchicalQueryClauseSegment
        (
            "HierarchicalQueryClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::HierarchicalQueryClause, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("ConnectByClauseSegment").to_matchable(),
                        Ref::new("StartWithClauseSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("StartWithClauseSegment").to_matchable(),
                        Ref::new("ConnectByClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // PivotSegment
        (
            "PivotSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("PIVOT").to_matchable(),
                    Ref::keyword("XML").optional().to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("FunctionSegment").to_matchable(),
                                Ref::new("AliasExpressionSegment").optional().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("PivotForInGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // UnpivotSegment
        (
            "UnpivotSegment".into(),
            NodeMatcher::new(SyntaxKind::UnpivotSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("UNPIVOT").to_matchable(),
                    Ref::new("UnpivotNullsGrammar").optional().to_matchable(),
                    Bracketed::new(vec![
                        optionally_bracketed(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("PivotForInGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // WithinGroupClauseSegment
        (
            "WithinGroupClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleWithinGroupClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("WITHIN").to_matchable(),
                    Ref::keyword("GROUP").to_matchable(),
                    Bracketed::new(vec![Ref::new("OrderByClauseSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ListaggOverflowClauseSegment
        (
            "ListaggOverflowClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleListaggOverflowClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("OVERFLOW").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ERROR").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("TRUNCATE").to_matchable(),
                            Ref::new("SingleQuotedIdentifierSegment")
                                .optional()
                                .to_matchable(),
                            one_of(vec![
                                Ref::keyword("WITH").to_matchable(),
                                Ref::keyword("WITHOUT").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Ref::keyword("COUNT").optional().to_matchable(),
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
        // NamedArgumentSegment
        (
            "NamedArgumentSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleNamedArgument, |_| {
                Sequence::new(vec![
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::new("RightArrowSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CommentStatementSegment
        (
            "CommentStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCommentStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("COMMENT").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("TABLE").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COLUMN").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("OPERATOR").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("INDEXTYPE").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("MATERIALIZED").to_matchable(),
                                Ref::keyword("VIEW").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("IS").to_matchable(),
                            one_of(vec![
                                Ref::new("QuotedLiteralSegment").to_matchable(),
                                Ref::keyword("NULL").to_matchable(),
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
        ),
        // TableReferenceSegment
        (
            "TableReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleTableReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|config| {
                        config.delimiter(one_of(vec![
                            Ref::new("DotSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::new("DotSegment").to_matchable(),
                                Ref::new("DotSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("AtSignSegment").to_matchable(),
                        ]));
                        config.terminators = vec![
                            Ref::keyword("ON").to_matchable(),
                            Ref::keyword("AS").to_matchable(),
                            Ref::keyword("USING").to_matchable(),
                            Ref::new("CommaSegment").to_matchable(),
                            Ref::new("CastOperatorSegment").to_matchable(),
                            Ref::new("StartSquareBracketSegment").to_matchable(),
                            Ref::new("StartBracketSegment").to_matchable(),
                            Ref::new("BinaryOperatorGrammar").to_matchable(),
                            Ref::new("ColonSegment").to_matchable(),
                            Ref::new("DelimiterGrammar").to_matchable(),
                            Ref::new("JoinLikeClauseGrammar").to_matchable(),
                        ];
                        config.allow_gaps = false;
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // FunctionNameSegment
        (
            "FunctionNameSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleFunctionName, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            Ref::new("DotSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Delimited::new(vec![
                        one_of(vec![
                            Ref::new("FunctionNameIdentifierSegment").to_matchable(),
                            Ref::new("QuotedIdentifierSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.delimiter(Ref::new("AtSignSegment"));
                    })
                    .to_matchable(),
                ])
                .config(|config| {
                    config.allow_gaps = false;
                })
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterIndexStatementSegment
        (
            "AlterIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterIndexStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Ref::new("IndexReferenceSegment").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("REBUILD").to_matchable(),
                            one_of(vec![
                                Ref::keyword("REVERSE").to_matchable(),
                                Ref::keyword("NOREVERSE").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MONITORING").to_matchable(),
                            Ref::keyword("USAGE").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NOMONITORING").to_matchable(),
                            Ref::keyword("USAGE").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("RENAME").to_matchable(),
                            Ref::keyword("TO").to_matchable(),
                            Ref::new("IndexReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("COMPILE").to_matchable(),
                        Ref::keyword("LOGGING").to_matchable(),
                        Ref::keyword("NOLOGGING").to_matchable(),
                        Ref::keyword("ENABLE").to_matchable(),
                        Ref::keyword("DISABLE").to_matchable(),
                        Ref::keyword("UNUSABLE").to_matchable(),
                        Ref::keyword("INVISIBLE").to_matchable(),
                        Ref::keyword("VISIBLE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterTableStatementSegment
        (
            "AlterTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterTableStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("TABLE").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("AlterTablePropertiesSegment").to_matchable(),
                        Ref::new("AlterTableColumnClausesSegment").to_matchable(),
                        Ref::new("AlterTableConstraintClauses").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterTablePropertiesSegment
        (
            "AlterTablePropertiesSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableProperties, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("RENAME").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("TableReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterTableColumnClausesSegment
        (
            "AlterTableColumnClausesSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableColumnClauses, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ADD").to_matchable(),
                            Ref::keyword("MODIFY").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::new("ColumnDefinitionSegment").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DROP").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("COLUMN").to_matchable(),
                                Ref::new("ColumnReferenceSegment").to_matchable(),
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
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RENAME").to_matchable(),
                        Ref::keyword("COLUMN").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterTableConstraintClauses
        (
            "AlterTableConstraintClauses".into(),
            NodeMatcher::new(SyntaxKind::AlterTableConstraintClauses, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("ADD").to_matchable(),
                        Ref::new("TableConstraintSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("DROP").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("PRIMARY").to_matchable(),
                                Ref::keyword("KEY").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("UNIQUE").to_matchable(),
                                Bracketed::new(vec![
                                    Ref::new("ColumnReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("CONSTRAINT").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("CASCADE").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RENAME").to_matchable(),
                        Ref::keyword("CONSTRAINT").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterSessionStatementSegment
        (
            "AlterSessionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterSessionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("SESSION").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("ADVISE").to_matchable(),
                            one_of(vec![
                                Ref::keyword("COMMIT").to_matchable(),
                                Ref::keyword("ROLLBACK").to_matchable(),
                                Ref::keyword("NOTHING").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SET").to_matchable(),
                            AnyNumberOf::new(vec![
                                Sequence::new(vec![
                                    Ref::new("ParameterNameSegment").to_matchable(),
                                    Ref::new("EqualsSegment").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("DEFAULT").to_matchable(),
                                        Ref::new("ExpressionSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|config| {
                                config.min_times = 1;
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("ENABLE").to_matchable(),
                                Ref::keyword("DISABLE").to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("GUARD").to_matchable(),
                                Ref::keyword("RESUMABLE").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("PARALLEL").to_matchable(),
                                    one_of(vec![
                                        Ref::keyword("DML").to_matchable(),
                                        Ref::keyword("DDL").to_matchable(),
                                        Ref::keyword("QUERY").to_matchable(),
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
            .to_matchable()
            .into(),
        ),
        // CreateTableStatementSegment
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateTableStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::new("TemporaryGrammar").optional().to_matchable(),
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
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("CommentClauseSegment").optional().to_matchable(),
                            Ref::new("OnCommitGrammar").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::new("OnCommitGrammar").optional().to_matchable(),
                            Ref::keyword("AS").to_matchable(),
                            optionally_bracketed(vec![
                                Ref::new("SelectableGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("LIKE").to_matchable(),
                            Ref::new("TableReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("TableEndClauseSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ColumnDefinitionSegment
        (
            "ColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleColumnDefinition, |_| {
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("DatatypeSegment").optional().to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("ColumnConstraintSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ENABLE").to_matchable(),
                                Ref::keyword("DISABLE").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("IdentityClauseGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateViewStatementSegment
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateViewStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NO").optional().to_matchable(),
                        Ref::keyword("FORCE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("EDITIONING").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("EDITIONABLE").to_matchable(),
                            Ref::keyword("EDITIONING").optional().to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("NONEDITIONABLE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("MATERIALIZED").optional().to_matchable(),
                    Ref::keyword("VIEW").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("BracketedColumnReferenceListGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    optionally_bracketed(vec![Ref::new("SelectableGrammar").to_matchable()])
                        .to_matchable(),
                    Ref::new("WithNoSchemaBindingClauseSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // TransactionStatementSegment
        (
            "TransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleTransactionStatement, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("START").to_matchable(),
                        Ref::keyword("COMMIT").to_matchable(),
                        Ref::keyword("ROLLBACK").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("TRANSACTION").to_matchable(),
                        Ref::keyword("WORK").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateProcedureStatementSegment
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateProcedureStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("EDITIONABLE").to_matchable(),
                        Ref::keyword("NONEDITIONABLE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("PROCEDURE").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    Ref::new("FunctionParameterListGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::new("SharingClauseGrammar").optional().to_matchable(),
                    // SQLFluff: AnyNumberOf(DefaultCollation, InvokerRights, AccessibleBy)
                    AnyNumberOf::new(vec![
                        Ref::new("DefaultCollationClauseGrammar").to_matchable(),
                        Ref::new("InvokerRightsClauseGrammar").to_matchable(),
                        Ref::new("AccessibleByClauseGrammar").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("IS").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("DeclareSegment").to_matchable()])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    Ref::new("BeginEndSegment").optional().to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DropProcedureStatementSegment
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleDropProcedureStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("PROCEDURE").to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DeclareSegment
        (
            "DeclareSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleDeclareSegment, |_| {
                Sequence::new(vec![
                    Ref::keyword("DECLARE").optional().to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    AnyNumberOf::new(vec![
                        Delimited::new(vec![
                            one_of(vec![
                                Sequence::new(vec![
                                    one_of(vec![
                                        Sequence::new(vec![
                                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                                            Ref::keyword("CONSTANT").optional().to_matchable(),
                                            one_of(vec![
                                                Ref::new("DatatypeSegment").to_matchable(),
                                                Ref::new("ColumnTypeReferenceSegment")
                                                    .to_matchable(),
                                                Ref::new("RowTypeReferenceSegment").to_matchable(),
                                            ])
                                            .to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Sequence::new(vec![
                                            Ref::keyword("PRAGMA").to_matchable(),
                                            Ref::new("FunctionSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::new("CollectionTypeDefinitionSegment").to_matchable(),
                                        Ref::new("RecordTypeDefinitionSegment").to_matchable(),
                                        Ref::new("RefCursorTypeDefinitionSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("NOT").to_matchable(),
                                        Ref::keyword("NULL").to_matchable(),
                                    ])
                                    .config(|config| {
                                        config.optional();
                                    })
                                    .to_matchable(),
                                    Sequence::new(vec![
                                        one_of(vec![
                                            Ref::new("AssignmentOperatorSegment").to_matchable(),
                                            Ref::keyword("DEFAULT").to_matchable(),
                                        ])
                                        .to_matchable(),
                                        Ref::new("ExpressionSegment").to_matchable(),
                                    ])
                                    .config(|config| {
                                        config.optional();
                                    })
                                    .to_matchable(),
                                    Ref::new("DelimiterGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("CreateProcedureStatementSegment").to_matchable(),
                                Ref::new("CreateFunctionStatementSegment").to_matchable(),
                                Ref::new("DeclareCursorVariableSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.delimiter(Ref::new("DelimiterGrammar"));
                            config.terminators = vec![
                                Ref::keyword("BEGIN").to_matchable(),
                                Ref::keyword("END").to_matchable(),
                            ];
                        })
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.min_times = 1;
                    })
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ColumnTypeReferenceSegment
        (
            "ColumnTypeReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleColumnTypeReference, |_| {
                Sequence::new(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("ModuloSegment").to_matchable(),
                    Ref::keyword("TYPE").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // RowTypeReferenceSegment
        (
            "RowTypeReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleRowTypeReference, |_| {
                Sequence::new(vec![
                    Ref::new("TableReferenceSegment").to_matchable(),
                    Ref::new("ModuloSegment").to_matchable(),
                    Ref::keyword("ROWTYPE").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CollectionTypeDefinitionSegment
        // SQLFluff: TYPE name IS [TABLE OF | VARRAY(n) OF] type [NOT NULL] [INDEX BY type]
        (
            "CollectionTypeDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::CollectionType, |_| {
                Sequence::new(vec![
                    Ref::keyword("TYPE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("IS").to_matchable(),
                    one_of(vec![
                        // TABLE OF type
                        Sequence::new(vec![
                            Ref::keyword("TABLE").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                            one_of(vec![
                                Ref::new("DatatypeSegment").to_matchable(),
                                Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                                Ref::new("RowTypeReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // VARRAY(n) OF type
                        Sequence::new(vec![
                            one_of(vec![
                                Ref::keyword("VARRAY").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("VARYING").optional().to_matchable(),
                                    Ref::keyword("ARRAY").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                                .to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                            one_of(vec![
                                Ref::new("DatatypeSegment").to_matchable(),
                                Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                                Ref::new("RowTypeReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Plain type (subtype definition)
                        one_of(vec![
                            Ref::new("DatatypeSegment").to_matchable(),
                            Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                            Ref::new("RowTypeReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("INDEX").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("DatatypeSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // RecordTypeDefinitionSegment
        // SQLFluff: TYPE name IS RECORD (field type [NOT NULL] [:= | DEFAULT expr], ...)
        (
            "RecordTypeDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::RecordType, |_| {
                Sequence::new(vec![
                    Ref::keyword("TYPE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("IS").to_matchable(),
                    Ref::keyword("RECORD").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![
                            Sequence::new(vec![
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                                one_of(vec![
                                    Ref::new("DatatypeSegment").to_matchable(),
                                    Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Sequence::new(vec![
                                        Ref::keyword("NOT").to_matchable(),
                                        Ref::keyword("NULL").to_matchable(),
                                    ])
                                    .config(|config| {
                                        config.optional();
                                    })
                                    .to_matchable(),
                                    one_of(vec![
                                        Ref::new("AssignmentOperatorSegment").to_matchable(),
                                        Ref::keyword("DEFAULT").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
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
        ),
        // RefCursorTypeDefinitionSegment
        (
            "RefCursorTypeDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::RefCursorType, |_| {
                Sequence::new(vec![
                    Ref::keyword("TYPE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::keyword("IS").to_matchable(),
                    Ref::keyword("REF").to_matchable(),
                    Ref::keyword("CURSOR").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RETURN").to_matchable(),
                        one_of(vec![
                            Ref::new("RowTypeReferenceSegment").to_matchable(),
                            Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DeclareCursorVariableSegment
        (
            "DeclareCursorVariableSegment".into(),
            NodeMatcher::new(SyntaxKind::DeclareCursorVariable, |_| {
                Sequence::new(vec![
                    Ref::keyword("CURSOR").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("FunctionParameterListGrammar")
                        .optional()
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RETURN").to_matchable(),
                        one_of(vec![
                            Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                            Ref::new("RowTypeReferenceSegment").to_matchable(),
                            Ref::new("DatatypeSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("IS").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("SelectStatementSegment").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ExecuteImmediateSegment
        (
            "ExecuteImmediateSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleExecuteImmediateStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXECUTE").to_matchable(),
                    Ref::keyword("IMMEDIATE").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("IntoClauseSegment").to_matchable(),
                        Ref::new("BulkCollectIntoClauseSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("IN").to_matchable(),
                                    Ref::keyword("OUT").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("IN").to_matchable(),
                                        Ref::keyword("OUT").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // BeginEndSegment
        (
            "BeginEndSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleBeginEndBlock, |_| {
                Sequence::new(vec![
                    Ref::new("DeclareSegment").optional().to_matchable(),
                    Ref::keyword("BEGIN").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("EXCEPTION").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("WHEN").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("OTHERS").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                                        AnyNumberOf::new(vec![
                                            Sequence::new(vec![
                                                Ref::keyword("OR").to_matchable(),
                                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                                            ])
                                            .to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::keyword("THEN").to_matchable(),
                                MetaSegment::indent().to_matchable(),
                                Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                                MetaSegment::dedent().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.min_times = 1;
                        })
                        .to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::new("ObjectReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateFunctionStatementSegment
        (
            "CreateFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("EDITIONABLE").to_matchable(),
                        Ref::keyword("NONEDITIONABLE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    Ref::new("FunctionParameterListGrammar")
                        .optional()
                        .to_matchable(),
                    Ref::keyword("RETURN").to_matchable(),
                    Ref::new("DatatypeSegment").to_matchable(),
                    Ref::new("SharingClauseGrammar").optional().to_matchable(),
                    AnyNumberOf::new(vec![
                        Ref::new("DefaultCollationClauseGrammar").to_matchable(),
                        Ref::new("InvokerRightsClauseGrammar").to_matchable(),
                        Ref::new("AccessibleByClauseGrammar").to_matchable(),
                        Ref::keyword("DETERMINISTIC").to_matchable(),
                        Ref::keyword("SHARD_ENABLE").to_matchable(),
                        Ref::new("ParallelEnableClauseGrammar").to_matchable(),
                        Ref::new("ResultCacheClauseGrammar").to_matchable(),
                        Ref::new("PipelinedClauseGrammar").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("IS").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    AnyNumberOf::new(vec![Ref::new("DeclareSegment").to_matchable()])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    Ref::new("BeginEndSegment").optional().to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterFunctionStatementSegment
        (
            "AlterFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterFunctionStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    one_of(vec![
                        Ref::keyword("FUNCTION").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("CompileClauseGrammar").to_matchable(),
                        Ref::keyword("EDITIONABLE").to_matchable(),
                        Ref::keyword("NONEDITIONABLE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateTypeStatementSegment
        (
            "CreateTypeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateTypeStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("TYPE").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("IS").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("ObjectTypeAndSubtypeDefGrammar").to_matchable(),
                        Ref::new("VarrayAndNestedTypeSpecGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateTypeBodyStatementSegment
        (
            "CreateTypeBodyStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateTypeBodyStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").optional().to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("TYPE").to_matchable(),
                    Ref::keyword("BODY").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("IS").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                    ])
                    .to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("ElementSpecificationGrammar").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreatePackageStatementSegment
        (
            "CreatePackageStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreatePackageStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("PACKAGE").to_matchable(),
                    Ref::keyword("BODY").optional().to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("IS").to_matchable(),
                        Ref::keyword("AS").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DeclareSegment").to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::new("ObjectReferenceSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterPackageStatementSegment
        (
            "AlterPackageStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterPackageStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("PACKAGE").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("CompileClauseGrammar").to_matchable(),
                        Ref::keyword("EDITIONABLE").to_matchable(),
                        Ref::keyword("NONEDITIONABLE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DropPackageStatementSegment
        (
            "DropPackageStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleDropPackageStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("PACKAGE").to_matchable(),
                    Ref::keyword("BODY").optional().to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateTriggerStatementSegment
        (
            "CreateTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateTriggerStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("OR").to_matchable(),
                        Ref::keyword("REPLACE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                    Ref::new("TriggerReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            one_of(vec![
                                Ref::keyword("BEFORE").to_matchable(),
                                Ref::keyword("AFTER").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("INSTEAD").to_matchable(),
                                Ref::keyword("OF").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("FOR").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DmlEventClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("EACH").to_matchable(),
                        Ref::keyword("ROW").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("ENABLE").to_matchable(),
                        Ref::keyword("DISABLE").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHEN").to_matchable(),
                        Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                    Ref::keyword("END").optional().to_matchable(),
                    Ref::new("TriggerReferenceSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DmlEventClauseSegment
        (
            "DmlEventClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::DmlEventClause, |_| {
                Sequence::new(vec![
                    Ref::new("DmlGrammar").to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("OR").to_matchable(),
                            Ref::new("DmlGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::new("TableReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterTriggerStatementSegment
        (
            "AlterTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterTriggerStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                    Ref::new("FunctionNameSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("CompileClauseGrammar").to_matchable(),
                        Ref::keyword("ENABLE").to_matchable(),
                        Ref::keyword("DISABLE").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("RENAME").to_matchable(),
                            Ref::keyword("TO").to_matchable(),
                            Ref::new("FunctionNameSegment").to_matchable(),
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
        // AssignmentStatementSegment
        // SQLFluff: AnyNumberOf(ObjectRef, Bracketed(subscript)?, DotSegment?,
        //           OneOf(TriggerCorrelation, SqlplusVariable)?, optional)
        //           := / DEFAULT  ExpressionSegment
        (
            "AssignmentStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentSegmentStatement, |_| {
                Sequence::new(vec![
                    AnyNumberOf::new(vec![
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                        Bracketed::new(vec![
                            one_of(vec![
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                                Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                        Ref::new("DotSegment").optional().to_matchable(),
                        one_of(vec![
                            Ref::new("TriggerCorrelationReferenceSegment").to_matchable(),
                            Ref::new("SqlplusVariableGrammar").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("AssignmentOperatorSegment").to_matchable(),
                        Ref::keyword("DEFAULT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // IfExpressionStatement
        (
            "IfExpressionStatement".into(),
            NodeMatcher::new(SyntaxKind::OracleIfThenStatement, |_| {
                Sequence::new(vec![
                    Ref::new("IfClauseSegment").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("ELSIF").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                            Ref::keyword("THEN").to_matchable(),
                            MetaSegment::indent().to_matchable(),
                            Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                            MetaSegment::dedent().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ELSE").to_matchable(),
                        MetaSegment::indent().to_matchable(),
                        Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                        MetaSegment::dedent().to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("IF").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // IfClauseSegment
        (
            "IfClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleIfClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("IF").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::keyword("THEN").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // NullStatementSegment
        (
            "NullStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleNullStatement, |_| {
                Ref::keyword("NULL").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ForLoopStatementSegment
        // SQLFluff: Sequence("FOR", Delimited(Sequence(id, OneOf(MUTABLE,IMMUTABLE)?)),
        //   "IN", Delimited(Sequence(REVERSE?, OneOf(range,expr,select), WHILE?, WHEN?)),
        //   LoopStatementSegment)
        (
            "ForLoopStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ForLoopStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                            one_of(vec![
                                Ref::keyword("MUTABLE").to_matchable(),
                                Ref::keyword("IMMUTABLE").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("IN").to_matchable(),
                    Delimited::new(vec![
                        Sequence::new(vec![
                            Ref::keyword("REVERSE").optional().to_matchable(),
                            one_of(vec![
                                Ref::new("IterationSteppedControlGrammar").to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("REPEAT").optional().to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    one_of(vec![
                                        Ref::keyword("VALUES").to_matchable(),
                                        Ref::keyword("INDICES").to_matchable(),
                                        Ref::keyword("PAIRS").to_matchable(),
                                    ])
                                    .to_matchable(),
                                    Ref::keyword("OF").to_matchable(),
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                                Bracketed::new(vec![
                                    Ref::new("SelectStatementSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("WHILE").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("WHEN").to_matchable(),
                                Ref::new("ExpressionSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("LoopStatementSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // WhileLoopStatementSegment
        (
            "WhileLoopStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WhileLoopStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("WHILE").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                    Ref::new("LoopStatementSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // LoopStatementSegment
        (
            "LoopStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleLoopStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("LOOP").to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("END").to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ForAllStatementSegment
        (
            "ForAllStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ForallStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("FORALL").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    Ref::keyword("IN").to_matchable(),
                    one_of(vec![
                        Ref::new("IterationSteppedControlGrammar").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("VALUES").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                            Ref::new("SingleIdentifierGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SAVE").to_matchable(),
                        Ref::keyword("EXCEPTIONS").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("DeleteStatementSegment").to_matchable(),
                        Ref::new("InsertStatementSegment").to_matchable(),
                        Ref::new("UpdateStatementSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // OpenStatementSegment
        (
            "OpenStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleOpenStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("OPEN").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("FunctionContentsSegment")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CloseStatementSegment
        (
            "CloseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CloseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CLOSE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // OpenForStatementSegment
        // SQLFluff: OPEN cursor FOR select/string/ident [USING [IN|OUT|IN OUT] expr, ...]
        (
            "OpenForStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleOpenForStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("OPEN").to_matchable(),
                    one_of(vec![
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                        Ref::new("SqlplusVariableGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    one_of(vec![
                        Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                        Ref::new("SelectStatementSegment").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Delimited::new(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("IN").to_matchable(),
                                    Ref::keyword("OUT").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("IN").to_matchable(),
                                        Ref::keyword("OUT").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                                one_of(vec![
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                    Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // FetchStatementSegment
        (
            "FetchStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleFetchStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("FETCH").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    one_of(vec![
                        Ref::new("IntoClauseSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::new("BulkCollectIntoClauseSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("LIMIT").to_matchable(),
                                one_of(vec![
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
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
        // IntoClauseSegment
        (
            "IntoClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleIntoClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("INTO").to_matchable(),
                    Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // BulkCollectIntoClauseSegment
        (
            "BulkCollectIntoClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::BulkCollectIntoClause, |_| {
                Sequence::new(vec![
                    Ref::keyword("BULK").to_matchable(),
                    Ref::keyword("COLLECT").to_matchable(),
                    Ref::keyword("INTO").to_matchable(),
                    Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ExitStatementSegment
        // SQLFluff: EXIT [label] [WHEN expression]
        // Exclude WHEN from the optional label to prevent it being consumed as an identifier
        (
            "ExitStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleExitStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("EXIT").to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .exclude(Ref::keyword("WHEN"))
                        .optional()
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHEN").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ContinueStatementSegment
        // SQLFluff: CONTINUE [label] [WHEN expression]
        (
            "ContinueStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ContinueStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CONTINUE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .exclude(Ref::keyword("WHEN"))
                        .optional()
                        .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("WHEN").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // RaiseStatementSegment
        (
            "RaiseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RaiseStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("RAISE").to_matchable(),
                    Ref::new("SingleIdentifierGrammar")
                        .optional()
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ReturnStatementSegment
        (
            "ReturnStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleReturnStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("RETURN").to_matchable(),
                    Ref::new("ExpressionSegment").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ReturningClauseSegment
        (
            "ReturningClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleReturningClause, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("RETURNING").to_matchable(),
                        Ref::keyword("RETURN").to_matchable(),
                    ])
                    .to_matchable(),
                    Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                    one_of(vec![
                        Ref::new("IntoClauseSegment").to_matchable(),
                        Ref::new("BulkCollectIntoClauseSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ValuesClauseSegment
        (
            "ValuesClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleValuesClause, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("VALUE").to_matchable(),
                        Ref::keyword("VALUES").to_matchable(),
                    ])
                    .to_matchable(),
                    optionally_bracketed(vec![
                        Delimited::new(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
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
        // DatabaseLinkReferenceSegment
        (
            "DatabaseLinkReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::DatabaseLinkReference, |_| {
                Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                    .config(|config| {
                        config.delimiter(Ref::new("DotSegment"));
                    })
                    .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateDatabaseLinkStatementSegment
        (
            "CreateDatabaseLinkStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateDatabaseLinkStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::keyword("SHARED").optional().to_matchable(),
                    Ref::keyword("PUBLIC").optional().to_matchable(),
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::keyword("LINK").to_matchable(),
                    Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CONNECT").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        one_of(vec![
                            Ref::keyword("CURRENT_USER").to_matchable(),
                            Sequence::new(vec![
                                Ref::new("RoleReferenceSegment").to_matchable(),
                                Ref::keyword("IDENTIFIED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("USING").to_matchable(),
                        Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DropDatabaseLinkStatementSegment
        (
            "DropDatabaseLinkStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleDropDatabaseLinkStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("PUBLIC").optional().to_matchable(),
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::keyword("LINK").to_matchable(),
                    Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterDatabaseLinkStatementSegment
        (
            "AlterDatabaseLinkStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterDatabaseLinkStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("SHARED").optional().to_matchable(),
                    Ref::keyword("PUBLIC").optional().to_matchable(),
                    Ref::keyword("DATABASE").to_matchable(),
                    Ref::keyword("LINK").to_matchable(),
                    Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CONNECT").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("RoleReferenceSegment").to_matchable(),
                        Ref::keyword("IDENTIFIED").to_matchable(),
                        Ref::keyword("BY").to_matchable(),
                        Ref::new("SingleIdentifierGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CreateSynonymStatementSegment
        (
            "CreateSynonymStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleCreateSynonymStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("CREATE").to_matchable(),
                    Ref::new("OrReplaceGrammar").optional().to_matchable(),
                    Ref::keyword("PUBLIC").optional().to_matchable(),
                    Ref::keyword("SYNONYM").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::new("AtSignSegment").to_matchable(),
                        Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DropSynonymStatementSegment
        (
            "DropSynonymStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleDropSynonymStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    Ref::keyword("PUBLIC").optional().to_matchable(),
                    Ref::keyword("SYNONYM").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Ref::keyword("FORCE").optional().to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // AlterSynonymStatementSegment
        (
            "AlterSynonymStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OracleAlterSynonymStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("ALTER").to_matchable(),
                    Ref::keyword("PUBLIC").optional().to_matchable(),
                    Ref::keyword("SYNONYM").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("EDITIONABLE").to_matchable(),
                        Ref::keyword("NONEDITIONABLE").to_matchable(),
                        Ref::keyword("COMPILE").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ---- Grammar replacements ----
    oracle.replace_grammar(
        "TemporaryGrammar",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("GLOBAL").to_matchable(),
                Ref::keyword("PRIVATE").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("TEMPORARY").to_matchable(),
        ])
        .config(|config| {
            config.optional();
        })
        .to_matchable(),
    );

    oracle.replace_grammar(
        "DropBehaviorGrammar",
        Sequence::new(vec![
            Sequence::new(vec![
                Ref::keyword("CASCADE").to_matchable(),
                Ref::keyword("CONSTRAINTS").to_matchable(),
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable(),
            Ref::keyword("PURGE").optional().to_matchable(),
        ])
        .config(|config| {
            config.optional();
        })
        .to_matchable(),
    );

    oracle.replace_grammar(
        "PostFunctionGrammar",
        AnyNumberOf::new(vec![
            Ref::new("WithinGroupClauseSegment").to_matchable(),
            Ref::new("FilterClauseGrammar").to_matchable(),
            Ref::new("OverClauseSegment").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    oracle.replace_grammar(
        "FunctionContentsExpressionGrammar",
        one_of(vec![
            Ref::new("ExpressionSegment").to_matchable(),
            Ref::new("NamedArgumentSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    oracle.replace_grammar(
        "DateTimeLiteralGrammar",
        Sequence::new(vec![
            one_of(vec![
                Ref::keyword("DATE").to_matchable(),
                Ref::keyword("TIME").to_matchable(),
                Ref::keyword("TIMESTAMP").to_matchable(),
                Ref::keyword("INTERVAL").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Sequence::new(vec![
                Ref::new("IntervalUnitsGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("IntervalUnitsGrammar").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable(),
        ])
        .to_matchable(),
    );

    oracle.replace_grammar(
        "PreTableFunctionKeywordsGrammar",
        one_of(vec![Ref::keyword("LATERAL").to_matchable()]).to_matchable(),
    );

    oracle.replace_grammar(
        "ConditionalCrossJoinKeywordsGrammar",
        Nothing::new().to_matchable(),
    );

    oracle.replace_grammar(
        "UnconditionalCrossJoinKeywordsGrammar",
        Ref::keyword("CROSS").to_matchable(),
    );

    oracle.replace_grammar(
        "FunctionParameterGrammar",
        Sequence::new(vec![
            Ref::new("ParameterNameSegment").to_matchable(),
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("IN").optional().to_matchable(),
                    one_of(vec![
                        Ref::new("DatatypeSegment").to_matchable(),
                        Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                        Ref::new("RowTypeReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("AssignmentOperatorSegment").to_matchable(),
                            Ref::keyword("DEFAULT").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IN").optional().to_matchable(),
                    Ref::keyword("OUT").to_matchable(),
                    Ref::keyword("NOCOPY").optional().to_matchable(),
                    one_of(vec![
                        Ref::new("DatatypeSegment").to_matchable(),
                        Ref::new("ColumnTypeReferenceSegment").to_matchable(),
                        Ref::new("RowTypeReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    oracle.replace_grammar(
        "SequenceMinValueGrammar",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("MINVALUE").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("NOMINVALUE").to_matchable(),
        ])
        .to_matchable(),
    );

    oracle.replace_grammar(
        "SequenceMaxValueGrammar",
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("MAXVALUE").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("NOMAXVALUE").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- Override FileSegment for Oracle batch (/) and @file support ----
    // SQLFluff: FileSegment.match_grammar = Sequence(AnyNumberOf(Ref("BatchSegment"), Ref("ExecuteFileSegment")))
    oracle.replace_grammar(
        "FileSegment",
        AnyNumberOf::new(vec![
            one_of(vec![
                Ref::new("BatchSegment").to_matchable(),
                Ref::new("ExecuteFileSegment").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- BatchSegment ----
    oracle.add([(
        "BatchSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleBatch, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Delimited::new(vec![Ref::new("StatementSegment").to_matchable()])
                        .config(|this| {
                            this.allow_trailing();
                            this.delimiter(
                                AnyNumberOf::new(vec![Ref::new("DelimiterGrammar").to_matchable()])
                                    .config(|config| config.min_times(1)),
                            );
                        })
                        .to_matchable(),
                    Ref::new("BatchDelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
                Ref::new("BatchDelimiterGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- ExecuteFileSegment ----
    oracle.add([(
        "ExecuteFileSegment".into(),
        NodeMatcher::new(SyntaxKind::ExecuteFileStatement, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("AtSignSegment").to_matchable(),
                        Ref::new("AtSignSegment").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("START").to_matchable(),
                ])
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("DotSegment").to_matchable(),
                    Ref::new("SlashSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- Override StatementSegment to include Oracle-specific statements ----
    // SQLFluff: ansi.StatementSegment.match_grammar.copy(insert=[...])
    // Uses copy() to append Oracle statements to the ANSI one_of list.
    oracle.replace_grammar(
        "StatementSegment",
        ansi::statement_segment().copy(
            Some(vec![
                Ref::new("CommentStatementSegment").to_matchable(),
                Ref::new("CreateProcedureStatementSegment").to_matchable(),
                Ref::new("DropProcedureStatementSegment").to_matchable(),
                Ref::new("AlterFunctionStatementSegment").to_matchable(),
                Ref::new("CreateTypeStatementSegment").to_matchable(),
                Ref::new("CreateTypeBodyStatementSegment").to_matchable(),
                Ref::new("CreatePackageStatementSegment").to_matchable(),
                Ref::new("AlterSessionStatementSegment").to_matchable(),
                Ref::new("DropPackageStatementSegment").to_matchable(),
                Ref::new("AlterPackageStatementSegment").to_matchable(),
                Ref::new("AlterTriggerStatementSegment").to_matchable(),
                Ref::new("BeginEndSegment").to_matchable(),
                Ref::new("AssignmentStatementSegment").to_matchable(),
                Ref::new("RecordTypeDefinitionSegment").to_matchable(),
                Ref::new("DeclareCursorVariableSegment").to_matchable(),
                Ref::new("ExecuteImmediateSegment").to_matchable(),
                Ref::new("FunctionSegment").to_matchable(),
                Ref::new("IfExpressionStatement").to_matchable(),
                Ref::new("CaseExpressionSegment").to_matchable(),
                Ref::new("NullStatementSegment").to_matchable(),
                Ref::new("ForLoopStatementSegment").to_matchable(),
                Ref::new("WhileLoopStatementSegment").to_matchable(),
                Ref::new("LoopStatementSegment").to_matchable(),
                Ref::new("ForAllStatementSegment").to_matchable(),
                Ref::new("OpenStatementSegment").to_matchable(),
                Ref::new("CloseStatementSegment").to_matchable(),
                Ref::new("OpenForStatementSegment").to_matchable(),
                Ref::new("FetchStatementSegment").to_matchable(),
                Ref::new("ExitStatementSegment").to_matchable(),
                Ref::new("ContinueStatementSegment").to_matchable(),
                Ref::new("RaiseStatementSegment").to_matchable(),
                Ref::new("ReturnStatementSegment").to_matchable(),
                Ref::new("AlterIndexStatementSegment").to_matchable(),
                Ref::new("CreateDatabaseLinkStatementSegment").to_matchable(),
                Ref::new("DropDatabaseLinkStatementSegment").to_matchable(),
                Ref::new("AlterDatabaseLinkStatementSegment").to_matchable(),
                Ref::new("CreateSynonymStatementSegment").to_matchable(),
                Ref::new("DropSynonymStatementSegment").to_matchable(),
                Ref::new("AlterSynonymStatementSegment").to_matchable(),
            ]),
            None,
            None,
            None,
            vec![],
            false,
        ),
    );

    // ---- Override UnorderedSelectStatementSegment ----
    // SQLFluff inserts HierarchicalQueryClause, Pivot, Unpivot before GroupBy,
    // and IntoClause/BulkCollectIntoClause before From.
    oracle.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            one_of(vec![
                Ref::new("IntoClauseSegment").to_matchable(),
                Ref::new("BulkCollectIntoClauseSegment").to_matchable(),
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("HierarchicalQueryClauseSegment")
                .optional()
                .to_matchable(),
            Ref::new("PivotSegment").optional().to_matchable(),
            Ref::new("UnpivotSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("OverlapsClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::new("OrderByClauseSegment").to_matchable(),
            Ref::new("LimitClauseSegment").to_matchable(),
            Ref::new("HierarchicalQueryClauseSegment").to_matchable(),
            Ref::new("PivotSegment").to_matchable(),
            Ref::new("UnpivotSegment").to_matchable(),
            Ref::keyword("LOG").to_matchable(),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    // ---- Override SelectStatementSegment ----
    // SQLFluff adds FOR UPDATE, ORDER BY, FETCH, LIMIT on top of the unordered grammar.
    oracle.replace_grammar(
        "SelectStatementSegment",
        Sequence::new(vec![
            Ref::new("SelectClauseSegment").to_matchable(),
            MetaSegment::dedent().to_matchable(),
            one_of(vec![
                Ref::new("IntoClauseSegment").to_matchable(),
                Ref::new("BulkCollectIntoClauseSegment").to_matchable(),
            ])
            .config(|config| {
                config.optional();
            })
            .to_matchable(),
            Ref::new("FromClauseSegment").optional().to_matchable(),
            Ref::new("WhereClauseSegment").optional().to_matchable(),
            Ref::new("HierarchicalQueryClauseSegment")
                .optional()
                .to_matchable(),
            Ref::new("PivotSegment").optional().to_matchable(),
            Ref::new("UnpivotSegment").optional().to_matchable(),
            Ref::new("GroupByClauseSegment").optional().to_matchable(),
            Ref::new("HavingClauseSegment").optional().to_matchable(),
            Ref::new("OverlapsClauseSegment").optional().to_matchable(),
            Ref::new("ForUpdateGrammar").optional().to_matchable(),
            Ref::new("OrderByClauseSegment").optional().to_matchable(),
            Ref::new("FetchClauseSegment").optional().to_matchable(),
            Ref::new("LimitClauseSegment").optional().to_matchable(),
            Ref::new("NamedWindowSegment").optional().to_matchable(),
            Ref::new("ForUpdateGrammar").optional().to_matchable(),
        ])
        .terminators(vec![
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::keyword("LOG").to_matchable(),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    // ---- Override ArithmeticBinaryOperatorGrammar to add MOD and ** ----
    oracle.replace_grammar(
        "ArithmeticBinaryOperatorGrammar",
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
            Ref::new("ModOperatorSegment").to_matchable(),
            Ref::new("PowerOperatorSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- Expression_D_Grammar ----
    // SQLFluff: Completely rewritten for Oracle with PlusJoinGrammar, subscript access,
    // trigger correlation references, implicit cursor attributes, etc.
    oracle.replace_grammar(
        "Expression_D_Grammar",
        Sequence::new(vec![
            one_of(vec![
                Ref::new("BareFunctionSegment").to_matchable(),
                Ref::new("FunctionSegment").to_matchable(),
                Ref::new("TriggerCorrelationReferenceSegment").to_matchable(),
                Bracketed::new(vec![
                    one_of(vec![
                        Ref::new("ExpressionSegment").to_matchable(),
                        Ref::new("SelectableGrammar").to_matchable(),
                        Delimited::new(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("FunctionSegment").to_matchable(),
                            Ref::new("LiteralGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| {
                    config.parse_mode(ParseMode::Greedy);
                })
                .to_matchable(),
                Ref::new("SelectStatementSegment").to_matchable(),
                Ref::new("LiteralGrammar").to_matchable(),
                Ref::new("IntervalExpressionSegment").to_matchable(),
                Ref::new("TypedStructLiteralSegment").to_matchable(),
                Ref::new("ArrayExpressionSegment").to_matchable(),
                Ref::new("ColumnReferenceSegment").to_matchable(),
                // NEW.* / OLD.* for triggers
                Sequence::new(vec![
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ObjectReferenceDelimiterGrammar").to_matchable(),
                    Ref::new("StarSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::new("DatatypeSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                        Ref::new("BooleanLiteralGrammar").to_matchable(),
                        Ref::new("NullLiteralSegment").to_matchable(),
                        Ref::new("DateTimeLiteralGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SqlplusSubstitutionVariableSegment").to_matchable(),
                Ref::new("ImplicitCursorAttributesGrammar").to_matchable(),
                // ObjectReference with optional subscript and trailing dot (PL/SQL array access)
                Sequence::new(vec![
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                            Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| {
                        config.optional();
                    })
                    .to_matchable(),
                    Ref::new("DotSegment").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|config| {
                config.terminators = vec![Ref::new("CommaSegment").to_matchable()];
            })
            .to_matchable(),
            Ref::new("AccessorGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- FromClauseTerminatorGrammar ----
    // Add FOR (for FOR UPDATE), CONNECT, START, PIVOT, UNPIVOT as FROM clause terminators
    oracle.replace_grammar(
        "FromClauseTerminatorGrammar",
        one_of(vec![
            Ref::keyword("WHERE").to_matchable(),
            Ref::keyword("LIMIT").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("GROUP").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ORDER").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("HAVING").to_matchable(),
            Ref::keyword("QUALIFY").to_matchable(),
            Ref::keyword("WINDOW").to_matchable(),
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::new("WithNoSchemaBindingClauseSegment").to_matchable(),
            Ref::new("WithDataClauseSegment").to_matchable(),
            Ref::keyword("FETCH").to_matchable(),
            // Oracle-specific additions
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                Ref::keyword("UPDATE").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("CONNECT").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("START").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("PIVOT").to_matchable(),
            Ref::keyword("UNPIVOT").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- WhereClauseTerminatorGrammar ----
    // Add FOR UPDATE, CONNECT BY, START WITH, PIVOT, UNPIVOT
    oracle.replace_grammar(
        "WhereClauseTerminatorGrammar",
        one_of(vec![
            Ref::keyword("LIMIT").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("GROUP").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ORDER").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("HAVING").to_matchable(),
            Ref::keyword("QUALIFY").to_matchable(),
            Ref::keyword("WINDOW").to_matchable(),
            Ref::keyword("OVERLAPS").to_matchable(),
            Ref::keyword("FETCH").to_matchable(),
            // Oracle-specific additions
            Sequence::new(vec![
                Ref::keyword("FOR").to_matchable(),
                Ref::keyword("UPDATE").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("CONNECT").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("START").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("PIVOT").to_matchable(),
            Ref::keyword("UNPIVOT").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- LiteralGrammar ----
    // SQLFluff inserts TriggerCorrelationReferenceSegment, SqlplusVariableGrammar,
    // LEVEL, ROWNUM, ANY before ArrayLiteralSegment.
    oracle.replace_grammar(
        "LiteralGrammar",
        one_of(vec![
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Ref::new("NumericLiteralSegment").to_matchable(),
            Ref::new("BooleanLiteralGrammar").to_matchable(),
            Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
            Ref::new("NullLiteralSegment").to_matchable(),
            Ref::new("DateTimeLiteralGrammar").to_matchable(),
            // Oracle-specific additions
            Ref::new("TriggerCorrelationReferenceSegment").to_matchable(),
            Ref::new("SqlplusVariableGrammar").to_matchable(),
            Ref::keyword("LEVEL").to_matchable(),
            Ref::keyword("ROWNUM").to_matchable(),
            Ref::keyword("ANY").to_matchable(),
            // ANSI items continued
            Ref::new("ArrayLiteralSegment").to_matchable(),
            Ref::new("TypedArrayLiteralSegment").to_matchable(),
            Ref::new("ObjectLiteralSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- ColumnConstraintDefaultGrammar ----
    // SQLFluff: OneOf(ansi_dialect.get_grammar("ColumnConstraintDefaultGrammar"), Ref("SequenceNextValGrammar"))
    oracle.replace_grammar(
        "ColumnConstraintDefaultGrammar",
        one_of(vec![
            Ref::new("ShorthandCastSegment").to_matchable(),
            Ref::new("LiteralGrammar").to_matchable(),
            Ref::new("FunctionSegment").to_matchable(),
            Ref::new("BareFunctionSegment").to_matchable(),
            Ref::new("ExpressionSegment").to_matchable(),
            Ref::new("SequenceNextValGrammar").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- SelectClauseTerminatorGrammar ----
    // SQLFluff adds BULK, INTO, FETCH as terminators
    oracle.replace_grammar(
        "SelectClauseTerminatorGrammar",
        one_of(vec![
            Ref::keyword("BULK").to_matchable(),
            Ref::keyword("INTO").to_matchable(),
            Ref::new("FromClauseSegment").to_matchable(),
            Ref::keyword("WHERE").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ORDER").to_matchable(),
                Ref::keyword("BY").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("LIMIT").to_matchable(),
            Ref::keyword("OVERLAPS").to_matchable(),
            Ref::new("SetOperatorSegment").to_matchable(),
            Ref::keyword("FETCH").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- UpdateStatementSegment: add RETURNING clause ----
    // SQLFluff: ansi.UpdateStatementSegment.match_grammar.copy(insert=[Ref("ReturningClauseSegment", optional=True)])
    oracle.add([(
        "UpdateStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleUpdateStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("UPDATE").to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("AliasExpressionSegment")
                    .exclude(Ref::keyword("SET"))
                    .optional()
                    .to_matchable(),
                Ref::new("SetClauseListSegment").to_matchable(),
                Ref::new("FromClauseSegment").optional().to_matchable(),
                Ref::new("WhereClauseSegment").optional().to_matchable(),
                Ref::new("ReturningClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- DeleteStatementSegment: add RETURNING clause ----
    // SQLFluff: ansi.DeleteStatementSegment.match_grammar.copy(insert=[Ref("ReturningClauseSegment", optional=True)])
    oracle.add([(
        "DeleteStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleDeleteStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("DELETE").to_matchable(),
                Ref::new("FromClauseSegment").to_matchable(),
                Ref::new("WhereClauseSegment").optional().to_matchable(),
                Ref::new("ReturningClauseSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- InsertStatementSegment ----
    // SQLFluff: supports INSERT INTO (subquery), INSERT ALL, RETURNING INTO, LOG ERRORS
    oracle.add([(
        "InsertStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleInsertStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("INSERT").to_matchable(),
                one_of(vec![
                    // Standard INSERT INTO
                    Sequence::new(vec![
                        Ref::keyword("INTO").to_matchable(),
                        one_of(vec![
                            Ref::new("TableReferenceSegment").to_matchable(),
                            Bracketed::new(vec![Ref::new("SelectStatementSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("AliasExpressionSegment")
                            .exclude(one_of(vec![
                                Ref::keyword("VALUES").to_matchable(),
                                Ref::keyword("VALUE").to_matchable(),
                                Ref::keyword("SET").to_matchable(),
                                Ref::keyword("SELECT").to_matchable(),
                                Ref::keyword("WITH").to_matchable(),
                            ]))
                            .optional()
                            .to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                        one_of(vec![
                            Ref::new("ValuesClauseSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SET").to_matchable(),
                                Delimited::new(vec![Ref::new("SetClauseSegment").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("SelectableGrammar").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                        Ref::new("ReturningClauseSegment").optional().to_matchable(),
                        // LOG ERRORS clause
                        Sequence::new(vec![
                            Ref::keyword("LOG").to_matchable(),
                            Ref::keyword("ERRORS").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("INTO").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                            Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("REJECT").to_matchable(),
                                Ref::keyword("LIMIT").to_matchable(),
                                one_of(vec![
                                    Ref::new("NumericLiteralSegment").to_matchable(),
                                    Ref::keyword("UNLIMITED").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // INSERT ALL
                    Sequence::new(vec![
                        Ref::keyword("ALL").to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("INTO").to_matchable(),
                                Ref::new("TableReferenceSegment").to_matchable(),
                                Ref::new("AliasExpressionSegment").optional().to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        Ref::new("ColumnReferenceSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                                Ref::new("ValuesClauseSegment").optional().to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.min_times = 1;
                        })
                        .to_matchable(),
                        Ref::new("SelectableGrammar").to_matchable(),
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

    // ---- OrderByClauseSegment: add SIBLINGS support ----
    // SQLFluff: ORDER [SIBLINGS] BY ...
    oracle.add([(
        "OrderByClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleOrderByClause, |_| {
            Sequence::new(vec![
                Ref::keyword("ORDER").to_matchable(),
                Ref::keyword("SIBLINGS").optional().to_matchable(),
                Ref::keyword("BY").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Delimited::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("ExpressionSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("ASC").to_matchable(),
                            Ref::keyword("DESC").to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("NULLS").to_matchable(),
                            one_of(vec![
                                Ref::keyword("FIRST").to_matchable(),
                                Ref::keyword("LAST").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        Ref::keyword("LIMIT").to_matchable(),
                        Ref::new("FrameClauseUnitGrammar").to_matchable(),
                    ]
                })
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- Comparison operator overrides to allow spaces ----
    // SQLFluff: GreaterThanOrEqualToSegment, LessThanOrEqualToSegment, NotEqualToSegment
    // Oracle allows spaces between composite comparison operators: `> =`, `< =`, `! =`
    oracle.add([
        (
            "GreaterThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("RawGreaterThanSegment").to_matchable(),
                        Ref::new("RawEqualsSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("RawNotSegment").to_matchable(),
                        Ref::new("RawLessThanSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "LessThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("RawLessThanSegment").to_matchable(),
                        Ref::new("RawEqualsSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("RawNotSegment").to_matchable(),
                        Ref::new("RawGreaterThanSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "NotEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                one_of(vec![
                    Sequence::new(vec![
                        Ref::new("RawNotSegment").to_matchable(),
                        Ref::new("RawEqualsSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::new("RawLessThanSegment").to_matchable(),
                        Ref::new("RawGreaterThanSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ---- CreateSequenceOptionsSegment override ----
    // SQLFluff Oracle uses NOMINVALUE/NOMAXVALUE (single keywords) instead of NO MINVALUE/NO MAXVALUE
    oracle.add([(
        "CreateSequenceOptionsSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateSequenceOptionsSegment, |_| {
            one_of(vec![
                Sequence::new(vec![
                    Ref::keyword("INCREMENT").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("START").to_matchable(),
                    Ref::keyword("WITH").optional().to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("SequenceMinValueGrammar").to_matchable(),
                Ref::new("SequenceMaxValueGrammar").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CACHE").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("NOCACHE").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("CYCLE").to_matchable(),
                    Ref::keyword("NOCYCLE").to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("ORDER").to_matchable(),
                    Ref::keyword("NOORDER").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- DropTypeStatementSegment override ----
    // SQLFluff: adds BODY keyword and FORCE/VALIDATE options
    oracle.add([(
        "DropTypeStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropTypeStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("DROP").to_matchable(),
                Ref::keyword("TYPE").to_matchable(),
                Ref::keyword("BODY").optional().to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("FORCE").to_matchable(),
                    Ref::keyword("VALIDATE").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Ref::new("DropBehaviorGrammar").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- AlterFunctionStatementSegment: add IF EXISTS ----
    // SQLFluff: Ref("IfExistsGrammar", optional=True) before function name
    oracle.add([(
        "AlterFunctionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleAlterFunctionStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                one_of(vec![
                    Ref::keyword("FUNCTION").to_matchable(),
                    Ref::keyword("PROCEDURE").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("FunctionNameSegment").to_matchable(),
                one_of(vec![
                    Ref::new("CompileClauseGrammar").to_matchable(),
                    Ref::keyword("EDITIONABLE").to_matchable(),
                    Ref::keyword("NONEDITIONABLE").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- AlterIndexStatementSegment: add PARAMETERS ----
    // SQLFluff includes PARAMETERS(quoted) option
    oracle.add([(
        "AlterIndexStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleAlterIndexStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("INDEX").to_matchable(),
                Ref::new("IndexReferenceSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("REBUILD").to_matchable(),
                        one_of(vec![
                            Ref::keyword("REVERSE").to_matchable(),
                            Ref::keyword("NOREVERSE").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("MONITORING").to_matchable(),
                        Ref::keyword("USAGE").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOMONITORING").to_matchable(),
                        Ref::keyword("USAGE").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PARAMETERS").to_matchable(),
                        Bracketed::new(vec![Ref::new("QuotedLiteralSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("RENAME").to_matchable(),
                        Ref::keyword("TO").to_matchable(),
                        Ref::new("IndexReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("COMPILE").to_matchable(),
                    Ref::keyword("LOGGING").to_matchable(),
                    Ref::keyword("NOLOGGING").to_matchable(),
                    Ref::keyword("ENABLE").to_matchable(),
                    Ref::keyword("DISABLE").to_matchable(),
                    Ref::keyword("UNUSABLE").to_matchable(),
                    Ref::keyword("INVISIBLE").to_matchable(),
                    Ref::keyword("VISIBLE").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- AlterDatabaseLinkStatementSegment: add AUTHENTICATED BY ----
    // SQLFluff includes DBLinkAuthenticationGrammar
    oracle.add([(
        "AlterDatabaseLinkStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleAlterDatabaseLinkStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("SHARED").optional().to_matchable(),
                Ref::keyword("PUBLIC").optional().to_matchable(),
                Ref::keyword("DATABASE").to_matchable(),
                Ref::keyword("LINK").to_matchable(),
                Ref::new("IfExistsGrammar").optional().to_matchable(),
                Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CONNECT").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("TO").to_matchable(),
                                Ref::new("RoleReferenceSegment").to_matchable(),
                                Ref::keyword("IDENTIFIED").to_matchable(),
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                                Ref::new("DBLinkAuthenticationGrammar")
                                    .optional()
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("WITH").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DBLinkAuthenticationGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- AlterSessionStatementSegment: fuller version ----
    // SQLFluff has many more SET options including ISOLATION_LEVEL with special handling
    oracle.add([(
        "AlterSessionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleAlterSessionStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("SESSION").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("ADVISE").to_matchable(),
                        one_of(vec![
                            Ref::keyword("COMMIT").to_matchable(),
                            Ref::keyword("ROLLBACK").to_matchable(),
                            Ref::keyword("NOTHING").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CLOSE").to_matchable(),
                        Ref::keyword("DATABASE").to_matchable(),
                        Ref::keyword("LINK").to_matchable(),
                        Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ENABLE").to_matchable(),
                            Ref::keyword("DISABLE").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("GUARD").to_matchable(),
                            Ref::keyword("RESUMABLE").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COMMIT").to_matchable(),
                                Ref::keyword("IN").to_matchable(),
                                Ref::keyword("PROCEDURE").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("PARALLEL").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("DML").to_matchable(),
                                    Ref::keyword("DDL").to_matchable(),
                                    Ref::keyword("QUERY").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SHARD").to_matchable(),
                                Ref::keyword("DDL").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FORCE").to_matchable(),
                        Ref::keyword("PARALLEL").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DML").to_matchable(),
                            Ref::keyword("DDL").to_matchable(),
                            Ref::keyword("QUERY").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("PARALLEL").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SYNC").to_matchable(),
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("PRIMARY").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SET").to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("ISOLATION_LEVEL").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("SERIALIZABLE").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("READ").to_matchable(),
                                        Ref::keyword("COMMITTED").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("CONSTRAINT").to_matchable(),
                                    Ref::keyword("CONSTRAINTS").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("IMMEDIATE").to_matchable(),
                                    Ref::keyword("DEFERRED").to_matchable(),
                                    Ref::keyword("DEFAULT").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("TIME_ZONE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("LOCAL").to_matchable(),
                                    Ref::keyword("DBTIMEZONE").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.min_times = 1;
                        })
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

    // ---- AlterTableStatementSegment: handle MODIFY ----
    // SQLFluff: supports MODIFY column
    oracle.add([(
        "AlterTableColumnClausesSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterTableColumnClauses, |_| {
            one_of(vec![
                // ADD / MODIFY
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("ADD").to_matchable(),
                        Ref::keyword("MODIFY").to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::new("ColumnDefinitionSegment").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                Ref::new("ColumnDefinitionSegment").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // DROP COLUMN / DROP (cols)
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("COLUMN").to_matchable(),
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // RENAME COLUMN
                Sequence::new(vec![
                    Ref::keyword("RENAME").to_matchable(),
                    Ref::keyword("COLUMN").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::keyword("TO").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                // SET UNUSED
                Sequence::new(vec![
                    Ref::keyword("SET").to_matchable(),
                    Ref::keyword("UNUSED").to_matchable(),
                    one_of(vec![
                        Sequence::new(vec![
                            Ref::keyword("COLUMN").to_matchable(),
                            Ref::new("ColumnReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
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

    // ---- ColumnDefinitionSegment: handle BYTE/CHAR qualifier in type brackets ----
    // SQLFluff: Bracketed(Anything(), OneOf("BYTE", "CHAR", optional=True), optional=True)
    oracle.add([(
        "ColumnDefinitionSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleColumnDefinition, |_| {
            Sequence::new(vec![
                Ref::new("SingleIdentifierGrammar").to_matchable(),
                one_of(vec![
                    // Column with data type
                    Sequence::new(vec![
                        Ref::new("DatatypeSegment").to_matchable(),
                        Bracketed::new(vec![
                            Ref::new("ExpressionSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("BYTE").to_matchable(),
                                Ref::keyword("CHAR").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.optional();
                        })
                        .to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::new("ColumnConstraintSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ENABLE").to_matchable(),
                                    Ref::keyword("DISABLE").to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("IdentityClauseGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // Column with only constraints (no data type)
                    AnyNumberOf::new(vec![
                        Sequence::new(vec![
                            Ref::new("ColumnConstraintSegment").to_matchable(),
                            one_of(vec![
                                Ref::keyword("ENABLE").to_matchable(),
                                Ref::keyword("DISABLE").to_matchable(),
                            ])
                            .config(|config| {
                                config.optional();
                            })
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

    // ---- FunctionContentsGrammar: add ListaggOverflow and JSONObjectContent ----
    // SQLFluff: ansi_dialect.get_grammar("FunctionContentsGrammar").copy(
    //     insert=[Ref("ListaggOverflowClauseSegment"), Ref("JSONObjectContentSegment")])
    {
        let existing = oracle.grammar("FunctionContentsGrammar");
        oracle.replace_grammar(
            "FunctionContentsGrammar",
            existing.copy(
                Some(vec![
                    Ref::new("ListaggOverflowClauseSegment").to_matchable(),
                    Ref::new("JSONObjectContentSegment").to_matchable(),
                ]),
                None,
                None,
                None,
                vec![],
                false,
            ),
        );
    }

    // ---- AccessStatementSegment: delegate to Grant/Revoke ----
    oracle.add([(
        "AccessStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
            one_of(vec![
                Ref::new("GrantStatementSegment").to_matchable(),
                Ref::new("RevokeStatementSegment").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- CreateUserStatementSegment ----
    // SQLFluff: CREATE USER with IDENTIFIED BY/EXTERNALLY/GLOBALLY
    oracle.add([(
        "CreateUserStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleCreateUserStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("USER").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("RoleReferenceSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("IDENTIFIED").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("EXTERNALLY").to_matchable(),
                                    Ref::keyword("GLOBALLY").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("AS").to_matchable(),
                                    one_of(vec![
                                        Ref::new("QuotedIdentifierSegment").to_matchable(),
                                        Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|config| {
                                    config.optional();
                                })
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NO").to_matchable(),
                        Ref::keyword("AUTHENTICATION").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("TABLESPACE").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TEMPORARY").to_matchable(),
                        Ref::keyword("TABLESPACE").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("QUOTA").to_matchable(),
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::keyword("UNLIMITED").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PROFILE").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PASSWORD").to_matchable(),
                        Ref::keyword("EXPIRE").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ACCOUNT").to_matchable(),
                        one_of(vec![
                            Ref::keyword("LOCK").to_matchable(),
                            Ref::keyword("UNLOCK").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CONTAINER").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("CURRENT").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
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

    // ---- AlterSessionStatementSegment: add ROW ARCHIVAL VISIBILITY and CONTAINER ----
    // SQLFluff has many SET options; our simplified version adds the missing ones
    oracle.add([(
        "AlterSessionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleAlterSessionStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("ALTER").to_matchable(),
                Ref::keyword("SESSION").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("ADVISE").to_matchable(),
                        one_of(vec![
                            Ref::keyword("COMMIT").to_matchable(),
                            Ref::keyword("ROLLBACK").to_matchable(),
                            Ref::keyword("NOTHING").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CLOSE").to_matchable(),
                        Ref::keyword("DATABASE").to_matchable(),
                        Ref::keyword("LINK").to_matchable(),
                        Ref::new("DatabaseLinkReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::keyword("ENABLE").to_matchable(),
                            Ref::keyword("DISABLE").to_matchable(),
                        ])
                        .to_matchable(),
                        one_of(vec![
                            Ref::keyword("GUARD").to_matchable(),
                            Ref::keyword("RESUMABLE").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("COMMIT").to_matchable(),
                                Ref::keyword("IN").to_matchable(),
                                Ref::keyword("PROCEDURE").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("PARALLEL").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("DML").to_matchable(),
                                    Ref::keyword("DDL").to_matchable(),
                                    Ref::keyword("QUERY").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("SHARD").to_matchable(),
                                Ref::keyword("DDL").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FORCE").to_matchable(),
                        Ref::keyword("PARALLEL").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DML").to_matchable(),
                            Ref::keyword("DDL").to_matchable(),
                            Ref::keyword("QUERY").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SYNC").to_matchable(),
                        Ref::keyword("WITH").to_matchable(),
                        Ref::keyword("PRIMARY").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("SET").to_matchable(),
                        AnyNumberOf::new(vec![
                            Sequence::new(vec![
                                Ref::keyword("ISOLATION_LEVEL").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("SERIALIZABLE").to_matchable(),
                                    Sequence::new(vec![
                                        Ref::keyword("READ").to_matchable(),
                                        Ref::keyword("COMMITTED").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("CONSTRAINT").to_matchable(),
                                    Ref::keyword("CONSTRAINTS").to_matchable(),
                                ])
                                .to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("IMMEDIATE").to_matchable(),
                                    Ref::keyword("DEFERRED").to_matchable(),
                                    Ref::keyword("DEFAULT").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("TIME_ZONE").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("LOCAL").to_matchable(),
                                    Ref::keyword("DBTIMEZONE").to_matchable(),
                                    Ref::new("QuotedLiteralSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            // ROW ARCHIVAL VISIBILITY = ACTIVE/ALL
                            Sequence::new(vec![
                                Ref::keyword("ROW").to_matchable(),
                                Ref::keyword("ARCHIVAL").to_matchable(),
                                Ref::keyword("VISIBILITY").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ACTIVE").to_matchable(),
                                    Ref::keyword("ALL").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            // CONTAINER = object
                            Sequence::new(vec![
                                Ref::keyword("CONTAINER").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                Ref::new("ObjectReferenceSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            // Generic: param = value
                            Sequence::new(vec![
                                Ref::new("ParameterNameSegment").to_matchable(),
                                Ref::new("EqualsSegment").to_matchable(),
                                one_of(vec![
                                    Ref::keyword("DEFAULT").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|config| {
                            config.min_times = 1;
                        })
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

    // ---- AccessPermissionSegment: Oracle system privileges ----
    // SQLFluff: long list of Oracle-specific privilege keywords
    oracle.add([(
        "AccessPermissionSegment".into(),
        one_of(vec![
            Ref::keyword("ADMINISTER").to_matchable(),
            Ref::keyword("ADVISOR").to_matchable(),
            Ref::keyword("ALL").to_matchable(),
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("ANALYZE").to_matchable(),
            Ref::keyword("AUDIT").to_matchable(),
            Ref::keyword("BACKUP").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("BECOME").to_matchable(),
                Ref::keyword("USER").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("COMMENT").to_matchable(),
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("DEBUG").to_matchable(),
            Ref::keyword("DELETE").to_matchable(),
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("EXECUTE").to_matchable(),
            Ref::keyword("EXEMPT").to_matchable(),
            Ref::keyword("FLASHBACK").to_matchable(),
            Ref::keyword("FORCE").to_matchable(),
            Ref::keyword("GRANT").to_matchable(),
            Ref::keyword("INDEX").to_matchable(),
            Ref::keyword("INHERIT").to_matchable(),
            Ref::keyword("INSERT").to_matchable(),
            Ref::keyword("KEEP").to_matchable(),
            Ref::keyword("LOCK").to_matchable(),
            Ref::keyword("LOGMINING").to_matchable(),
            Ref::keyword("MANAGE").to_matchable(),
            Ref::keyword("MERGE").to_matchable(),
            Ref::keyword("READ").to_matchable(),
            Ref::keyword("REFERENCES").to_matchable(),
            Ref::keyword("RESTRICTED").to_matchable(),
            Ref::keyword("SELECT").to_matchable(),
            Ref::keyword("SET").to_matchable(),
            Ref::keyword("TRANSLATE").to_matchable(),
            Ref::keyword("UNDER").to_matchable(),
            Ref::keyword("UNLIMITED").to_matchable(),
            Ref::keyword("UPDATE").to_matchable(),
            Ref::keyword("USE").to_matchable(),
            Ref::keyword("WRITE").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // ---- AccessObjectSegment: Oracle access object types ----
    // SQLFluff: SESSION, TABLE, VIEW, etc. as standalone keywords matching access targets
    oracle.add([(
        "AccessObjectSegment".into(),
        one_of(vec![
            Sequence::new(vec![
                Ref::keyword("CONNECT").to_matchable(),
                Ref::keyword("SESSION").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("CLUSTER").to_matchable(),
            Ref::keyword("CONTAINER").to_matchable(),
            Ref::keyword("CONTEXT").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("DATABASE").to_matchable(),
                one_of(vec![
                    Ref::keyword("LINK").to_matchable(),
                    Ref::keyword("TRIGGER").to_matchable(),
                ])
                .config(|c| c.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("DICTIONARY").to_matchable(),
            Ref::keyword("DIMENSION").to_matchable(),
            Ref::keyword("DIRECTORY").to_matchable(),
            Ref::keyword("EDITION").to_matchable(),
            Ref::keyword("HIERARCHY").to_matchable(),
            Ref::keyword("INDEX").to_matchable(),
            Ref::keyword("INDEXTYPE").to_matchable(),
            Ref::keyword("JOB").to_matchable(),
            Ref::keyword("LIBRARY").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("LOCKDOWN").to_matchable(),
                Ref::keyword("PROFILE").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("MATERIALIZED").to_matchable(),
                Ref::keyword("VIEW").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("MINING").to_matchable(),
                Ref::keyword("MODEL").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("OPERATOR").to_matchable(),
            Ref::keyword("OUTLINE").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("PLUGGABLE").to_matchable(),
                Ref::keyword("DATABASE").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("PRIVILEGE").to_matchable(),
            Ref::keyword("PRIVILEGES").to_matchable(),
            Ref::keyword("PROCEDURE").to_matchable(),
            Ref::keyword("PROFILE").to_matchable(),
            Ref::keyword("PROGRAM").to_matchable(),
            Ref::keyword("ROLE").to_matchable(),
            Ref::keyword("SCHEDULER").to_matchable(),
            Ref::keyword("SEQUENCE").to_matchable(),
            Ref::keyword("SESSION").to_matchable(),
            Ref::keyword("SYNONYM").to_matchable(),
            Ref::keyword("SYSTEM").to_matchable(),
            Ref::keyword("TABLE").to_matchable(),
            Ref::keyword("TABLESPACE").to_matchable(),
            Ref::keyword("TRANSACTION").to_matchable(),
            Ref::keyword("TRIGGER").to_matchable(),
            Ref::keyword("TYPE").to_matchable(),
            Ref::keyword("USER").to_matchable(),
            Ref::keyword("VIEW").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // ---- AccessPermissionsSegment: Oracle permission+object combos ----
    // SQLFluff: Delimited(Sequence(permission, ANY?, object?), role)
    oracle.add([(
        "AccessPermissionsSegment".into(),
        Delimited::new(vec![
            Sequence::new(vec![
                Ref::new("AccessPermissionSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("ANY").to_matchable(),
                    Ref::keyword("PUBLIC").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Ref::new("AccessObjectSegment").optional().to_matchable(),
            ])
            .to_matchable(),
            Ref::new("RoleReferenceSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // ---- GrantStatementSegment ----
    // SQLFluff: Oracle GRANT with system privs, ON clause, CONTAINER
    oracle.add([(
        "GrantStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("GRANT").to_matchable(),
                Ref::new("AccessPermissionsSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    one_of(vec![
                        Ref::keyword("USER").to_matchable(),
                        Ref::keyword("DIRECTORY").to_matchable(),
                        Ref::keyword("EDITION").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MINING").to_matchable(),
                            Ref::keyword("MODEL").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("JAVA").to_matchable(),
                            one_of(vec![
                                Ref::keyword("SOURCE").to_matchable(),
                                Ref::keyword("RESOURCE").to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("SQL").to_matchable(),
                            Ref::keyword("TRANSLATION").to_matchable(),
                            Ref::keyword("PROFILE").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Ref::keyword("TO").to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Ref::keyword("PUBLIC").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("IDENTIFIED").to_matchable(),
                    Ref::keyword("BY").to_matchable(),
                    Delimited::new(vec![Ref::new("SingleIdentifierGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WITH").to_matchable(),
                    one_of(vec![
                        Ref::keyword("ADMIN").to_matchable(),
                        Ref::keyword("DELEGATE").to_matchable(),
                        Ref::keyword("GRANT").to_matchable(),
                        Ref::keyword("HIERARCHY").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("OPTION").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CONTAINER").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("CURRENT").to_matchable(),
                        Ref::keyword("ALL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- RevokeStatementSegment ----
    // SQLFluff: Oracle REVOKE
    oracle.add([(
        "RevokeStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("REVOKE").to_matchable(),
                Ref::new("AccessPermissionsSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ON").to_matchable(),
                    one_of(vec![
                        Ref::keyword("USER").to_matchable(),
                        Ref::keyword("DIRECTORY").to_matchable(),
                        Ref::keyword("EDITION").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("MINING").to_matchable(),
                            Ref::keyword("MODEL").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                Delimited::new(vec![
                    one_of(vec![
                        Ref::keyword("PUBLIC").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("CASCADE").to_matchable(),
                        Ref::keyword("CONSTRAINTS").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FORCE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CONTAINER").to_matchable(),
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::keyword("CURRENT").to_matchable(),
                        Ref::keyword("ALL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- Fix CreateUserStatementSegment QUOTA with size unit ----
    oracle.add([(
        "CreateUserStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleCreateUserStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("USER").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("RoleReferenceSegment").to_matchable(),
                one_of(vec![
                    Sequence::new(vec![
                        Ref::keyword("IDENTIFIED").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::keyword("BY").to_matchable(),
                                Ref::new("SingleIdentifierGrammar").to_matchable(),
                            ])
                            .to_matchable(),
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("EXTERNALLY").to_matchable(),
                                    Ref::keyword("GLOBALLY").to_matchable(),
                                ])
                                .to_matchable(),
                                Sequence::new(vec![
                                    Ref::keyword("AS").to_matchable(),
                                    one_of(vec![
                                        Ref::new("QuotedIdentifierSegment").to_matchable(),
                                        Ref::new("SingleQuotedIdentifierSegment").to_matchable(),
                                    ])
                                    .to_matchable(),
                                ])
                                .config(|config| config.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NO").to_matchable(),
                        Ref::keyword("AUTHENTICATION").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::keyword("TABLESPACE").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("LOCAL").optional().to_matchable(),
                        Ref::keyword("TEMPORARY").to_matchable(),
                        Ref::keyword("TABLESPACE").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // QUOTA size ON tablespace — size can be "10M", "5G" etc.
                    Sequence::new(vec![
                        Ref::keyword("QUOTA").to_matchable(),
                        one_of(vec![
                            Sequence::new(vec![
                                Ref::new("NumericLiteralSegment").to_matchable(),
                                // Size suffix like K, M, G, T, P, E
                                Ref::new("SingleIdentifierGrammar")
                                    .optional()
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::keyword("UNLIMITED").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("ON").to_matchable(),
                        Ref::new("ObjectReferenceSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PROFILE").to_matchable(),
                        one_of(vec![
                            Ref::keyword("DEFAULT").to_matchable(),
                            Ref::new("ObjectReferenceSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("PASSWORD").to_matchable(),
                        Ref::keyword("EXPIRE").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ACCOUNT").to_matchable(),
                        one_of(vec![
                            Ref::keyword("LOCK").to_matchable(),
                            Ref::keyword("UNLOCK").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("ENABLE").to_matchable(),
                        Ref::keyword("EDITIONS").to_matchable(),
                    ])
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("CONTAINER").to_matchable(),
                        Ref::new("EqualsSegment").to_matchable(),
                        one_of(vec![
                            Ref::keyword("CURRENT").to_matchable(),
                            Ref::keyword("ALL").to_matchable(),
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

    // ---- CreatePackageStatementSegment: add IF NOT EXISTS ----
    oracle.add([(
        "CreatePackageStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleCreatePackageStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OR").to_matchable(),
                    Ref::keyword("REPLACE").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("EDITIONABLE").to_matchable(),
                    Ref::keyword("NONEDITIONABLE").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                Ref::keyword("PACKAGE").to_matchable(),
                Ref::keyword("BODY").optional().to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Ref::new("SharingClauseGrammar").optional().to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::new("DefaultCollationClauseGrammar").to_matchable(),
                    Ref::new("InvokerRightsClauseGrammar").to_matchable(),
                    Ref::new("AccessibleByClauseGrammar").to_matchable(),
                ])
                .config(|config| {
                    config.optional();
                })
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("IS").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DeclareSegment").to_matchable(),
                Ref::keyword("END").to_matchable(),
                Ref::new("ObjectReferenceSegment").optional().to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- SingleIdentifierGrammar: add SqlplusSubstitutionVariable ----
    // SQLFluff: ansi SingleIdentifierGrammar.copy(insert=[Ref("SqlplusSubstitutionVariableSegment")])
    {
        let existing = oracle.grammar("SingleIdentifierGrammar");
        oracle.replace_grammar(
            "SingleIdentifierGrammar",
            existing.copy(
                Some(vec![
                    Ref::new("SqlplusSubstitutionVariableSegment").to_matchable(),
                ]),
                None,
                None,
                None,
                vec![],
                false,
            ),
        );
    }

    // ---- BaseExpressionElementGrammar: add ConnectByRoot + SqlplusSubstitutionVariable ----
    // SQLFluff: ansi BaseExpressionElementGrammar.copy(insert=[ConnectByRootGrammar, SqlplusSubstitutionVariableSegment])
    {
        let existing = oracle.grammar("BaseExpressionElementGrammar");
        oracle.replace_grammar(
            "BaseExpressionElementGrammar",
            existing.copy(
                Some(vec![
                    Ref::new("ConnectByRootGrammar").to_matchable(),
                    Ref::new("SqlplusSubstitutionVariableSegment").to_matchable(),
                    Ref::new("TriggerPredicatesGrammar").to_matchable(),
                ]),
                None,
                None,
                None,
                vec![],
                false,
            ),
        );
    }

    // ---- TableExpressionSegment: add SqlplusSubstitutionVariable ----
    // SQLFluff: ansi.TableExpressionSegment.match_grammar.copy(insert=[Ref("SqlplusSubstitutionVariableSegment")])
    oracle.replace_grammar(
        "TableExpressionSegment",
        one_of(vec![
            Ref::new("ValuesClauseSegment").to_matchable(),
            Ref::new("BareFunctionSegment").to_matchable(),
            Ref::new("FunctionSegment").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()]).to_matchable(),
            Bracketed::new(vec![Ref::new("MergeStatementSegment").to_matchable()]).to_matchable(),
            Ref::new("SqlplusSubstitutionVariableSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // ---- Fix GRANT: add QUERY REWRITE to AccessPermissionSegment ----
    oracle.add([(
        "AccessPermissionSegment".into(),
        one_of(vec![
            Ref::keyword("ADMINISTER").to_matchable(),
            Ref::keyword("ADVISOR").to_matchable(),
            Ref::keyword("ALL").to_matchable(),
            Ref::keyword("ALTER").to_matchable(),
            Ref::keyword("ANALYZE").to_matchable(),
            Ref::keyword("AUDIT").to_matchable(),
            Ref::keyword("BACKUP").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("BECOME").to_matchable(),
                Ref::keyword("USER").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("COMMENT").to_matchable(),
            Ref::keyword("CREATE").to_matchable(),
            Ref::keyword("DEBUG").to_matchable(),
            Ref::keyword("DELETE").to_matchable(),
            Ref::keyword("DROP").to_matchable(),
            Ref::keyword("EXECUTE").to_matchable(),
            Ref::keyword("EXEMPT").to_matchable(),
            Ref::keyword("FLASHBACK").to_matchable(),
            Ref::keyword("FORCE").to_matchable(),
            Ref::keyword("GRANT").to_matchable(),
            Ref::keyword("INDEX").to_matchable(),
            Ref::keyword("INHERIT").to_matchable(),
            Ref::keyword("INSERT").to_matchable(),
            Ref::keyword("KEEP").to_matchable(),
            Ref::keyword("LOCK").to_matchable(),
            Ref::keyword("LOGMINING").to_matchable(),
            Ref::keyword("MANAGE").to_matchable(),
            Ref::keyword("MERGE").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("ON").to_matchable(),
                Ref::keyword("COMMIT").to_matchable(),
                Ref::keyword("REFRESH").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("PURGE").to_matchable(),
            // SQLFluff: Sequence(Ref.keyword("GLOBAL", optional=True), "QUERY", "REWRITE")
            Sequence::new(vec![
                Ref::keyword("GLOBAL").optional().to_matchable(),
                Ref::keyword("QUERY").to_matchable(),
                Ref::keyword("REWRITE").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("READ").to_matchable(),
            Ref::keyword("REDEFINE").to_matchable(),
            Ref::keyword("REFERENCES").to_matchable(),
            Ref::keyword("RESTRICTED").to_matchable(),
            Ref::keyword("RESUMABLE").to_matchable(),
            Ref::keyword("SELECT").to_matchable(),
            Ref::keyword("SET").to_matchable(),
            Ref::keyword("SIGN").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("TABLE").to_matchable(),
                Ref::keyword("RETENTION").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("TRANSLATE").to_matchable(),
            Ref::keyword("UNDER").to_matchable(),
            Ref::keyword("UNLIMITED").to_matchable(),
            Ref::keyword("UPDATE").to_matchable(),
            Ref::keyword("USE").to_matchable(),
            Ref::keyword("WRITE").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // ---- Fix CreateTypeStatementSegment: add IF NOT EXISTS + VARRAY OF (type) NOT PERSISTABLE ----
    oracle.add([(
        "CreateTypeStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleCreateTypeStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").optional().to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OR").to_matchable(),
                    Ref::keyword("REPLACE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("EDITIONABLE").to_matchable(),
                    Ref::keyword("NONEDITIONABLE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Ref::keyword("TYPE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("ObjectReferenceSegment").to_matchable(),
                Ref::keyword("FORCE").optional().to_matchable(),
                Ref::new("SharingClauseGrammar").optional().to_matchable(),
                Ref::new("DefaultCollationClauseGrammar")
                    .optional()
                    .to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::new("InvokerRightsClauseGrammar").to_matchable(),
                    Ref::new("AccessibleByClauseGrammar").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("IS").to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::new("ObjectTypeAndSubtypeDefGrammar").to_matchable(),
                    Ref::new("VarrayAndNestedTypeSpecGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- Fix VarrayAndNestedTypeSpecGrammar: support (type) NOT PERSISTABLE ----
    oracle.add([(
        "VarrayAndNestedTypeSpecGrammar".into(),
        Sequence::new(vec![
            one_of(vec![
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("VARRAY").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("VARYING").optional().to_matchable(),
                            Ref::keyword("ARRAY").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Bracketed::new(vec![Ref::new("NumericLiteralSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
            ])
            .to_matchable(),
            Ref::keyword("OF").to_matchable(),
            one_of(vec![
                // Bracketed type with optional NOT PERSISTABLE
                Sequence::new(vec![
                    Bracketed::new(vec![
                        Sequence::new(vec![
                            Ref::new("DatatypeSegment").to_matchable(),
                            Sequence::new(vec![
                                Ref::keyword("NOT").to_matchable(),
                                Ref::keyword("NULL").to_matchable(),
                            ])
                            .config(|config| config.optional())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("NOT").optional().to_matchable(),
                    Ref::keyword("PERSISTABLE").optional().to_matchable(),
                ])
                .to_matchable(),
                // Unbracketed type
                Sequence::new(vec![
                    Ref::new("DatatypeSegment").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("NOT").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // ---- Fix CreateTriggerStatementSegment: add REFERENCING clause ----
    // SQLFluff includes ReferencingClauseSegment after DmlEventClauseSegment
    oracle.add([(
        "ReferencingClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleReferencingClause, |_| {
            Sequence::new(vec![
                Ref::keyword("REFERENCING").to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("TriggerCorrelationNameSegment").to_matchable(),
                        Ref::keyword("AS").optional().to_matchable(),
                        Ref::new("NakedIdentifierSegment").to_matchable(),
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

    oracle.add([(
        "CreateTriggerStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::OracleCreateTriggerStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("OR").to_matchable(),
                    Ref::keyword("REPLACE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("EDITIONABLE").to_matchable(),
                    Ref::keyword("NONEDITIONABLE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Ref::keyword("TRIGGER").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TriggerReferenceSegment").to_matchable(),
                Ref::new("SharingClauseGrammar").optional().to_matchable(),
                Ref::new("DefaultCollationClauseGrammar")
                    .optional()
                    .to_matchable(),
                // Trigger timing and event
                Sequence::new(vec![
                    one_of(vec![
                        one_of(vec![
                            Ref::keyword("BEFORE").to_matchable(),
                            Ref::keyword("AFTER").to_matchable(),
                        ])
                        .to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("INSTEAD").to_matchable(),
                            Ref::keyword("OF").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("DmlEventClauseSegment").to_matchable(),
                ])
                .to_matchable(),
                // SQLFluff: ReferencingClauseSegment
                Ref::new("ReferencingClauseSegment")
                    .optional()
                    .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("EACH").to_matchable(),
                    Ref::keyword("ROW").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                // CROSSEDITION
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FORWARD").to_matchable(),
                        Ref::keyword("REVERSE").to_matchable(),
                    ])
                    .config(|config| config.optional())
                    .to_matchable(),
                    Ref::keyword("CROSSEDITION").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                // FOLLOWS / PRECEDES
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("FOLLOWS").to_matchable(),
                        Ref::keyword("PRECEDES").to_matchable(),
                    ])
                    .to_matchable(),
                    Delimited::new(vec![Ref::new("TriggerReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("ENABLE").to_matchable(),
                    Ref::keyword("DISABLE").to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("WHEN").to_matchable(),
                    Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                        .to_matchable(),
                ])
                .config(|config| config.optional())
                .to_matchable(),
                // Body: compound trigger or statements
                one_of(vec![
                    Ref::new("CompoundTriggerBlock").to_matchable(),
                    Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("END").optional().to_matchable(),
                Ref::new("TriggerReferenceSegment")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- CompoundTriggerBlock ----
    // SQLFluff: COMPOUND TRIGGER [DeclareSegment] AnyNumberOf(TimingPointSectionSegment)
    oracle.add([(
        "CompoundTriggerBlock".into(),
        NodeMatcher::new(SyntaxKind::CompoundTriggerStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("COMPOUND").to_matchable(),
                Ref::keyword("TRIGGER").to_matchable(),
                Ref::new("DeclareSegment").optional().to_matchable(),
                AnyNumberOf::new(vec![Ref::new("TimingPointSectionSegment").to_matchable()])
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ---- TimingPointSectionSegment ----
    oracle.add([(
        "TimingPointSectionSegment".into(),
        NodeMatcher::new(SyntaxKind::TimingPointSection, |_| {
            Sequence::new(vec![
                Ref::new("TimingPointGrammar").to_matchable(),
                Ref::keyword("IS").to_matchable(),
                Ref::keyword("BEGIN").to_matchable(),
                Ref::new("OneOrMoreStatementsGrammar").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("END").to_matchable(),
                    Ref::new("TimingPointGrammar").to_matchable(),
                ])
                .to_matchable(),
                Ref::new("DelimiterGrammar").to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    oracle
}
