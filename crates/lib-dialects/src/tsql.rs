// T-SQL (Transact-SQL) dialect implementation for Microsoft SQL Server

use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::Ref;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::lookahead::LookaheadExclude;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use crate::{ansi, tsql_keywords};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| dialect.expand())
}

pub fn raw_dialect() -> Dialect {
    // Start with ANSI SQL as the base dialect and customize for T-SQL
    let mut dialect = ansi::raw_dialect();
    dialect.name = DialectKind::Tsql;

    // Extend ANSI keywords with T-SQL specific keywords
    // IMPORTANT: Don't clear ANSI keywords as they contain fundamental SQL keywords
    dialect
        .sets_mut("reserved_keywords")
        .extend(tsql_keywords::tsql_additional_reserved_keywords());
    dialect
        .sets_mut("unreserved_keywords")
        .extend(tsql_keywords::tsql_additional_unreserved_keywords());

    // Add table hint keywords to unreserved keywords
    dialect.sets_mut("unreserved_keywords").extend([
        "NOLOCK",
        "READUNCOMMITTED",
        "READCOMMITTED",
        "REPEATABLEREAD",
        "SERIALIZABLE",
        "READPAST",
        "ROWLOCK",
        "TABLOCK",
        "TABLOCKX",
        "UPDLOCK",
        "XLOCK",
        "NOEXPAND",
        "INDEX",
        "FORCESEEK",
        "FORCESCAN",
        "HOLDLOCK",
        "SNAPSHOT",
    ]);

    // T-SQL specific operators
    dialect.sets_mut("operator_symbols").extend([
        "%=", "&=", "*=", "+=", "-=", "/=", "^=", "|=", // Compound assignment
        "!<", "!>", // Special comparison operators
    ]);

    // T-SQL supports square brackets for identifiers and @ for variables
    // Insert square bracket identifier before individual bracket matchers to ensure it's matched first
    dialect.insert_lexer_matchers(
        vec![
            // Square brackets for identifiers: [Column Name]
            Matcher::regex(
                "tsql_square_bracket_identifier",
                r"\[[^\]]*\]",
                SyntaxKind::DoubleQuote,
            ),
        ],
        "start_square_bracket",
    );

    // Insert other T-SQL specific matchers
    dialect.insert_lexer_matchers(
        vec![
            // Variables: @MyVar (local) or @@ROWCOUNT (global/system)
            Matcher::regex(
                "tsql_variable",
                r"@@?[a-zA-Z_][a-zA-Z0-9_]*",
                SyntaxKind::TsqlVariable,
            ),
        ],
        "equals",
    );

    // T-SQL specific lexer patches:
    // 1. T-SQL only uses -- for inline comments, not # (which is used in temp table names)
    // 2. Update word pattern to allow # at the beginning (temp tables) and end (SQL Server 2017+ syntax)
    dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"--[^\n]*", SyntaxKind::InlineComment),
        Matcher::regex(
            "word",
            r"##?[a-zA-Z0-9_]+|[0-9a-zA-Z_]+#?",
            SyntaxKind::Word,
        ),
    ]);

    // Since T-SQL uses square brackets as quoted identifiers and the lexer
    // already maps them to SyntaxKind::DoubleQuote, the ANSI QuotedIdentifierSegment
    // should handle them correctly. No additional parser configuration needed.

    // Add T-SQL specific bare functions
    dialect.sets_mut("bare_functions").extend([
        "CURRENT_TIMESTAMP",
        "CURRENT_USER",
        "SESSION_USER",
        "SYSTEM_USER",
        "USER",
    ]);

    // Add aggregate and other functions
    dialect
        .sets_mut("aggregate_functions")
        .extend(["STRING_AGG"]);

    dialect
        .sets_mut("special_functions")
        .extend(["COALESCE", "NULLIF", "ISNULL"]);

    // T-SQL datetime units
    dialect.sets_mut("datetime_units").extend([
        "YEAR",
        "YY",
        "YYYY",
        "QUARTER",
        "QQ",
        "Q",
        "MONTH",
        "MM",
        "M",
        "DAYOFYEAR",
        "DY",
        "Y",
        "DAY",
        "DD",
        "D",
        "WEEK",
        "WK",
        "WW",
        "WEEKDAY",
        "DW",
        "HOUR",
        "HH",
        "MINUTE",
        "MI",
        "N",
        "SECOND",
        "SS",
        "S",
        "MILLISECOND",
        "MS",
        "MICROSECOND",
        "MCS",
        "NANOSECOND",
        "NS",
    ]);

    // Add T-SQL specific date functions
    dialect.sets_mut("date_part_function_name").extend([
        "DATEADD",
        "DATEDIFF",
        "DATENAME",
        "DATEPART",
        "DAY",
        "MONTH",
        "YEAR",
        "GETDATE",
        "GETUTCDATE",
        "SYSDATETIME",
        "SYSUTCDATETIME",
        "SYSDATETIMEOFFSET",
    ]);

    // Add T-SQL string functions
    dialect.sets_mut("scalar_functions").extend([
        "SUBSTRING",
        "CHARINDEX",
        "LEN",
        "LEFT",
        "RIGHT",
        "LTRIM",
        "RTRIM",
        "REPLACE",
        "STUFF",
        "PATINDEX",
        "QUOTENAME",
        "REPLICATE",
        "REVERSE",
        "SPACE",
        "STR",
        "UNICODE",
    ]);

    // T-SQL specific value table functions
    dialect.sets_mut("value_table_functions").extend([
        "OPENROWSET",
        "OPENQUERY",
        "OPENDATASOURCE",
        "OPENXML",
    ]);

    // Add T-SQL specific grammar

    // TOP clause support (e.g., SELECT TOP 10, TOP (10) PERCENT, TOP 5 WITH TIES)
    // T-SQL allows DISTINCT/ALL followed by TOP
    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        AnyNumberOf::new(vec![
            Ref::keyword("DISTINCT").to_matchable(),
            Ref::keyword("ALL").to_matchable(),
            // TOP alone
            Sequence::new(vec![
                // https://docs.microsoft.com/en-us/sql/t-sql/queries/top-transact-sql
                Ref::keyword("TOP").to_matchable(),
                optionally_bracketed(vec![Ref::new("ExpressionSegment").to_matchable()])
                    .to_matchable(),
                Ref::keyword("PERCENT").optional().to_matchable(),
                Ref::keyword("WITH").optional().to_matchable(),
                Ref::keyword("TIES").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL supports CTEs with DML statements (INSERT, UPDATE, DELETE, MERGE)
    // We add these to NonWithSelectableGrammar so WithCompoundStatementSegment can use them
    dialect.add([(
        "NonWithSelectableGrammar".into(),
        one_of(vec![
            Ref::new("SetExpressionSegment").to_matchable(),
            optionally_bracketed(vec![Ref::new("SelectStatementSegment").to_matchable()])
                .to_matchable(),
            Ref::new("NonSetSelectableGrammar").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
            Ref::new("MergeStatementSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL assignment operator segment
    dialect.add([(
        "AssignmentOperatorSegment".into(),
        NodeMatcher::new(SyntaxKind::AssignmentOperator, |_| {
            Ref::new("RawEqualsSegment").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override NakedIdentifierSegment to support T-SQL identifiers with # at the end
    // T-SQL allows temporary table names like #temp or ##global
    dialect.add([(
        "NakedIdentifierSegment".into(),
        SegmentGenerator::new(|dialect| {
            // Generate the anti template from the set of reserved keywords
            let reserved_keywords = dialect.sets("reserved_keywords");
            let pattern = reserved_keywords.iter().join("|");
            let anti_template = format!("^({pattern})$");

            // T-SQL pattern: supports both temp tables (#temp, ##global) and identifiers ending with #
            // Pattern explanation:
            // - ##?[A-Za-z][A-Za-z0-9_]*    matches temp tables: #temp or ##global (case insensitive)
            // - [A-Za-z0-9_]*[A-Za-z][A-Za-z0-9_]*#?   matches regular identifiers with optional # at end
            RegexParser::new(
                r"(##?[A-Za-z][A-Za-z0-9_]*|[A-Za-z0-9_]*[A-Za-z][A-Za-z0-9_]*#?)",
                SyntaxKind::NakedIdentifier,
            )
            .anti_template(&anti_template)
            .to_matchable()
        })
        .into(),
    )]);

    // DECLARE statement for variable declarations
    // Syntax: DECLARE @var1 INT = 10, @var2 VARCHAR(50) = 'text'
    dialect.add([
        (
            "DeclareStatementSegment".into(),
            Ref::new("DeclareStatementGrammar").to_matchable().into(),
        ),
        (
            "DeclareStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("DECLARE").to_matchable(),
                // Multiple variables can be declared with comma separation
                Delimited::new(vec![
                    Sequence::new(vec![
                        Ref::new("TsqlVariableSegment").to_matchable(),
                        Sequence::new(vec![Ref::keyword("AS").to_matchable()])
                            .config(|this| this.optional())
                            .to_matchable(),
                        one_of(vec![
                            // Regular variable declaration
                            Sequence::new(vec![
                                Ref::new("DatatypeSegment").to_matchable(),
                                Sequence::new(vec![
                                    Ref::new("AssignmentOperatorSegment").to_matchable(),
                                    Ref::new("ExpressionSegment").to_matchable(),
                                ])
                                .config(|this| this.optional())
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            // Table variable declaration
                            Sequence::new(vec![
                                Ref::keyword("TABLE").to_matchable(),
                                Bracketed::new(vec![
                                    Delimited::new(vec![
                                        one_of(vec![
                                            Ref::new("TableConstraintSegment").to_matchable(),
                                            Ref::new("ColumnDefinitionSegment").to_matchable(),
                                        ])
                                        .to_matchable(),
                                    ])
                                    .config(|this| this.allow_trailing())
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
            .into(),
        ),
    ]);

    // SET statement for variables
    dialect.add([
        (
            "SetVariableStatementSegment".into(),
            Ref::new("SetVariableStatementGrammar")
                .to_matchable()
                .into(),
        ),
        (
            "SetVariableStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("SET").to_matchable(),
                one_of(vec![
                    // Variable assignment
                    Sequence::new(vec![
                        Ref::new("TsqlVariableSegment").to_matchable(),
                        Ref::new("AssignmentOperatorSegment").to_matchable(),
                        Ref::new("ExpressionSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // SET DEADLOCK_PRIORITY
                    Sequence::new(vec![
                        Ref::keyword("DEADLOCK_PRIORITY").to_matchable(),
                        one_of(vec![
                            Ref::keyword("LOW").to_matchable(),
                            Ref::keyword("NORMAL").to_matchable(),
                            Ref::keyword("HIGH").to_matchable(),
                            Ref::new("NumericLiteralSegment").to_matchable(), // Positive numbers
                            Sequence::new(vec![
                                // Negative numbers
                                Ref::new("MinusSegment").to_matchable(),
                                Ref::new("NumericLiteralSegment").to_matchable(),
                            ])
                            .to_matchable(),
                            Ref::new("TsqlVariableSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // SET options - supports both individual and shared ON/OFF
                    one_of(vec![
                        // Individual ON/OFF: SET NOCOUNT ON, XACT_ABORT OFF
                        Delimited::new(vec![
                            Sequence::new(vec![
                                one_of(vec![
                                    Ref::keyword("NOCOUNT").to_matchable(),
                                    Ref::keyword("XACT_ABORT").to_matchable(),
                                    Ref::keyword("QUOTED_IDENTIFIER").to_matchable(),
                                    Ref::keyword("ANSI_NULLS").to_matchable(),
                                    Ref::keyword("ANSI_PADDING").to_matchable(),
                                    Ref::keyword("ANSI_WARNINGS").to_matchable(),
                                    Ref::keyword("ARITHABORT").to_matchable(),
                                    Ref::keyword("CONCAT_NULL_YIELDS_NULL").to_matchable(),
                                    Ref::keyword("NUMERIC_ROUNDABORT").to_matchable(),
                                ])
                                .to_matchable(),
                                one_of(vec![
                                    Ref::keyword("ON").to_matchable(),
                                    Ref::keyword("OFF").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Shared ON/OFF: SET NOCOUNT, XACT_ABORT ON
                        Sequence::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::keyword("NOCOUNT").to_matchable(),
                                    Ref::keyword("XACT_ABORT").to_matchable(),
                                    Ref::keyword("QUOTED_IDENTIFIER").to_matchable(),
                                    Ref::keyword("ANSI_NULLS").to_matchable(),
                                    Ref::keyword("ANSI_PADDING").to_matchable(),
                                    Ref::keyword("ANSI_WARNINGS").to_matchable(),
                                    Ref::keyword("ARITHABORT").to_matchable(),
                                    Ref::keyword("CONCAT_NULL_YIELDS_NULL").to_matchable(),
                                    Ref::keyword("NUMERIC_ROUNDABORT").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .to_matchable(),
                            one_of(vec![
                                Ref::keyword("ON").to_matchable(),
                                Ref::keyword("OFF").to_matchable(),
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
            .into(),
        ),
    ]);

    // PRINT statement
    dialect.add([
        (
            "PrintStatementSegment".into(),
            Ref::new("PrintStatementGrammar").to_matchable().into(),
        ),
        (
            "PrintStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("PRINT").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // BEGIN...END blocks for grouping multiple statements
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            Sequence::new(vec![
                Ref::keyword("BEGIN").to_matchable(),
                MetaSegment::indent().to_matchable(),
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        one_of(vec![
                            Ref::new("SelectableGrammar").to_matchable(),
                            Ref::new("InsertStatementSegment").to_matchable(),
                            Ref::new("UpdateStatementSegment").to_matchable(),
                            Ref::new("DeleteStatementSegment").to_matchable(),
                            Ref::new("CreateTableStatementSegment").to_matchable(),
                            Ref::new("DropTableStatementSegment").to_matchable(),
                            Ref::new("DeclareStatementSegment").to_matchable(),
                            Ref::new("SetVariableStatementSegment").to_matchable(),
                            Ref::new("PrintStatementSegment").to_matchable(),
                            Ref::new("IfStatementSegment").to_matchable(),
                            Ref::new("WhileStatementSegment").to_matchable(),
                            Ref::new("TryBlockSegment").to_matchable(),
                            Ref::new("GotoStatementSegment").to_matchable(),
                            Ref::new("LabelSegment").to_matchable(),
                            Ref::new("BeginEndBlockSegment").to_matchable(),
                        ])
                        .to_matchable(),
                        Ref::new("DelimiterGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.terminators = vec![
                        // Terminate on END keyword
                        Ref::keyword("END").to_matchable(),
                        // Also terminate on statement keywords to help with boundary detection
                        Ref::keyword("SELECT").to_matchable(),
                        Ref::keyword("INSERT").to_matchable(),
                        Ref::keyword("UPDATE").to_matchable(),
                        Ref::keyword("DELETE").to_matchable(),
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("DROP").to_matchable(),
                        Ref::keyword("DECLARE").to_matchable(),
                        Ref::keyword("SET").to_matchable(),
                        Ref::keyword("PRINT").to_matchable(),
                        Ref::keyword("IF").to_matchable(),
                        Ref::keyword("WHILE").to_matchable(),
                        Ref::keyword("BEGIN").to_matchable(),
                        Ref::keyword("GOTO").to_matchable(),
                    ];
                })
                .config(|this| this.min_times(0))
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Ref::new("BeginEndBlockSegment").to_matchable().into(),
        ),
    ]);

    // TRY...CATCH blocks
    dialect.add([(
        "TryBlockSegment".into(),
        Sequence::new(vec![
            Ref::keyword("BEGIN").to_matchable(),
            Ref::keyword("TRY").to_matchable(),
            MetaSegment::indent().to_matchable(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![Ref::keyword("END").to_matchable()];
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::keyword("END").to_matchable(),
            Ref::keyword("TRY").to_matchable(),
            Ref::keyword("BEGIN").to_matchable(),
            Ref::keyword("CATCH").to_matchable(),
            MetaSegment::indent().to_matchable(),
            AnyNumberOf::new(vec![
                Sequence::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.terminators = vec![Ref::keyword("END").to_matchable()];
            })
            .to_matchable(),
            MetaSegment::dedent().to_matchable(),
            Ref::keyword("END").to_matchable(),
            Ref::keyword("CATCH").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // GOTO statement and labels
    dialect.add([
        (
            "GotoStatementSegment".into(),
            Sequence::new(vec![
                Ref::keyword("GOTO").to_matchable(),
                Ref::new("NakedIdentifierSegment").to_matchable(), // Label name
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LabelSegment".into(),
            Sequence::new(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(), // Label name
                Ref::new("ColonSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // IF...ELSE statement
    dialect.add([
        (
            "IfStatementSegment".into(),
            Ref::new("IfStatementGrammar").to_matchable().into(),
        ),
        (
            "IfStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("IF").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::new("StatementSegment").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("ELSE").to_matchable(),
                    Ref::new("StatementSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // WHILE loop
    dialect.add([
        (
            "WhileStatementSegment".into(),
            Ref::new("WhileStatementGrammar").to_matchable().into(),
        ),
        (
            "WhileStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("WHILE").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::new("StatementSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // PIVOT and UNPIVOT support
    dialect.add([
        (
            "PivotUnpivotSegment".into(),
            NodeMatcher::new(SyntaxKind::TableExpression, |_| {
                Ref::new("PivotUnpivotGrammar").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PivotUnpivotGrammar".into(),
            one_of(vec![
                // PIVOT (SUM(Amount) FOR Month IN ([Jan], [Feb], [Mar]))
                Sequence::new(vec![
                    Ref::keyword("PIVOT").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("FunctionSegment").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
                        Bracketed::new(vec![
                            Delimited::new(vec![Ref::new("LiteralGrammar").to_matchable()])
                                .to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                // UNPIVOT (Value FOR Month IN ([Jan], [Feb], [Mar]))
                Sequence::new(vec![
                    Ref::keyword("UNPIVOT").to_matchable(),
                    Bracketed::new(vec![
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::keyword("FOR").to_matchable(),
                        Ref::new("ColumnReferenceSegment").to_matchable(),
                        Ref::keyword("IN").to_matchable(),
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
            .into(),
        ),
    ]);

    // Override TransactionStatementSegment to require TRANSACTION/WORK after BEGIN
    // This prevents BEGIN from being parsed as a transaction when it should be a BEGIN...END block
    dialect.replace_grammar(
        "TransactionStatementSegment",
        NodeMatcher::new(SyntaxKind::TransactionStatement, |_| {
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("START").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("BEGIN").to_matchable(),
                        one_of(vec![
                            Ref::keyword("TRANSACTION").to_matchable(),
                            Ref::keyword("WORK").to_matchable(),
                            Ref::keyword("TRAN").to_matchable(), // T-SQL also supports TRAN
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("COMMIT").to_matchable(),
                    Ref::keyword("ROLLBACK").to_matchable(),
                    Ref::keyword("SAVE").to_matchable(), // T-SQL savepoints
                ])
                .to_matchable(),
                one_of(vec![
                    Ref::keyword("TRANSACTION").to_matchable(),
                    Ref::keyword("WORK").to_matchable(),
                    Ref::keyword("TRAN").to_matchable(), // T-SQL abbreviation
                ])
                .config(|this| this.optional())
                .to_matchable(),
                // Optional transaction/savepoint name
                Ref::new("SingleIdentifierGrammar")
                    .optional()
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // GO batch separator - T-SQL uses GO to separate batches
    dialect.add([
        (
            "BatchSeparatorSegment".into(),
            Ref::new("BatchSeparatorGrammar").to_matchable().into(),
        ),
        (
            "BatchSeparatorGrammar".into(),
            Ref::keyword("GO").to_matchable().into(),
        ),
        (
            "BatchDelimiterGrammar".into(),
            Ref::new("BatchSeparatorGrammar").to_matchable().into(),
        ),
    ]);

    // Override FileSegment to handle T-SQL batch separators (GO statements)
    dialect.replace_grammar(
        "FileSegment",
        AnyNumberOf::new(vec![
            one_of(vec![
                Ref::new("StatementSegment").to_matchable(),
                Ref::new("BatchDelimiterGrammar").to_matchable(),
            ])
            .to_matchable(),
            Ref::new("DelimiterGrammar").optional().to_matchable(),
        ])
        .to_matchable(),
    );

    // Add T-SQL specific statement types to the statement segment
    dialect.replace_grammar(
        "StatementSegment",
        one_of(vec![
            // T-SQL specific statements (BEGIN...END blocks must come first to avoid transaction conflicts)
            Ref::new("BeginEndBlockGrammar").to_matchable(),
            Ref::new("TryBlockSegment").to_matchable(),
            Ref::new("AtomicBlockSegment").to_matchable(),
            Ref::new("DeclareStatementGrammar").to_matchable(),
            Ref::new("SetVariableStatementGrammar").to_matchable(),
            Ref::new("PrintStatementGrammar").to_matchable(),
            Ref::new("IfStatementGrammar").to_matchable(),
            Ref::new("WhileStatementGrammar").to_matchable(),
            Ref::new("GotoStatementSegment").to_matchable(),
            Ref::new("LabelSegment").to_matchable(),
            Ref::new("BatchSeparatorGrammar").to_matchable(),
            Ref::new("UseStatementGrammar").to_matchable(),
            // Include all ANSI statement types
            Ref::new("SelectableGrammar").to_matchable(),
            Ref::new("MergeStatementSegment").to_matchable(),
            Ref::new("InsertStatementSegment").to_matchable(),
            Ref::new("TransactionStatementSegment").to_matchable(),
            Ref::new("DropTableStatementSegment").to_matchable(),
            Ref::new("DropViewStatementSegment").to_matchable(),
            Ref::new("CreateUserStatementSegment").to_matchable(),
            Ref::new("DropUserStatementSegment").to_matchable(),
            Ref::new("TruncateStatementSegment").to_matchable(),
            Ref::new("AccessStatementSegment").to_matchable(),
            Ref::new("CreateTableStatementSegment").to_matchable(),
            Ref::new("CreateRoleStatementSegment").to_matchable(),
            Ref::new("DropRoleStatementSegment").to_matchable(),
            Ref::new("AlterTableStatementSegment").to_matchable(),
            Ref::new("CreateSchemaStatementSegment").to_matchable(),
            Ref::new("SetSchemaStatementSegment").to_matchable(),
            Ref::new("DropSchemaStatementSegment").to_matchable(),
            Ref::new("DropTypeStatementSegment").to_matchable(),
            Ref::new("CreateDatabaseStatementSegment").to_matchable(),
            Ref::new("DropDatabaseStatementSegment").to_matchable(),
            Ref::new("CreateIndexStatementSegment").to_matchable(),
            Ref::new("DropIndexStatementSegment").to_matchable(),
            Ref::new("CreateViewStatementSegment").to_matchable(),
            Ref::new("DeleteStatementSegment").to_matchable(),
            Ref::new("UpdateStatementSegment").to_matchable(),
            Ref::new("CreateCastStatementSegment").to_matchable(),
            Ref::new("DropCastStatementSegment").to_matchable(),
            Ref::new("CreateFunctionStatementSegment").to_matchable(),
            Ref::new("DropFunctionStatementSegment").to_matchable(),
            Ref::new("CreateProcedureStatementSegment").to_matchable(),
            Ref::new("DropProcedureStatementSegment").to_matchable(),
            Ref::new("CreateModelStatementSegment").to_matchable(),
            Ref::new("DropModelStatementSegment").to_matchable(),
            Ref::new("DescribeStatementSegment").to_matchable(),
            Ref::new("ExplainStatementSegment").to_matchable(),
            Ref::new("CreateSequenceStatementSegment").to_matchable(),
            Ref::new("AlterSequenceStatementSegment").to_matchable(),
            Ref::new("DropSequenceStatementSegment").to_matchable(),
            Ref::new("CreateTriggerStatementSegment").to_matchable(),
            Ref::new("DropTriggerStatementSegment").to_matchable(),
        ])
        .config(|this| this.terminators = vec![Ref::new("DelimiterGrammar").to_matchable()])
        .to_matchable(),
    );

    // USE statement for changing database context
    dialect.add([
        (
            "UseStatementSegment".into(),
            Ref::new("UseStatementGrammar").to_matchable().into(),
        ),
        (
            "UseStatementGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("USE").to_matchable(),
                Ref::new("DatabaseReferenceSegment").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Add variable reference support for T-SQL @ and @@ variables
    dialect.add([
        (
            "TsqlVariableSegment".into(),
            TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable)
                .to_matchable()
                .into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(SyntaxKind::ParameterizedExpression, |_| {
                Ref::new("TsqlVariableSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TsqlTableVariableSegment".into(),
            NodeMatcher::new(SyntaxKind::TableReference, |_| {
                Ref::new("TsqlVariableSegment").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Update TableReferenceSegment to support T-SQL table variables
    // Temp tables are now handled as regular ObjectReferenceSegment since they use word tokens
    dialect.replace_grammar(
        "TableReferenceSegment",
        one_of(vec![
            Ref::new("ObjectReferenceSegment").to_matchable(),
            Ref::new("TsqlVariableSegment").to_matchable(),
        ])
        .to_matchable(),
    );

    // Update TableExpressionSegment to include PIVOT/UNPIVOT
    dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec![
            Ref::new("ValuesClauseSegment").to_matchable(),
            Ref::new("BareFunctionSegment").to_matchable(),
            Ref::new("FunctionSegment").to_matchable(),
            Ref::new("TableReferenceSegment").to_matchable(),
            Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()]).to_matchable(),
            Sequence::new(vec![
                Ref::new("TableReferenceSegment").to_matchable(),
                Ref::new("PivotUnpivotGrammar").to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // Table hints support - Example: SELECT * FROM Users WITH (NOLOCK)
    dialect.add([
        (
            "TableHintSegment".into(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("TableHintElement").to_matchable()])
                        .to_matchable(),
                ])
                .config(|this| this.parse_mode = ParseMode::Greedy)
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TableHintElement".into(),
            one_of(vec![
                // Simple hints (just keywords)
                Ref::keyword("NOLOCK").to_matchable(),
                Ref::keyword("READUNCOMMITTED").to_matchable(),
                Ref::keyword("READCOMMITTED").to_matchable(),
                Ref::keyword("REPEATABLEREAD").to_matchable(),
                Ref::keyword("SERIALIZABLE").to_matchable(),
                Ref::keyword("READPAST").to_matchable(),
                Ref::keyword("ROWLOCK").to_matchable(),
                Ref::keyword("TABLOCK").to_matchable(),
                Ref::keyword("TABLOCKX").to_matchable(),
                Ref::keyword("UPDLOCK").to_matchable(),
                Ref::keyword("XLOCK").to_matchable(),
                Ref::keyword("NOEXPAND").to_matchable(),
                Ref::keyword("FORCESEEK").to_matchable(),
                Ref::keyword("FORCESCAN").to_matchable(),
                Ref::keyword("HOLDLOCK").to_matchable(),
                Ref::keyword("SNAPSHOT").to_matchable(),
                // INDEX hint with parameter
                Sequence::new(vec![
                    Ref::keyword("INDEX").to_matchable(),
                    Bracketed::new(vec![
                        one_of(vec![
                            Ref::new("NumericLiteralSegment").to_matchable(),
                            Ref::new("NakedIdentifierSegment").to_matchable(),
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
    ]);

    // Define PostTableExpressionGrammar to include T-SQL table hints
    dialect.add([(
        "PostTableExpressionGrammar".into(),
        Ref::new("TableHintSegment")
            .optional()
            .to_matchable()
            .into(),
    )]);

    // Override FromExpressionElementSegment to ensure table hints are parsed correctly
    // The LookaheadExclude prevents WITH from being parsed as an alias when followed by (
    dialect.replace_grammar(
        "FromExpressionElementSegment",
        Sequence::new(vec![
            Ref::new("PreTableFunctionKeywordsGrammar")
                .optional()
                .to_matchable(),
            optionally_bracketed(vec![Ref::new("TableExpressionSegment").to_matchable()])
                .to_matchable(),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec![
                    Ref::new("FromClauseTerminatorGrammar").to_matchable(),
                    Ref::new("SamplingExpressionSegment").to_matchable(),
                    Ref::new("JoinLikeClauseGrammar").to_matchable(),
                    LookaheadExclude::new("WITH", "(").to_matchable(), // Prevents WITH from being parsed as alias when followed by (
                ]))
                .optional()
                .to_matchable(),
            Sequence::new(vec![
                Ref::keyword("WITH").to_matchable(),
                Ref::keyword("OFFSET").to_matchable(),
                Ref::new("AliasExpressionSegment").to_matchable(),
            ])
            .config(|this| this.optional())
            .to_matchable(),
            Ref::new("SamplingExpressionSegment")
                .optional()
                .to_matchable(),
            Ref::new("PostTableExpressionGrammar")
                .optional()
                .to_matchable(), // T-SQL table hints
        ])
        .to_matchable(),
    );

    // Update JoinClauseSegment to handle APPLY syntax properly
    dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec![
            // Standard JOIN syntax
            Sequence::new(vec![
                Ref::new("JoinTypeKeywordsGrammar")
                    .optional()
                    .to_matchable(),
                Ref::new("JoinKeywordsGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(),
                AnyNumberOf::new(vec![Ref::new("NestedJoinGrammar").to_matchable()]).to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Sequence::new(vec![
                    Conditional::new(MetaSegment::indent())
                        .indented_using_on()
                        .to_matchable(),
                    one_of(vec![
                        Ref::new("JoinOnConditionSegment").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("USING").to_matchable(),
                            MetaSegment::indent().to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.parse_mode = ParseMode::Greedy)
                            .to_matchable(),
                            MetaSegment::dedent().to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    Conditional::new(MetaSegment::dedent())
                        .indented_using_on()
                        .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            // NATURAL JOIN
            Sequence::new(vec![
                Ref::new("NaturalJoinKeywordsGrammar").to_matchable(),
                Ref::new("JoinKeywordsGrammar").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable(),
            // T-SQL APPLY syntax
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("CROSS").to_matchable(),
                    Ref::keyword("OUTER").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("APPLY").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(),
                MetaSegment::dedent().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL specific data type handling for MAX keyword and -1
    // Override BracketedArguments to accept MAX keyword and negative numbers
    dialect.replace_grammar(
        "BracketedArguments",
        Bracketed::new(vec![
            Delimited::new(vec![
                one_of(vec![
                    Ref::new("LiteralGrammar").to_matchable(),
                    Ref::keyword("MAX").to_matchable(),
                    // Support negative numbers like -1 for NVARCHAR(-1)
                    Sequence::new(vec![
                        Ref::new("SignedSegmentGrammar").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .config(|this| {
                this.optional();
            })
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // APPLY clause support (CROSS APPLY and OUTER APPLY)
    // APPLY invokes a table-valued function for each row of the outer table
    // CROSS APPLY: Like INNER JOIN - returns only rows with results
    // OUTER APPLY: Like LEFT JOIN - returns all rows, NULLs when no results
    dialect.add([(
        "ApplyClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::JoinClause,
            |_| // APPLY is classified as a join type
            Sequence::new(vec![
                one_of(vec![Ref::keyword("CROSS").to_matchable(), Ref::keyword("OUTER").to_matchable()]).to_matchable(),
                Ref::keyword("APPLY").to_matchable(),
                MetaSegment::indent().to_matchable(),
                Ref::new("FromExpressionElementSegment").to_matchable(), // The function or subquery
                MetaSegment::dedent().to_matchable()
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // Add JoinLikeClauseGrammar for T-SQL to include APPLY
    // This allows APPLY to be used wherever joins are allowed
    dialect.add([(
        "JoinLikeClauseGrammar".into(),
        Ref::new("ApplyClauseSegment").to_matchable().into(),
    )]);

    // WITHIN GROUP support for ordered set aggregate functions
    dialect.add([(
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
    )]);

    // Override PostFunctionGrammar to include WITHIN GROUP
    dialect.add([(
        "PostFunctionGrammar".into(),
        AnyNumberOf::new(vec![
            Ref::new("WithinGroupClauseSegment").to_matchable(),
            Ref::new("OverClauseSegment").to_matchable(),
            Ref::new("FilterClauseGrammar").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL IDENTITY constraint support
    dialect.add([(
        "IdentityConstraintGrammar".into(),
        Sequence::new(vec![
            Ref::keyword("IDENTITY").to_matchable(),
            Bracketed::new(vec![
                Ref::new("NumericLiteralSegment").to_matchable(), // seed
                Ref::new("CommaSegment").to_matchable(),
                Ref::new("NumericLiteralSegment").to_matchable(), // increment
            ])
            .config(|this| this.optional())
            .to_matchable(), // IDENTITY() can be empty
        ])
        .to_matchable()
        .into(),
    )]);

    // Extend ColumnConstraintSegment to include T-SQL specific constraints
    dialect.add([(
        "ColumnConstraintSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
            Sequence::new(vec![
                Sequence::new(vec![
                    Ref::keyword("CONSTRAINT").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                one_of(vec![
                    // NOT NULL / NULL
                    Sequence::new(vec![
                        Ref::keyword("NOT").optional().to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                    ])
                    .to_matchable(),
                    // CHECK constraint
                    Sequence::new(vec![
                        Ref::keyword("CHECK").to_matchable(),
                        Bracketed::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                    // DEFAULT constraint
                    Sequence::new(vec![
                        Ref::keyword("DEFAULT").to_matchable(),
                        Ref::new("ColumnConstraintDefaultGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("PrimaryKeyGrammar").to_matchable(),
                    Ref::new("UniqueKeyGrammar").to_matchable(),
                    Ref::new("IdentityConstraintGrammar").to_matchable(), // T-SQL IDENTITY
                    Ref::new("AutoIncrementGrammar").to_matchable(), // Keep ANSI AUTO_INCREMENT
                    Ref::new("ReferenceDefinitionGrammar").to_matchable(),
                    Ref::new("CommentClauseSegment").to_matchable(),
                    // COLLATE
                    Sequence::new(vec![
                        Ref::keyword("COLLATE").to_matchable(),
                        Ref::new("CollationReferenceSegment").to_matchable(),
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

    // Add T-SQL variable support to LiteralGrammar
    dialect.add([(
        "LiteralGrammar".into(),
        one_of(vec![
            Ref::new("QuotedLiteralSegment").to_matchable(),
            Ref::new("NumericLiteralSegment").to_matchable(),
            Ref::new("BooleanLiteralGrammar").to_matchable(),
            Ref::new("QualifiedNumericLiteralSegment").to_matchable(),
            Ref::new("NullLiteralSegment").to_matchable(),
            Ref::new("DateTimeLiteralGrammar").to_matchable(),
            Ref::new("ArrayLiteralSegment").to_matchable(),
            Ref::new("TypedArrayLiteralSegment").to_matchable(),
            Ref::new("ObjectLiteralSegment").to_matchable(),
            Ref::new("ParameterizedSegment").to_matchable(), // Add T-SQL variables
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE PROCEDURE support
    dialect.add([
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_| {
                Sequence::new(vec![
                    one_of(vec![
                        Ref::keyword("CREATE").to_matchable(),
                        Ref::keyword("ALTER").to_matchable(),
                        Sequence::new(vec![
                            Ref::keyword("CREATE").to_matchable(),
                            Ref::keyword("OR").to_matchable(),
                            Ref::keyword("ALTER").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    one_of(vec![
                        Ref::keyword("PROC").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                    // Optional version number
                    Sequence::new(vec![
                        Ref::new("SemicolonSegment").to_matchable(),
                        Ref::new("NumericLiteralSegment").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    MetaSegment::indent().to_matchable(),
                    // Optional parameter list
                    Ref::new("ProcedureParameterListGrammar")
                        .optional()
                        .to_matchable(),
                    // Procedure options
                    Sequence::new(vec![
                        Ref::keyword("WITH").to_matchable(),
                        Delimited::new(vec![
                            Ref::keyword("ENCRYPTION").to_matchable(),
                            Ref::keyword("RECOMPILE").to_matchable(),
                            Ref::keyword("NATIVE_COMPILATION").to_matchable(),
                            Ref::keyword("SCHEMABINDING").to_matchable(),
                            Ref::new("ExecuteAsClauseGrammar").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("FOR").to_matchable(),
                        Ref::keyword("REPLICATION").to_matchable(),
                    ])
                    .config(|this| this.optional())
                    .to_matchable(),
                    MetaSegment::dedent().to_matchable(),
                    Ref::keyword("AS").to_matchable(),
                    Ref::new("ProcedureDefinitionGrammar").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_| {
                Sequence::new(vec![
                    Ref::keyword("DROP").to_matchable(),
                    one_of(vec![
                        Ref::keyword("PROC").to_matchable(),
                        Ref::keyword("PROCEDURE").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::new("IfExistsGrammar").optional().to_matchable(),
                    Delimited::new(vec![Ref::new("ObjectReferenceSegment").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterListGrammar".into(),
            one_of(vec![
                // Bracketed parameter list: (param1, param2, param3)
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("ProcedureParameterGrammar").to_matchable()])
                        .config(|this| this.optional())
                        .to_matchable(),
                ])
                .to_matchable(),
                // Unbracketed parameter list: param1, param2, param3
                Delimited::new(vec![Ref::new("ProcedureParameterGrammar").to_matchable()])
                    .config(|this| this.optional())
                    .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TsqlDatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_| {
                one_of(vec![
                    // Square bracket data type like [int], [varchar](100)
                    Sequence::new(vec![
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DataTypeIdentifier)
                            .to_matchable(),
                        Ref::new("BracketedArguments").optional().to_matchable(),
                    ])
                    .to_matchable(),
                    // Regular data type (includes DatatypeIdentifierSegment for user-defined types)
                    Ref::new("DatatypeSegment").to_matchable(),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterGrammar".into(),
            Sequence::new(vec![
                Ref::new("ParameterNameSegment").to_matchable(),
                Ref::new("TsqlDatatypeSegment").to_matchable(),
                // Optional VARYING keyword (for cursors and some special types)
                Ref::keyword("VARYING").optional().to_matchable(),
                // Optional NULL/NOT NULL
                Sequence::new(vec![
                    Ref::keyword("NOT").optional().to_matchable(),
                    Ref::keyword("NULL").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                // Optional default value
                Sequence::new(vec![
                    Ref::new("EqualsSegment").to_matchable(),
                    one_of(vec![
                        Ref::new("LiteralGrammar").to_matchable(),
                        Ref::keyword("NULL").to_matchable(),
                        // Function calls as defaults (e.g., NEWID())
                        Ref::new("FunctionSegment").to_matchable(),
                        // String literal with prefix (e.g., N'foo')
                        Sequence::new(vec![
                            Ref::new("NakedIdentifierSegment").to_matchable(), // N, B, X etc.
                            Ref::new("QuotedLiteralSegment").to_matchable(),
                        ])
                        .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
                // Optional parameter modifiers (can appear in any order)
                AnyNumberOf::new(vec![
                    one_of(vec![
                        Ref::keyword("OUT").to_matchable(),
                        Ref::keyword("OUTPUT").to_matchable(),
                        Ref::keyword("READONLY").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ParameterNameSegment".into(),
            Ref::new("TsqlVariableSegment").to_matchable().into(),
        ),
        (
            "ExecuteAsClauseGrammar".into(),
            Sequence::new(vec![
                Ref::keyword("EXECUTE").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                one_of(vec![
                    Ref::keyword("CALLER").to_matchable(),
                    Ref::keyword("SELF").to_matchable(),
                    Ref::keyword("OWNER").to_matchable(),
                    Ref::new("QuotedLiteralSegment").to_matchable(), // user name
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureDefinitionGrammar".into(),
            one_of(vec![
                // External CLR procedures (check this first as it's simpler)
                Sequence::new(vec![
                    Ref::keyword("EXTERNAL").to_matchable(),
                    Ref::keyword("NAME").to_matchable(),
                    Ref::new("ObjectReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                // Atomic blocks for natively compiled procedures
                Ref::new("AtomicBlockSegment").to_matchable(),
                // Single statement or block
                Ref::new("StatementSegment").to_matchable(),
                // Multiple statements for procedures without BEGIN...END
                AnyNumberOf::new(vec![
                    Sequence::new(vec![
                        Ref::new("StatementSegment").to_matchable(),
                        Ref::new("DelimiterGrammar").optional().to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .config(|this| {
                    this.min_times(2); // At least 2 statements to use this branch
                    this.parse_mode = ParseMode::Greedy;
                    // Don't terminate on delimiters, keep consuming statements
                    this.terminators = vec![Ref::new("BatchSeparatorGrammar").to_matchable()];
                })
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureStatementSegment".into(),
            // Just use StatementSegment for now - the ordering should handle precedence
            Ref::new("StatementSegment").to_matchable().into(),
        ),
        (
            "AtomicBlockSegment".into(),
            Sequence::new(vec![
                Ref::keyword("BEGIN").to_matchable(),
                Ref::keyword("ATOMIC").to_matchable(),
                Ref::keyword("WITH").to_matchable(),
                Bracketed::new(vec![
                    Delimited::new(vec![Ref::new("AtomicBlockOptionGrammar").to_matchable()])
                        .to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::indent().to_matchable(),
                AnyNumberOf::new(vec![
                    Ref::new("StatementSegment").to_matchable(),
                    Ref::new("DelimiterGrammar").optional().to_matchable(),
                ])
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                Ref::keyword("END").to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AtomicBlockOptionGrammar".into(),
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("LANGUAGE").to_matchable(),
                    Ref::keyword("DATEFIRST").to_matchable(),
                    Ref::keyword("DATEFORMAT").to_matchable(),
                    Ref::keyword("DELAYED_DURABILITY").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("TRANSACTION").to_matchable(),
                        Ref::keyword("ISOLATION").to_matchable(),
                        Ref::keyword("LEVEL").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("NumericLiteralSegment").to_matchable(),
                    Ref::new("NakedIdentifierSegment").to_matchable(),
                    // N'string' syntax for Unicode strings
                    Sequence::new(vec![
                        Ref::new("NakedIdentifierSegment").to_matchable(), // N prefix
                        Ref::new("QuotedLiteralSegment").to_matchable(),
                    ])
                    .to_matchable(),
                    // Special handling for multi-word isolation levels
                    Sequence::new(vec![
                        Ref::keyword("REPEATABLE").to_matchable(),
                        Ref::keyword("READ").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("SERIALIZABLE").to_matchable(),
                    Ref::keyword("SNAPSHOT").to_matchable(),
                    Ref::keyword("ON").to_matchable(),
                    Ref::keyword("OFF").to_matchable(),
                    // Date format values
                    Ref::keyword("MDY").to_matchable(),
                    Ref::keyword("DMY").to_matchable(),
                    Ref::keyword("YMD").to_matchable(),
                    Ref::keyword("YDM").to_matchable(),
                    Ref::keyword("MYD").to_matchable(),
                    Ref::keyword("DYM").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL supports alternative alias syntax: AliasName = Expression
    // The parser distinguishes between column references (table1.column1)
    // and alias assignments (AliasName = table1.column1)
    dialect.replace_grammar(
        "SelectClauseElementSegment",
        one_of(vec![
            // T-SQL alias equals pattern: AliasName = Expression
            Sequence::new(vec![
                Ref::new("NakedIdentifierSegment").to_matchable(),
                StringParser::new("=", SyntaxKind::RawComparisonOperator).to_matchable(),
                one_of(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::new("BaseExpressionElementGrammar").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Wildcard expressions
            Ref::new("WildcardExpressionSegment").to_matchable(),
            // Everything else
            Sequence::new(vec![
                Ref::new("BaseExpressionElementGrammar").to_matchable(),
                Ref::new("AliasExpressionSegment").optional().to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable(),
    );

    // T-SQL CREATE TABLE with Azure Synapse Analytics support
    dialect.replace_grammar(
        "CreateTableStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec![
                Ref::keyword("CREATE").to_matchable(),
                Ref::keyword("TABLE").to_matchable(),
                Ref::new("IfNotExistsGrammar").optional().to_matchable(),
                Ref::new("TableReferenceSegment").to_matchable(),
                one_of(vec![
                    // Regular CREATE TABLE with column definitions
                    Sequence::new(vec![
                        Bracketed::new(vec![
                            Delimited::new(vec![
                                one_of(vec![
                                    Ref::new("TableConstraintSegment").to_matchable(),
                                    Ref::new("ColumnDefinitionSegment").to_matchable(),
                                ])
                                .to_matchable(),
                            ])
                            .config(|this| this.allow_trailing())
                            .to_matchable(),
                        ])
                        .to_matchable(),
                        // Azure Synapse table options
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![Ref::new("TableOptionGrammar").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                    ])
                    .to_matchable(),
                    // CREATE TABLE AS SELECT with optional WITH clause before AS
                    Sequence::new(vec![
                        // Azure Synapse table options (required for CTAS)
                        Sequence::new(vec![
                            Ref::keyword("WITH").to_matchable(),
                            Bracketed::new(vec![
                                Delimited::new(vec![Ref::new("TableOptionGrammar").to_matchable()])
                                    .to_matchable(),
                            ])
                            .to_matchable(),
                        ])
                        .config(|this| this.optional())
                        .to_matchable(),
                        Ref::keyword("AS").to_matchable(),
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

    dialect.add([(
        "TableOptionGrammar".into(),
        one_of(vec![
            // Azure Synapse distribution options
            Sequence::new(vec![
                Ref::keyword("DISTRIBUTION").to_matchable(),
                Ref::new("EqualsSegment").to_matchable(),
                one_of(vec![
                    Ref::keyword("ROUND_ROBIN").to_matchable(),
                    Ref::keyword("REPLICATE").to_matchable(),
                    Sequence::new(vec![
                        Ref::keyword("HASH").to_matchable(),
                        Bracketed::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Azure Synapse index options
            one_of(vec![
                Ref::keyword("HEAP").to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CLUSTERED").to_matchable(),
                    Ref::keyword("COLUMNSTORE").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                ])
                .to_matchable(),
                Sequence::new(vec![
                    Ref::keyword("CLUSTERED").to_matchable(),
                    Ref::keyword("INDEX").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ColumnReferenceSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            // Other table options
            Sequence::new(vec![
                Ref::keyword("PARTITION").to_matchable(),
                Bracketed::new(vec![
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                    Ref::keyword("RANGE").to_matchable(),
                    one_of(vec![
                        Ref::keyword("LEFT").to_matchable(),
                        Ref::keyword("RIGHT").to_matchable(),
                    ])
                    .to_matchable(),
                    Ref::keyword("FOR").to_matchable(),
                    Ref::keyword("VALUES").to_matchable(),
                    Bracketed::new(vec![
                        Delimited::new(vec![Ref::new("ExpressionSegment").to_matchable()])
                            .to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL uses + for both arithmetic and string concatenation
    dialect.add([(
        "StringBinaryOperatorGrammar".into(),
        one_of(vec![
            Ref::new("ConcatSegment").to_matchable(), // Standard || operator
            Ref::new("PlusSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific data type identifier - allows case-insensitive user-defined types
    dialect.add([(
        "DatatypeIdentifierSegment".into(),
        SegmentGenerator::new(|_| {
            // Generate the anti template from the set of reserved keywords
            let anti_template = format!("^({})$", "NOT");

            one_of(vec![
                // Case-insensitive pattern for T-SQL data type identifiers (including UDTs)
                RegexParser::new("[A-Za-z_][A-Za-z0-9_]*", SyntaxKind::DataTypeIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable(),
                Ref::new("SingleIdentifierGrammar")
                    .exclude(Ref::new("NakedIdentifierSegment"))
                    .to_matchable(),
            ])
            .to_matchable()
        })
        .into(),
    )]);

    // expand() must be called after all grammar modifications

    dialect
}
