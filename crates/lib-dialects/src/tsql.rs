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
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

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
        "OPENJSON",
        "JSON",
        "XML",
        "BROWSE",
        "AUTO",
        "PATH",
        "RAW",
        "EXPLICIT",
        "ROOT",
        "INCLUDE_NULL_VALUES",
        "WITHOUT_ARRAY_WRAPPER",
        "TYPE",
        "ELEMENTS",
        "XSINIL",
        "ABSENT",
        "BASE64",
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
            // Unicode string literals: N'text'
            Matcher::regex(
                "unicode_single_quote",
                r"N'([^']|'')*'",
                SyntaxKind::UnicodeSingleQuote,
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
        AnyNumberOf::new(vec_of_erased![
            Ref::keyword("DISTINCT"),
            Ref::keyword("ALL"),
            // TOP alone
            Sequence::new(vec_of_erased![
                // https://docs.microsoft.com/en-us/sql/t-sql/queries/top-transact-sql
                Ref::keyword("TOP"),
                optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                Ref::keyword("PERCENT").optional(),
                Ref::keyword("WITH").optional(),
                Ref::keyword("TIES").optional()
            ]),
        ])
        .to_matchable(),
    );

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
            Sequence::new(vec_of_erased![
                Ref::keyword("DECLARE"),
                // Multiple variables can be declared with comma separation
                Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    Sequence::new(vec![Ref::keyword("AS").to_matchable()])
                        .config(|this| this.optional()),
                    one_of(vec_of_erased![
                        // Regular variable declaration
                        Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("AssignmentOperatorSegment"),
                                Ref::new("ExpressionSegment")
                            ])
                            .config(|this| this.optional())
                        ]),
                        // Table variable declaration
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TABLE"),
                            Bracketed::new(vec![
                                Delimited::new(vec![
                                    one_of(vec_of_erased![
                                        Ref::new("TableConstraintSegment"),
                                        Ref::new("ColumnDefinitionSegment")
                                    ])
                                    .to_matchable()
                                ])
                                .config(|this| this.allow_trailing())
                                .to_matchable(),
                            ]),
                        ])
                    ])
                ])])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // SET statement for variables and options
    dialect.add([
        (
            "SetVariableStatementSegment".into(),
            Ref::new("SetVariableStatementGrammar")
                .to_matchable()
                .into(),
        ),
        (
            "SetVariableStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                one_of(vec_of_erased![
                    // Variable assignment: SET @var = value
                    Sequence::new(vec_of_erased![
                        Ref::new("TsqlVariableSegment"),
                        Ref::new("AssignmentOperatorSegment"),
                        Ref::new("ExpressionSegment")
                    ]),
                    // SET DEADLOCK_PRIORITY
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEADLOCK_PRIORITY"),
                        one_of(vec_of_erased![
                            Ref::keyword("LOW"),
                            Ref::keyword("NORMAL"),
                            Ref::keyword("HIGH"),
                            Ref::new("NumericLiteralSegment"), // Positive numbers
                            Sequence::new(vec_of_erased![
                                // Negative numbers
                                Ref::new("MinusSegment"),
                                Ref::new("NumericLiteralSegment")
                            ]),
                            Ref::new("TsqlVariableSegment")
                        ])
                    ]),
                    // Individual SET option: SET NOCOUNT ON
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("NOCOUNT"),
                            Ref::keyword("XACT_ABORT"),
                            Ref::keyword("QUOTED_IDENTIFIER"),
                            Ref::keyword("ANSI_NULLS"),
                            Ref::keyword("ANSI_PADDING"),
                            Ref::keyword("ANSI_WARNINGS"),
                            Ref::keyword("ARITHABORT"),
                            Ref::keyword("CONCAT_NULL_YIELDS_NULL"),
                            Ref::keyword("NUMERIC_ROUNDABORT")
                        ]),
                        one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                    ]),
                    // Multiple options with shared value: SET NOCOUNT, XACT_ABORT ON
                    Sequence::new(vec_of_erased![
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::keyword("NOCOUNT"),
                            Ref::keyword("XACT_ABORT"),
                            Ref::keyword("QUOTED_IDENTIFIER"),
                            Ref::keyword("ANSI_NULLS"),
                            Ref::keyword("ANSI_PADDING"),
                            Ref::keyword("ANSI_WARNINGS"),
                            Ref::keyword("ARITHABORT"),
                            Ref::keyword("CONCAT_NULL_YIELDS_NULL"),
                            Ref::keyword("NUMERIC_ROUNDABORT")
                        ])]),
                        one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                    ]),
                    // SET TRANSACTION ISOLATION LEVEL
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TRANSACTION"),
                        Ref::keyword("ISOLATION"),
                        Ref::keyword("LEVEL"),
                        one_of(vec_of_erased![
                            Ref::keyword("SNAPSHOT"),
                            Ref::keyword("SERIALIZABLE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("REPEATABLE"),
                                Ref::keyword("READ")
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("READ"),
                                one_of(vec_of_erased![
                                    Ref::keyword("COMMITTED"),
                                    Ref::keyword("UNCOMMITTED")
                                ])
                            ])
                        ])
                    ])
                ])
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
            Sequence::new(vec_of_erased![
                Ref::keyword("PRINT"),
                Ref::new("ExpressionSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // BEGIN...END blocks for grouping multiple statements
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"),
                        Ref::new("InsertStatementSegment"),
                        Ref::new("UpdateStatementSegment"),
                        Ref::new("DeleteStatementSegment"),
                        Ref::new("CreateTableStatementSegment"),
                        Ref::new("DropTableStatementSegment"),
                        Ref::new("DeclareStatementSegment"),
                        Ref::new("SetVariableStatementSegment"),
                        Ref::new("PrintStatementSegment"),
                        Ref::new("IfStatementSegment"),
                        Ref::new("WhileStatementSegment"),
                        Ref::new("TryBlockSegment"),
                        Ref::new("GotoStatementSegment"),
                        Ref::new("LabelSegment"),
                        Ref::new("ExecuteStatementSegment"),
                        Ref::new("BeginEndBlockSegment")
                    ]),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        // Terminate on END keyword
                        Ref::keyword("END"),
                        // Also terminate on statement keywords to help with boundary detection
                        Ref::keyword("SELECT"),
                        Ref::keyword("INSERT"),
                        Ref::keyword("UPDATE"),
                        Ref::keyword("DELETE"),
                        Ref::keyword("CREATE"),
                        Ref::keyword("DROP"),
                        Ref::keyword("DECLARE"),
                        Ref::keyword("SET"),
                        Ref::keyword("PRINT"),
                        Ref::keyword("IF"),
                        Ref::keyword("WHILE"),
                        Ref::keyword("BEGIN"),
                        Ref::keyword("GOTO")
                    ];
                })
                .config(|this| this.min_times(0)),
                MetaSegment::dedent(),
                Ref::keyword("END")
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
        Sequence::new(vec_of_erased![
            Ref::keyword("BEGIN"),
            Ref::keyword("TRY"),
            MetaSegment::indent(),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("StatementSegment"),
                Ref::new("DelimiterGrammar").optional()
            ])])
            .config(|this| {
                this.terminators = vec_of_erased![Ref::keyword("END")];
            }),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("TRY"),
            Ref::keyword("BEGIN"),
            Ref::keyword("CATCH"),
            MetaSegment::indent(),
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::new("StatementSegment"),
                Ref::new("DelimiterGrammar").optional()
            ])])
            .config(|this| {
                this.terminators = vec_of_erased![Ref::keyword("END")];
            }),
            MetaSegment::dedent(),
            Ref::keyword("END"),
            Ref::keyword("CATCH")
        ])
        .to_matchable()
        .into(),
    )]);

    // GOTO statement and labels
    dialect.add([
        (
            "GotoStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("GOTO"),
                Ref::new("NakedIdentifierSegment") // Label name
            ])
            .to_matchable()
            .into(),
        ),
        (
            "LabelSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::new("NakedIdentifierSegment"), // Label name
                Ref::new("ColonSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // EXECUTE/EXEC statements
    dialect.add([
        (
            "ExecuteStatementSegment".into(),
            Ref::new("ExecuteStatementGrammar").to_matchable().into(),
        ),
        (
            "ExecuteStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("EXEC"),
                    Ref::keyword("EXECUTE")
                ]),
                // Optional return value capture
                Sequence::new(vec_of_erased![
                    Ref::new("TsqlVariableSegment"),
                    Ref::new("AssignmentOperatorSegment")
                ])
                .config(|this| this.optional()),
                // What to execute
                one_of(vec_of_erased![
                    // Dynamic SQL (expression in parentheses)
                    Bracketed::new(vec_of_erased![
                        Ref::new("ExpressionSegment") // SQL string expression
                    ]),
                    // Stored procedure with optional parameters
                    Sequence::new(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"), // Procedure name
                        // Optional parameters
                        AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                            // First parameter doesn't need comma
                            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                                // Named parameter: @param = value
                                Sequence::new(vec_of_erased![
                                    Ref::new("TsqlVariableSegment"),
                                    Ref::new("AssignmentOperatorSegment"),
                                    Ref::new("ExpressionSegment"),
                                    // Optional OUTPUT keyword
                                    Ref::keyword("OUTPUT").optional()
                                ]),
                                // Positional parameter
                                Sequence::new(vec_of_erased![
                                    Ref::new("ExpressionSegment"),
                                    // Optional OUTPUT keyword
                                    Ref::keyword("OUTPUT").optional()
                                ])
                            ])]),
                            // Subsequent parameters need comma
                            Sequence::new(vec_of_erased![
                                Ref::new("CommaSegment"),
                                one_of(vec_of_erased![
                                    // Named parameter: @param = value
                                    Sequence::new(vec_of_erased![
                                        Ref::new("TsqlVariableSegment"),
                                        Ref::new("AssignmentOperatorSegment"),
                                        Ref::new("ExpressionSegment"),
                                        // Optional OUTPUT keyword
                                        Ref::keyword("OUTPUT").optional()
                                    ]),
                                    // Positional parameter
                                    Sequence::new(vec_of_erased![
                                        Ref::new("ExpressionSegment"),
                                        // Optional OUTPUT keyword
                                        Ref::keyword("OUTPUT").optional()
                                    ])
                                ])
                            ])
                        ])])
                    ])
                ]),
                // Optional WITH clause for additional options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::keyword("RECOMPILE"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESULT"),
                            Ref::keyword("SETS"),
                            one_of(vec_of_erased![
                                Ref::keyword("UNDEFINED"),
                                Ref::keyword("NONE")
                            ])
                        ])
                    ])])
                ])
                .config(|this| this.optional())
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
            Sequence::new(vec_of_erased![
                Ref::keyword("IF"),
                Ref::new("ExpressionSegment"),
                MetaSegment::indent(),
                // Use a constrained statement that terminates on ELSE at the same level
                one_of(vec_of_erased![
                    // BEGIN...END block (already handles its own delimiters)
                    Ref::new("BeginEndBlockSegment"),
                    // Single statement (with optional delimiter)
                    Sequence::new(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("DelimiterGrammar").optional()
                    ])
                ])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        Ref::keyword("ELSE"),
                        // Also terminate on GO batch separator
                        Ref::new("BatchSeparatorGrammar")
                    ];
                }),
                MetaSegment::dedent(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ELSE"),
                    MetaSegment::indent(),
                    one_of(vec_of_erased![
                        // BEGIN...END block (already handles its own delimiters)
                        Ref::new("BeginEndBlockSegment"),
                        // Single statement (with optional delimiter)
                        Sequence::new(vec_of_erased![
                            Ref::new("StatementSegment"),
                            Ref::new("DelimiterGrammar").optional()
                        ])
                    ]),
                    MetaSegment::dedent()
                ])
                .config(|this| this.optional())
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
            Sequence::new(vec_of_erased![
                Ref::keyword("WHILE"),
                Ref::new("ExpressionSegment"),
                Ref::new("StatementSegment")
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // FOR JSON/XML/BROWSE clause for SELECT statements
    dialect.add([(
        "ForClauseSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("FOR"),
            one_of(vec_of_erased![
                // FOR JSON
                Sequence::new(vec_of_erased![
                    Ref::keyword("JSON"),
                    one_of(vec_of_erased![Ref::keyword("AUTO"), Ref::keyword("PATH")]),
                    // Optional modifiers
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("CommaSegment"),
                        one_of(vec_of_erased![
                            // ROOT option
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ROOT"),
                                Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                                    Ref::new("QuotedLiteralSegment")
                                ])])
                                .config(|this| this.optional())
                            ]),
                            // INCLUDE_NULL_VALUES
                            Ref::keyword("INCLUDE_NULL_VALUES"),
                            // WITHOUT_ARRAY_WRAPPER
                            Ref::keyword("WITHOUT_ARRAY_WRAPPER")
                        ])
                    ])])
                ]),
                // FOR XML
                Sequence::new(vec_of_erased![
                    Ref::keyword("XML"),
                    one_of(vec_of_erased![
                        Ref::keyword("RAW"),
                        Ref::keyword("AUTO"),
                        Ref::keyword("EXPLICIT"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PATH"),
                            Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                                Ref::new("QuotedLiteralSegment")
                            ])])
                            .config(|this| this.optional())
                        ])
                    ]),
                    // Optional modifiers
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("CommaSegment"),
                        one_of(vec_of_erased![
                            // TYPE
                            Ref::keyword("TYPE"),
                            // ROOT option
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ROOT"),
                                Sequence::new(vec_of_erased![Bracketed::new(vec_of_erased![
                                    Ref::new("QuotedLiteralSegment")
                                ])])
                                .config(|this| this.optional())
                            ]),
                            // ELEMENTS
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ELEMENTS"),
                                one_of(vec_of_erased![
                                    Ref::keyword("XSINIL"),
                                    Ref::keyword("ABSENT")
                                ])
                                .config(|this| this.optional())
                            ]),
                            // BINARY BASE64
                            Sequence::new(vec_of_erased![
                                Ref::keyword("BINARY"),
                                Ref::keyword("BASE64")
                            ])
                        ])
                    ])])
                ]),
                // FOR BROWSE
                Ref::keyword("BROWSE")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // OPENJSON table-valued function
    dialect.add([(
        "OpenJsonSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("OPENJSON"),
            Bracketed::new(vec_of_erased![
                Ref::new("ExpressionSegment"), // JSON expression
                // Optional path
                Sequence::new(vec_of_erased![
                    Ref::new("CommaSegment"),
                    Ref::new("QuotedLiteralSegment") // JSON path
                ])
                .config(|this| this.optional())
            ]),
            // Optional WITH clause for schema definition
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"), // Column name
                        Ref::new("DatatypeSegment"),        // Data type
                        // Optional JSON path
                        Sequence::new(vec_of_erased![Ref::new("QuotedLiteralSegment")])
                            .config(|this| this.optional()),
                        // Optional AS JSON
                        Sequence::new(vec_of_erased![Ref::keyword("AS"), Ref::keyword("JSON")])
                            .config(|this| this.optional())
                    ])
                ])])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // Override CREATE INDEX for T-SQL specific syntax
    dialect.replace_grammar(
        "CreateIndexStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                // UNIQUE is optional
                Ref::keyword("UNIQUE").optional(),
                // CLUSTERED or NONCLUSTERED
                one_of(vec_of_erased![
                    Ref::keyword("CLUSTERED"),
                    Ref::keyword("NONCLUSTERED")
                ])
                .config(|this| this.optional()),
                // COLUMNSTORE (for columnstore indexes)
                Ref::keyword("COLUMNSTORE").optional(),
                one_of(vec_of_erased![Ref::keyword("INDEX"), Ref::keyword("STATISTICS")]),
                Ref::new("IndexReferenceSegment"),
                Ref::keyword("ON"),
                Ref::new("TableReferenceSegment"),
                // Column list (optional for columnstore)
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        // Optional ASC/DESC
                        one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                            .config(|this| this.optional())
                    ])
                ])])
                .config(|this| this.optional()),
                // Optional INCLUDE clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("INCLUDE"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment")
                    ])])
                ])
                .config(|this| this.optional()),
                // Optional WHERE clause for filtered indexes
                Sequence::new(vec_of_erased![
                    Ref::keyword("WHERE"),
                    Ref::new("ExpressionSegment")
                ])
                .config(|this| this.optional()),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        one_of(vec_of_erased![
                            // Simple options
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    Ref::keyword("PAD_INDEX"),
                                    Ref::keyword("FILLFACTOR"),
                                    Ref::keyword("SORT_IN_TEMPDB"),
                                    Ref::keyword("IGNORE_DUP_KEY"),
                                    Ref::keyword("STATISTICS_NORECOMPUTE"),
                                    Ref::keyword("STATISTICS_INCREMENTAL"),
                                    Ref::keyword("DROP_EXISTING"),
                                    Ref::keyword("RESUMABLE"),
                                    Ref::keyword("ALLOW_ROW_LOCKS"),
                                    Ref::keyword("ALLOW_PAGE_LOCKS"),
                                    Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY"),
                                    Ref::keyword("MAXDOP")
                                ]),
                                Ref::new("AssignmentOperatorSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("ON"),
                                    Ref::keyword("OFF"),
                                    Ref::new("NumericLiteralSegment")
                                ])
                            ]),
                            // DATA_COMPRESSION option
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DATA_COMPRESSION"),
                                Ref::new("AssignmentOperatorSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("NONE"),
                                    Ref::keyword("ROW"),
                                    Ref::keyword("PAGE"),
                                    Ref::keyword("COLUMNSTORE"),
                                    Ref::keyword("COLUMNSTORE_ARCHIVE")
                                ]),
                                // Optional ON PARTITIONS clause
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ON"),
                                    Ref::keyword("PARTITIONS"),
                                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                        one_of(vec_of_erased![
                                            // Single partition number
                                            Ref::new("NumericLiteralSegment"),
                                            // Range of partitions
                                            Sequence::new(vec_of_erased![
                                                Ref::new("NumericLiteralSegment"),
                                                Ref::keyword("TO"),
                                                Ref::new("NumericLiteralSegment")
                                            ])
                                        ])
                                    ])])
                                ])
                                .config(|this| this.optional())
                            ]),
                            // ONLINE option with sub-options
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ONLINE"),
                                Ref::new("AssignmentOperatorSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("OFF"),
                                    Ref::keyword("ON"),
                                    // ONLINE = ON with WAIT_AT_LOW_PRIORITY
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("ON"),
                                        Bracketed::new(vec_of_erased![
                                            Ref::keyword("WAIT_AT_LOW_PRIORITY"),
                                            Bracketed::new(vec_of_erased![Delimited::new(
                                                vec_of_erased![
                                                    // MAX_DURATION
                                                    Sequence::new(vec_of_erased![
                                                        Ref::keyword("MAX_DURATION"),
                                                        Ref::new("AssignmentOperatorSegment"),
                                                        Ref::new("NumericLiteralSegment"),
                                                        Ref::keyword("MINUTES").optional()
                                                    ]),
                                                    // ABORT_AFTER_WAIT
                                                    Sequence::new(vec_of_erased![
                                                        Ref::keyword("ABORT_AFTER_WAIT"),
                                                        Ref::new("AssignmentOperatorSegment"),
                                                        one_of(vec_of_erased![
                                                            Ref::keyword("NONE"),
                                                            Ref::keyword("SELF"),
                                                            Ref::keyword("BLOCKERS")
                                                        ])
                                                    ])
                                                ]
                                            )])
                                        ])
                                    ])
                                ])
                            ])
                        ])
                    ])])
                ])
                .config(|this| this.optional()),
                // Optional ON filegroup/partition_scheme
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"), // filegroup or partition scheme
                        Ref::keyword("PRIMARY")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    );

    // Add UPDATE/DROP STATISTICS statements
    dialect.add([
        (
            "UpdateStatisticsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("UPDATE"),
                    Ref::keyword("STATISTICS"),
                    Ref::new("TableReferenceSegment"),
                    // Optional specific statistics or list
                    one_of(vec_of_erased![
                        // Single statistics name
                        Ref::new("ObjectReferenceSegment"),
                        // List of statistics in parentheses
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Ref::new("ObjectReferenceSegment")
                        ])])
                    ])
                    .config(|this| this.optional()),
                    // Optional WITH options
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::keyword("FULLSCAN"),
                            Ref::keyword("RESAMPLE"),
                            Ref::keyword("NORECOMPUTE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SAMPLE"),
                                Ref::new("NumericLiteralSegment"),
                                one_of(vec_of_erased![Ref::keyword("PERCENT"), Ref::keyword("ROWS")])
                            ])
                        ])])
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropStatisticsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("STATISTICS"),
                    // Table.StatisticsName format
                    Sequence::new(vec_of_erased![
                        Ref::new("TableReferenceSegment"),
                        Ref::new("DotSegment"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // WAITFOR statement
    dialect.add([
        (
            "WaitforStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WAITFOR"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DELAY"),
                            Ref::new("ExpressionSegment") // Time expression like '02:00'
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("TIME"),
                            Ref::new("ExpressionSegment") // Time expression like '22:20'
                        ])
                    ]),
                    // Optional TIMEOUT
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TIMEOUT"),
                        Ref::new("NumericLiteralSegment")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // CREATE TYPE statement
    dialect.add([
        (
            "CreateTypeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTypeStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("TYPE"),
                    Ref::new("ObjectReferenceSegment"),
                    one_of(vec_of_erased![
                        // CREATE TYPE name FROM type
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::new("ObjectReferenceSegment")
                        ]),
                        // CREATE TYPE name AS TABLE (...)
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS"),
                            Ref::keyword("TABLE"),
                            Bracketed::new(vec_of_erased![
                                Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::new("TableConstraintSegment"),
                                        Ref::new("ColumnDefinitionSegment")
                                    ])
                                ])
                                .config(|this| this.allow_trailing())
                            ])
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // BULK INSERT statement
    dialect.add([
        (
            "BulkInsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("BULK"),
                    Ref::keyword("INSERT"),
                    Ref::new("TableReferenceSegment"),
                    Ref::keyword("FROM"),
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("BulkInsertWithSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BulkInsertWithSegment".into(),
            NodeMatcher::new(SyntaxKind::WithDataClause, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            one_of(vec_of_erased![
                                // Numeric options: BATCHSIZE = 1024
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("BATCHSIZE"),
                                        Ref::keyword("FIRSTROW"),
                                        Ref::keyword("KILOBYTES_PER_BATCH"),
                                        Ref::keyword("LASTROW"),
                                        Ref::keyword("MAXERRORS"),
                                        Ref::keyword("ROWS_PER_BATCH")
                                    ]),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("NumericLiteralSegment")
                                ]),
                                // String options: FORMAT = 'CSV'
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("CODEPAGE"),
                                        Ref::keyword("DATAFILETYPE"),
                                        Ref::keyword("DATA_SOURCE"),
                                        Ref::keyword("ERRORFILE"),
                                        Ref::keyword("ERRORFILE_DATA_SOURCE"),
                                        Ref::keyword("FORMATFILE_DATA_SOURCE"),
                                        Ref::keyword("ROWTERMINATOR"),
                                        Ref::keyword("FORMAT"),
                                        Ref::keyword("FIELDQUOTE"),
                                        Ref::keyword("FORMATFILE"),
                                        Ref::keyword("FIELDTERMINATOR")
                                    ]),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("QuotedLiteralSegment")
                                ]),
                                // ORDER clause: ORDER (col1 ASC, col2 DESC)
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("ORDER"),
                                    Bracketed::new(vec_of_erased![
                                        Delimited::new(vec_of_erased![
                                            Sequence::new(vec_of_erased![
                                                Ref::new("ColumnReferenceSegment"),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("ASC"),
                                                    Ref::keyword("DESC")
                                                ])
                                                .config(|this| this.optional())
                                            ])
                                        ])
                                    ])
                                ]),
                                // Boolean flags
                                Ref::keyword("CHECK_CONSTRAINTS"),
                                Ref::keyword("FIRE_TRIGGERS"),
                                Ref::keyword("KEEPIDENTITY"),
                                Ref::keyword("KEEPNULLS"),
                                Ref::keyword("TABLOCK")
                            ])
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // CREATE PARTITION FUNCTION statement
    dialect.add([
        (
            "CreatePartitionFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("PARTITION"),
                    Ref::keyword("FUNCTION"),
                    Ref::new("ObjectReferenceSegment"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("DatatypeSegment")
                    ]),
                    Ref::keyword("AS"),
                    Ref::keyword("RANGE"),
                    one_of(vec_of_erased![
                        Ref::keyword("LEFT"),
                        Ref::keyword("RIGHT")
                    ]),
                    Ref::keyword("FOR"),
                    Ref::keyword("VALUES"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            Ref::new("LiteralGrammar")
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ALTER PARTITION FUNCTION statement
    dialect.add([
        (
            "AlterPartitionFunctionSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterFunctionStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("PARTITION"),
                    Ref::keyword("FUNCTION"),
                    Ref::new("ObjectReferenceSegment"),
                    Bracketed::new(vec_of_erased![]), // Empty brackets ()
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SPLIT"),
                            Ref::keyword("RANGE"),
                            Bracketed::new(vec_of_erased![
                                Ref::new("LiteralGrammar")
                            ])
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("MERGE"),
                            Ref::keyword("RANGE"),
                            Bracketed::new(vec_of_erased![
                                Ref::new("LiteralGrammar")
                            ])
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // CREATE PARTITION SCHEME statement
    dialect.add([
        (
            "CreatePartitionSchemeSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("PARTITION"),
                    Ref::keyword("SCHEME"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("AS"),
                    Ref::keyword("PARTITION"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("ALL").optional(),
                    Ref::keyword("TO"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("ObjectReferenceSegment"),
                                Ref::keyword("PRIMARY")
                            ])
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ALTER PARTITION SCHEME statement
    dialect.add([
        (
            "AlterPartitionSchemeSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterDatabaseStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("PARTITION"),
                    Ref::keyword("SCHEME"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("NEXT"),
                    Ref::keyword("USED"),
                    Ref::new("ObjectReferenceSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // CREATE FULLTEXT INDEX statement
    dialect.add([
        (
            "CreateFullTextIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("FULLTEXT"),
                    Ref::keyword("INDEX"),
                    Ref::keyword("ON"),
                    Ref::new("TableReferenceSegment"),
                    // Column specifications
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                // Optional column options
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        // TYPE COLUMN datatype
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("TYPE"),
                                            Ref::keyword("COLUMN"),
                                            Ref::new("DatatypeSegment")
                                        ]),
                                        // LANGUAGE (number | 'string' | nothing)
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("LANGUAGE"),
                                            one_of(vec_of_erased![
                                                Ref::new("NumericLiteralSegment"),
                                                Ref::new("QuotedLiteralSegment")
                                            ])
                                            .config(|this| this.optional())
                                        ]),
                                        // STATISTICAL_SEMANTICS
                                        Ref::keyword("STATISTICAL_SEMANTICS")
                                    ])
                                ])
                                .config(|this| this.optional())
                            ])
                        ])
                    ]),
                    // KEY INDEX clause
                    Sequence::new(vec_of_erased![
                        Ref::keyword("KEY"),
                        Ref::keyword("INDEX"),
                        Ref::new("ObjectReferenceSegment"),
                        // Optional catalog/filegroup options
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ON"),
                            Delimited::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    Ref::new("ObjectReferenceSegment"),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("FILEGROUP"),
                                        Ref::new("ObjectReferenceSegment")
                                    ])
                                ])
                            ])
                            .config(|this| this.allow_trailing())
                        ])
                        .config(|this| this.optional())
                    ]),
                    // Optional WITH clause
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    // CHANGE_TRACKING [=] (MANUAL | AUTO | OFF [, NO POPULATION])
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("CHANGE_TRACKING"),
                                        Ref::new("EqualsSegment").optional(),
                                        one_of(vec_of_erased![
                                            Ref::keyword("MANUAL"),
                                            Ref::keyword("AUTO"),
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("OFF"),
                                                Sequence::new(vec_of_erased![
                                                    Ref::keyword("NO"),
                                                    Ref::keyword("POPULATION")
                                                ])
                                                .config(|this| this.optional())
                                            ])
                                        ])
                                    ]),
                                    // STOPLIST [=] (OFF | SYSTEM | stoplist_name)
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("STOPLIST"),
                                        Ref::new("EqualsSegment").optional(),
                                        one_of(vec_of_erased![
                                            Ref::keyword("OFF"),
                                            Ref::keyword("SYSTEM"),
                                            Ref::new("ObjectReferenceSegment")
                                        ])
                                    ]),
                                    // SEARCH PROPERTY LIST [=] property_list_name
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("SEARCH"),
                                        Ref::keyword("PROPERTY"),
                                        Ref::keyword("LIST"),
                                        Ref::new("EqualsSegment").optional(),
                                        Ref::new("ObjectReferenceSegment")
                                    ])
                                ])
                            ])
                        ])
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ALTER INDEX statement
    dialect.add([
        (
            "AlterIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterIndexStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("INDEX"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::keyword("ALL")
                    ]),
                    Ref::keyword("ON"),
                    Ref::new("TableReferenceSegment"),
                    one_of(vec_of_erased![
                        // REBUILD [PARTITION = partition_number | ALL] [WITH (...)]
                        Sequence::new(vec_of_erased![
                            Ref::keyword("REBUILD"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITION"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("ALL"),
                                    Ref::new("NumericLiteralSegment")
                                ])
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Bracketed::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![
                                        one_of(vec_of_erased![
                                            Sequence::new(vec_of_erased![
                                                one_of(vec_of_erased![
                                                    Ref::keyword("PAD_INDEX"),
                                                    Ref::keyword("FILLFACTOR"),
                                                    Ref::keyword("SORT_IN_TEMPDB"),
                                                    Ref::keyword("IGNORE_DUP_KEY"),
                                                    Ref::keyword("STATISTICS_NORECOMPUTE"),
                                                    Ref::keyword("STATISTICS_INCREMENTAL"),
                                                    Ref::keyword("RESUMABLE"),
                                                    Ref::keyword("ALLOW_ROW_LOCKS"),
                                                    Ref::keyword("ALLOW_PAGE_LOCKS"),
                                                    Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY")
                                                ]),
                                                Ref::new("EqualsSegment"),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("ON"),
                                                    Ref::keyword("OFF")
                                                ])
                                            ]),
                                            Sequence::new(vec_of_erased![
                                                one_of(vec_of_erased![
                                                    Ref::keyword("MAXDOP"),
                                                    Ref::keyword("MAX_DURATION")
                                                ]),
                                                Ref::new("EqualsSegment"),
                                                Ref::new("NumericLiteralSegment"),
                                                Ref::keyword("MINUTES").optional()
                                            ]),
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("DATA_COMPRESSION"),
                                                Ref::new("EqualsSegment"),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("NONE"),
                                                    Ref::keyword("ROW"),
                                                    Ref::keyword("PAGE"),
                                                    Ref::keyword("COLUMNSTORE"),
                                                    Ref::keyword("COLUMNSTORE_ARCHIVE")
                                                ])
                                            ]),
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("ONLINE"),
                                                Ref::new("EqualsSegment"),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("ON"),
                                                    Ref::keyword("OFF")
                                                ])
                                            ])
                                        ])
                                    ])
                                ])
                            ])
                            .config(|this| this.optional())
                        ]),
                        // REORGANIZE [PARTITION = partition_number] [WITH (...)]
                        Sequence::new(vec_of_erased![
                            Ref::keyword("REORGANIZE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITION"),
                                Ref::new("EqualsSegment"),
                                Ref::new("NumericLiteralSegment")
                            ])
                            .config(|this| this.optional()),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Bracketed::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![
                                        Sequence::new(vec_of_erased![
                                            one_of(vec_of_erased![
                                                Ref::keyword("LOB_COMPACTION"),
                                                Ref::keyword("COMPRESS_ALL_ROW_GROUPS")
                                            ]),
                                            Ref::new("EqualsSegment"),
                                            one_of(vec_of_erased![
                                                Ref::keyword("ON"),
                                                Ref::keyword("OFF")
                                            ])
                                        ])
                                    ])
                                ])
                            ])
                            .config(|this| this.optional())
                        ]),
                        // SET (option = value, ...)
                        Sequence::new(vec_of_erased![
                            Ref::keyword("SET"),
                            Bracketed::new(vec_of_erased![
                                Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Sequence::new(vec_of_erased![
                                            one_of(vec_of_erased![
                                                Ref::keyword("ALLOW_ROW_LOCKS"),
                                                Ref::keyword("ALLOW_PAGE_LOCKS"),
                                                Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY"),
                                                Ref::keyword("IGNORE_DUP_KEY"),
                                                Ref::keyword("STATISTICS_NORECOMPUTE")
                                            ]),
                                            Ref::new("EqualsSegment"),
                                            one_of(vec_of_erased![
                                                Ref::keyword("ON"),
                                                Ref::keyword("OFF")
                                            ])
                                        ]),
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("COMPRESSION_DELAY"),
                                            Ref::new("EqualsSegment"),
                                            Ref::new("NumericLiteralSegment"),
                                            Ref::keyword("MINUTES").optional()
                                        ])
                                    ])
                                ])
                            ])
                        ]),
                        // RESUME [WITH (...)]
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RESUME"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("WITH"),
                                Bracketed::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![
                                        Sequence::new(vec_of_erased![
                                            one_of(vec_of_erased![
                                                Ref::keyword("MAXDOP"),
                                                Ref::keyword("MAX_DURATION")
                                            ]),
                                            Ref::new("EqualsSegment"),
                                            Ref::new("NumericLiteralSegment"),
                                            Ref::keyword("MINUTES").optional()
                                        ])
                                    ])
                                ])
                            ])
                            .config(|this| this.optional())
                        ]),
                        // Simple operations without options
                        Ref::keyword("DISABLE"),
                        Ref::keyword("PAUSE"),
                        Ref::keyword("ABORT")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // ALTER TABLE SWITCH statement
    dialect.add([
        (
            "AlterTableSwitchStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableSwitchStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    Ref::keyword("TABLE"),
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("SWITCH"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PARTITION"),
                        Ref::new("NumericLiteralSegment")
                    ])
                    .config(|this| this.optional()),
                    Ref::keyword("TO"),
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PARTITION"),
                        Ref::new("NumericLiteralSegment")
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        one_of(vec_of_erased![
                            // WAIT_AT_LOW_PRIORITY option
                            Bracketed::new(vec_of_erased![
                                Ref::keyword("WAIT_AT_LOW_PRIORITY"),
                                Bracketed::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("MAX_DURATION"),
                                            Ref::new("EqualsSegment"),
                                            Ref::new("NumericLiteralSegment"),
                                            Ref::keyword("MINUTES").optional()
                                        ]),
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("ABORT_AFTER_WAIT"),
                                            Ref::new("EqualsSegment"),
                                            one_of(vec_of_erased![
                                                Ref::keyword("NONE"),
                                                Ref::keyword("SELF"),
                                                Ref::keyword("BLOCKERS")
                                            ])
                                        ])
                                    ])
                                ])
                            ]),
                            // TRUNCATE_TARGET option (Azure Synapse Analytics)
                            Bracketed::new(vec_of_erased![
                                Ref::keyword("TRUNCATE_TARGET"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("ON"),
                                    Ref::keyword("OFF")
                                ])
                            ])
                        ])
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
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
            one_of(vec_of_erased![
                // PIVOT (SUM(Amount) FOR Month IN ([Jan], [Feb], [Mar]))
                Sequence::new(vec_of_erased![
                    Ref::keyword("PIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "LiteralGrammar"
                        )])])
                    ])
                ]),
                // UNPIVOT (Value FOR Month IN ([Jan], [Feb], [Mar]))
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNPIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])])
                    ])
                ])
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
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("START"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BEGIN"),
                        one_of(vec_of_erased![
                            Ref::keyword("TRANSACTION"),
                            Ref::keyword("WORK"),
                            Ref::keyword("TRAN") // T-SQL also supports TRAN
                        ])
                    ]),
                    Ref::keyword("COMMIT"),
                    Ref::keyword("ROLLBACK"),
                    Ref::keyword("SAVE") // T-SQL savepoints
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("TRANSACTION"),
                    Ref::keyword("WORK"),
                    Ref::keyword("TRAN") // T-SQL abbreviation
                ])
                .config(|this| this.optional()),
                // Optional transaction/savepoint name
                Ref::new("SingleIdentifierGrammar").optional()
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // GO batch separator - T-SQL uses GO to separate batches
    dialect.add([
        (
            "BatchSeparatorSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("GO"),
                // GO can optionally be followed by a count (e.g., GO 10)
                Ref::new("NumericLiteralSegment").optional()
            ])
            .to_matchable()
            .into(),
        ),
        (
            "BatchSeparatorGrammar".into(),
            Ref::new("BatchSeparatorSegment").to_matchable().into(),
        ),
        (
            "BatchDelimiterGrammar".into(),
            Ref::new("BatchSeparatorGrammar").to_matchable().into(),
        ),
    ]);

    // Override FileSegment to handle T-SQL batch separators (GO statements)
    // This creates a file structure where GO separates batches of statements
    dialect.replace_grammar(
        "FileSegment",
        Sequence::new(vec_of_erased![
            // Allow GO at the start of the file
            AnyNumberOf::new(vec_of_erased![
                Ref::new("BatchDelimiterGrammar"),
                Ref::new("DelimiterGrammar").optional()
            ]),
            // Main content: statements optionally separated by GO
            AnyNumberOf::new(vec_of_erased![
                Ref::new("StatementSegment"),
                Ref::new("DelimiterGrammar").optional(),
                // GO acts as a batch separator
                Sequence::new(vec_of_erased![
                    Ref::new("BatchDelimiterGrammar"),
                    Ref::new("DelimiterGrammar").optional()
                ])
                .config(|this| this.optional())
            ])
        ])
        .to_matchable(),
    );

    // Add T-SQL specific statement types to the statement segment
    dialect.replace_grammar(
        "StatementSegment",
        one_of(vec_of_erased![
            // T-SQL specific statements (BEGIN...END blocks must come first to avoid transaction conflicts)
            Ref::new("BeginEndBlockGrammar"),
            Ref::new("TryBlockSegment"),
            Ref::new("AtomicBlockSegment"),
            Ref::new("DeclareStatementGrammar"),
            Ref::new("SetVariableStatementSegment"),
            Ref::new("PrintStatementGrammar"),
            Ref::new("IfStatementGrammar"),
            Ref::new("WhileStatementGrammar"),
            Ref::new("GotoStatementSegment"),
            Ref::new("LabelSegment"),
            Ref::new("ExecuteStatementGrammar"),
            Ref::new("UseStatementGrammar"),
            Ref::new("WaitforStatementSegment"),
            Ref::new("CreateTypeStatementSegment"),
            Ref::new("BulkInsertStatementSegment"),
            Ref::new("CreatePartitionFunctionSegment"),
            Ref::new("AlterPartitionFunctionSegment"),
            Ref::new("CreatePartitionSchemeSegment"),
            Ref::new("AlterPartitionSchemeSegment"),
            Ref::new("CreateFullTextIndexStatementSegment"),
            Ref::new("AlterIndexStatementSegment"),
            Ref::new("AlterTableSwitchStatementSegment"),
            Ref::new("CreateExternalDataSourceStatementSegment"),
            Ref::new("CreateExternalFileFormatStatementSegment"),
            Ref::new("CreateExternalTableStatementSegment"),
            Ref::new("DropExternalTableStatementSegment"),
            Ref::new("CreateLoginStatementSegment"),
            Ref::new("CreateSecurityPolicyStatementSegment"),
            Ref::new("AlterSecurityPolicyStatementSegment"),
            Ref::new("DropSecurityPolicyStatementSegment"),
            Ref::new("DisableTriggerStatementSegment"),
            Ref::new("RaiserrorStatementSegment"),
            // Cursor statements
            Ref::new("DeclareCursorStatementSegment"),
            Ref::new("OpenCursorStatementSegment"),
            Ref::new("FetchCursorStatementSegment"),
            Ref::new("CloseCursorStatementSegment"),
            Ref::new("DeallocateCursorStatementSegment"),
            Ref::new("CreateSynonymStatementSegment"),
            Ref::new("DropSynonymStatementSegment"),
            // Include all ANSI statement types
            Ref::new("SelectableGrammar"),
            Ref::new("MergeStatementSegment"),
            Ref::new("InsertStatementSegment"),
            Ref::new("TransactionStatementSegment"),
            Ref::new("DropTableStatementSegment"),
            Ref::new("DropViewStatementSegment"),
            Ref::new("CreateUserStatementSegment"),
            Ref::new("DropUserStatementSegment"),
            Ref::new("TruncateStatementSegment"),
            Ref::new("AccessStatementSegment"),
            Ref::new("CreateTableStatementSegment"),
            Ref::new("CreateRoleStatementSegment"),
            Ref::new("DropRoleStatementSegment"),
            Ref::new("AlterTableStatementSegment"),
            Ref::new("CreateSchemaStatementSegment"),
            Ref::new("SetSchemaStatementSegment"),
            Ref::new("DropSchemaStatementSegment"),
            Ref::new("DropTypeStatementSegment"),
            Ref::new("CreateDatabaseStatementSegment"),
            Ref::new("DropDatabaseStatementSegment"),
            Ref::new("CreateIndexStatementSegment"),
            Ref::new("DropIndexStatementSegment"),
            Ref::new("UpdateStatisticsStatementSegment"),
            Ref::new("DropStatisticsStatementSegment"),
            Ref::new("CreateViewStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("CreateCastStatementSegment"),
            Ref::new("DropCastStatementSegment"),
            Ref::new("CreateFunctionStatementSegment"),
            Ref::new("DropFunctionStatementSegment"),
            Ref::new("CreateProcedureStatementSegment"),
            Ref::new("DropProcedureStatementSegment"),
            Ref::new("CreateModelStatementSegment"),
            Ref::new("DropModelStatementSegment"),
            Ref::new("DescribeStatementSegment"),
            Ref::new("ExplainStatementSegment"),
            Ref::new("CreateSequenceStatementSegment"),
            Ref::new("AlterSequenceStatementSegment"),
            Ref::new("DropSequenceStatementSegment"),
            Ref::new("CreateTriggerStatementSegment"),
            Ref::new("DropTriggerStatementSegment")
        ])
        .config(|this| this.terminators = vec_of_erased![Ref::new("DelimiterGrammar")])
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
            Sequence::new(vec_of_erased![
                Ref::keyword("USE"),
                Ref::new("DatabaseReferenceSegment")
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
        one_of(vec_of_erased![
            Ref::new("ObjectReferenceSegment"),
            Ref::new("TsqlVariableSegment"),
        ])
        .to_matchable(),
    );

    // Update TableExpressionSegment to include PIVOT/UNPIVOT and OPENJSON
    dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            Ref::new("FunctionSegment"),
            Ref::new("TableReferenceSegment"),
            Ref::new("OpenJsonSegment"),
            Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
            Sequence::new(vec_of_erased![
                Ref::new("TableReferenceSegment"),
                Ref::new("PivotUnpivotGrammar")
            ])
        ])
        .to_matchable(),
    );

    // Table hints support - Example: SELECT * FROM Users WITH (NOLOCK)
    dialect.add([
        (
            "TableHintSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "TableHintElement"
                )])])
                .config(|this| this.parse_mode = ParseMode::Greedy)
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TableHintElement".into(),
            one_of(vec_of_erased![
                // Simple hints (just keywords)
                Ref::keyword("NOLOCK"),
                Ref::keyword("READUNCOMMITTED"),
                Ref::keyword("READCOMMITTED"),
                Ref::keyword("REPEATABLEREAD"),
                Ref::keyword("SERIALIZABLE"),
                Ref::keyword("READPAST"),
                Ref::keyword("ROWLOCK"),
                Ref::keyword("TABLOCK"),
                Ref::keyword("TABLOCKX"),
                Ref::keyword("UPDLOCK"),
                Ref::keyword("XLOCK"),
                Ref::keyword("NOEXPAND"),
                Ref::keyword("FORCESEEK"),
                Ref::keyword("FORCESCAN"),
                Ref::keyword("HOLDLOCK"),
                Ref::keyword("SNAPSHOT"),
                // INDEX hint with parameter
                Sequence::new(vec_of_erased![
                    Ref::keyword("INDEX"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("NakedIdentifierSegment")
                    ])])
                ])
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
        Sequence::new(vec_of_erased![
            Ref::new("PreTableFunctionKeywordsGrammar").optional(),
            optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::new("FromClauseTerminatorGrammar"),
                    Ref::new("SamplingExpressionSegment"),
                    Ref::new("JoinLikeClauseGrammar"),
                    LookaheadExclude::new("WITH", "(") // Prevents WITH from being parsed as alias when followed by (
                ]))
                .optional(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("OFFSET"),
                Ref::new("AliasExpressionSegment")
            ])
            .config(|this| this.optional()),
            Ref::new("SamplingExpressionSegment").optional(),
            Ref::new("PostTableExpressionGrammar").optional() // T-SQL table hints
        ])
        .to_matchable(),
    );

    // Update JoinClauseSegment to handle APPLY syntax properly
    dialect.replace_grammar(
        "JoinClauseSegment",
        one_of(vec_of_erased![
            // Standard JOIN syntax
            Sequence::new(vec_of_erased![
                Ref::new("JoinTypeKeywordsGrammar").optional(),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                AnyNumberOf::new(vec_of_erased![Ref::new("NestedJoinGrammar")]),
                MetaSegment::dedent(),
                Sequence::new(vec_of_erased![
                    Conditional::new(MetaSegment::indent()).indented_using_on(),
                    one_of(vec_of_erased![
                        Ref::new("JoinOnConditionSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("USING"),
                            MetaSegment::indent(),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("SingleIdentifierGrammar")
                            ])])
                            .config(|this| this.parse_mode = ParseMode::Greedy),
                            MetaSegment::dedent(),
                        ])
                    ]),
                    Conditional::new(MetaSegment::dedent()).indented_using_on(),
                ])
                .config(|this| this.optional())
            ]),
            // NATURAL JOIN
            Sequence::new(vec_of_erased![
                Ref::new("NaturalJoinKeywordsGrammar"),
                Ref::new("JoinKeywordsGrammar"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent(),
            ]),
            // T-SQL APPLY syntax
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("CROSS"), Ref::keyword("OUTER")]),
                Ref::keyword("APPLY"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent(),
            ])
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
                    Sequence::new(vec_of_erased![
                        Ref::new("SignedSegmentGrammar"),
                        Ref::new("NumericLiteralSegment")
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
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("CROSS"), Ref::keyword("OUTER")]),
                Ref::keyword("APPLY"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"), // The function or subquery
                MetaSegment::dedent()
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
            Sequence::new(vec_of_erased![
                Ref::keyword("WITHIN"),
                Ref::keyword("GROUP"),
                Bracketed::new(vec_of_erased![Ref::new("OrderByClauseSegment").optional()])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override PostFunctionGrammar to include WITHIN GROUP
    dialect.add([(
        "PostFunctionGrammar".into(),
        AnyNumberOf::new(vec_of_erased![
            Ref::new("WithinGroupClauseSegment"),
            Ref::new("OverClauseSegment"),
            Ref::new("FilterClauseGrammar")
        ])
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL IDENTITY constraint support
    dialect.add([(
        "IdentityConstraintGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("IDENTITY"),
            Bracketed::new(vec_of_erased![
                Ref::new("NumericLiteralSegment"), // seed
                Ref::new("CommaSegment"),
                Ref::new("NumericLiteralSegment") // increment
            ])
            .config(|this| this.optional()) // IDENTITY() can be empty
        ])
        .to_matchable()
        .into(),
    )]);

    // Extend ColumnConstraintSegment to include T-SQL specific constraints
    dialect.add([(
        "ColumnConstraintSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_| {
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment"), // Constraint name
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    // NOT NULL / NULL
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NOT").optional(),
                        Ref::keyword("NULL"),
                    ]),
                    // CHECK constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CHECK"),
                        Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                    ]),
                    // DEFAULT constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        Ref::new("ColumnConstraintDefaultGrammar"),
                    ]),
                    Ref::new("PrimaryKeyGrammar"),
                    Ref::new("UniqueKeyGrammar"),
                    Ref::new("IdentityConstraintGrammar"), // T-SQL IDENTITY
                    Ref::new("AutoIncrementGrammar"),      // Keep ANSI AUTO_INCREMENT
                    Ref::new("ReferenceDefinitionGrammar"),
                    Ref::new("CommentClauseSegment"),
                    // COLLATE
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLLATE"),
                        Ref::new("CollationReferenceSegment"),
                    ]),
                ]),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add Unicode literal segment for N'...' strings
    dialect.add([(
        "UnicodeLiteralSegment".into(),
        TypedParser::new(SyntaxKind::UnicodeSingleQuote, SyntaxKind::QuotedLiteral)
            .to_matchable()
            .into(),
    )]);

    // Add T-SQL variable support to LiteralGrammar
    dialect.add([(
        "LiteralGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("QuotedLiteralSegment"),
            Ref::new("UnicodeLiteralSegment"), // Add Unicode strings
            Ref::new("NumericLiteralSegment"),
            Ref::new("BooleanLiteralGrammar"),
            Ref::new("QualifiedNumericLiteralSegment"),
            Ref::new("NullLiteralSegment"),
            Ref::new("DateTimeLiteralGrammar"),
            Ref::new("ArrayLiteralSegment"),
            Ref::new("TypedArrayLiteralSegment"),
            Ref::new("ObjectLiteralSegment"),
            Ref::new("ParameterizedSegment") // Add T-SQL variables
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE PROCEDURE support
    dialect.add([
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("CREATE"),
                        Ref::keyword("ALTER"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CREATE"),
                            Ref::keyword("OR"),
                            Ref::keyword("ALTER")
                        ])
                    ]),
                    one_of(vec_of_erased![
                        Ref::keyword("PROC"),
                        Ref::keyword("PROCEDURE")
                    ]),
                    Ref::new("ObjectReferenceSegment"),
                    // Optional version number
                    Sequence::new(vec_of_erased![
                        Ref::new("SemicolonSegment"),
                        Ref::new("NumericLiteralSegment")
                    ])
                    .config(|this| this.optional()),
                    MetaSegment::indent(),
                    // Optional parameter list
                    Ref::new("ProcedureParameterListGrammar").optional(),
                    // Procedure options
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Delimited::new(vec_of_erased![
                            Ref::keyword("ENCRYPTION"),
                            Ref::keyword("RECOMPILE"),
                            Ref::keyword("NATIVE_COMPILATION"),
                            Ref::keyword("SCHEMABINDING"),
                            Ref::new("ExecuteAsClauseGrammar")
                        ])
                    ])
                    .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOR"),
                        Ref::keyword("REPLICATION")
                    ])
                    .config(|this| this.optional()),
                    MetaSegment::dedent(),
                    Ref::keyword("AS"),
                    Ref::new("ProcedureDefinitionGrammar")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    one_of(vec_of_erased![
                        Ref::keyword("PROC"),
                        Ref::keyword("PROCEDURE")
                    ]),
                    Ref::new("IfExistsGrammar").optional(),
                    Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterListGrammar".into(),
            one_of(vec_of_erased![
                // Bracketed parameter list: (param1, param2, param3)
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("ProcedureParameterGrammar")])
                        .config(|this| this.optional())
                ]),
                // Unbracketed parameter list: param1, param2, param3
                Delimited::new(vec_of_erased![Ref::new("ProcedureParameterGrammar")])
                    .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TsqlDatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_| {
                one_of(vec_of_erased![
                    // Square bracket data type like [int], [varchar](100)
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DataTypeIdentifier),
                        Ref::new("BracketedArguments").optional()
                    ]),
                    // Regular data type (includes DatatypeIdentifierSegment for user-defined types)
                    Ref::new("DatatypeSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureParameterGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("ParameterNameSegment"),
                Ref::new("TsqlDatatypeSegment"),
                // Optional VARYING keyword (for cursors and some special types)
                Ref::keyword("VARYING").optional(),
                // Optional NULL/NOT NULL
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT").optional(),
                    Ref::keyword("NULL")
                ])
                .config(|this| this.optional()),
                // Optional default value
                Sequence::new(vec_of_erased![
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::new("LiteralGrammar"),
                        Ref::keyword("NULL"),
                        // Function calls as defaults (e.g., NEWID())
                        Ref::new("FunctionSegment"),
                        // String literal with prefix (e.g., N'foo')
                        Sequence::new(vec_of_erased![
                            Ref::new("NakedIdentifierSegment"), // N, B, X etc.
                            Ref::new("QuotedLiteralSegment")
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Optional parameter modifiers (can appear in any order)
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::keyword("OUT"),
                    Ref::keyword("OUTPUT"),
                    Ref::keyword("READONLY")
                ])])
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
            Sequence::new(vec_of_erased![
                Ref::keyword("EXECUTE"),
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    Ref::keyword("CALLER"),
                    Ref::keyword("SELF"),
                    Ref::keyword("OWNER"),
                    Ref::new("QuotedLiteralSegment") // user name
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "ProcedureDefinitionGrammar".into(),
            one_of(vec_of_erased![
                // External CLR procedures (check this first as it's simpler)
                Sequence::new(vec_of_erased![
                    Ref::keyword("EXTERNAL"),
                    Ref::keyword("NAME"),
                    Ref::new("ObjectReferenceSegment")
                ]),
                // Atomic blocks for natively compiled procedures
                Ref::new("AtomicBlockSegment"),
                // Single statement or block
                Ref::new("StatementSegment"),
                // Multiple statements for procedures without BEGIN...END
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.min_times(2); // At least 2 statements to use this branch
                    this.parse_mode = ParseMode::Greedy;
                    // Don't terminate on delimiters, keep consuming statements
                    this.terminators = vec_of_erased![Ref::new("BatchSeparatorGrammar")];
                })
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
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                Ref::keyword("ATOMIC"),
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "AtomicBlockOptionGrammar"
                )])]),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ]),
                MetaSegment::dedent(),
                Ref::keyword("END")
            ])
            .to_matchable()
            .into(),
        ),
        (
            "AtomicBlockOptionGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("LANGUAGE"),
                    Ref::keyword("DATEFIRST"),
                    Ref::keyword("DATEFORMAT"),
                    Ref::keyword("DELAYED_DURABILITY"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("TRANSACTION"),
                        Ref::keyword("ISOLATION"),
                        Ref::keyword("LEVEL")
                    ])
                ]),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("UnicodeLiteralSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("NakedIdentifierSegment"),
                    // Special handling for multi-word isolation levels
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPEATABLE"),
                        Ref::keyword("READ")
                    ]),
                    Ref::keyword("SERIALIZABLE"),
                    Ref::keyword("SNAPSHOT"),
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
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
        one_of(vec_of_erased![
            // T-SQL alias equals pattern: AliasName = Expression
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("NakedIdentifierSegment"),
                    Ref::new("QuotedIdentifierSegment")
                ]),
                // Use AssignmentOperator instead of RawComparisonOperator to distinguish from WHERE clause comparisons
                StringParser::new("=", SyntaxKind::AssignmentOperator),
                one_of(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("BaseExpressionElementGrammar")
                ])
            ]),
            // Wildcard expressions
            Ref::new("WildcardExpressionSegment"),
            // Everything else
            Sequence::new(vec_of_erased![
                Ref::new("BaseExpressionElementGrammar"),
                Ref::new("AliasExpressionSegment").optional(),
            ]),
        ])
        .to_matchable(),
    );

    // Override UnorderedSelectStatementSegment to add FOR clause
    dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        Sequence::new(vec_of_erased![
            Ref::new("SelectClauseSegment"),
            MetaSegment::dedent(),
            Ref::new("FromClauseSegment").optional(),
            Ref::new("WhereClauseSegment").optional(),
            Ref::new("GroupByClauseSegment").optional(),
            Ref::new("HavingClauseSegment").optional(),
            Ref::new("OverlapsClauseSegment").optional(),
            Ref::new("NamedWindowSegment").optional(),
            // T-SQL specific: FOR JSON/XML/BROWSE clause
            Ref::new("ForClauseSegment").optional()
        ])
        .terminators(vec_of_erased![
            Ref::new("SetOperatorSegment"),
            Ref::new("WithNoSchemaBindingClauseSegment"),
            Ref::new("WithDataClauseSegment"),
            Ref::new("OrderByClauseSegment"),
            Ref::new("LimitClauseSegment")
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    // Override SelectStatementSegment to add FOR clause after ORDER BY
    dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec_of_erased![
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("FetchClauseSegment").optional(),
                Ref::new("LimitClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional(),
                // T-SQL specific: FOR JSON/XML/BROWSE clause
                Ref::new("ForClauseSegment").optional()
            ]),
            None,
            None,
            None,
            vec_of_erased![
                Ref::new("SetOperatorSegment"),
                Ref::new("WithNoSchemaBindingClauseSegment"),
                Ref::new("WithDataClauseSegment")
            ],
            true,
        ),
    );

    // T-SQL CREATE TABLE with Azure Synapse Analytics support
    dialect.replace_grammar(
        "CreateTableStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("TABLE"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("TableReferenceSegment"),
                one_of(vec_of_erased![
                    // Regular CREATE TABLE with column definitions
                    Sequence::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("TableConstraintSegment"),
                                Ref::new("ColumnDefinitionSegment")
                            ])])
                            .config(|this| this.allow_trailing())
                        ]),
                        // Azure Synapse table options
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("TableOptionGrammar")
                            ])])
                        ])
                        .config(|this| this.optional())
                    ]),
                    // CREATE TABLE AS SELECT with optional WITH clause before AS
                    Sequence::new(vec_of_erased![
                        // Azure Synapse table options (required for CTAS)
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("TableOptionGrammar")
                            ])])
                        ])
                        .config(|this| this.optional()),
                        Ref::keyword("AS"),
                        optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    dialect.add([(
        "TableOptionGrammar".into(),
        one_of(vec_of_erased![
            // Azure Synapse distribution options
            Sequence::new(vec_of_erased![
                Ref::keyword("DISTRIBUTION"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ROUND_ROBIN"),
                    Ref::keyword("REPLICATE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("HASH"),
                        Bracketed::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                    ])
                ])
            ]),
            // Azure Synapse index options
            one_of(vec_of_erased![
                Ref::keyword("HEAP"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTERED"),
                    Ref::keyword("COLUMNSTORE"),
                    Ref::keyword("INDEX")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTERED"),
                    Ref::keyword("INDEX"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])])
                ])
            ]),
            // Other table options
            Sequence::new(vec_of_erased![
                Ref::keyword("PARTITION"),
                Bracketed::new(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::keyword("RANGE"),
                    one_of(vec_of_erased![Ref::keyword("LEFT"), Ref::keyword("RIGHT")]),
                    Ref::keyword("FOR"),
                    Ref::keyword("VALUES"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    )])])
                ])
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL uses + for both arithmetic and string concatenation
    dialect.add([(
        "StringBinaryOperatorGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("ConcatSegment"), // Standard || operator
            Ref::new("PlusSegment"),   // T-SQL + operator for string concatenation
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

    // CREATE EXTERNAL DATA SOURCE
    dialect.add([(
        "CreateExternalDataSourceStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalDataSourceStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("EXTERNAL"),
                Ref::keyword("DATA"),
                Ref::keyword("SOURCE"),
                Ref::new("ObjectReferenceSegment"),
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("ExternalDataSourceOptionGrammar")
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ExternalDataSourceOptionGrammar".into(),
        one_of(vec_of_erased![
            // LOCATION = 'connection_string'
            Sequence::new(vec_of_erased![
                Ref::keyword("LOCATION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // CREDENTIAL = credential_name
            Sequence::new(vec_of_erased![
                Ref::keyword("CREDENTIAL"),
                Ref::new("EqualsSegment"),
                Ref::new("ObjectReferenceSegment")
            ]),
            // PUSHDOWN = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("PUSHDOWN"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
            ]),
            // CONNECTION_OPTIONS = 'options'
            Sequence::new(vec_of_erased![
                Ref::keyword("CONNECTION_OPTIONS"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // CREATE EXTERNAL FILE FORMAT
    dialect.add([(
        "CreateExternalFileFormatStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalFileFormatStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("EXTERNAL"),
                Ref::keyword("FILE"),
                Ref::keyword("FORMAT"),
                Ref::new("ObjectReferenceSegment"),
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("ExternalFileFormatOptionGrammar")
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ExternalFileFormatOptionGrammar".into(),
        one_of(vec_of_erased![
            // FORMAT_TYPE = format_type
            Sequence::new(vec_of_erased![
                Ref::keyword("FORMAT_TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("DELIMITEDTEXT"),
                    Ref::keyword("RCFILE"),
                    Ref::keyword("ORC"),
                    Ref::keyword("PARQUET"),
                    Ref::keyword("JSON"),
                    Ref::keyword("DELTA")
                ])
            ]),
            // FORMAT_OPTIONS (...)
            Sequence::new(vec_of_erased![
                Ref::keyword("FORMAT_OPTIONS"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("FormatOptionGrammar")
                    ])
                ])
            ]),
            // SERDE_METHOD = 'serde_class'
            Sequence::new(vec_of_erased![
                Ref::keyword("SERDE_METHOD"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // DATA_COMPRESSION = 'compression_codec'
            Sequence::new(vec_of_erased![
                Ref::keyword("DATA_COMPRESSION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // ENCODING = 'encoding_type'
            Sequence::new(vec_of_erased![
                Ref::keyword("ENCODING"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "FormatOptionGrammar".into(),
        one_of(vec_of_erased![
            // FIELD_TERMINATOR = 'delimiter'
            Sequence::new(vec_of_erased![
                Ref::keyword("FIELD_TERMINATOR"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // STRING_DELIMITER = 'delimiter'
            Sequence::new(vec_of_erased![
                Ref::keyword("STRING_DELIMITER"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // FIRST_ROW = number
            Sequence::new(vec_of_erased![
                Ref::keyword("FIRST_ROW"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment")
            ]),
            // USE_TYPE_DEFAULT = True/False
            Sequence::new(vec_of_erased![
                Ref::keyword("USE_TYPE_DEFAULT"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("TRUE"),
                    Ref::keyword("FALSE")
                ])
            ]),
            // DATE_FORMAT = 'format'
            Sequence::new(vec_of_erased![
                Ref::keyword("DATE_FORMAT"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // CREATE EXTERNAL TABLE
    dialect.add([(
        "CreateExternalTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateExternalTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("EXTERNAL"),
                Ref::keyword("TABLE"),
                Ref::new("ObjectReferenceSegment"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("ColumnDefinitionSegment")
                    ])
                    .config(|this| this.allow_trailing())
                ]),
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("ExternalTableOptionGrammar")
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "ExternalTableOptionGrammar".into(),
        one_of(vec_of_erased![
            // LOCATION = 'path'
            Sequence::new(vec_of_erased![
                Ref::keyword("LOCATION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // DATA_SOURCE = data_source_name
            Sequence::new(vec_of_erased![
                Ref::keyword("DATA_SOURCE"),
                Ref::new("EqualsSegment"),
                Ref::new("ObjectReferenceSegment")
            ]),
            // FILE_FORMAT = file_format_name
            Sequence::new(vec_of_erased![
                Ref::keyword("FILE_FORMAT"),
                Ref::new("EqualsSegment"),
                Ref::new("ObjectReferenceSegment")
            ]),
            // REJECT_TYPE = VALUE/PERCENTAGE
            Sequence::new(vec_of_erased![
                Ref::keyword("REJECT_TYPE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("VALUE"),
                    Ref::keyword("PERCENTAGE")
                ])
            ]),
            // REJECT_VALUE = number
            Sequence::new(vec_of_erased![
                Ref::keyword("REJECT_VALUE"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment")
            ]),
            // REJECT_SAMPLE_VALUE = number
            Sequence::new(vec_of_erased![
                Ref::keyword("REJECT_SAMPLE_VALUE"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment")
            ]),
            // REJECTED_ROW_LOCATION = 'path'
            Sequence::new(vec_of_erased![
                Ref::keyword("REJECTED_ROW_LOCATION"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // DROP EXTERNAL TABLE
    dialect.add([(
        "DropExternalTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropExternalTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("EXTERNAL"),
                Ref::keyword("TABLE"),
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CREATE LOGIN
    dialect.add([(
        "CreateLoginStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateLoginStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("LOGIN"),
                Ref::new("ObjectReferenceSegment"),
                one_of(vec_of_erased![
                    // WITH PASSWORD = 'password' [MUST_CHANGE] [, options]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("PASSWORD"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                        Ref::keyword("MUST_CHANGE").optional(),
                        // Additional options after MUST_CHANGE
                        AnyNumberOf::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("CommaSegment"),
                                Ref::new("LoginOptionGrammar")
                            ])
                        ])
                    ]),
                    // FROM WINDOWS
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::keyword("WINDOWS")
                    ]),
                    // FROM EXTERNAL PROVIDER
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::keyword("EXTERNAL"),
                        Ref::keyword("PROVIDER")
                    ]),
                    // FROM CERTIFICATE certificate_name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::keyword("CERTIFICATE"),
                        Ref::new("ObjectReferenceSegment")
                    ]),
                    // FROM ASYMMETRIC KEY key_name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::keyword("ASYMMETRIC"),
                        Ref::keyword("KEY"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "LoginOptionGrammar".into(),
        one_of(vec_of_erased![
            // CHECK_EXPIRATION = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECK_EXPIRATION"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
            ]),
            // CHECK_POLICY = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECK_POLICY"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
            ]),
            // DEFAULT_DATABASE = database_name
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT_DATABASE"),
                Ref::new("EqualsSegment"),
                Ref::new("DatabaseReferenceSegment")
            ]),
            // DEFAULT_LANGUAGE = language
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT_LANGUAGE"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment")
            ]),
            // SID = 0x...
            Sequence::new(vec_of_erased![
                Ref::keyword("SID"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment")
            ]),
            // CREDENTIAL = credential_name
            Sequence::new(vec_of_erased![
                Ref::keyword("CREDENTIAL"),
                Ref::new("EqualsSegment"),
                Ref::new("ObjectReferenceSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Override CREATE USER to support T-SQL specific syntax
    dialect.replace_grammar(
        "CreateUserStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateUserStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("USER"),
                Ref::new("ObjectReferenceSegment"),
                one_of(vec_of_erased![
                    // FOR/FROM LOGIN login_name
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("FOR"),
                            Ref::keyword("FROM")
                        ]),
                        Ref::keyword("LOGIN"),
                        Ref::new("ObjectReferenceSegment"),
                        // Optional WITH options
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Delimited::new(vec_of_erased![
                                Ref::new("UserOptionGrammar")
                            ])
                        ])
                        .config(|this| this.optional())
                    ]),
                    // WITH PASSWORD = 'password' [, SID = 0x...]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Delimited::new(vec_of_erased![
                            Ref::new("UserOptionGrammar")
                        ])
                    ]),
                    // FROM EXTERNAL PROVIDER
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::keyword("EXTERNAL"),
                        Ref::keyword("PROVIDER")
                    ]),
                    // WITHOUT LOGIN [WITH DEFAULT_SCHEMA = schema]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITHOUT"),
                        Ref::keyword("LOGIN"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Ref::keyword("DEFAULT_SCHEMA"),
                            Ref::new("EqualsSegment"),
                            Ref::new("ObjectReferenceSegment")
                        ])
                        .config(|this| this.optional())
                    ]),
                    // FOR CERTIFICATE certificate_name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOR"),
                        Ref::keyword("CERTIFICATE"),
                        Ref::new("ObjectReferenceSegment")
                    ]),
                    // FOR ASYMMETRIC KEY key_name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOR"),
                        Ref::keyword("ASYMMETRIC"),
                        Ref::keyword("KEY"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    dialect.add([(
        "UserOptionGrammar".into(),
        one_of(vec_of_erased![
            // PASSWORD = 'password'
            Sequence::new(vec_of_erased![
                Ref::keyword("PASSWORD"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            // SID = 0x...
            Sequence::new(vec_of_erased![
                Ref::keyword("SID"),
                Ref::new("EqualsSegment"),
                Ref::new("NumericLiteralSegment")
            ]),
            // DEFAULT_SCHEMA = schema_name
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT_SCHEMA"),
                Ref::new("EqualsSegment"),
                Ref::new("ObjectReferenceSegment")
            ]),
            // ALLOW_ENCRYPTED_VALUE_MODIFICATIONS = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("ALLOW_ENCRYPTED_VALUE_MODIFICATIONS"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Override DROP USER to support T-SQL specific syntax
    dialect.replace_grammar(
        "DropUserStatementSegment",
        NodeMatcher::new(SyntaxKind::DropUserStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("USER"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // CREATE SECURITY POLICY
    dialect.add([(
        "CreateSecurityPolicyStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateSecurityPolicyStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("SECURITY"),
                Ref::keyword("POLICY"),
                Ref::new("ObjectReferenceSegment"),
                // One or more ADD clauses
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("SecurityPolicyAddClause")
                ])
                .config(|this| this.min_times(1)),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![
                        Delimited::new(vec_of_erased![
                            Ref::new("SecurityPolicyOptionGrammar")
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Optional NOT FOR REPLICATION
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("FOR"),
                    Ref::keyword("REPLICATION")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "SecurityPolicyAddClause".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("ADD"),
            one_of(vec_of_erased![
                Ref::keyword("FILTER"),
                Ref::keyword("BLOCK")
            ]),
            Ref::keyword("PREDICATE"),
            // Function call: schema.function(column)
            Ref::new("FunctionSegment"),
            Ref::keyword("ON"),
            Ref::new("ObjectReferenceSegment"),
            // Optional AFTER INSERT/UPDATE/DELETE
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("AFTER"),
                    Ref::keyword("BEFORE")
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("UPDATE"),
                    Ref::keyword("DELETE")
                ])
            ])
            .config(|this| this.optional()),
            Ref::new("CommaSegment").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "SecurityPolicyOptionGrammar".into(),
        one_of(vec_of_erased![
            // STATE = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("STATE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
            ]),
            // SCHEMABINDING = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("SCHEMABINDING"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("OFF")
                ])
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // ALTER SECURITY POLICY
    dialect.add([(
        "AlterSecurityPolicyStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterSecurityPolicyStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("SECURITY"),
                Ref::keyword("POLICY"),
                Ref::new("ObjectReferenceSegment"),
                one_of(vec_of_erased![
                    // ADD/DROP/ALTER clauses
                    AnyNumberOf::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("SecurityPolicyAddClause"),
                            Ref::new("SecurityPolicyDropClause"),
                            Ref::new("SecurityPolicyAlterClause")
                        ])
                    ])
                    .config(|this| this.min_times(1)),
                    // WITH clause only
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![
                                Ref::new("SecurityPolicyOptionGrammar")
                            ])
                        ])
                    ])
                ]),
                // Optional NOT FOR REPLICATION
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("FOR"),
                    Ref::keyword("REPLICATION")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "SecurityPolicyDropClause".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            one_of(vec_of_erased![
                Ref::keyword("FILTER"),
                Ref::keyword("BLOCK")
            ]),
            Ref::keyword("PREDICATE"),
            Ref::keyword("ON"),
            Ref::new("ObjectReferenceSegment"),
            Ref::new("CommaSegment").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "SecurityPolicyAlterClause".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("ALTER"),
            one_of(vec_of_erased![
                Ref::keyword("FILTER"),
                Ref::keyword("BLOCK")
            ]),
            Ref::keyword("PREDICATE"),
            // Function call: schema.function(column)
            Ref::new("FunctionSegment"),
            Ref::keyword("ON"),
            Ref::new("ObjectReferenceSegment"),
            // Optional AFTER INSERT/UPDATE/DELETE
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("AFTER"),
                    Ref::keyword("BEFORE")
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("UPDATE"),
                    Ref::keyword("DELETE")
                ])
            ])
            .config(|this| this.optional()),
            Ref::new("CommaSegment").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    // DROP SECURITY POLICY
    dialect.add([(
        "DropSecurityPolicyStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropSecurityPolicyStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("SECURITY"),
                Ref::keyword("POLICY"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override CREATE TRIGGER to support CREATE OR ALTER TRIGGER and T-SQL specific features
    dialect.add([(
        "CreateTriggerStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateTriggerStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("OR"),
                    Ref::keyword("ALTER")
                ])
                .config(|this| this.optional()),
                Ref::keyword("TRIGGER"),
                Ref::new("TriggerReferenceSegment"),
                Ref::keyword("ON"),
                one_of(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALL"),
                        Ref::keyword("SERVER")
                    ]),
                    Ref::keyword("DATABASE")
                ]),
                // WITH clause for encryption options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::keyword("ENCRYPTION"),
                        Ref::keyword("NATIVE_COMPILATION"),
                        Ref::keyword("SCHEMABINDING")
                    ]),
                    Ref::new("ExecuteAsClause").optional()
                ])
                .config(|this| this.optional()),
                // Trigger timing
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOR"),
                        Delimited::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar")
                        ])
                        .config(|this| this.optional())
                    ]),
                    Ref::keyword("AFTER"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("INSTEAD"),
                        Ref::keyword("OF")
                    ])
                ])
                .config(|this| this.optional()),
                // Trigger events
                Delimited::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("UPDATE"),
                    Ref::keyword("DELETE"),
                    // DDL events for DATABASE/ALL SERVER triggers
                    Ref::new("SingleIdentifierGrammar")
                ])
                .config(|this| this.optional()),
                // Additional options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("APPEND")
                ])
                .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("FOR"),
                    Ref::keyword("REPLICATION")
                ])
                .config(|this| this.optional()),
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    // Single statement
                    Ref::new("StatementSegment"),
                    // Multiple statements in a BEGIN...END block
                    Ref::new("BeginEndBlockSegment")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add DISABLE TRIGGER statement
    dialect.add([(
        "DisableTriggerStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DISABLE"),
                Ref::keyword("TRIGGER"),
                one_of(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        Ref::new("TriggerReferenceSegment")
                    ]),
                    Ref::keyword("ALL")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"),
                        Ref::keyword("DATABASE"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ALL"),
                            Ref::keyword("SERVER")
                        ])
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // RAISERROR statement
    dialect.add([(
        "RaiserrorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("RAISERROR"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        // Message: numeric message ID, string literal, or variable
                        one_of(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("TsqlVariableSegment")
                        ]),
                        // Severity: numeric literal or variable
                        one_of(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("TsqlVariableSegment")
                        ]),
                        // State: numeric literal or variable
                        one_of(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("TsqlVariableSegment")
                        ])
                    ])
                    // Optional arguments for message formatting
                    .config(|this| {
                        this.allow_trailing();
                    })
                ]),
                // WITH options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Delimited::new(vec_of_erased![
                        Ref::keyword("LOG"),
                        Ref::keyword("NOWAIT"),
                        Ref::keyword("SETERROR")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // DECLARE CURSOR statement
    dialect.add([(
        "DeclareCursorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DeclareCursorStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DECLARE"),
                Ref::new("NakedIdentifierSegment"),
                Ref::keyword("CURSOR"),
                // Optional scope
                one_of(vec_of_erased![
                    Ref::keyword("LOCAL"),
                    Ref::keyword("GLOBAL")
                ])
                .config(|this| this.optional()),
                // Optional scroll behavior
                one_of(vec_of_erased![
                    Ref::keyword("FORWARD_ONLY"),
                    Ref::keyword("SCROLL")
                ])
                .config(|this| this.optional()),
                // Optional cursor type
                one_of(vec_of_erased![
                    Ref::keyword("STATIC"),
                    Ref::keyword("KEYSET"),
                    Ref::keyword("DYNAMIC"),
                    Ref::keyword("FAST_FORWARD")
                ])
                .config(|this| this.optional()),
                // Optional concurrency
                one_of(vec_of_erased![
                    Ref::keyword("READ_ONLY"),
                    Ref::keyword("SCROLL_LOCKS"),
                    Ref::keyword("OPTIMISTIC")
                ])
                .config(|this| this.optional()),
                // Optional TYPE_WARNING
                Ref::keyword("TYPE_WARNING").optional(),
                Ref::keyword("FOR"),
                Ref::new("SelectStatementSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Cursor name grammar - cursor name can be a variable or identifier
    dialect.add([(
        "CursorNameGrammar".into(),
        one_of(vec_of_erased![
            Sequence::new(vec_of_erased![
                Ref::keyword("GLOBAL").optional(),
                Ref::new("NakedIdentifierSegment")
            ]),
            Ref::new("ParameterNameSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // OPEN cursor statement
    dialect.add([(
        "OpenCursorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("OPEN"),
                Ref::new("CursorNameGrammar")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CLOSE cursor statement
    dialect.add([(
        "CloseCursorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CLOSE"),
                Ref::new("CursorNameGrammar")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // DEALLOCATE cursor statement
    dialect.add([(
        "DeallocateCursorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DEALLOCATE"),
                Ref::new("CursorNameGrammar")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // FETCH cursor statement
    dialect.add([(
        "FetchCursorStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("FETCH"),
                // Optional position
                one_of(vec_of_erased![
                    Ref::keyword("NEXT"),
                    Ref::keyword("PRIOR"),
                    Ref::keyword("FIRST"),
                    Ref::keyword("LAST"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ABSOLUTE"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RELATIVE"),
                        Ref::new("NumericLiteralSegment")
                    ])
                ])
                .config(|this| this.optional()),
                Ref::keyword("FROM"),
                Ref::new("CursorNameGrammar"),
                // Optional INTO clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTO"),
                    Delimited::new(vec_of_erased![
                        Ref::new("ParameterNameSegment")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add cursor statements to statement list (they're already in the list from before)
    // Just need to ensure DeclareCursorStatementSegment is recognized as a valid declare variant
    
    // CREATE SYNONYM statement
    dialect.add([(
        "CreateSynonymStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateSynonymStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("SYNONYM"),
                Ref::new("SynonymReferenceSegment"),
                Ref::keyword("FOR"),
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // DROP SYNONYM statement
    dialect.add([(
        "DropSynonymStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropSynonymStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("SYNONYM"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("SynonymReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Synonym reference segment - can have schema but not database/server
    dialect.add([(
        "SynonymReferenceSegment".into(),
        NodeMatcher::new(SyntaxKind::ObjectReference, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"),
                AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("SingleIdentifierGrammar")
                    ])
                ])
                .config(|this| this.max_times(1)) // Only allow schema.synonym, not server.db.schema.synonym
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);
    
    // expand() must be called after all grammar modifications

    dialect
}
