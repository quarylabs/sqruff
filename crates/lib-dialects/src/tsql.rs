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
    // 2. Update word pattern to allow # at the end (SQL Server 2017+ syntax)
    dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r"--[^\n]*", SyntaxKind::InlineComment),
        Matcher::regex("word", r"[0-9a-zA-Z_]+#?", SyntaxKind::Word),
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
            // - ##?[A-Z][A-Z0-9_]*    matches temp tables: #temp or ##global
            // - [A-Z0-9_]*[A-Z][A-Z0-9_]*#?   matches regular identifiers with optional # at end
            RegexParser::new(
                "(##?[A-Z][A-Z0-9_]*|[A-Z0-9_]*[A-Z][A-Z0-9_]*#?)",
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
                    Ref::new("TsqlVariableSegment"),
                    Ref::new("DatatypeSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::new("AssignmentOperatorSegment"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional())
                ])])
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
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::new("TsqlVariableSegment"),
                Ref::new("AssignmentOperatorSegment"),
                Ref::new("ExpressionSegment")
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
            Ref::new("BeginEndBlockGrammar").to_matchable().into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"),
                        Ref::new("InsertStatementSegment"),
                        Ref::new("UpdateStatementSegment"),
                        Ref::new("DeleteStatementSegment"),
                        Ref::new("DeclareStatementSegment"),
                        Ref::new("SetVariableStatementSegment"),
                        Ref::new("PrintStatementSegment"),
                        Ref::new("IfStatementSegment"),
                        Ref::new("WhileStatementSegment"),
                        Ref::new("BeginEndBlockSegment")
                    ]),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| this.min_times(0)),
                MetaSegment::dedent(),
                Ref::keyword("END")
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
                Ref::new("StatementSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ELSE"),
                    Ref::new("StatementSegment")
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

    // GO batch separator
    dialect.add([
        (
            "BatchSeparatorSegment".into(),
            Ref::new("BatchSeparatorGrammar").to_matchable().into(),
        ),
        (
            "BatchSeparatorGrammar".into(),
            Ref::keyword("GO").to_matchable().into(),
        ),
    ]);

    // Add T-SQL specific statement types to the statement segment
    dialect.replace_grammar(
        "StatementSegment",
        one_of(vec_of_erased![
            // T-SQL specific statements
            Ref::new("DeclareStatementGrammar"),
            Ref::new("SetVariableStatementGrammar"),
            Ref::new("PrintStatementGrammar"),
            Ref::new("BeginEndBlockGrammar"),
            Ref::new("IfStatementGrammar"),
            Ref::new("WhileStatementGrammar"),
            Ref::new("BatchSeparatorGrammar"),
            Ref::new("UseStatementGrammar"),
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
            Ref::new("CreateViewStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("CreateCastStatementSegment"),
            Ref::new("DropCastStatementSegment"),
            Ref::new("CreateFunctionStatementSegment"),
            Ref::new("DropFunctionStatementSegment"),
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

    // Update TableExpressionSegment to include PIVOT/UNPIVOT
    dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            Ref::new("FunctionSegment"),
            Ref::new("TableReferenceSegment"),
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

    // Add T-SQL variable support to LiteralGrammar
    dialect.add([(
        "LiteralGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("QuotedLiteralSegment"),
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

    // T-SQL supports alternative alias syntax: AliasName = Expression
    // The parser distinguishes between column references (table1.column1)
    // and alias assignments (AliasName = table1.column1)
    dialect.replace_grammar(
        "SelectClauseElementSegment",
        one_of(vec_of_erased![
            // T-SQL alias equals pattern: AliasName = Expression
            Sequence::new(vec_of_erased![
                Ref::new("NakedIdentifierSegment"),
                StringParser::new("=", SyntaxKind::RawComparisonOperator),
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

    // expand() must be called after all grammar modifications

    dialect
}
