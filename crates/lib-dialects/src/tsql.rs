// T-SQL (Transact-SQL) dialect implementation for Microsoft SQL Server

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
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser};
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
    // T-SQL inherits from ANSI SQL and adds additional reserved words
    // IMPORTANT: Don't clear ANSI keywords as they contain fundamental SQL keywords like FROM, SELECT, etc.
    dialect
        .sets_mut("reserved_keywords")
        .extend(tsql_keywords::tsql_additional_reserved_keywords());
    dialect
        .sets_mut("unreserved_keywords")
        .extend(tsql_keywords::tsql_additional_unreserved_keywords());

    // Add table hint keywords to unreserved keywords
    // These are used in WITH (NOLOCK) style hints on tables
    dialect.sets_mut("unreserved_keywords").extend([
        "NOLOCK",          // No shared locks, allows dirty reads
        "READUNCOMMITTED", // Same as NOLOCK
        "READCOMMITTED",   // Default isolation level
        "REPEATABLEREAD",  // Hold locks until transaction completes
        "SERIALIZABLE",    // Highest isolation level
        "READPAST",        // Skip locked rows
        "ROWLOCK",         // Force row-level locks
        "TABLOCK",         // Force table-level shared lock
        "TABLOCKX",        // Force table-level exclusive lock
        "UPDLOCK",         // Use update locks instead of shared
        "XLOCK",           // Force exclusive locks
        "NOEXPAND",        // Don't expand indexed views
        "INDEX",           // Force specific index usage
        "FORCESEEK",       // Force index seek operation
        "FORCESCAN",       // Force index scan operation
        "HOLDLOCK",        // Same as SERIALIZABLE
        "SNAPSHOT",        // Use snapshot isolation
    ]);

    // T-SQL specific operators
    // Compound assignment operators and special comparison operators
    dialect.sets_mut("operator_symbols").extend([
        "%=", // Modulo assignment
        "&=", // Bitwise AND assignment
        "*=", // Multiply assignment
        "+=", // Add assignment
        "-=", // Subtract assignment
        "/=", // Divide assignment
        "^=", // Bitwise XOR assignment
        "|=", // Bitwise OR assignment
        "!<", // Not less than
        "!>", // Not greater than
    ]);

    // T-SQL supports square brackets for identifiers and @ for variables
    // Insert these matchers before the equals matcher for proper precedence
    dialect.insert_lexer_matchers(
        vec![
            // Square brackets for identifiers with spaces/reserved words: [Column Name]
            Matcher::regex(
                "tsql_square_bracket_identifier",
                r"\[[^\]]*\]",
                SyntaxKind::DoubleQuote,
            ),
            // Variables start with @ (local) or @@ (global/system)
            // Examples: @MyVar, @@ROWCOUNT, @@IDENTITY
            Matcher::regex(
                "tsql_variable",
                r"@@?[a-zA-Z_][a-zA-Z0-9_]*",
                SyntaxKind::TsqlVariable,
            ),
        ],
        "equals",
    );

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

    // TOP clause support - wrapped in SelectClauseModifierSegment
    // Supports: SELECT TOP 10 ..., SELECT TOP (10) PERCENT ..., SELECT TOP 5 WITH TIES ...
    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        NodeMatcher::new(
            SyntaxKind::SelectClauseModifier,
            one_of(vec_of_erased![
                // Keep ANSI's DISTINCT/ALL
                Ref::keyword("DISTINCT"),
                Ref::keyword("ALL"),
                // Add T-SQL's TOP clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("TOP"),
                    // TOP can take a number, variable, or expression in parentheses
                    one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"), // TOP 10
                        Ref::new("TsqlVariableSegment"),   // TOP @RowCount
                        Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::new("NumericLiteralSegment"), // TOP (10)
                            Ref::new("TsqlVariableSegment"),   // TOP (@RowCount)
                            Ref::new("ExpressionSegment")      // TOP (SELECT COUNT(*)/2 FROM...)
                        ])])
                    ]),
                    Ref::keyword("PERCENT").optional(), // TOP 50 PERCENT
                    Ref::keyword("WITH").optional(),    // WITH TIES requires both keywords
                    Ref::keyword("TIES").optional()     // Returns all ties for last place
                ])
            ])
            .to_matchable(),
        )
        .to_matchable(),
    );

    // Add T-SQL assignment operator segment
    // Uses RawEqualsSegment instead of EqualsSegment to avoid being wrapped
    // in a comparison_operator node, which would be semantically incorrect
    dialect.add([(
        "AssignmentOperatorSegment".into(),
        NodeMatcher::new(
            SyntaxKind::AssignmentOperator,
            Ref::new("RawEqualsSegment").to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // DECLARE statement for variable declarations
    // Syntax: DECLARE @var1 INT = 10, @var2 VARCHAR(50) = 'text'
    // Note: T-SQL uses = for both assignment and comparison. We use AssignmentOperator
    // to distinguish assignment context from comparison context.
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
                    Ref::new("TsqlVariableSegment"), // @variable_name
                    Ref::new("DatatypeSegment"),     // INT, VARCHAR(50), etc.
                    Sequence::new(vec_of_erased![
                        // Optional initialization
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
    // These are like { } in C-style languages
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            Ref::new("BeginEndBlockGrammar").to_matchable().into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                MetaSegment::indent(), // Increase indentation level
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    // All allowed statement types within BEGIN...END
                    one_of(vec_of_erased![
                        Ref::new("SelectableGrammar"), // SELECT, CTEs
                        Ref::new("InsertStatementSegment"),
                        Ref::new("UpdateStatementSegment"),
                        Ref::new("DeleteStatementSegment"),
                        Ref::new("DeclareStatementSegment"),
                        Ref::new("SetVariableStatementSegment"),
                        Ref::new("PrintStatementSegment"),
                        Ref::new("IfStatementSegment"),
                        Ref::new("WhileStatementSegment"),
                        Ref::new("BeginEndBlockSegment") // Allow nested BEGIN...END
                    ]),
                    Ref::new("DelimiterGrammar").optional() // Semicolons are optional in T-SQL
                ])])
                .config(|this| this.min_times(0)), // Allow empty blocks
                MetaSegment::dedent(), // Decrease indentation level
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

    // PIVOT and UNPIVOT support for transforming rows to columns and vice versa
    dialect.add([
        (
            "PivotUnpivotSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableExpression,
                Ref::new("PivotUnpivotGrammar").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            "PivotUnpivotGrammar".into(),
            one_of(vec_of_erased![
                // PIVOT rotates rows into columns
                // Example: PIVOT (SUM(Amount) FOR Month IN ([Jan], [Feb], [Mar]))
                Sequence::new(vec_of_erased![
                    Ref::keyword("PIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionSegment"), // Aggregate function (SUM, AVG, etc.)
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"), // Column to pivot on
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "LiteralGrammar" // List of values to become columns
                        )])])
                    ])
                ]),
                // UNPIVOT rotates columns into rows (reverse of PIVOT)
                // Example: UNPIVOT (Value FOR Month IN ([Jan], [Feb], [Mar]))
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNPIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"), // Value column name
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"), // New column for unpivoted names
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment" // Columns to unpivot
                        )])])
                    ])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // GO batch separator (special handling as it's not really a statement)
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
    // We extend the ANSI statement_segment() rather than replacing it completely
    // IMPORTANT: References must use Grammar suffix for grammar definitions
    dialect.replace_grammar(
        "StatementSegment",
        NodeMatcher::new(
            SyntaxKind::Statement,
            one_of(vec_of_erased![
                // T-SQL specific statements (using Grammar suffix)
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
        )
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
    // Note: We add this as a new segment type instead of replacing SingleIdentifierGrammar
    dialect.add([
        (
            // Basic variable segment that matches @variable or @@variable tokens
            "TsqlVariableSegment".into(),
            TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable)
                .to_matchable()
                .into(),
        ),
        (
            // Wrap variables as parameterized expressions for use in queries
            "ParameterizedSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ParameterizedExpression,
                Ref::new("TsqlVariableSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
            // Special handling for table variables like @TempTable
            "TsqlTableVariableSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableReference,
                Ref::new("TsqlVariableSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
    ]);

    // Update TableReferenceSegment to support T-SQL table variables directly
    // (Must be done after TsqlVariableSegment is defined)
    dialect.replace_grammar(
        "TableReferenceSegment",
        one_of(vec_of_erased![
            Ref::new("ObjectReferenceSegment"), // Original object references
            Ref::new("TsqlVariableSegment"),    // T-SQL table variables like @MyTable
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


    // Table hints support - properly structured as table hint segments
    // Example: SELECT * FROM Users WITH (NOLOCK)
    dialect.add([
        (
            "TableHintSegment".into(),
            one_of(vec_of_erased![
                // Try aggressive regex matching for the entire table hint pattern
                RegexParser::new(
                    r"WITH\s*\(\s*(NOLOCK|READUNCOMMITTED|READCOMMITTED|REPEATABLEREAD|SERIALIZABLE|READPAST|ROWLOCK|TABLOCK|TABLOCKX|UPDLOCK|XLOCK|NOEXPAND|FORCESEEK|FORCESCAN|HOLDLOCK|SNAPSHOT)\s*\)",
                    SyntaxKind::TableExpression  // Use existing syntax kind
                ),
                // Fallback to original sequence-based approach
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "TableHintElement"
                    )])])
                    .config(|this| this.parse_mode = ParseMode::Greedy)
                ])
            ])
            .to_matchable()
            .into(),
        ),
        (
            "TableHintElement".into(),
            one_of(vec_of_erased![
                // Simple hints (just keywords) - see comment at top for meanings
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
                // Example: WITH (INDEX(IX_Users_Email))
                Sequence::new(vec_of_erased![
                    Ref::keyword("INDEX"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),  // INDEX(0) for clustered
                        Ref::new("NakedIdentifierSegment")  // INDEX(IX_IndexName)
                    ])])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);


    // INVESTIGATION LOG: Table hints parsing issue
    // Problem: `FROM Users WITH (NOLOCK)` parses WITH as alias instead of table hint
    //
    // Previous attempts:
    // 1. Override FromExpressionElementSegment with explicit patterns - FAILED
    // 2. Use exclude(Ref::keyword("WITH")) on alias patterns - FAILED
    // 3. Reorder patterns to prioritize table hints - FAILED
    // 4. Use base ANSI behavior - FAILED
    //
    // Current hypothesis: Issue is with AliasExpressionSegment specifically
    // New approach: Override AliasExpressionSegment to exclude table hint keywords
    // RESULT: Still failed - WITH still parsed as alias
    //
    // New hypothesis: WITH is being tokenized as identifier, not keyword
    // Testing approach: Create simple keyword test
    // RESULT: Lookahead exclude also failed - confirms tokenization issue
    //
    // Investigation: Check lexer configuration and keyword handling
    // FINDINGS: WITH is correctly in reserved keywords (line 196 in tsql_keywords.rs)
    //
    // New approach: Create more aggressive table hint parsing
    // Try using a regex or compound matcher for "WITH(...)" pattern
    // RESULT: Regex approach also failed - still parsing WITH as alias
    //
    // CRITICAL INSIGHT: The issue must be in parser ordering!
    // AliasExpressionSegment is being tried BEFORE PostTableExpressionGrammar
    // Need to investigate FromExpressionElementSegment sequence order
    //
    // INVESTIGATION RESULTS: Found the root cause!
    // ANSI FromExpressionElementSegment sequence:
    // 1. TableExpressionSegment
    // 2. AliasExpressionSegment (with excludes, but NOT PostTableExpressionGrammar)
    // 3. WITH OFFSET sequence
    // 4. SamplingExpressionSegment
    // 5. PostTableExpressionGrammar (LAST!)
    //
    // The problem: AliasExpressionSegment excludes some elements but NOT table hints
    // Solution: Add table hint exclusions to AliasExpressionSegment excludes
    //
    // FINAL SOLUTION: Override ANSI's FromExpressionElementSegment to add table hint exclusions
    // to the existing AliasExpressionSegment exclude list
    // RESULT: Still failed - even enhanced excludes don't work
    //
    // DEEPER INVESTIGATION NEEDED: The excludes mechanism itself may be broken
    // or there's something about how T-SQL keywords are handled that's different
    //
    // INTERESTING: `WITH (NOLOCK)` by itself parses fine (no alias errors)
    // This confirms the TableHintSegment works when not in FROM clause context
    // The issue is specifically with FromExpressionElementSegment parsing order

    // LAST RESORT: Try completely different approach
    // Replace PostTableExpressionGrammar with TableHintSegment directly in the sequence
    // This bypasses the problematic alias parsing entirely
    // RESULT: Still failed - Pattern 2 (alias + hints) still matches WITH as alias
    //
    // CONCLUSION: This is a fundamental issue with how the parser processes T-SQL
    // The exclude() mechanism appears to be non-functional for this use case
    // A complete rewrite of the parsing logic would be required to fix this
    //
    // DEEPER INVESTIGATION: Exploring alternative approaches
    // 1. Custom lexer matchers for table hints
    // 2. Anti-template mechanism analysis
    // 3. Token stream debugging
    // 4. Custom parser implementation
    //
    // APPROACH 1: Add custom lexer matcher to recognize "WITH(" as single token
    // This prevents alias parser from seeing just "WITH"
    // RESULT: Failed - still parsing WITH as alias
    //
    // APPROACH 2: Investigate anti-template mechanism for NakedIdentifierSegment
    // FINDINGS: exclude() requires both .exclude() call AND config.exclude setting
    // Trying corrected exclude syntax with config closure
    // RESULT: Still failed - even correct exclude syntax doesn't work
    //
    // APPROACH 3: Check if T-SQL reserved keywords are flowing to NakedIdentifierSegment
    // Testing by overriding NakedIdentifierSegment directly
    // RESULT: Still failed - even forcing WITH exclusion in anti-template doesn't work
    //
    // INTERESTING: "WITH" alone parses fine (no identifier errors)
    // This suggests WITH is correctly tokenized as keyword, not identifier
    // The issue is really with the parsing precedence in FromExpressionElementSegment
    //
    // APPROACH 4: Create completely custom table hint-aware parser
    // Since WITH is correctly tokenized as keyword, create custom parser that looks ahead
    // RESULT: Still failed - issue persists even with custom parser
    //
    // FINAL ANALYSIS: This is a fundamental architectural limitation
    // The issue is that even when table hints are matched first, the AL05 linter rule
    // still reports "WITH" as an unused alias. This suggests the problem may be:
    // 1. The linter is analyzing the wrong AST structure
    // 2. The table hint parsing succeeds but creates the wrong node type
    // 3. There's a deeper issue with how T-SQL hints are represented in the AST

    // Create custom T-SQL table reference parser with explicit table hint handling
    dialect.add([(
        "TSqlTableReferenceWithHintsSegment".into(),
        one_of(vec_of_erased![
            // Direct table hint matching - tries this first
            Sequence::new(vec_of_erased![
                Ref::new("TableExpressionSegment"),
                Ref::new("TableHintSegment")
            ]),
            // Fallback to regular table reference
            Ref::new("TableExpressionSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // Define PostTableExpressionGrammar to include T-SQL table hints
    dialect.add([
        (
            "PostTableExpressionGrammar".into(),
            Ref::new("TableHintSegment")
                .optional()
                .to_matchable()
                .into(),
        ),
    ]);

    // SOLUTION: Override FromExpressionElementSegment to ensure LookaheadExclude is properly applied
    // This is the correct fix for WITH(NOLOCK) parsing issues
    // The key insight is that T-SQL needs to preserve the ANSI exclude pattern while using its own PostTableExpressionGrammar
    dialect.replace_grammar(
        "FromExpressionElementSegment",
        NodeMatcher::new(
            SyntaxKind::FromExpressionElement,
            Sequence::new(vec_of_erased![
                Ref::new("PreTableFunctionKeywordsGrammar").optional(),
                optionally_bracketed(vec_of_erased![Ref::new("TableExpressionSegment")]),
                Ref::new("AliasExpressionSegment")
                    .exclude(one_of(vec_of_erased![
                        Ref::new("FromClauseTerminatorGrammar"),
                        Ref::new("SamplingExpressionSegment"),
                        Ref::new("JoinLikeClauseGrammar"),
                        LookaheadExclude::new("WITH", "(")  // Prevents WITH from being parsed as alias when followed by (
                    ]))
                    .optional(),
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("OFFSET"),
                    Ref::new("AliasExpressionSegment")
                ])
                .config(|this| this.optional()),
                Ref::new("SamplingExpressionSegment").optional(),
                Ref::new("PostTableExpressionGrammar").optional()  // T-SQL table hints
            ])
            .to_matchable(),
        )
        .to_matchable(),
    );
    
    

    // Update JoinClauseSegment to handle APPLY syntax properly
    dialect.replace_grammar(
        "JoinClauseSegment",
        NodeMatcher::new(
            SyntaxKind::JoinClause,
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
        )
        .to_matchable(),
    );

    // T-SQL specific data type handling for MAX keyword and -1
    // Override BracketedArguments to accept MAX keyword and negative numbers
    dialect.replace_grammar(
        "BracketedArguments",
        NodeMatcher::new(
            SyntaxKind::BracketedArguments,
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
        )
        .to_matchable(),
    );

    // APPLY clause support (CROSS APPLY and OUTER APPLY)
    // APPLY is T-SQL's way to invoke a table-valued function for each row
    // of the outer table expression. It's like a JOIN but can reference
    // columns from the outer query.
    //
    // CROSS APPLY: Similar to INNER JOIN - returns only rows where the
    //              function returns results
    // OUTER APPLY: Similar to LEFT JOIN - returns all rows from left side,
    //              with NULLs when function returns no results
    //
    // Example: SELECT * FROM Customers c
    //          CROSS APPLY dbo.GetOrdersForCustomer(c.CustomerID) o
    dialect.add([(
        "ApplyClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::JoinClause, // APPLY is classified as a join type
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
    // This is used primarily with STRING_AGG in T-SQL to specify the order
    // of concatenated values within each group
    // Example: STRING_AGG(name, ',') WITHIN GROUP (ORDER BY hire_date)
    dialect.add([(
        "WithinGroupClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::WithingroupClause,
            Sequence::new(vec_of_erased![
                Ref::keyword("WITHIN"),
                Ref::keyword("GROUP"),
                Bracketed::new(vec_of_erased![
                    // ORDER BY is optional - if omitted, order is undefined
                    Ref::new("OrderByClauseSegment").optional()
                ])
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // Override PostFunctionGrammar to include WITHIN GROUP
    // This allows aggregate functions to be followed by these clauses:
    // - WITHIN GROUP: For ordered aggregates (T-SQL specific)
    // - OVER: For window functions (ANSI standard)
    // - FILTER: For filtered aggregates (ANSI standard)
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

    // Add T-SQL variable support to LiteralGrammar for use in expressions
    // This MUST be done before expand() is called in dialect() function
    // so that the expression grammars include variable support
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
    // This must be done before expand() to ensure proper parsing
    //
    // The challenge here is that the parser needs to distinguish between:
    // 1. Column references like: table1.column1
    // 2. T-SQL alias assignment like: AliasName = table1.column1
    //
    // The key insight is that we need to handle the ambiguity at the
    // select clause level, not at the expression level.

    // Override the select_clause_element function used by ANSI
    dialect.replace_grammar(
        "SelectClauseElementSegment",
        one_of(vec_of_erased![
            // T-SQL alias equals pattern MUST come first
            // This will match: AliasName = <any expression>
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
    // Override StringBinaryOperatorGrammar to include the + operator
    // This fixes string concatenation in parentheses like: (first_name + ' ' + last_name)
    dialect.add([(
        "StringBinaryOperatorGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("ConcatSegment"), // Standard || operator
            Ref::new("PlusSegment"),   // T-SQL + operator for string concatenation
        ])
        .to_matchable()
        .into(),
    )]);

    // Define PostTableExpressionGrammar to include T-SQL table hints
    // This leverages the ANSI LookaheadExclude mechanism for WITH(NOLOCK) parsing
    dialect.add([
        (
            "PostTableExpressionGrammar".into(),
            Ref::new("TableHintSegment")
                .optional()
                .to_matchable()
                .into(),
        ),
    ]);

    // CRITICAL: expand() must be called after all grammar modifications
    // This method recursively expands all grammar references and builds
    // the final parser. Without this, grammar references won't be resolved
    // and the parser will fail at runtime.

    dialect
}
