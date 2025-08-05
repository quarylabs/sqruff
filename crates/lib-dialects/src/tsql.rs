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
    raw_dialect()
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
        "OPTION",
        "PATH",
        "RAW",
        "EXPLICIT",
        "ROOT",
        "INCLUDE_NULL_VALUES",
        "WITHOUT_ARRAY_WRAPPER",
        "TYPE",
        "ELEMENTS",
        "XSINIL",
        "STRICT",
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
        "VIEW_METADATA",
        "ROWS", // Allow 'AS rows' alias
        "R",    // Allow 'AS r' alias
        "SINGLE_BLOB",
        "SINGLE_CLOB",
        "SINGLE_NCLOB",
        "FORMATFILE",
        "FIRSTROW",
        "LASTROW",
        "MAXERRORS",
        "CODEPAGE",
        "XML_COMPRESSION",
        "WAIT_AT_LOW_PRIORITY",
        "ABORT_AFTER_WAIT",
        "COMPRESS_ALL_ROW_GROUPS",
        "LOB_COMPACTION",
        "COMPRESSION_DELAY",
        "OPTIMIZE_FOR_SEQUENTIAL_KEY",
        "PARTITIONS",
        "COLUMNSTORE_ARCHIVE",
        "SYSTEM_VERSIONING",
        "HISTORY_TABLE",
        "DATA_CONSISTENCY_CHECK",
        "HISTORY_RETENTION_PERIOD",
        "FILESTREAM_ON",
        "DATA_DELETION",
        "FILTER_COLUMN",
        "RETENTION_PERIOD",
        "INFINITE",
        "PERIOD",
        "SYSTEM_TIME",
        "PERSISTED",
        "GENERATED",
        "UNDEFINED",
        "ALWAYS",
        "Partition",
        "LOB_COMPACTION",
        "COMPRESSION_DELAY",
        "OPTIMIZE_FOR_SEQUENTIAL_KEY",
        "PARTITIONS",
        "COLUMNSTORE_ARCHIVE",
        "SYSTEM_VERSIONING",
        "HISTORY_TABLE",
        "DATA_CONSISTENCY_CHECK",
        "HISTORY_RETENTION_PERIOD",
        "FILESTREAM_ON",
        "DATA_DELETION",
        "FILTER_COLUMN",
        "RETENTION_PERIOD",
        "INFINITE",
        "PERIOD",
        "SYSTEM_TIME",
        "PERSISTED",
        "Partition",
        "LIST",
        "POPULATION",
        "FILESTREAM",
        "MASKED",
        "FUNCTION",
        "REPLICATION",
        "ENCRYPTED",
        "COLUMN_ENCRYPTION_KEY",
        "ENCRYPTION_TYPE",
        "RANDOMIZED",
        "ALGORITHM",
        "HIDDEN",
        "START",
        "END",
        "ROW",
        "DEFAULT_DATABASE",
        "DEFAULT_LANGUAGE",
        "USER_DB",
        "DW_BIN_TEMP",
        "LOCATION",
        "DISTRIBUTION",
        "ROUND_ROBIN",
        "REPLICATE",
        "HASH",
    ]);

    // T-SQL specific operators
    dialect.sets_mut("operator_symbols").extend([
        "%=", "&=", "*=", "+=", "-=", "/=", "^=", "|=", // Compound assignment
        "!<", "!>", // Special comparison operators (non-spaced versions)
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
                r"@@?[\p{L}_][\p{L}\p{N}_]*",
                SyntaxKind::TsqlVariable,
            ),
            // Special T-SQL $action variable for MERGE OUTPUT clause
            Matcher::regex(
                "tsql_action_variable",
                r"\$action",
                SyntaxKind::TsqlVariable,
            ),
            // Unicode string literals: N'text'
            Matcher::regex(
                "unicode_single_quote",
                r"N'([^']|'')*'",
                SyntaxKind::UnicodeSingleQuote,
            ),
            // Hexadecimal literals: 0x123ABC
            Matcher::regex("hex_literal", r"0x[0-9a-fA-F]+", SyntaxKind::NumericLiteral),
            // Azure Blob Storage URLs for COPY INTO
            Matcher::regex(
                "azure_blob_storage_url",
                r"'https://[^']*\.blob\.core\.windows\.net/[^']*'",
                SyntaxKind::QuotedLiteral,
            ),
            // Azure Data Lake Storage Gen2 URLs for COPY INTO
            Matcher::regex(
                "azure_data_lake_storage_url",
                r"'https://[^']*\.dfs\.core\.windows\.net/[^']*'",
                SyntaxKind::QuotedLiteral,
            ),
            // Compound assignment operators - must come before individual operators
            Matcher::string(
                "addition_assignment",
                "+=",
                SyntaxKind::AdditionAssignmentSegment,
            ),
            Matcher::string(
                "subtraction_assignment",
                "-=",
                SyntaxKind::SubtractionAssignmentSegment,
            ),
            Matcher::string(
                "multiplication_assignment",
                "*=",
                SyntaxKind::MultiplicationAssignmentSegment,
            ),
            Matcher::string(
                "division_assignment",
                "/=",
                SyntaxKind::DivisionAssignmentSegment,
            ),
            Matcher::string(
                "modulus_assignment",
                "%=",
                SyntaxKind::ModulusAssignmentSegment,
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
            r"##?[\p{L}\p{N}_]+|[\p{N}\p{L}_]+#?",
            SyntaxKind::Word,
        ),
    ]);

    // NOTE: Keyword matching is handled differently in Sqruff's architecture.
    // Keywords are NOT matched during lexing - they are identified during parsing.
    // The lexer produces word tokens, and the parser uses StringParser to match
    // keywords based on their text content.
    //
    // NOTE: T-SQL has known limitations where keywords are lexed as word tokens
    // in certain contexts (e.g., inside procedure bodies after AS, after THROW
    // statements). This affects ~1.26% of T-SQL code. See docs/tsql_limitations.md
    // for details and workarounds.
    //
    // Adding keyword matchers to the lexer would require significant architectural
    // changes and static keyword definitions, which conflicts with the dynamic
    // keyword sets loaded from tsql_keywords module.

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

    // Add T-SQL string and date functions
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
        "CONVERT", // T-SQL conversion function
        "DATEADD", // T-SQL date functions
        "DATEDIFF",
        "DATENAME",
        "DATEPART",
        "GETDATE",
        "GETUTCDATE",
        "SYSDATETIME",
        "SYSUTCDATETIME",
        "SYSDATETIMEOFFSET",
        // T-SQL window functions
        "ROW_NUMBER",
        "RANK",
        "DENSE_RANK",
        "NTILE",
        "LAG",
        "LEAD",
        "FIRST_VALUE",
        "LAST_VALUE",
    ]);

    // T-SQL specific value table functions
    dialect.sets_mut("value_table_functions").extend([
        "OPENROWSET",
        "OPENQUERY",
        "OPENDATASOURCE",
        "OPENXML",
    ]);

    // Override ObjectReferenceSegment to support T-SQL's multi-line object references
    // T-SQL allows object references to span multiple lines with dots on the next line
    // e.g., [database].[schema]
    //       .[table]
    dialect.replace_grammar(
        "ObjectReferenceSegment",
        one_of(vec_of_erased![
            // T-SQL syntax with leading dots (for .table, ..table, ...table)
            Sequence::new(vec_of_erased![
                // At least one leading dot
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("DotSegment"),
                        Ref::new("DotSegment")
                    ]), // ...table
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("DotSegment")
                    ]), // ..table
                    Ref::new("DotSegment") // .table
                ]),
                // Then the identifier parts
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]).config(
                    |this| {
                        this.delimiter(Ref::new("DotSegment"));
                        this.allow_gaps = true; // Allow gaps for multi-line
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    }
                )
            ]),
            // T-SQL double-dot syntax: server..table, database..table (default schema)
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"), // server/database name
                Ref::new("DotSegment"),
                Ref::new("DotSegment"),
                // Then the remaining identifier parts
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]).config(
                    |this| {
                        this.delimiter(Ref::new("DotSegment"));
                        this.allow_gaps = true; // Allow gaps for multi-line
                        this.terminators =
                            vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
                    }
                )
            ]),
            // Standard object reference (no leading dots)
            Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]).config(|this| {
                this.delimiter(Ref::new("DotSegment"));
                this.allow_gaps = true; // Allow gaps for multi-line
                this.terminators = vec_of_erased![Ref::new("ObjectReferenceTerminatorGrammar")];
            })
        ])
        .to_matchable(),
    );

    // NOTE: T-SQL CASE expressions are now supported in SELECT clauses via TsqlCaseExpressionSegment
    // The previous parsing issue was resolved by creating T-SQL specific CASE expressions that use
    // StringParser instead of Ref::keyword to handle T-SQL's lexing behavior where CASE, WHEN,
    // THEN, ELSE are lexed as Word tokens in SELECT contexts but Keyword tokens in WHERE contexts.

    // T-SQL specific CASE expression - handle T-SQL's unique lexing behavior where CASE/WHEN/THEN/ELSE/END
    // are lexed as 'word' in SELECT contexts but 'keyword' in WHERE contexts
    dialect.add([(
        "CaseExpressionSegment".into(),
        NodeMatcher::new(SyntaxKind::CaseExpression, |_| {
            one_of(vec_of_erased![
                // Searched CASE: CASE WHEN condition THEN result [WHEN condition THEN result]... [ELSE result] END
                Sequence::new(vec_of_erased![
                    Ref::keyword("CASE"),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("WHEN"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("THEN"),
                        Ref::new("ExpressionSegment")
                    ])]),
                    Ref::new("ElseClauseSegment").optional(),
                    Ref::keyword("END")
                ]),
                // Simple CASE: CASE expression WHEN value THEN result [WHEN value THEN result]... [ELSE result] END
                Sequence::new(vec_of_erased![
                    Ref::keyword("CASE"),
                    Ref::new("ExpressionSegment"),
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::keyword("WHEN"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("THEN"),
                        Ref::new("ExpressionSegment")
                    ])]),
                    Ref::new("ElseClauseSegment").optional(),
                    Ref::keyword("END")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override ColumnReferenceSegment to exclude CASE keywords that should be parsed as CaseExpressionSegment
    dialect.add([(
        "ColumnReferenceSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnReference, |_| {
            Delimited::new(vec![
                // Exclude CASE keywords from being parsed as column references
                Ref::new("SingleIdentifierGrammar")
                    .exclude(Ref::keyword("CASE"))
                    .exclude(Ref::keyword("WHEN"))
                    .exclude(Ref::keyword("THEN"))
                    .exclude(Ref::keyword("ELSE"))
                    .exclude(Ref::keyword("END"))
                    .to_matchable(),
            ])
            .config(|this| this.delimiter(Ref::new("ObjectReferenceDelimiterGrammar")))
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override Expression_C_Grammar to handle EXISTS functions and prioritize CaseExpressionSegment
    dialect.add([(
        "Expression_C_Grammar".into(),
        one_of(vec_of_erased![
            // PRIORITY: Try word-aware EXISTS function first for T-SQL word tokens
            Ref::new("WordAwareExistsFunctionSegment"),
            // Sequence for "EXISTS" with a bracketed selectable grammar (from ANSI)
            Sequence::new(vec_of_erased![
                Ref::keyword("EXISTS"),
                Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")])
            ]),
            // Put CaseExpressionSegment SECOND
            Ref::new("CaseExpressionSegment"),
            Ref::new("Expression_D_Grammar"),
        ])
        .to_matchable()
        .into(),
    )]);

    // INVESTIGATION RESULT: Grammar replacement approach failed
    // The issue is that ANSI Expression_C_Grammar, Expression_D_Grammar, and BaseExpressionElementGrammar
    // are all inherited by T-SQL but not redefined in T-SQL's own library.
    // replace_grammar() can only work on grammars that exist in the current dialect's library.

    // NEXT APPROACH: Test if the CaseExpressionSegment override actually works in isolation
    // by creating a simple test case without the complex grammar hierarchy.

    // dialect.add([
    //     (
    //         "CaseExpressionSegment".into(),
    //         NodeMatcher::new(SyntaxKind::CaseExpression, |_| {
    //             one_of(vec_of_erased![
    //                 // Simple CASE: CASE expression WHEN value THEN result ... END
    //                 Sequence::new(vec_of_erased![
    //                     StringParser::new("CASE", SyntaxKind::Keyword),
    //                     Ref::new("ExpressionSegment"),
    //                     MetaSegment::implicit_indent(),
    //                     AnyNumberOf::new(vec_of_erased![Ref::new("WhenClauseSegment")],).config(
    //                         |this| {
    //                             this.reset_terminators = true;
    //                             this.terminators = vec_of_erased![
    //                                 StringParser::new("ELSE", SyntaxKind::Keyword),
    //                                 StringParser::new("END", SyntaxKind::Keyword)
    //                             ];
    //                         }
    //                     ),
    //                     Ref::new("ElseClauseSegment").optional(),
    //                     MetaSegment::dedent(),
    //                     StringParser::new("END", SyntaxKind::Keyword),
    //                 ]),
    //                 // Searched CASE: CASE WHEN condition THEN result ... END
    //                 Sequence::new(vec_of_erased![
    //                     StringParser::new("CASE", SyntaxKind::Keyword),
    //                     MetaSegment::implicit_indent(),
    //                     AnyNumberOf::new(vec_of_erased![Ref::new("WhenClauseSegment")],).config(
    //                         |this| {
    //                             this.reset_terminators = true;
    //                             this.terminators = vec_of_erased![
    //                                 StringParser::new("ELSE", SyntaxKind::Keyword),
    //                                 StringParser::new("END", SyntaxKind::Keyword)
    //                             ];
    //                         }
    //                     ),
    //                     Ref::new("ElseClauseSegment").optional(),
    //                     MetaSegment::dedent(),
    //                     StringParser::new("END", SyntaxKind::Keyword),
    //                 ]),
    //             ])
    //             .config(|this| {
    //                 this.terminators = vec_of_erased![
    //                     Ref::new("ComparisonOperatorGrammar"),
    //                     Ref::new("CommaSegment"),
    //                     Ref::new("BinaryOperatorGrammar")
    //                 ]
    //             })
    //             .to_matchable()
    //         })
    //         .to_matchable()
    //         .into(),
    //     ),
    //     (
    //         "TsqlWhenClauseSegment".into(),
    //         NodeMatcher::new(SyntaxKind::WhenClause, |_| {
    //             Sequence::new(vec_of_erased![
    //                 StringParser::new("WHEN", SyntaxKind::Keyword),
    //                 Sequence::new(vec_of_erased![
    //                     MetaSegment::implicit_indent(),
    //                     Ref::new("ExpressionSegment"),
    //                     MetaSegment::dedent(),
    //                 ]),
    //                 Conditional::new(MetaSegment::indent()).indented_then(),
    //                 StringParser::new("THEN", SyntaxKind::Keyword),
    //                 Conditional::new(MetaSegment::implicit_indent()).indented_then_contents(),
    //                 Ref::new("ExpressionSegment"),
    //                 Conditional::new(MetaSegment::dedent()).indented_then_contents(),
    //                 Conditional::new(MetaSegment::dedent()).indented_then(),
    //             ])
    //             .to_matchable()
    //         })
    //         .to_matchable()
    //         .into(),
    //     ),
    //     (
    //         "TsqlElseClauseSegment".into(),
    //         NodeMatcher::new(SyntaxKind::ElseClause, |_| {
    //             Sequence::new(vec![
    //                 StringParser::new("ELSE", SyntaxKind::Keyword).to_matchable(),
    //                 MetaSegment::implicit_indent().to_matchable(),
    //                 Ref::new("ExpressionSegment").to_matchable(),
    //                 MetaSegment::dedent().to_matchable(),
    //             ])
    //             .to_matchable()
    //         })
    //         .to_matchable()
    //         .into(),
    //     ),
    // ]);

    // Add OPENROWSET segment for T-SQL specific syntax
    dialect.add([(
        "OpenRowSetSegment".into(),
        NodeMatcher::new(SyntaxKind::Function, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("OPENROWSET"),
                Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                    // BULK syntax: OPENROWSET(BULK 'file_path', ...) - Check this first
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BULK"),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("UnicodeLiteralSegment")
                        ]),
                        // Optional parameters after file path
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("CommaSegment"),
                            one_of(vec_of_erased![
                                // Simple keywords like SINGLE_BLOB, SINGLE_CLOB, etc.
                                Ref::keyword("SINGLE_BLOB"),
                                Ref::keyword("SINGLE_CLOB"),
                                Ref::keyword("SINGLE_NCLOB"),
                                // Named parameters: PARAM = value
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("FORMATFILE"),
                                        Ref::keyword("FIRSTROW"),
                                        Ref::keyword("LASTROW"),
                                        Ref::keyword("MAXERRORS"),
                                        Ref::keyword("FORMAT"),
                                        Ref::keyword("CODEPAGE"),
                                        Ref::keyword("ERRORFILE"),
                                        Ref::keyword("FIELDTERMINATOR"),
                                        Ref::keyword("ROWTERMINATOR"),
                                        Ref::keyword("FIELDQUOTE"),
                                        Ref::keyword("DATA_SOURCE")
                                    ]),
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![
                                        Ref::new("QuotedLiteralSegment"),
                                        Ref::new("UnicodeLiteralSegment"),
                                        Ref::new("NumericLiteralSegment")
                                    ])
                                ])
                            ])
                        ])])
                    ]),
                    // Provider syntax: OPENROWSET('provider', ...)
                    // Can use either commas or semicolons as separators
                    Sequence::new(vec_of_erased![
                        Ref::new("QuotedLiteralSegment"), // Provider name
                        one_of(vec_of_erased![
                            // Standard comma-separated syntax
                            Sequence::new(vec_of_erased![
                                Ref::new("CommaSegment"),
                                Ref::new("QuotedLiteralSegment"), // Connection string
                                Ref::new("CommaSegment"),
                                one_of(vec_of_erased![
                                    Ref::new("ObjectReferenceSegment"), // Table/view name
                                    Ref::new("QuotedLiteralSegment")    // Query string
                                ])
                            ]),
                            // Semicolon-separated syntax (e.g., for Jet OLEDB provider)
                            Sequence::new(vec_of_erased![
                                Ref::new("CommaSegment"),
                                Ref::new("QuotedLiteralSegment"), // Data source
                                Ref::new("SemicolonSegment"),
                                Ref::new("QuotedLiteralSegment"), // User ID
                                Ref::new("SemicolonSegment"),
                                Ref::new("QuotedLiteralSegment"), // Password
                                Ref::new("CommaSegment"),
                                Ref::new("ObjectReferenceSegment") // Table name
                            ])
                        ])
                    ])
                ])])
                .config(|this| this.parse_mode(ParseMode::Greedy)) // WITH clause removed - now handled by FromExpressionElementSegment
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL specific grammar

    // TOP clause support (e.g., SELECT TOP 10, TOP (10) PERCENT, TOP 5 WITH TIES)

    // Define TopClauseSegment for reuse in SELECT, DELETE, UPDATE, INSERT
    dialect.add([(
        "TopClauseSegment".into(),
        Sequence::new(vec_of_erased![
            // https://docs.microsoft.com/en-us/sql/t-sql/queries/top-transact-sql
            Ref::keyword("TOP"),
            optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
            Ref::keyword("PERCENT").optional(),
            // WITH TIES is only valid in SELECT, not in DELETE/UPDATE/INSERT
            Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("TIES")])
                .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL allows DISTINCT/ALL followed by TOP
    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        AnyNumberOf::new(vec_of_erased![
            Ref::keyword("DISTINCT"),
            Ref::keyword("ALL"),
            Ref::new("TopClauseSegment")
        ])
        .to_matchable(),
    );

    // Override SelectClauseTerminatorGrammar to include FOR as a terminator
    // This prevents FOR JSON/XML/BROWSE from being parsed as part of SELECT clause
    // IMPORTANT: END is excluded from terminators to allow CASE expressions to parse correctly.
    // The custom SelectClauseSegment above handles procedural constructs differently.
    dialect.add([(
        "SelectClauseTerminatorGrammar".into(),
        one_of(vec_of_erased![
            Ref::keyword("FROM"),
            Ref::keyword("WHERE"),
            Ref::keyword("INTO"), // T-SQL supports SELECT INTO
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("LIMIT"),
            Ref::keyword("OVERLAPS"),
            Ref::new("SetOperatorSegment"),
            Ref::keyword("FETCH"),
            // T-SQL specific: FOR JSON/XML/BROWSE
            Ref::keyword("FOR"),
            // T-SQL specific: GO batch delimiter
            Ref::new("BatchDelimiterGrammar"),
            // T-SQL specific: OPTION clause
            Ref::keyword("OPTION"),
            // T-SQL specific: Statement keywords that should terminate SELECT clause
            Ref::keyword("CREATE"),
            Ref::keyword("DROP"),
            Ref::keyword("ALTER"),
            Ref::keyword("INSERT"),
            Ref::keyword("UPDATE"),
            Ref::keyword("DELETE"),
            // NOTE: MERGE removed from terminators to allow MERGE statements to parse
            Ref::keyword("DECLARE"),
            Ref::keyword("SET"),
            Ref::keyword("BEGIN"),
            // END is excluded to allow CASE expressions to parse
            Ref::keyword("IF"),
            Ref::keyword("WHILE"),
            Ref::keyword("EXEC"),
            Ref::keyword("EXECUTE"),
        ])
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL assignment operator segments
    dialect.add([
        (
            "AssignmentOperatorSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentOperator, |_| {
                one_of(vec_of_erased![
                    Ref::new("RawEqualsSegment"),
                    Ref::new("AdditionAssignmentSegment"),
                    Ref::new("SubtractionAssignmentSegment"),
                    Ref::new("MultiplicationAssignmentSegment"),
                    Ref::new("DivisionAssignmentSegment"),
                    Ref::new("ModulusAssignmentSegment"),
                    Ref::new("BitwiseXorAssignmentSegment"),
                    Ref::new("BitwiseAndAssignmentSegment"),
                    Ref::new("BitwiseOrAssignmentSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // Addition assignment (+=) - uses lexer token
        (
            "AdditionAssignmentSegment".into(),
            TypedParser::new(
                SyntaxKind::AdditionAssignmentSegment,
                SyntaxKind::AdditionAssignmentSegment,
            )
            .to_matchable()
            .into(),
        ),
        // Subtraction assignment (-=) - uses lexer token
        (
            "SubtractionAssignmentSegment".into(),
            TypedParser::new(
                SyntaxKind::SubtractionAssignmentSegment,
                SyntaxKind::SubtractionAssignmentSegment,
            )
            .to_matchable()
            .into(),
        ),
        // Multiplication assignment (*=) - uses lexer token
        (
            "MultiplicationAssignmentSegment".into(),
            TypedParser::new(
                SyntaxKind::MultiplicationAssignmentSegment,
                SyntaxKind::MultiplicationAssignmentSegment,
            )
            .to_matchable()
            .into(),
        ),
        // Division assignment (/=) - uses lexer token
        (
            "DivisionAssignmentSegment".into(),
            TypedParser::new(
                SyntaxKind::DivisionAssignmentSegment,
                SyntaxKind::DivisionAssignmentSegment,
            )
            .to_matchable()
            .into(),
        ),
        // Modulus assignment (%=) - uses lexer token
        (
            "ModulusAssignmentSegment".into(),
            TypedParser::new(
                SyntaxKind::ModulusAssignmentSegment,
                SyntaxKind::ModulusAssignmentSegment,
            )
            .to_matchable()
            .into(),
        ),
        // Bitwise XOR assignment (^=)
        (
            "BitwiseXorAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("BitwiseXorSegment"),
                    Ref::new("RawEqualsSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // Bitwise AND assignment (&=)
        (
            "BitwiseAndAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("BitwiseAndSegment"),
                    Ref::new("RawEqualsSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // Bitwise OR assignment (|=)
        (
            "BitwiseOrAssignmentSegment".into(),
            NodeMatcher::new(SyntaxKind::AssignmentOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("BitwiseOrSegment"),
                    Ref::new("RawEqualsSegment")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override NakedIdentifierSegment to support T-SQL identifiers with # at the end
    // T-SQL allows temporary table names like #temp or ##global
    dialect.add([(
        "NakedIdentifierSegment".into(),
        SegmentGenerator::new(|dialect| {
            // Generate the anti template from truly reserved keywords (reserved - unreserved)
            // This allows unreserved keywords like ROWS to be used as identifiers
            let reserved_keywords = dialect.sets("reserved_keywords");
            let unreserved_keywords = dialect.sets("unreserved_keywords");
            let truly_reserved: std::collections::HashSet<_> = reserved_keywords
                .difference(&unreserved_keywords)
                .collect();
            let pattern = truly_reserved.iter().join("|");
            let anti_template = format!("^({pattern})$");

            // T-SQL pattern: supports both temp tables (#temp, ##global) and identifiers ending with #
            // Pattern explanation:
            // - ##?[A-Za-z0-9_\u{0080}-\u{FFFF}]+    matches temp tables: #temp, ##global, #3, etc (with Unicode support)
            // - [A-Za-z0-9_\u{0080}-\u{FFFF}]*[A-Za-z\u{0080}-\u{FFFF}][A-Za-z0-9_\u{0080}-\u{FFFF}]*#?   matches regular identifiers with optional # at end (with Unicode support)
            // Unicode range \u{0080}-\u{FFFF} covers most common Unicode characters
            RegexParser::new(
                r"(##?[A-Za-z0-9_\u{0080}-\u{FFFF}]+|[A-Za-z0-9_\u{0080}-\u{FFFF}]*[A-Za-z\u{0080}-\u{FFFF}][A-Za-z0-9_\u{0080}-\u{FFFF}]*#?)",
                SyntaxKind::NakedIdentifier,
            )
            .anti_template(&anti_template)
            .to_matchable()
        })
        .into(),
    )]);

    // Override ColumnDefinitionSegment to support T-SQL specific features
    dialect.add([(
        "ColumnDefinitionSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("SingleIdentifierGrammar"), // Column name
                one_of(vec_of_erased![
                    // Regular column: datatype [column modifiers] [constraints]
                    Sequence::new(vec_of_erased![
                        Ref::new("DatatypeSegment"), // Column type
                        // Flexible column modifiers in any order
                        AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                            // IDENTITY specification
                            Sequence::new(vec_of_erased![
                                Ref::keyword("IDENTITY"),
                                Bracketed::new(vec_of_erased![
                                    Ref::new("NumericLiteralSegment"), // seed
                                    Ref::new("CommaSegment"),
                                    Ref::new("NumericLiteralSegment") // increment
                                ])
                                .config(|this| this.optional()) // IDENTITY can be without parameters
                            ]),
                            // DEFAULT constraint
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                one_of(vec_of_erased![
                                    // Parenthesized expressions: DEFAULT (expression)
                                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                                    // Direct expression: DEFAULT expression
                                    Ref::new("ExpressionSegment")
                                ])
                            ]),
                            // FILESTREAM
                            Ref::keyword("FILESTREAM"),
                            // MASKED WITH (FUNCTION = '...')
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MASKED"),
                                Ref::keyword("WITH"),
                                Bracketed::new(vec_of_erased![
                                    Ref::keyword("FUNCTION"),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("QuotedLiteralSegment")
                                ])
                            ]),
                            // ENCRYPTED WITH (...)
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ENCRYPTED"),
                                Ref::keyword("WITH"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        // COLUMN_ENCRYPTION_KEY = key_name
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("COLUMN_ENCRYPTION_KEY"),
                                            Ref::new("EqualsSegment"),
                                            Ref::new("SingleIdentifierGrammar")
                                        ]),
                                        // ENCRYPTION_TYPE = RANDOMIZED
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("ENCRYPTION_TYPE"),
                                            Ref::new("EqualsSegment"),
                                            Ref::keyword("RANDOMIZED")
                                        ]),
                                        // ALGORITHM = 'algorithm_name'
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("ALGORITHM"),
                                            Ref::new("EqualsSegment"),
                                            Ref::new("QuotedLiteralSegment")
                                        ])
                                    ])
                                ])])
                            ]),
                            // GENERATED ALWAYS AS ROW START/END HIDDEN for temporal tables
                            Sequence::new(vec_of_erased![
                                Ref::keyword("GENERATED"),
                                Ref::keyword("ALWAYS"),
                                Ref::keyword("AS"),
                                Ref::keyword("ROW"),
                                one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("END")]),
                                Ref::keyword("HIDDEN").optional()
                            ]),
                            // COLLATE clause
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COLLATE"),
                                Ref::new("SingleIdentifierGrammar")
                            ])
                        ])])
                        .config(|this| this.optional()),
                        // Column constraints
                        AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")])
                            .config(|this| this.optional())
                    ]),
                    // Computed column: AS expression [PERSISTED] [constraints]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                        Ref::keyword("PERSISTED").optional(),
                        AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")])
                            .config(|this| this.optional())
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override TableConstraintSegment to support T-SQL specific features
    dialect.add([(
        "TableConstraintSegment".into(),
        NodeMatcher::new(SyntaxKind::TableConstraint, |_| {
            Sequence::new(vec_of_erased![
                // Optional constraint name
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    // PRIMARY KEY constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PRIMARY"),
                        Ref::keyword("KEY"),
                        one_of(vec_of_erased![
                            Ref::keyword("CLUSTERED"),
                            Ref::keyword("NONCLUSTERED")
                        ])
                        .config(|this| this.optional()),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|this| this.optional())
                            ])
                        ])])
                    ]),
                    // UNIQUE constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("UNIQUE"),
                        one_of(vec_of_erased![
                            Ref::keyword("CLUSTERED"),
                            Ref::keyword("NONCLUSTERED")
                        ])
                        .config(|this| this.optional()),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|this| this.optional())
                            ])
                        ])])
                    ]),
                    // FOREIGN KEY constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOREIGN"),
                        Ref::keyword("KEY"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])]),
                        Ref::keyword("REFERENCES"),
                        Ref::new("TableReferenceSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])]),
                        // Optional ON DELETE/UPDATE actions
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::keyword("ON"),
                            one_of(vec_of_erased![
                                Ref::keyword("DELETE"),
                                Ref::keyword("UPDATE")
                            ]),
                            one_of(vec_of_erased![
                                Ref::keyword("CASCADE"),
                                Ref::keyword("RESTRICT"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("SET"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("NULL"),
                                        Ref::keyword("DEFAULT")
                                    ])
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("NO"),
                                    Ref::keyword("ACTION")
                                ])
                            ])
                        ])])
                        .config(|this| this.optional()),
                        // Optional UNIQUE keyword (for foreign key constraints)
                        Ref::keyword("UNIQUE").optional()
                    ]),
                    // CHECK constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CHECK"),
                        Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
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
            NodeMatcher::new(SyntaxKind::DeclareStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DECLARE"),
                    MetaSegment::indent(),
                    // Multiple variables can be declared with comma separation
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("ParameterNameSegment"),
                        Sequence::new(vec![Ref::keyword("AS").to_matchable()])
                            .config(|this| this.optional()),
                        one_of(vec_of_erased![
                            // Table variable declaration - MUST come before DatatypeSegment
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
                                ])
                                .config(|this| this.parse_mode = ParseMode::Greedy),
                            ]),
                            // Regular variable declaration - excluding TABLE keyword
                            Sequence::new(vec_of_erased![
                                Ref::new("DatatypeSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::new("AssignmentOperatorSegment"),
                                    Ref::new("ExpressionSegment")
                                ])
                                .config(|this| this.optional())
                            ])
                        ])
                    ])])
                    .config(|this| this.allow_trailing()),
                    MetaSegment::dedent()
                ])
                .to_matchable()
            })
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
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    // Variable assignment: SET @var = value or SET @var1 = value1, @var2 = value2
                    Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("TsqlVariableSegment"),
                        Ref::new("AssignmentOperatorSegment"),
                        Ref::new("ExpressionSegment")
                    ])])
                    .config(|this| this.allow_trailing()),
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
                    ]),
                    // SET IDENTITY_INSERT table_name ON/OFF
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IDENTITY_INSERT"),
                        Ref::new("TableReferenceSegment"),
                        one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                    ])
                ]),
                MetaSegment::dedent()
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // PRINT statement
    // Handle T-SQL's lexing behavior where PRINT can be lexed as word in procedure contexts
    dialect.add([(
        "PrintStatementSegment".into(),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("PRINT"),
                // Also accept PRINT as word token in T-SQL procedure bodies
                StringParser::new("PRINT", SyntaxKind::Keyword)
            ]),
            Ref::new("ExpressionSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // BEGIN...END blocks for grouping multiple statements
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            NodeMatcher::new(SyntaxKind::BeginEndBlock, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("BEGIN"),
                        // Also accept BEGIN as naked identifier in T-SQL
                        StringParser::new("BEGIN", SyntaxKind::NakedIdentifier),
                        // Also accept BEGIN as word token in T-SQL procedure bodies
                        StringParser::new("BEGIN", SyntaxKind::Keyword)
                    ]),
                    Ref::new("DelimiterGrammar").optional(),
                    MetaSegment::indent(),
                    // Allow any number of statements with optional delimiters (like SQLFluff's OneOrMoreStatementsGrammar)
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("DelimiterGrammar").optional()
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            Ref::keyword("END"),
                            // Also terminate on END as naked identifier
                            StringParser::new("END", SyntaxKind::NakedIdentifier),
                            // Also terminate on END as word token
                            StringParser::new("END", SyntaxKind::Keyword)
                        ];
                    }),
                    MetaSegment::dedent(),
                    one_of(vec_of_erased![
                        Ref::keyword("END"),
                        // Also accept END as naked identifier
                        StringParser::new("END", SyntaxKind::NakedIdentifier),
                        // Also accept END as word token in T-SQL procedure bodies
                        StringParser::new("END", SyntaxKind::Keyword)
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Ref::new("BeginEndBlockSegment").to_matchable().into(),
        ),
    ]);

    // TRY...CATCH blocks - full implementation with correct structure
    dialect.add([(
        "TryBlockSegment".into(),
        NodeMatcher::new(SyntaxKind::TryCatchStatement, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("BEGIN"),
                    StringParser::new("BEGIN", SyntaxKind::Keyword),
                    StringParser::new("BEGIN", SyntaxKind::Word) // Support word tokens
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("TRY"),
                    StringParser::new("TRY", SyntaxKind::Keyword),
                    StringParser::new("TRY", SyntaxKind::Word) // Support word tokens
                ]),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("WordAwareStatementSegment") // Allow word-aware statements in TRY block
                    ]),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        Ref::keyword("END"),
                        StringParser::new("END", SyntaxKind::Keyword),
                        StringParser::new("END", SyntaxKind::Word)
                    ];
                }),
                MetaSegment::dedent(),
                one_of(vec_of_erased![
                    Ref::keyword("END"),
                    StringParser::new("END", SyntaxKind::Keyword),
                    StringParser::new("END", SyntaxKind::Word) // Support word tokens
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("TRY"),
                    StringParser::new("TRY", SyntaxKind::Keyword),
                    StringParser::new("TRY", SyntaxKind::Word) // Support word tokens
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("BEGIN"),
                    StringParser::new("BEGIN", SyntaxKind::Keyword),
                    StringParser::new("BEGIN", SyntaxKind::Word) // Support word tokens
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("CATCH"),
                    StringParser::new("CATCH", SyntaxKind::Keyword),
                    StringParser::new("CATCH", SyntaxKind::Word) // Support word tokens
                ]),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("WordAwareStatementSegment") // Allow word-aware statements in CATCH block
                    ]),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        Ref::keyword("END"),
                        StringParser::new("END", SyntaxKind::Keyword),
                        StringParser::new("END", SyntaxKind::Word)
                    ];
                }),
                MetaSegment::dedent(),
                one_of(vec_of_erased![
                    Ref::keyword("END"),
                    StringParser::new("END", SyntaxKind::Keyword),
                    StringParser::new("END", SyntaxKind::Word) // Support word tokens
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("CATCH"),
                    StringParser::new("CATCH", SyntaxKind::Keyword),
                    StringParser::new("CATCH", SyntaxKind::Word) // Support word tokens
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware TRY/CATCH parser for T-SQL contexts where keywords are lexed as words
    dialect.add([(
        "WordAwareTryCatchSegment".into(),
        NodeMatcher::new(SyntaxKind::TryCatchStatement, |_| {
            Sequence::new(vec_of_erased![
                StringParser::new("BEGIN", SyntaxKind::Word),
                StringParser::new("TRY", SyntaxKind::Word),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("WordAwareStatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![StringParser::new("END", SyntaxKind::Word)];
                }),
                MetaSegment::dedent(),
                StringParser::new("END", SyntaxKind::Word),
                StringParser::new("TRY", SyntaxKind::Word),
                StringParser::new("BEGIN", SyntaxKind::Word),
                StringParser::new("CATCH", SyntaxKind::Word),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("WordAwareStatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![StringParser::new("END", SyntaxKind::Word)];
                }),
                MetaSegment::dedent(),
                StringParser::new("END", SyntaxKind::Word),
                StringParser::new("CATCH", SyntaxKind::Word)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // GOTO statement and labels
    dialect.add([
        (
            "GotoStatementSegment".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("GOTO"),
                    StringParser::new("GOTO", SyntaxKind::Word)
                ]),
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
                    Ref::keyword("EXECUTE"),
                    // Also accept EXEC/EXECUTE as word tokens in T-SQL procedure bodies
                    StringParser::new("EXEC", SyntaxKind::Keyword),
                    StringParser::new("EXECUTE", SyntaxKind::Keyword)
                ])
                .config(|this| this.terminators = vec![]),
                // Optional return value capture
                Sequence::new(vec_of_erased![
                    Ref::new("TsqlVariableSegment"),
                    Ref::new("AssignmentOperatorSegment")
                ])
                .config(|this| this.optional()),
                // What to execute
                one_of(vec_of_erased![
                    // Dynamic SQL (expression or parameterized query in parentheses)
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ExpressionSegment"
                    )])]),
                    // Execute stored procedure variable
                    Ref::new("TsqlVariableSegment"),
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
                                    one_of(vec_of_erased![
                                        Ref::new("ExpressionSegment"),
                                        Ref::keyword("DEFAULT")
                                    ]),
                                    // Optional OUTPUT keyword
                                    Ref::keyword("OUTPUT").optional()
                                ]),
                                // Positional parameter
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::new("ExpressionSegment"),
                                        Ref::keyword("DEFAULT")
                                    ]),
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
                                        one_of(vec_of_erased![
                                            Ref::new("ExpressionSegment"),
                                            Ref::keyword("DEFAULT")
                                        ]),
                                        // Optional OUTPUT keyword
                                        Ref::keyword("OUTPUT").optional()
                                    ]),
                                    // Positional parameter
                                    Sequence::new(vec_of_erased![
                                        one_of(vec_of_erased![
                                            Ref::new("ExpressionSegment"),
                                            Ref::keyword("DEFAULT")
                                        ]),
                                        // Optional OUTPUT keyword
                                        Ref::keyword("OUTPUT").optional()
                                    ])
                                ])
                            ])
                        ])])
                        .config(|this| {
                            // Stop parsing parameters when we encounter optional clauses
                            this.terminators = vec_of_erased![
                                Ref::keyword("AS"),   // AS USER clause
                                Ref::keyword("WITH"), // WITH RECOMPILE or WITH RESULT SETS
                                Ref::keyword("AT")    // AT linked_server clause
                            ];
                        })
                    ])
                ]),
                // Optional AS USER clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::keyword("USER"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment")
                ])
                .config(|this| this.optional()),
                // Optional WITH RECOMPILE clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("RECOMPILE")
                ])
                .config(|this| this.optional()),
                // Optional WITH RESULT SETS clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Ref::keyword("RESULT"),
                    Ref::keyword("SETS"),
                    one_of(vec_of_erased![
                        Ref::keyword("NONE"),
                        Ref::keyword("UNDEFINED"),
                        // T-SQL WITH RESULT SETS patterns:
                        // Single result set: ((column definitions))
                        // Multiple result sets: ( (result_set_1), (result_set_2) )
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("ResultSetColumnDefinitionSegment")
                            ])])
                        ])])
                    ])
                ])
                .config(|this| this.optional()),
                // Optional AT linked_server clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("AT"),
                    Ref::new("SingleIdentifierGrammar")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // RECONFIGURE statement
    dialect.add([
        (
            "ReconfigureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ReconfigureStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("RECONFIGURE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("OVERRIDE")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // RENAME OBJECT statement (Azure Synapse Analytics specific)
        (
            "RenameObjectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RenameObjectStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("RENAME"),
                    one_of(vec_of_erased![
                        // RENAME OBJECT syntax
                        Sequence::new(vec_of_erased![
                            Ref::keyword("OBJECT"),
                            Ref::new("ObjectReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("ObjectReferenceSegment")
                        ]),
                        // RENAME DATABASE syntax
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DATABASE"),
                            Ref::new("ObjectReferenceSegment"),
                            Ref::keyword("TO"),
                            Ref::new("ObjectReferenceSegment")
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // SET CONTEXT_INFO statement
        (
            "SetContextInfoStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetContextInfoStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("SET"),
                    Ref::keyword("CONTEXT_INFO"),
                    one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("TsqlVariableSegment"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("NULL")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // IF statements segment - kept for compatibility
    dialect.add([(
        "IfStatementsSegment".into(),
        NodeMatcher::new(SyntaxKind::IfStatements, |_| {
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("WordAwareStatementSegment")
                ]),
                Ref::new("DelimiterGrammar").optional()
            ])])
            .config(|this| {
                this.min_times(1);
                this.terminators = vec_of_erased![
                    Ref::keyword("ELSE"),
                    Ref::keyword("END"),
                    Ref::new("BatchSeparatorGrammar")
                ];
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ELSE statement segment - separate from IF to ensure proper indentation levels
    dialect.add([(
        "ElseStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ElseStatement, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ELSE"),
                    StringParser::new("ELSE", SyntaxKind::Keyword),
                    StringParser::new("else", SyntaxKind::Keyword)
                ]),
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("WordAwareStatementSegment")
                ]),
                MetaSegment::dedent()
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ELSE IF statement segment - handles ELSE IF as a single statement type
    dialect.add([(
        "ElseIfStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::ElseIfStatement, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ELSE"),
                    StringParser::new("ELSE", SyntaxKind::Keyword),
                    StringParser::new("else", SyntaxKind::Keyword)
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("IF"),
                    StringParser::new("IF", SyntaxKind::Keyword),
                    StringParser::new("if", SyntaxKind::Keyword)
                ]),
                one_of(vec_of_erased![
                    Ref::new("WordAwareExpressionSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    Ref::new("StatementSegment"),
                    Ref::new("WordAwareStatementSegment")
                ]),
                MetaSegment::dedent()
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL IF statement - structured like BigQuery with explicit indent/dedent
    dialect.add([(
        "IfStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::IfStatement, |_| {
            Sequence::new(vec_of_erased![
                // Main IF clause: IF condition
                Ref::keyword("IF"),
                MetaSegment::indent(),
                Ref::new("ExpressionSegment"),
                Ref::new("IfStatementsSegment"),
                MetaSegment::dedent(),
                // ELSE IF clauses: ELSE IF condition (two keywords)
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("ELSE"),
                    Ref::keyword("IF"),
                    MetaSegment::indent(),
                    Ref::new("ExpressionSegment"),
                    Ref::new("IfStatementsSegment"),
                    MetaSegment::dedent()
                ])]),
                // Optional ELSE clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("ELSE"),
                    MetaSegment::indent(),
                    Ref::new("IfStatementsSegment"),
                    MetaSegment::dedent()
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);


    // Special identifier segment for bare procedure names that excludes statement keywords
    dialect.add([(
        "BareProcedureIdentifierSegment".into(),
        // Match any identifier except statement keywords (case-insensitive)
        RegexParser::new(
            r"[A-Za-z_@#][A-Za-z0-9_@$#]*",
            SyntaxKind::NakedIdentifier,
        )
        .anti_template(
            "(?i)^(RAISERROR|PRINT|RETURN|THROW|WAITFOR|ROLLBACK|COMMIT|SAVE|BEGIN|END|IF|ELSE|WHILE|BREAK|CONTINUE|GOTO|TRY|CATCH|DECLARE|SET|SELECT|INSERT|UPDATE|DELETE|MERGE)$"
        )
        .to_matchable()
        .into(),
    )]);

    // Bare procedure call (without EXECUTE keyword)
    // This matches: sp_help 'table' or dbo.myproc @param = 1
    // But should NOT match: RAISERROR (...) or other statement keywords
    dialect.add([(
        "BareProcedureCallStatementSegment".into(),
        Sequence::new(vec_of_erased![
            // Match object reference but ensure it's not a single keyword like RAISERROR
            one_of(vec_of_erased![
                // Multi-part object references are always valid (e.g., dbo.sp_help)
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("DotSegment"),
                    Ref::new("SingleIdentifierGrammar"),
                    // Optional additional parts
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("SingleIdentifierGrammar")
                    ])])
                ]),
                // Single identifier that is NOT a statement keyword
                Ref::new("BareProcedureIdentifierSegment")
            ]),
            // Optional parameters (same as ExecuteStatementGrammar)
            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                // First parameter doesn't need comma
                Sequence::new(vec_of_erased![one_of(vec_of_erased![
                    // Named parameter: @param = value
                    Sequence::new(vec_of_erased![
                        Ref::new("TsqlVariableSegment"),
                        Ref::new("AssignmentOperatorSegment"),
                        one_of(vec_of_erased![
                            Ref::new("ExpressionSegment"),
                            Ref::keyword("DEFAULT")
                        ]),
                        // Optional OUTPUT keyword
                        Ref::keyword("OUTPUT").optional()
                    ]),
                    // Positional parameter
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("ExpressionSegment"),
                            Ref::keyword("DEFAULT")
                        ]),
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
                            one_of(vec_of_erased![
                                Ref::new("ExpressionSegment"),
                                Ref::keyword("DEFAULT")
                            ]),
                            // Optional OUTPUT keyword
                            Ref::keyword("OUTPUT").optional()
                        ]),
                        // Positional parameter
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::new("ExpressionSegment"),
                                Ref::keyword("DEFAULT")
                            ]),
                            // Optional OUTPUT keyword
                            Ref::keyword("OUTPUT").optional()
                        ])
                    ])
                ])
            ])])
        ])
        .to_matchable()
        .into(),
    )]);

    // WHILE loop
    dialect.add([
        (
            "WhileStatementSegment".into(),
            Ref::new("WhileStatementGrammar").to_matchable().into(),
        ),
        (
            "WhileStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("WHILE"),
                    // Also accept WHILE as word token in T-SQL procedure bodies
                    StringParser::new("WHILE", SyntaxKind::Keyword)
                ]),
                Ref::new("ExpressionSegment"),
                one_of(vec_of_erased![
                    // Try word-aware parsers first for procedures with word tokens
                    Ref::new("WordAwareBeginEndBlockSegment"),
                    Ref::new("WordAwareStatementSegment"),
                    Ref::new("StatementSegment")
                ])
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

    // OPTION clause for query hints - MERGE references removed to test conflicts
    dialect.add([(
        "OptionClauseSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("OPTION"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                vec_of_erased![
                    // Join hints - MERGE re-enabled after resolving keyword conflicts
                    Sequence::new(vec_of_erased![Ref::keyword("MERGE"), Ref::keyword("JOIN")]),
                    Sequence::new(vec_of_erased![Ref::keyword("HASH"), Ref::keyword("JOIN")]),
                    Sequence::new(vec_of_erased![Ref::keyword("LOOP"), Ref::keyword("JOIN")]),
                    // Union hints - MERGE re-enabled after resolving keyword conflicts
                    Sequence::new(vec_of_erased![Ref::keyword("MERGE"), Ref::keyword("UNION")]),
                    Sequence::new(vec_of_erased![Ref::keyword("HASH"), Ref::keyword("UNION")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONCAT"),
                        Ref::keyword("UNION")
                    ]),
                    // Group hints
                    Sequence::new(vec_of_erased![Ref::keyword("HASH"), Ref::keyword("GROUP")]),
                    Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("GROUP")]),
                    // FAST n
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FAST"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    // MAXDOP n
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MAXDOP"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    // MAXRECURSION n
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MAXRECURSION"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    // OPTIMIZE FOR
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OPTIMIZE"),
                        Ref::keyword("FOR"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("TsqlVariableSegment"),
                                one_of(vec_of_erased![
                                    // @parameter = value
                                    Sequence::new(vec_of_erased![
                                        Ref::new("EqualsSegment"),
                                        Ref::new("LiteralGrammar")
                                    ]),
                                    // @parameter UNKNOWN
                                    Ref::keyword("UNKNOWN")
                                ])
                            ])
                        ])])
                    ]),
                    // RECOMPILE
                    Ref::keyword("RECOMPILE"),
                    // ROBUST PLAN
                    Sequence::new(vec_of_erased![Ref::keyword("ROBUST"), Ref::keyword("PLAN")]),
                    // FORCE ORDER
                    Sequence::new(vec_of_erased![Ref::keyword("FORCE"), Ref::keyword("ORDER")]),
                    // KEEP PLAN
                    Sequence::new(vec_of_erased![Ref::keyword("KEEP"), Ref::keyword("PLAN")]),
                    // KEEPFIXED PLAN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("KEEPFIXED"),
                        Ref::keyword("PLAN")
                    ]),
                    // EXPAND VIEWS
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXPAND"),
                        Ref::keyword("VIEWS")
                    ]),
                    // PARAMETERIZATION SIMPLE/FORCED
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PARAMETERIZATION"),
                        one_of(vec_of_erased![
                            Ref::keyword("SIMPLE"),
                            Ref::keyword("FORCED")
                        ])
                    ]),
                    // USE HINT
                    Sequence::new(vec_of_erased![
                        Ref::keyword("USE"),
                        Ref::keyword("HINT"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "QuotedLiteralSegment"
                        )])])
                    ]),
                    // QUERYTRACEON
                    Sequence::new(vec_of_erased![
                        Ref::keyword("QUERYTRACEON"),
                        Ref::new("NumericLiteralSegment")
                    ]),
                    // LABEL = 'label_name' (Azure Synapse Analytics)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LABEL"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("UnicodeLiteralSegment")
                        ])
                    ])
                ]
            )])])
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
                    Ref::new("LiteralGrammar") // JSON path - supports Unicode strings
                ])
                .config(|this| this.optional())
            ]),
            // Optional WITH clause for schema definition
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"), // Column name (naked or bracketed)
                        Ref::new("DatatypeSegment"),         // Data type
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
                Ref::keyword("INDEX"),
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
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])])
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
                    one_of(vec_of_erased![
                        // WITH (option = value, ...) - bracketed form
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                            vec_of_erased![
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
                                        Bracketed::new(vec_of_erased![Delimited::new(
                                            vec_of_erased![one_of(vec_of_erased![
                                                // Single partition number
                                                Ref::new("NumericLiteralSegment"),
                                                // Range of partitions
                                                Sequence::new(vec_of_erased![
                                                    Ref::new("NumericLiteralSegment"),
                                                    Ref::keyword("TO"),
                                                    Ref::new("NumericLiteralSegment")
                                                ])
                                            ])]
                                        )])
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
                                ]),
                                // COMPRESSION_DELAY option
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("COMPRESSION_DELAY"),
                                    Ref::new("AssignmentOperatorSegment"),
                                    Ref::new("NumericLiteralSegment"),
                                    Ref::keyword("MINUTES").optional()
                                ])
                            ]
                        )])]),
                        // WITH option = value - non-bracketed form (single option only)
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("FILLFACTOR"),
                                Ref::keyword("DATA_COMPRESSION")
                            ]),
                            Ref::new("AssignmentOperatorSegment"),
                            one_of(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::keyword("NONE"),
                                Ref::keyword("ROW"),
                                Ref::keyword("PAGE"),
                                Ref::keyword("COLUMNSTORE"),
                                Ref::keyword("COLUMNSTORE_ARCHIVE")
                            ]),
                            // Optional ON PARTITIONS for DATA_COMPRESSION
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
                        ])
                    ])
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
        .to_matchable(),
    );

    // Add CREATE/UPDATE/DROP STATISTICS statements
    dialect.add([
        (
            "CreateStatisticsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("STATISTICS"),
                    Ref::new("ObjectReferenceSegment"), // Statistics name
                    Ref::keyword("ON"),
                    Ref::new("TableReferenceSegment"),
                    // Column list in parentheses
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])]),
                    // Optional WITH options
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::keyword("FULLSCAN"),
                            Ref::keyword("NORECOMPUTE"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SAMPLE"),
                                Ref::new("NumericLiteralSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("PERCENT"),
                                    Ref::keyword("ROWS")
                                ])
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("STATS_STREAM"),
                                Ref::new("EqualsSegment"),
                                Ref::new("ExpressionSegment")
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
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ObjectReferenceSegment"
                        )])])
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
                                one_of(vec_of_erased![
                                    Ref::keyword("PERCENT"),
                                    Ref::keyword("ROWS")
                                ])
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
            NodeMatcher::new(SyntaxKind::DropIndexStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    Ref::keyword("STATISTICS"),
                    // Allow multiple statistics to be dropped (comma-separated)
                    Delimited::new(vec_of_erased![
                        // Just use ObjectReferenceSegment which handles multi-part names
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override DROP INDEX for T-SQL specific syntax: DROP INDEX index_name ON table_name
    dialect.replace_grammar(
        "DropIndexStatementSegment",
        NodeMatcher::new(SyntaxKind::DropIndexStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("INDEX"),
                Ref::new("ObjectReferenceSegment"), // Index name
                Ref::keyword("ON"),
                Ref::new("TableReferenceSegment") // Table name
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // WAITFOR statement
    dialect.add([(
        "WaitforStatementSegment".into(),
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
        .into(),
    )]);

    // CREATE TYPE statement
    dialect.add([(
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
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("TableConstraintSegment"),
                                Ref::new("ColumnDefinitionSegment")
                            ])])
                            .config(|this| this.allow_trailing())
                        ])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

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
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
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
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        Ref::new("ColumnReferenceSegment"),
                                        one_of(vec_of_erased![
                                            Ref::keyword("ASC"),
                                            Ref::keyword("DESC")
                                        ])
                                        .config(|this| this.optional())
                                    ])
                                ])])
                            ]),
                            // Boolean flags
                            Ref::keyword("CHECK_CONSTRAINTS"),
                            Ref::keyword("FIRE_TRIGGERS"),
                            Ref::keyword("KEEPIDENTITY"),
                            Ref::keyword("KEEPNULLS"),
                            Ref::keyword("TABLOCK")
                        ]
                    )])])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // CREATE PARTITION FUNCTION statement
    dialect.add([(
        "CreatePartitionFunctionSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Ref::keyword("PARTITION"),
            Ref::keyword("FUNCTION"),
            Ref::new("ObjectReferenceSegment"),
            Bracketed::new(vec_of_erased![Ref::new("DatatypeSegment")]),
            Ref::keyword("AS"),
            Ref::keyword("RANGE"),
            one_of(vec_of_erased![Ref::keyword("LEFT"), Ref::keyword("RIGHT")]),
            Ref::keyword("FOR"),
            Ref::keyword("VALUES"),
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "LiteralGrammar"
            )])])
        ])
        .to_matchable()
        .into(),
    )]);

    // ALTER PARTITION FUNCTION statement
    dialect.add([(
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
                        Bracketed::new(vec_of_erased![Ref::new("LiteralGrammar")])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MERGE"),
                        Ref::keyword("RANGE"),
                        Bracketed::new(vec_of_erased![Ref::new("LiteralGrammar")])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CREATE PARTITION SCHEME statement
    dialect.add([(
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
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                    vec_of_erased![Ref::new("ObjectReferenceSegment"), Ref::keyword("PRIMARY")]
                )])])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ALTER PARTITION SCHEME statement
    dialect.add([(
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
    )]);

    // CREATE FULLTEXT INDEX statement
    dialect.add([(
        "CreateFullTextIndexStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("FULLTEXT"),
                Ref::keyword("INDEX"),
                Ref::keyword("ON"),
                Ref::new("TableReferenceSegment"),
                // Column specifications
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        // Optional column options
                        Sequence::new(vec_of_erased![one_of(vec_of_erased![
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
                        ])])
                        .config(|this| this.optional())
                    ])
                ])]),
                // KEY INDEX clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("KEY"),
                    Ref::keyword("INDEX"),
                    Ref::new("ObjectReferenceSegment"),
                    // Optional catalog/filegroup options
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ON"),
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            Ref::new("ObjectReferenceSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILEGROUP"),
                                Ref::new("ObjectReferenceSegment")
                            ])
                        ])])
                        .config(|this| this.allow_trailing())
                    ])
                    .config(|this| this.optional())
                ]),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            // CHANGE_TRACKING [=] (MANUAL | AUTO | OFF)
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CHANGE_TRACKING"),
                                Ref::new("EqualsSegment").optional(),
                                one_of(vec_of_erased![
                                    Ref::keyword("MANUAL"),
                                    Ref::keyword("AUTO"),
                                    Ref::keyword("OFF")
                                ])
                            ]),
                            // NO POPULATION
                            Sequence::new(vec_of_erased![
                                Ref::keyword("NO"),
                                Ref::keyword("POPULATION")
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
                        ]
                    )])])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ALTER INDEX statement
    dialect.add([(
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
                    // REBUILD [PARTITION = partition_number | ALL] [Partition = N] [WITH (...)]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REBUILD"),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITION"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("ALL"),
                                    Ref::new("NumericLiteralSegment")
                                ])
                            ]),
                            // Support "Partition = N" syntax (capitalized Partition)
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PARTITION"),
                                Ref::new("EqualsSegment"),
                                Ref::new("NumericLiteralSegment")
                            ])
                        ])
                        .config(|this| this.optional()),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
                                    Sequence::new(vec_of_erased![
                                        one_of(vec_of_erased![
                                            Ref::keyword("PAD_INDEX"),
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
                                        Ref::keyword("FILLFACTOR"),
                                        Ref::new("EqualsSegment"),
                                        Ref::new("NumericLiteralSegment")
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
                                        ]),
                                        // Support ON PARTITIONS clause
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("ON"),
                                            Ref::keyword("PARTITIONS"),
                                            Bracketed::new(vec_of_erased![Delimited::new(
                                                vec_of_erased![one_of(vec_of_erased![
                                                    Ref::new("NumericLiteralSegment"),
                                                    Sequence::new(vec_of_erased![
                                                        Ref::new("NumericLiteralSegment"),
                                                        Ref::keyword("TO"),
                                                        Ref::new("NumericLiteralSegment")
                                                    ])
                                                ])]
                                            )])
                                        ])
                                        .config(|this| this.optional())
                                    ]),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("XML_COMPRESSION"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::keyword("ON"),
                                            Ref::keyword("OFF")
                                        ]),
                                        // Support ON PARTITIONS clause
                                        Sequence::new(vec_of_erased![
                                            Ref::keyword("ON"),
                                            Ref::keyword("PARTITIONS"),
                                            Bracketed::new(vec_of_erased![Delimited::new(
                                                vec_of_erased![one_of(vec_of_erased![
                                                    Ref::new("NumericLiteralSegment"),
                                                    Sequence::new(vec_of_erased![
                                                        Ref::new("NumericLiteralSegment"),
                                                        Ref::keyword("TO"),
                                                        Ref::new("NumericLiteralSegment")
                                                    ])
                                                ])]
                                            )])
                                        ])
                                        .config(|this| this.optional())
                                    ]),
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("ONLINE"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::keyword("ON"),
                                            Ref::keyword("OFF"),
                                            // Support ONLINE = ON (WAIT_AT_LOW_PRIORITY(...))
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("ON"),
                                                Bracketed::new(vec_of_erased![
                                                    Ref::keyword("WAIT_AT_LOW_PRIORITY"),
                                                    Bracketed::new(vec_of_erased![Delimited::new(
                                                        vec_of_erased![Sequence::new(
                                                            vec_of_erased![
                                                                one_of(vec_of_erased![
                                                                    Ref::keyword("MAX_DURATION"),
                                                                    Ref::keyword(
                                                                        "ABORT_AFTER_WAIT"
                                                                    )
                                                                ]),
                                                                Ref::new("EqualsSegment"),
                                                                one_of(vec_of_erased![
                                                                    Ref::new(
                                                                        "NumericLiteralSegment"
                                                                    ),
                                                                    Ref::keyword("SELF"),
                                                                    Ref::keyword("BLOCKERS"),
                                                                    Ref::keyword("NONE")
                                                                ])
                                                            ]
                                                        )]
                                                    )])
                                                ])
                                            ])
                                        ])
                                    ])
                                ]
                            )])])
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
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("LOB_COMPACTION"),
                                        Ref::keyword("COMPRESS_ALL_ROW_GROUPS")
                                    ]),
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                                ])
                            ])])
                        ])
                        .config(|this| this.optional())
                    ]),
                    // SET (option = value, ...)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                            vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        Ref::keyword("ALLOW_ROW_LOCKS"),
                                        Ref::keyword("ALLOW_PAGE_LOCKS"),
                                        Ref::keyword("OPTIMIZE_FOR_SEQUENTIAL_KEY"),
                                        Ref::keyword("IGNORE_DUP_KEY"),
                                        Ref::keyword("STATISTICS_NORECOMPUTE")
                                    ]),
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("COMPRESSION_DELAY"),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("NumericLiteralSegment"),
                                    Ref::keyword("MINUTES").optional()
                                ])
                            ]
                        )])])
                    ]),
                    // RESUME [WITH (...)]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RESUME"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                                vec_of_erased![
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
                                        Ref::keyword("WAIT_AT_LOW_PRIORITY"),
                                        Bracketed::new(vec_of_erased![Delimited::new(
                                            vec_of_erased![Sequence::new(vec_of_erased![
                                                one_of(vec_of_erased![
                                                    Ref::keyword("MAX_DURATION"),
                                                    Ref::keyword("ABORT_AFTER_WAIT")
                                                ]),
                                                Ref::new("EqualsSegment"),
                                                one_of(vec_of_erased![
                                                    Ref::new("NumericLiteralSegment"),
                                                    Ref::keyword("SELF"),
                                                    Ref::keyword("BLOCKERS"),
                                                    Ref::keyword("NONE")
                                                ])
                                            ])]
                                        )])
                                    ])
                                ]
                            )])])
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
    )]);

    // Define specialized ALTER TABLE column addition for mixed operations
    dialect.add([(
        "AlterTableAddColumnSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterTableActionSegment, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::new("SingleIdentifierGrammar"), // Column name
                Ref::new("DatatypeSegment"),         // Column type
                // Column modifiers and constraints
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    // Nullability
                    Ref::keyword("NULL"),
                    Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("NULL")]),
                    // IDENTITY specification
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IDENTITY"),
                        Bracketed::new(vec_of_erased![
                            Ref::new("NumericLiteralSegment"), // seed
                            Ref::new("CommaSegment"),
                            Ref::new("NumericLiteralSegment") // increment
                        ])
                        .config(|this| this.optional())
                    ]),
                    // DEFAULT constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        one_of(vec_of_erased![
                            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                            Ref::new("ExpressionSegment")
                        ])
                    ]),
                    // Named constraint: CONSTRAINT name constraint_type
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("SingleIdentifierGrammar"), // constraint name
                        one_of(vec_of_erased![
                            Ref::keyword("UNIQUE"),
                            Ref::keyword("PRIMARY"), // Will be followed by KEY
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PRIMARY"),
                                Ref::keyword("KEY")
                            ]),
                            // FOREIGN KEY references
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FOREIGN"),
                                Ref::keyword("KEY"),
                                Ref::keyword("REFERENCES"),
                                Ref::new("TableReferenceSegment"),
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    Ref::new("ColumnReferenceSegment")
                                ])])
                                .config(|this| this.optional())
                            ])
                        ])
                    ]),
                    // COLLATE clause
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLLATE"),
                        Ref::new("SingleIdentifierGrammar")
                    ]),
                    // FILESTREAM
                    Ref::keyword("FILESTREAM")
                ])])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL DROP COLUMN list segment - simple pattern without NodeMatcher
    dialect.add([(
        "TsqlDropColumnListSegment".into(),
        Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")])
            .to_matchable()
            .into(),
    )]);

    // T-SQL ALTER TABLE DROP COLUMN support (standalone grammar)
    // T-SQL supports: DROP COLUMN col1, col2, col3
    dialect.add([(
        "AlterTableDropColumnGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("DROP"),
            Ref::keyword("COLUMN"),
            Ref::new("IfExistsGrammar").optional(),
            Ref::new("TsqlDropColumnListSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // Define individual ALTER TABLE operations for reuse (keep existing for compatibility)
    dialect.add([(
        "TsqlAlterTableOperationGrammar".into(),
        one_of(vec_of_erased![
            // ADD column (constraints are handled within ColumnDefinitionSegment)
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::new("ColumnDefinitionSegment")
            ]),
            // ADD CONSTRAINT with DEFAULT, PRIMARY KEY, FOREIGN KEY, etc.
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment"),
                one_of(vec_of_erased![
                    // DEFAULT constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment")
                    ]),
                    // PRIMARY KEY CLUSTERED
                    Sequence::new(vec_of_erased![
                        Ref::keyword("PRIMARY"),
                        Ref::keyword("KEY"),
                        Ref::keyword("CLUSTERED").optional(),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])])
                    ]),
                    // FOREIGN KEY ... REFERENCES ... with ON UPDATE/DELETE actions
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOREIGN"),
                        Ref::keyword("KEY"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])]),
                        Ref::keyword("REFERENCES"),
                        Ref::new("TableReferenceSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])]),
                        // Optional ON UPDATE/DELETE actions
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::keyword("ON"),
                            one_of(vec_of_erased![
                                Ref::keyword("UPDATE"),
                                Ref::keyword("DELETE")
                            ]),
                            one_of(vec_of_erased![
                                Ref::keyword("CASCADE"),
                                Ref::keyword("RESTRICT"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("SET"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("NULL"),
                                        Ref::keyword("DEFAULT")
                                    ])
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("NO"),
                                    Ref::keyword("ACTION")
                                ])
                            ])
                        ])])
                        .config(|this| this.optional())
                    ])
                ])
            ]),
            // WITH CHECK ADD CONSTRAINT ... FOREIGN KEY
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("CHECK"),
                Ref::keyword("ADD"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment"),
                Ref::keyword("FOREIGN"),
                Ref::keyword("KEY"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ColumnReferenceSegment"
                )])]),
                Ref::keyword("REFERENCES"),
                Ref::new("TableReferenceSegment"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ColumnReferenceSegment"
                )])])
            ]),
            // CHECK CONSTRAINT constraint_name
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECK"),
                Ref::keyword("CONSTRAINT"),
                Ref::new("ObjectReferenceSegment")
            ]),
            // DROP COLUMN with IF EXISTS and multi-column support (matching SQLFluff)
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("COLUMN"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("TsqlDropColumnListSegment")
            ]),
            // DROP PERIOD FOR SYSTEM_TIME
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("PERIOD"),
                Ref::keyword("FOR"),
                Ref::keyword("SYSTEM_TIME")
            ]),
            // ADD PERIOD FOR SYSTEM_TIME (column1, column2)
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                Ref::keyword("PERIOD"),
                Ref::keyword("FOR"),
                Ref::keyword("SYSTEM_TIME"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ColumnReferenceSegment"
                )])])
            ]),
            // ALTER COLUMN
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("COLUMN"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DatatypeSegment")
            ]),
            // SET (option = value, ...)
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"), // option name like SYSTEM_VERSIONING
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::keyword("ON"),
                            Ref::keyword("OFF"),
                            Ref::new("LiteralGrammar"),
                            Ref::new("ObjectReferenceSegment"),
                            Ref::new("QuotedLiteralSegment"),
                            // Handle complex option values like OFF(HISTORY_TABLE = ...)
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::new("NakedIdentifierSegment"),
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![
                                        Ref::new("LiteralGrammar"),
                                        Ref::new("ObjectReferenceSegment"),
                                        Ref::keyword("INFINITE"),
                                        Sequence::new(vec_of_erased![
                                            Ref::new("NumericLiteralSegment"),
                                            one_of(vec_of_erased![
                                                Ref::keyword("YEAR"),
                                                Ref::keyword("YEARS"),
                                                Ref::keyword("MONTH"),
                                                Ref::keyword("MONTHS"),
                                                Ref::keyword("DAY"),
                                                Ref::keyword("DAYS")
                                            ])
                                        ])
                                    ])
                                ])
                            ])])
                        ])
                    ])
                ])])
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Backup: Original complex T-SQL ALTER TABLE grammar (commented out for debugging)
    /*
    dialect.replace_grammar(
        "AlterTableStatementSegmentComplex",
        NodeMatcher::new(SyntaxKind::AlterTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("TABLE"),
                Ref::new("TableReferenceSegment"),
                Delimited::new(vec_of_erased![
                    one_of(vec_of_erased![
                    // ADD clauses
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ADD"),
                        one_of(vec_of_erased![
                            // ADD column_definition(s) - can be multiple separated by commas
                            Delimited::new(vec_of_erased![
                                Ref::new("ColumnDefinitionSegment")
                            ]),
                            // ADD CONSTRAINT
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CONSTRAINT"),
                                Ref::new("ObjectReferenceSegment"),
                                one_of(vec_of_erased![
                                    // DEFAULT constraint
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("DEFAULT"),
                                        Ref::new("ExpressionSegment"),
                                        Ref::keyword("FOR"),
                                        Ref::new("ColumnReferenceSegment")
                                    ]),
                                    // PRIMARY KEY
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("PRIMARY"),
                                        Ref::keyword("KEY"),
                                        Ref::keyword("CLUSTERED").optional(),
                                        Ref::new("BracketedColumnReferenceListGrammar")
                                    ]),
                                    // FOREIGN KEY
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("FOREIGN"),
                                        Ref::keyword("KEY"),
                                        Ref::new("BracketedColumnReferenceListGrammar"),
                                        Ref::keyword("REFERENCES"),
                                        Ref::new("TableReferenceSegment"),
                                        Ref::new("BracketedColumnReferenceListGrammar"),
                                        // ON UPDATE/DELETE actions
                                        AnyNumberOf::new(vec_of_erased![
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("ON"),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("UPDATE"),
                                                    Ref::keyword("DELETE")
                                                ]),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("CASCADE"),
                                                    Ref::keyword("RESTRICT"),
                                                    Ref::keyword("SET"),
                                                    Ref::keyword("NO")
                                                ]),
                                                one_of(vec_of_erased![
                                                    Ref::keyword("NULL"),
                                                    Ref::keyword("DEFAULT"),
                                                    Ref::keyword("ACTION")
                                                ]).config(|this| this.optional())
                                            ])
                                        ])
                                    ])
                                ])
                            ]),
                            // ADD computed column
                            Sequence::new(vec_of_erased![
                                Ref::new("SingleIdentifierGrammar"),
                                Ref::keyword("AS"),
                                one_of(vec_of_erased![
                                    Ref::new("ExpressionSegment"),
                                    // Support both bracketed and non-bracketed expressions
                                    Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                                ]),
                                Ref::keyword("PERSISTED").optional(),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("NOT"),
                                    Ref::keyword("NULL")
                                ]).config(|this| this.optional())
                            ]),
                            // ADD PERIOD FOR SYSTEM_TIME
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PERIOD"),
                                Ref::keyword("FOR"),
                                Ref::keyword("SYSTEM_TIME"),
                                Bracketed::new(vec_of_erased![
                                    Delimited::new(vec_of_erased![
                                        Ref::new("ColumnReferenceSegment")
                                    ])
                                ])
                            ])
                        ])
                    ]),
                    // ALTER COLUMN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ALTER"),
                        Ref::keyword("COLUMN"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("DatatypeSegment")
                    ]),
                    // DROP clauses
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DROP"),
                        one_of(vec_of_erased![
                            // DROP COLUMN - simplified version matching ANSI structure
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COLUMN"),
                                Ref::new("SingleIdentifierGrammar")
                            ]),
                            // DROP CONSTRAINT [IF EXISTS] constraint_name
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CONSTRAINT"),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("IF"),
                                    Ref::keyword("EXISTS")
                                ]).config(|this| this.optional()),
                                Ref::new("ObjectReferenceSegment")
                            ]),
                            // DROP PERIOD FOR SYSTEM_TIME
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PERIOD"),
                                Ref::keyword("FOR"),
                                Ref::keyword("SYSTEM_TIME")
                            ])
                        ])
                    ]),
                    // SET options
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    // SYSTEM_VERSIONING
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("SYSTEM_VERSIONING"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::keyword("ON"),
                                            Ref::keyword("OFF"),
                                            // OFF with options
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("OFF"),
                                                Bracketed::new(vec_of_erased![
                                                    Delimited::new(vec_of_erased![
                                                        one_of(vec_of_erased![
                                                            Sequence::new(vec_of_erased![
                                                                Ref::keyword("HISTORY_TABLE"),
                                                                Ref::new("EqualsSegment"),
                                                                Ref::new("ObjectReferenceSegment")
                                                            ]),
                                                            Sequence::new(vec_of_erased![
                                                                Ref::keyword("DATA_CONSISTENCY_CHECK"),
                                                                Ref::new("EqualsSegment"),
                                                                one_of(vec_of_erased![
                                                                    Ref::keyword("ON"),
                                                                    Ref::keyword("OFF")
                                                                ])
                                                            ]),
                                                            Sequence::new(vec_of_erased![
                                                                Ref::keyword("HISTORY_RETENTION_PERIOD"),
                                                                Ref::new("EqualsSegment"),
                                                                one_of(vec_of_erased![
                                                                    Ref::keyword("INFINITE"),
                                                                    Sequence::new(vec_of_erased![
                                                                        Ref::new("NumericLiteralSegment"),
                                                                        one_of(vec_of_erased![
                                                                            Ref::keyword("YEAR"),
                                                                            Ref::keyword("YEARS"),
                                                                            Ref::keyword("MONTH"),
                                                                            Ref::keyword("MONTHS"),
                                                                            Ref::keyword("DAY"),
                                                                            Ref::keyword("DAYS")
                                                                        ])
                                                                    ])
                                                                ])
                                                            ])
                                                        ])
                                                    ])
                                                ]).config(|this| this.optional())
                                            ])
                                        ])
                                    ]),
                                    // FILESTREAM_ON
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("FILESTREAM_ON"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::new("QuotedLiteralSegment"),
                                            Ref::new("QuotedIdentifierSegment"), // Handle double-quoted values
                                            Ref::new("NakedIdentifierSegment")
                                        ])
                                    ]),
                                    // DATA_DELETION
                                    Sequence::new(vec_of_erased![
                                        Ref::keyword("DATA_DELETION"),
                                        Ref::new("EqualsSegment"),
                                        one_of(vec_of_erased![
                                            Ref::keyword("ON"),
                                            Sequence::new(vec_of_erased![
                                                Ref::keyword("OFF"),
                                                Bracketed::new(vec_of_erased![
                                                    Delimited::new(vec_of_erased![
                                                        one_of(vec_of_erased![
                                                            Sequence::new(vec_of_erased![
                                                                Ref::keyword("FILTER_COLUMN"),
                                                                Ref::new("EqualsSegment"),
                                                                Ref::new("ColumnReferenceSegment")
                                                            ]),
                                                            Sequence::new(vec_of_erased![
                                                                Ref::keyword("RETENTION_PERIOD"),
                                                                Ref::new("EqualsSegment"),
                                                                one_of(vec_of_erased![
                                                                    Ref::keyword("INFINITE"),
                                                                    Sequence::new(vec_of_erased![
                                                                        Ref::new("NumericLiteralSegment"),
                                                                        one_of(vec_of_erased![
                                                                            Ref::keyword("YEAR"),
                                                                            Ref::keyword("YEARS"),
                                                                            Ref::keyword("DAY"),
                                                                            Ref::keyword("DAYS")
                                                                        ])
                                                                    ])
                                                                ])
                                                            ])
                                                        ])
                                                    ])
                                                ]).config(|this| this.optional())
                                            ])
                                        ])
                                    ])
                                ])
                            ])
                        ])
                    ]),
                    // WITH CHECK ADD CONSTRAINT (for foreign keys)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("CHECK"),
                        Ref::keyword("ADD"),
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment"),
                        Ref::keyword("FOREIGN"),
                        Ref::keyword("KEY"),
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        Ref::keyword("REFERENCES"),
                        Ref::new("TableReferenceSegment"),
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        // ON UPDATE/DELETE actions
                        AnyNumberOf::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ON"),
                                one_of(vec_of_erased![
                                    Ref::keyword("UPDATE"),
                                    Ref::keyword("DELETE")
                                ]),
                                one_of(vec_of_erased![
                                    Ref::keyword("CASCADE"),
                                    Ref::keyword("RESTRICT"),
                                    Ref::keyword("SET"),
                                    Ref::keyword("NO")
                                ]),
                                one_of(vec_of_erased![
                                    Ref::keyword("NULL"),
                                    Ref::keyword("DEFAULT"),
                                    Ref::keyword("ACTION")
                                ]).config(|this| this.optional())
                            ])
                        ])
                    ]),
                    // CHECK CONSTRAINT
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CHECK"),
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable(),
    );
    */

    // T-SQL ALTER TABLE OPTIONS GRAMMAR
    // Define the grammar for individual ALTER TABLE operations
    dialect.add([(
        "TsqlAlterTableOptionsGrammar".into(),
        one_of(vec_of_erased![
            // ADD operations (column or constraint)
            Sequence::new(vec_of_erased![
                Ref::keyword("ADD"),
                one_of(vec_of_erased![
                    // ADD COLUMN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN").optional(),
                        Ref::new("ColumnDefinitionSegment")
                    ]),
                    // ADD CONSTRAINT
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Ref::new("ObjectReferenceSegment"),
                        one_of(vec_of_erased![
                            // PRIMARY KEY
                            Sequence::new(vec_of_erased![
                                Ref::keyword("PRIMARY"),
                                Ref::keyword("KEY"),
                                Ref::keyword("CLUSTERED").optional(),
                                Ref::new("BracketedColumnReferenceListGrammar")
                            ]),
                            // UNIQUE
                            Sequence::new(vec_of_erased![
                                Ref::keyword("UNIQUE"),
                                Ref::keyword("CLUSTERED").optional(),
                                Ref::new("BracketedColumnReferenceListGrammar")
                            ]),
                            // DEFAULT
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DEFAULT"),
                                Ref::new("ExpressionSegment"),
                                Ref::keyword("FOR"),
                                Ref::new("ColumnReferenceSegment")
                            ]),
                            // FOREIGN KEY
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FOREIGN"),
                                Ref::keyword("KEY"),
                                Ref::new("BracketedColumnReferenceListGrammar"),
                                Ref::keyword("REFERENCES"),
                                Ref::new("TableReferenceSegment"),
                                Ref::new("BracketedColumnReferenceListGrammar")
                            ])
                        ])
                    ])
                ])
            ]),
            // DROP operations
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                one_of(vec_of_erased![
                    // DROP COLUMN - single column works, multi-column has parser limitation
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLUMN"),
                        Ref::new("IfExistsGrammar").optional(),
                        Ref::new("TsqlDropColumnListSegment")
                    ]),
                    // DROP CONSTRAINT
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONSTRAINT"),
                        Sequence::new(vec_of_erased![Ref::keyword("IF"), Ref::keyword("EXISTS")])
                            .config(|this| this.optional()),
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
            ]),
            // ALTER COLUMN
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("COLUMN"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DatatypeSegment")
            ]),
            // RENAME operations
            Sequence::new(vec_of_erased![
                Ref::keyword("RENAME"),
                one_of(vec_of_erased![Ref::keyword("AS"), Ref::keyword("TO")])
                    .config(|this| this.optional()),
                Ref::new("TableReferenceSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Override ANSI ALTER TABLE statement to use T-SQL specific grammar
    dialect.replace_grammar(
        "AlterTableStatementSegment",
        NodeMatcher::new(SyntaxKind::AlterTableStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("TABLE"),
                Ref::new("TableReferenceSegment"),
                // Use T-SQL specific options grammar instead of ANSI
                Delimited::new(vec_of_erased![Ref::new("TsqlAlterTableOptionsGrammar")])
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // ALTER TABLE SWITCH statement
    dialect.add([(
        "AlterTableSwitchStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterTableSwitchStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("TABLE"),
                Ref::new("TableReferenceSegment"),
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
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
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
                            ])])
                        ]),
                        // TRUNCATE_TARGET option (Azure Synapse Analytics)
                        Bracketed::new(vec_of_erased![
                            Ref::keyword("TRUNCATE_TARGET"),
                            Ref::new("EqualsSegment"),
                            one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
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
                // Optional transaction/savepoint name (can be identifier or variable)
                one_of(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("ParameterNameSegment")
                ])
                .config(|this| this.optional())
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
                one_of(vec_of_erased![
                    Ref::keyword("GO"),
                    // Also accept GO as word token for word-aware contexts
                    StringParser::new("GO", SyntaxKind::Word)
                ]),
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

    // Add BatchSegment that contains multiple statements like SQLFluff
    dialect.add([(
        "BatchSegment".into(),
        NodeMatcher::new(SyntaxKind::Batch, |_| {
            one_of(vec_of_erased![
                // Try normal statement parsing first
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("StatementSegment"),
                    AnyNumberOf::new(vec_of_erased![
                        Ref::new("DelimiterGrammar"),      // Optional semicolons in T-SQL
                        Ref::new("BatchDelimiterGrammar")  // Also allow GO to terminate statements
                    ])
                    .config(|this| this.optional())
                ])])
                .config(|this| this.min_times(1)), // At least one statement required
                // Fallback to word-aware batch parsing
                Ref::new("WordAwareBatchSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override FileSegment to handle T-SQL batch separators (GO statements)
    // This creates a file structure where GO separates batches like SQLFluff
    dialect.replace_grammar(
        "FileSegment",
        Sequence::new(vec_of_erased![
            // Allow any number of GO statements at the start of the file
            AnyNumberOf::new(vec_of_erased![
                Ref::new("BatchDelimiterGrammar"),
                Ref::new("DelimiterGrammar").optional()
            ]),
            // Main content: Batch followed by optional GO-separated batches
            Sequence::new(vec_of_erased![
                // First batch
                Ref::new("BatchSegment"),
                // Any number of GO-separated batches
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("BatchDelimiterGrammar"),
                    Ref::new("DelimiterGrammar").optional(),
                    Ref::new("BatchSegment")
                ])]),
                // Allow trailing GO statements at the end of the file
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("BatchDelimiterGrammar"),
                    Ref::new("DelimiterGrammar").optional()
                ])
            ])
            .config(|this| this.optional()) // The entire content is optional for empty files
        ])
        .to_matchable(),
    );

    // Add SELECT INTO statement as a separate construct for T-SQL
    dialect.add([(
        "SelectIntoStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SelectStatement, |_| {
            Sequence::new(vec_of_erased![
                // Use the standard SelectClauseSegment which has proper parsing
                Ref::new("SelectClauseSegment"),
                // INTO clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTO"),
                    Ref::new("TableReferenceSegment")
                ]),
                // Rest of SELECT statement
                Ref::new("FromClauseSegment").optional(),
                Ref::new("WhereClauseSegment").optional(),
                Ref::new("GroupByClauseSegment").optional(),
                Ref::new("HavingClauseSegment").optional(),
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("OptionClauseSegment").optional()
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add T-SQL specific statement types to the statement segment
    dialect.replace_grammar(
        "StatementSegment",
        one_of(vec_of_erased![
            // TryBlockSegment MUST be first to prevent BeginEndBlockSegment from matching "BEGIN TRY"
            Ref::new("TryBlockSegment"),
            // T-SQL specific SELECT INTO (must come before regular SelectableGrammar)
            Ref::new("SelectIntoStatementSegment"),
            // Other T-SQL specific statements
            Ref::new("BeginEndBlockSegment"),
            Ref::new("ThrowStatementSegment"),
            Ref::new("AtomicBlockSegment"),
            // Removed BatchSeparatorSegment - GO should be a batch separator, not a statement
            Ref::new("DeclareStatementSegment"),
            Ref::new("SetVariableStatementSegment"),
            Ref::new("PrintStatementSegment"),
            Ref::new("IfStatementSegment"),
            Ref::new("WhileStatementSegment"),
            Ref::new("BreakStatementSegment"),
            Ref::new("ContinueStatementSegment"),
            Ref::new("GotoStatementSegment"),
            Ref::new("LabelSegment"),
            Ref::new("ExecuteStatementSegment"),
            Ref::new("ReconfigureStatementSegment"),
            Ref::new("UseStatementSegment"),
            Ref::new("WaitforStatementSegment"),
            Ref::new("CreateTypeStatementSegment"),
            Ref::new("BulkInsertStatementSegment"),
            Ref::new("TsqlCopyIntoStatementSegment"),
            Ref::new("CreatePartitionFunctionSegment"),
            Ref::new("AlterPartitionFunctionSegment"),
            Ref::new("CreatePartitionSchemeSegment"),
            Ref::new("AlterPartitionSchemeSegment"),
            Ref::new("CreateFullTextIndexStatementSegment"),
            Ref::new("AlterIndexStatementSegment"),
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
            Ref::new("ReturnStatementSegment"),
            // Cursor statements
            Ref::new("DeclareCursorStatementSegment"),
            Ref::new("OpenCursorStatementSegment"),
            Ref::new("FetchCursorStatementSegment"),
            Ref::new("CloseCursorStatementSegment"),
            Ref::new("DeallocateCursorStatementSegment"),
            // Symmetric key operations
            Ref::new("OpenSymmetricKeyStatementSegment"),
            Ref::new("CreateSynonymStatementSegment"),
            Ref::new("DropSynonymStatementSegment"),
            Ref::new("RenameObjectStatementSegment"),
            Ref::new("SetContextInfoStatementSegment"),
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
            Ref::new("TsqlGrantStatementSegment"),
            Ref::new("TsqlDenyStatementSegment"),
            Ref::new("TsqlRevokeStatementSegment"),
            // Enhanced CREATE TABLE handles both keywords and word tokens
            Ref::new("CreateTableStatementSegment"),
            Ref::new("CreateRoleStatementSegment"),
            Ref::new("DropRoleStatementSegment"),
            Ref::new("AlterTableStatementSegment"),
            Ref::new("AlterTableSwitchStatementSegment"),
            Ref::new("CreateSchemaStatementSegment"),
            Ref::new("SetSchemaStatementSegment"),
            Ref::new("DropSchemaStatementSegment"),
            Ref::new("DropTypeStatementSegment"),
            Ref::new("CreateDatabaseStatementSegment"),
            Ref::new("CreateDatabaseScopedCredentialStatementSegment"),
            Ref::new("CreateMasterKeyStatementSegment"),
            Ref::new("AlterMasterKeyStatementSegment"),
            Ref::new("DropMasterKeyStatementSegment"),
            Ref::new("DropDatabaseStatementSegment"),
            // Word-aware CREATE INDEX must come before regular CREATE INDEX
            Ref::new("WordAwareCreateIndexStatementSegment"),
            Ref::new("CreateIndexStatementSegment"),
            Ref::new("DropIndexStatementSegment"),
            Ref::new("CreateStatisticsStatementSegment"),
            Ref::new("UpdateStatisticsStatementSegment"),
            Ref::new("DropStatisticsStatementSegment"),
            Ref::new("CreateViewStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("CreateCastStatementSegment"),
            Ref::new("DropCastStatementSegment"),
            Ref::new("CreateFunctionStatementSegment"),
            Ref::new("CreateOrAlterFunctionStatementSegment"),
            Ref::new("AlterFunctionStatementSegment"),
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
            Ref::new("DropTriggerStatementSegment"),
            // Bare procedure call (without EXECUTE) - MUST be last to avoid conflicts
            Ref::new("BareProcedureCallStatementSegment")
        ])
        .config(|this| {
            this.terminators = vec_of_erased![
                Ref::new("DelimiterGrammar"),
                Ref::new("BatchSeparatorGrammar") // Ensure GO terminates statements
            ]
        })
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
            "TsqlActionVariableSegment".into(),
            // Match $action token specifically (lexer now tokenizes it as TsqlVariable)
            StringParser::new("$action", SyntaxKind::TsqlVariable)
                .to_matchable()
                .into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(SyntaxKind::ParameterizedExpression, |_| {
                one_of(vec_of_erased![
                    Ref::new("TsqlVariableSegment"),
                    Ref::new("TsqlActionVariableSegment"),
                ])
                .to_matchable()
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

    // Add T-SQL specific ObjectReferenceSegment that supports dot-prefixed references
    dialect.add([(
        "TsqlDotPrefixedReferenceSegment".into(),
        NodeMatcher::new(SyntaxKind::ObjectReference, |_| {
            Sequence::new(vec_of_erased![
                // One or more leading dots
                one_of(vec_of_erased![
                    // Three dots: ...[table]
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("DotSegment"),
                        Ref::new("DotSegment"),
                    ]),
                    // Two dots: ..[table]
                    Sequence::new(vec_of_erased![
                        Ref::new("DotSegment"),
                        Ref::new("DotSegment"),
                    ]),
                    // One dot: .[table]
                    Ref::new("DotSegment"),
                ]),
                // Table identifier (supports both naked/quoted and square-bracketed identifiers)
                one_of(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("QuotedIdentifierSegment"),
                ]),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Update TableReferenceSegment to support T-SQL table variables and dot-prefixed references
    // Temp tables are now handled as regular ObjectReferenceSegment since they use word tokens
    dialect.replace_grammar(
        "TableReferenceSegment",
        one_of(vec_of_erased![
            Ref::new("TsqlDotPrefixedReferenceSegment"),
            Ref::new("ObjectReferenceSegment"),
            Ref::new("TsqlVariableSegment"),
        ])
        .to_matchable(),
    );

    // Add CollationSegment for COLLATE clauses
    dialect.add([(
        "CollationSegment".into(),
        NodeMatcher::new(SyntaxKind::Identifier, |_| {
            Ref::new("SingleIdentifierGrammar").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Update TableExpressionSegment to include PIVOT/UNPIVOT, OPENJSON, and OPENROWSET
    dialect.replace_grammar(
        "TableExpressionSegment",
        one_of(vec_of_erased![
            Ref::new("ValuesClauseSegment"),
            Ref::new("BareFunctionSegment"),
            // T-SQL specific functions must come before generic FunctionSegment
            Ref::new("OpenRowSetSegment"), // OPENROWSET function (alias handled by FromExpressionElementSegment)
            Ref::new("OpenQuerySegment"),  // Add OPENQUERY support
            Ref::new("OpenDataSourceSegment"), // Add OPENDATASOURCE support
            Ref::new("FunctionSegment"),
            Ref::new("TableReferenceSegment"),
            Ref::new("OpenJsonSegment"),
            Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                Ref::new("SelectableGrammar"),
                Ref::new("MergeStatementSegment") // MERGE can be used in subquery with OUTPUT
            ])]),
            Sequence::new(vec_of_erased![
                Ref::new("TableReferenceSegment"),
                Ref::new("PivotUnpivotGrammar")
            ]),
            // Table-valued function calls (e.g., dbo.GetReports(123))
            Sequence::new(vec_of_erased![
                Ref::new("ObjectReferenceSegment"),
                Bracketed::new(vec_of_erased![
                    Ref::new("FunctionContentsGrammar").optional()
                ])
                .config(|this| this.parse_mode(ParseMode::Greedy))
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
                Ref::keyword("PAGLOCK"),
                Ref::keyword("TABLOCK"),
                Ref::keyword("TABLOCKX"),
                Ref::keyword("UPDLOCK"),
                Ref::keyword("XLOCK"),
                Ref::keyword("NOEXPAND"),
                // FORCESEEK with optional index hint
                Sequence::new(vec_of_erased![
                    Ref::keyword("FORCESEEK"),
                    // Optional index specification
                    Bracketed::new(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"),
                        // Optional column list
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "NakedIdentifierSegment"
                        )])])
                        .config(|this| this.optional())
                    ])
                    .config(|this| this.optional())
                ]),
                Ref::keyword("FORCESCAN"),
                Ref::keyword("HOLDLOCK"),
                Ref::keyword("SNAPSHOT"),
                // INDEX hint with parameter(s) - can specify multiple indexes
                Sequence::new(vec_of_erased![
                    Ref::keyword("INDEX"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("NakedIdentifierSegment")
                        ]
                    )])])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Override DropFunctionStatementSegment to support comma-delimited function names in T-SQL
    dialect.add([(
        "DropFunctionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropFunctionStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("FUNCTION"),
                Ref::new("IfExistsGrammar").optional(),
                Delimited::new(vec_of_erased![Ref::new("FunctionNameSegment")])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override BaseExpressionElementGrammar to prioritize CaseExpressionSegment over DatatypeSegment
    // The ANSI version includes DatatypeSegment which was matching "CASE" as a data type
    // Use add() with explicit precedence - this will override the base version
    dialect.add([(
        "BaseExpressionElementGrammar".into(),
        one_of(vec_of_erased![
            // CRITICAL: Put CaseExpressionSegment FIRST to ensure it matches before any data type parsing
            Ref::new("CaseExpressionSegment"),
            Ref::new("LiteralGrammar"),
            Ref::new("BareFunctionSegment"),
            Ref::new("IntervalExpressionSegment"),
            Ref::new("FunctionSegment"),
            Ref::new("ColumnReferenceSegment"),
            // Add TsqlVariableSegment for T-SQL variables like @variable
            Ref::new("TsqlVariableSegment"),
            Ref::new("ExpressionSegment"),
            Sequence::new(vec_of_erased![
                Ref::new("DatatypeSegment"),
                Ref::new("LiteralGrammar"),
            ])
        ])
        .config(|this| {
            // These terminators allow better performance by giving a signal
            // of a likely complete match if they come after a match.
            this.terminators = vec_of_erased![
                Ref::keyword("AS"),
                Ref::keyword("FROM"),
                Ref::keyword("WHERE"),
                Ref::keyword("ORDER"),
                Ref::keyword("GROUP"),
                Ref::keyword("HAVING"),
                Ref::keyword("UNION"),
                Ref::keyword("EXCEPT"),
                Ref::keyword("INTERSECT"),
                Ref::keyword("INTO"),
                Ref::keyword("SET"),
                Ref::keyword("VALUES"),
                Ref::keyword("WITH"),
                Ref::new("CommaSegment"),
                Ref::new("SemicolonSegment"),
                Ref::new("StartBracketSegment"),
                Bracketed::new(vec_of_erased![Ref::keyword("SELECT")]),
            ];
        })
        .to_matchable()
        .into(),
    )]);

    // DISABLED: Override Expression_C_Grammar to use T-SQL specific CASE expressions
    // Let T-SQL use ANSI's Expression_C_Grammar with overridden CaseExpressionSegment
    // dialect.add([(
    //     "Expression_C_Grammar".into(),
    //     one_of(vec![
    //         // Sequence for "EXISTS" with a bracketed selectable grammar
    //         Sequence::new(vec![
    //             Ref::keyword("EXISTS").to_matchable(),
    //             Bracketed::new(vec![Ref::new("SelectableGrammar").to_matchable()])
    //                 .to_matchable(),
    //         ])
    //         .to_matchable(),
    //         // Sequence for Expression_D_Grammar or T-SQL specific CaseExpressionSegment
    //         // followed by any number of TimeZoneGrammar
    //         Sequence::new(vec![
    //             one_of(vec![
    //                 Ref::new("Expression_D_Grammar").to_matchable(),
    //                 Ref::new("CaseExpressionSegment").to_matchable(),
    //             ])
    //             .to_matchable(),
    //             AnyNumberOf::new(vec![Ref::new("TimeZoneGrammar").to_matchable()])
    //                 .config(|this| this.optional())
    //                 .to_matchable(),
    //         ])
    //         .to_matchable(),
    //         Ref::new("ShorthandCastSegment").to_matchable(),
    //     ])
    //     .config(|this| this.terminators = vec_of_erased![Ref::new("CommaSegment")])
    //     .to_matchable()
    //     .into(),
    // )]);

    // Override Expression_D_Grammar to include T-SQL specific expressions like NEXT VALUE FOR
    dialect.add([(
        "Expression_D_Grammar".into(),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                // Add word-aware NEXT VALUE FOR first for word token contexts
                Ref::new("WordAwareNextValueForSegment"),
                // Add regular NEXT VALUE FOR for keyword contexts
                Ref::new("NextValueForSegment"),
                Ref::new("BareFunctionSegment"),
                Ref::new("FunctionSegment"),
                Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::new("ExpressionSegment"),
                    Ref::new("SelectableGrammar"),
                    Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                ])])
                .config(|this| this.parse_mode(ParseMode::Greedy)),
                Ref::new("SelectStatementSegment"),
                Ref::new("LiteralGrammar"),
                Ref::new("IntervalExpressionSegment"),
                Ref::new("TypedStructLiteralSegment"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("DatatypeSegment")
            ]),
            AnyNumberOf::new(vec_of_erased![Ref::new("ArrayAccessorSegment")])
        ])
        .to_matchable()
        .into(),
    )]);

    // Define PostTableExpressionGrammar to include T-SQL table hints and PIVOT/UNPIVOT
    dialect.add([(
        "PostTableExpressionGrammar".into(),
        one_of(vec_of_erased![
            // WITH (hints) syntax
            Ref::new("TableHintSegment"),
            // Simplified (hint) syntax - just bracketed hints without WITH
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "TableHintElement"
            )])]),
            // PIVOT/UNPIVOT
            Ref::new("PivotUnpivotStatementSegment"),
        ])
        .config(|this| this.optional())
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
            // Support both WITH OFFSET and OPENROWSET WITH column definitions
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                one_of(vec_of_erased![
                    // ANSI WITH OFFSET syntax
                    Sequence::new(vec_of_erased![
                        Ref::keyword("OFFSET"),
                        Ref::new("AliasExpressionSegment")
                    ]),
                    // OPENROWSET WITH column definitions syntax
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"), // Column name (can be bracketed)
                            Ref::new("DatatypeSegment"),         // Data type
                            // Optional COLLATE clause
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COLLATE"),
                                Ref::new("CollationSegment")
                            ])
                            .config(|this| this.optional()),
                            // Optional JSON path expression for JSON data
                            Sequence::new(vec_of_erased![
                                Ref::keyword("STRICT").optional(),
                                Ref::new("QuotedLiteralSegment")
                            ])
                            .config(|this| this.optional()),
                            // Handle erroneous trailing tokens (like numbers) in column definitions
                            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::new("SingleIdentifierGrammar")
                            ])])
                            .config(|this| this.max_times(5)) // Limit to prevent runaway parsing
                        ])
                    ])])
                ])
            ])
            .config(|this| this.optional()),
            // Alias can come either before or after WITH clause
            Ref::new("AliasExpressionSegment")
                .exclude(one_of(vec_of_erased![
                    Ref::new("FromClauseTerminatorGrammar"),
                    Ref::new("SamplingExpressionSegment"),
                    Ref::new("JoinLikeClauseGrammar"),
                    LookaheadExclude::new("WITH", "("), // Prevents WITH from being parsed as alias when followed by (
                    Ref::keyword("GO"), // Prevents GO from being parsed as alias (it's a batch separator)
                    Ref::keyword("FOR"), // Prevents FOR from being parsed as alias (FOR JSON/XML/BROWSE clauses)
                    Ref::keyword("OPTION") // Prevents OPTION from being parsed as alias (for query hints)
                ]))
                .optional(),
            Ref::new("SamplingExpressionSegment").optional(),
            Ref::new("PostTableExpressionGrammar").optional() // T-SQL table hints
        ])
        .to_matchable(),
    );

    // T-SQL Join Hints Grammar - now enabled for SQLFluff compatibility
    // T-SQL supports join hints: HASH, MERGE, LOOP
    // These can be combined with any join type
    // Examples: INNER HASH JOIN, LEFT OUTER MERGE JOIN, LOOP JOIN
    dialect.add([(
        "TsqlJoinHintGrammar".into(),
        one_of(vec_of_erased![
            Ref::keyword("HASH"),
            Ref::keyword("MERGE"),
            Ref::keyword("LOOP")
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific join type grammar with hints - now enabled for SQLFluff compatibility
    // T-SQL syntax: [join_type] [join_hint] JOIN
    // Examples: INNER HASH JOIN, FULL OUTER MERGE JOIN, LOOP JOIN
    dialect.add([(
        "TsqlJoinTypeKeywordsGrammar".into(),
        Sequence::new(vec_of_erased![
            // Optional join type - all combinations explicitly listed for robustness
            one_of(vec_of_erased![
                // Simple join types
                Ref::keyword("INNER"),
                Ref::keyword("LEFT"),
                Ref::keyword("RIGHT"),
                Ref::keyword("FULL"),
                // Explicit OUTER combinations
                Sequence::new(vec_of_erased![Ref::keyword("LEFT"), Ref::keyword("OUTER")]),
                Sequence::new(vec_of_erased![Ref::keyword("RIGHT"), Ref::keyword("OUTER")]),
                Sequence::new(vec_of_erased![Ref::keyword("FULL"), Ref::keyword("OUTER")])
            ])
            .config(|this| this.optional()),
            // Optional join hint (HASH, MERGE, LOOP)
            Ref::new("TsqlJoinHintGrammar").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific JoinClauseSegment with algorithm hints support
    // This replaces the ANSI JoinClauseSegment to support T-SQL JOIN hints
    // Examples: INNER HASH JOIN, FULL OUTER MERGE JOIN, LOOP JOIN
    dialect.add([(
        "JoinClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::JoinClause, |_| {
            one_of(vec_of_erased![
                // Standard JOIN with optional T-SQL hints
                Sequence::new(vec_of_erased![
                    Ref::new("TsqlJoinTypeKeywordsGrammar").optional(),
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
                // Natural JOIN (fallback to ANSI)
                Sequence::new(vec_of_erased![
                    Ref::new("NaturalJoinKeywordsGrammar"),
                    Ref::new("JoinKeywordsGrammar"),
                    MetaSegment::indent(),
                    Ref::new("FromExpressionElementSegment"),
                    MetaSegment::dedent(),
                ]),
                // Extended Natural JOIN (fallback to ANSI)
                Sequence::new(vec_of_erased![
                    Ref::new("ExtendedNaturalJoinKeywordsGrammar"),
                    MetaSegment::indent(),
                    Ref::new("FromExpressionElementSegment"),
                    MetaSegment::dedent(),
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Enable NATURAL JOIN support for T-SQL (inherits ANSI implementation)

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

    // T-SQL Nested JOIN Grammar - enables parsing of nested JOIN structures
    // This is critical for parsing constructs like:
    // FROM table1 LEFT JOIN table2 LEFT JOIN table3 ON ... ON ...
    // Based on SQLFluff's implementation which uses recursive JOIN structures
    // Override the empty NestedJoinGrammar from ANSI dialect
    dialect.add([(
        "NestedJoinGrammar".into(),
        one_of(vec_of_erased![
            // Self-referencing JoinClauseSegment allows recursion
            Ref::new("JoinClauseSegment"),
            // Also support APPLY clauses in nested contexts
            Ref::new("ApplyClauseSegment")
        ])
        .to_matchable()
        .into(),
    )]);

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

    // Define JoinLikeClauseGrammar for T-SQL to include both regular JOINs and APPLY
    // This allows both JOIN clauses and APPLY to be used wherever joins are allowed
    dialect.add([(
        "JoinLikeClauseGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("JoinClauseSegment"),
            Ref::new("ApplyClauseSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // Override FromExpressionSegment to ensure T-SQL join patterns are properly recognized
    // This is needed because T-SQL has different join syntax (e.g., FULL OUTER MERGE JOIN)
    dialect.replace_grammar(
        "FromExpressionSegment",
        optionally_bracketed(vec_of_erased![Sequence::new(vec_of_erased![
            MetaSegment::indent(),
            one_of(vec_of_erased![
                Ref::new("FromExpressionElementSegment"),
                Bracketed::new(vec_of_erased![Ref::new("FromExpressionSegment")])
            ])
            .config(|this| this.terminators = vec_of_erased![
                Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
                Ref::keyword("WHERE"),
                Ref::keyword("HAVING"),
                Ref::keyword("FOR"),    // Fix for FOR JSON/XML/BROWSE clauses
                Ref::keyword("OPTION"), // T-SQL OPTION clause
            ]),
            MetaSegment::dedent(),
            Conditional::new(MetaSegment::indent()).indented_joins(),
            AnyNumberOf::new(vec_of_erased![
                // Use JoinLikeClauseGrammar which includes both JoinClauseSegment and ApplyClauseSegment
                Ref::new("JoinLikeClauseGrammar")
            ])
            .config(|this| {
                this.optional();
                this.terminators = vec_of_erased![
                    Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
                    Sequence::new(vec_of_erased![Ref::keyword("GROUP"), Ref::keyword("BY")]),
                    Ref::keyword("LIMIT"),
                    Ref::keyword("WHERE"),
                    Ref::keyword("PIVOT"),
                    Ref::keyword("UNPIVOT"),
                    Ref::keyword("FOR"),
                    Ref::keyword("OPTION"),
                    Ref::new("SetOperatorSegment"),
                    Ref::new("WithNoSchemaBindingClauseSegment"),
                    Ref::new("DelimiterGrammar")
                ];
            }),
            Conditional::new(MetaSegment::dedent()).indented_joins(),
            Ref::new("PostTableExpressionGrammar").optional()
        ])])
        .to_matchable(),
    );

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

    // Remove custom OverClauseSegment definition - rely on ANSI inheritance
    // The ANSI version should work with proper WindowSpecificationSegment

    // Override PostFunctionGrammar to include WITHIN GROUP and support sequences
    dialect.add([(
        "PostFunctionGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::new("WithinGroupClauseSegment").optional(),
            Ref::new("OverClauseSegment").optional(),
            Ref::new("FilterClauseGrammar").optional()
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

    // Override CreateSequenceOptionsSegment to support T-SQL AS datatype clause
    dialect.replace_grammar(
        "CreateSequenceOptionsSegment",
        one_of(vec_of_erased![
            // AS datatype (T-SQL specific, must come first)
            Sequence::new(vec_of_erased![
                Ref::keyword("AS"),
                Ref::new("DatatypeSegment")
            ]),
            // START WITH
            Sequence::new(vec_of_erased![
                Ref::keyword("START"),
                Ref::keyword("WITH"),
                Ref::new("NumericLiteralSegment")
            ]),
            // INCREMENT BY
            Sequence::new(vec_of_erased![
                Ref::keyword("INCREMENT"),
                Ref::keyword("BY"),
                Ref::new("NumericLiteralSegment")
            ]),
            // MINVALUE / NO MINVALUE
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("MINVALUE"),
                    Ref::new("NumericLiteralSegment")
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MINVALUE")])
            ]),
            // MAXVALUE / NO MAXVALUE
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("MAXVALUE"),
                    Ref::new("NumericLiteralSegment")
                ]),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("MAXVALUE")])
            ]),
            // CACHE
            Sequence::new(vec_of_erased![
                Ref::keyword("CACHE"),
                Ref::new("NumericLiteralSegment").optional()
            ]),
            // CYCLE / NO CYCLE
            one_of(vec_of_erased![
                Ref::keyword("CYCLE"),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("CYCLE")])
            ]),
            // ORDER / NO ORDER (T-SQL specific)
            one_of(vec_of_erased![
                Ref::keyword("ORDER"),
                Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("ORDER")])
            ])
        ])
        .to_matchable(),
    );

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
                    // NOT NULL / NULL [NOT FOR REPLICATION]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NOT").optional(),
                        Ref::keyword("NULL"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("NOT"),
                            Ref::keyword("FOR"),
                            Ref::keyword("REPLICATION")
                        ])
                        .config(|this| this.optional())
                    ]),
                    // CHECK constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CHECK"),
                        Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                    ]),
                    // DEFAULT constraint
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DEFAULT"),
                        one_of(vec_of_erased![
                            // NEXT VALUE FOR sequence_name
                            Sequence::new(vec_of_erased![
                                Ref::keyword("NEXT"),
                                Ref::keyword("VALUE"),
                                Ref::keyword("FOR"),
                                Ref::new("ObjectReferenceSegment") // sequence name
                            ]),
                            // Standard default values
                            Ref::new("ColumnConstraintDefaultGrammar"),
                        ]),
                    ]),
                    Ref::new("PrimaryKeyGrammar"),
                    Ref::new("UniqueKeyGrammar"),
                    Ref::new("IdentityConstraintGrammar"), // T-SQL IDENTITY
                    Ref::new("AutoIncrementGrammar"),      // Keep ANSI AUTO_INCREMENT
                    Ref::new("ReferenceDefinitionGrammar"),
                    // Inline FOREIGN KEY without constraint name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOREIGN"),
                        Ref::keyword("KEY"),
                        Ref::keyword("REFERENCES"),
                        Ref::new("TableReferenceSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ColumnReferenceSegment"
                        )])])
                    ]),
                    // Simple UNIQUE without parentheses (for inline column constraint)
                    Ref::keyword("UNIQUE"),
                    Ref::new("CommentClauseSegment"),
                    // COLLATE
                    Sequence::new(vec_of_erased![
                        Ref::keyword("COLLATE"),
                        Ref::new("CollationReferenceSegment"),
                    ]),
                    // FILESTREAM
                    Ref::keyword("FILESTREAM"),
                    // MASKED WITH (FUNCTION = 'function_name')
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MASKED"),
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![
                            Ref::keyword("FUNCTION"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment")
                        ])
                    ]),
                    // GENERATED ALWAYS AS ROW START/END HIDDEN
                    Sequence::new(vec_of_erased![
                        Ref::keyword("GENERATED"),
                        Ref::keyword("ALWAYS"),
                        Ref::keyword("AS"),
                        Ref::keyword("ROW"),
                        one_of(vec_of_erased![Ref::keyword("START"), Ref::keyword("END")]),
                        Ref::keyword("HIDDEN").optional()
                    ]),
                    // ENCRYPTED WITH (encryption parameters)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ENCRYPTED"),
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                one_of(vec_of_erased![
                                    Ref::keyword("COLUMN_ENCRYPTION_KEY"),
                                    Ref::keyword("ENCRYPTION_TYPE"),
                                    Ref::keyword("ALGORITHM")
                                ]),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::new("QuotedLiteralSegment"),
                                    Ref::new("NakedIdentifierSegment")
                                ])
                            ])
                        ])])
                    ]),
                ]),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override ColumnConstraintDefaultGrammar to support T-SQL expressions
    dialect.add([(
        "ColumnConstraintDefaultGrammar".into(),
        one_of(vec_of_erased![
            Ref::new("ShorthandCastSegment"),
            Ref::new("LiteralGrammar"),
            Ref::new("FunctionSegment"),
            Ref::new("BareFunctionSegment"),
            // Add ExpressionSegment for complex expressions like ((-1))
            Ref::new("ExpressionSegment"),
            // Add bracketed expressions
            Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
        ])
        .to_matchable()
        .into(),
    )]);

    // Override PrimaryKeyGrammar to support CLUSTERED/NONCLUSTERED
    // Note: Column list is handled by TableConstraintSegment, not here
    dialect.add([(
        "PrimaryKeyGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("PRIMARY"),
            Ref::keyword("KEY"),
            one_of(vec_of_erased![
                Ref::keyword("CLUSTERED"),
                Ref::keyword("NONCLUSTERED")
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // Override UniqueKeyGrammar to support CLUSTERED/NONCLUSTERED and column lists
    dialect.add([(
        "UniqueKeyGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("UNIQUE"),
            one_of(vec_of_erased![
                Ref::keyword("CLUSTERED"),
                Ref::keyword("NONCLUSTERED")
            ])
            .config(|this| this.optional()),
            // Optional column list with ASC/DESC for T-SQL
            // This is truly optional to allow standalone UNIQUE constraints
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                        .config(|this| this.optional())
                ])
            ])])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // Override TableConstraintSegment to support T-SQL specific syntax
    dialect.replace_grammar(
        "TableConstraintSegment",
        NodeMatcher::new(SyntaxKind::TableConstraint, |_| {
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("CONSTRAINT"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("UNIQUE"),
                        one_of(vec_of_erased![
                            Ref::keyword("CLUSTERED"),
                            Ref::keyword("NONCLUSTERED")
                        ])
                        .config(|this| this.optional()),
                        // T-SQL supports column list with ASC/DESC
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|this| this.optional())
                            ])
                        ])])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("PrimaryKeyGrammar"),
                        // T-SQL supports column list with ASC/DESC
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|this| this.optional())
                            ])
                        ])])
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::new("ForeignKeyGrammar"),
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        Ref::new("ReferenceDefinitionGrammar")
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Override ReferenceDefinitionGrammar to support optional FOREIGN KEY prefix
    dialect.add([(
        "ReferenceDefinitionGrammar".into(),
        Sequence::new(vec_of_erased![
            // Optional FOREIGN KEY keywords
            Sequence::new(vec_of_erased![Ref::keyword("FOREIGN"), Ref::keyword("KEY")])
                .config(|this| this.optional()),
            Ref::keyword("REFERENCES"),
            Ref::new("TableReferenceSegment"), // Table reference
            // Optional column list in parentheses
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "ColumnReferenceSegment"
            )])])
            .config(|this| this.optional()),
            // Optional referential actions
            AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                one_of(vec_of_erased![
                    Ref::keyword("DELETE"),
                    Ref::keyword("UPDATE")
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("CASCADE"),
                    Ref::keyword("RESTRICT"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("SET"),
                        one_of(vec_of_erased![
                            Ref::keyword("NULL"),
                            Ref::keyword("DEFAULT")
                        ])
                    ]),
                    Sequence::new(vec_of_erased![Ref::keyword("NO"), Ref::keyword("ACTION")])
                ])
            ])])
            .config(|this| this.optional())
        ])
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

    // Add BracketedColumnDefinitionListGrammar for table definitions
    dialect.add([(
        "BracketedColumnDefinitionListGrammar".into(),
        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
            vec_of_erased![
                Ref::new("TableConstraintSegment"),
                Ref::new("ColumnDefinitionSegment"),
                // PERIOD FOR SYSTEM_TIME for temporal tables
                Sequence::new(vec_of_erased![
                    Ref::keyword("PERIOD"),
                    Ref::keyword("FOR"),
                    Ref::keyword("SYSTEM_TIME"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("CommaSegment"),
                        Ref::new("SingleIdentifierGrammar")
                    ])
                ])
            ]
        )])])
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
                    // CREATE/ALTER as keyword or word token
                    one_of(vec_of_erased![
                        Ref::keyword("CREATE"),
                        StringParser::new("CREATE", SyntaxKind::Word),
                        Ref::keyword("ALTER"),
                        StringParser::new("ALTER", SyntaxKind::Word),
                        Sequence::new(vec_of_erased![
                            one_of(vec_of_erased![
                                Ref::keyword("CREATE"),
                                StringParser::new("CREATE", SyntaxKind::Word)
                            ]),
                            one_of(vec_of_erased![
                                Ref::keyword("OR"),
                                StringParser::new("OR", SyntaxKind::Word)
                            ]),
                            one_of(vec_of_erased![
                                Ref::keyword("ALTER"),
                                StringParser::new("ALTER", SyntaxKind::Word)
                            ])
                        ])
                    ]),
                    // PROC/PROCEDURE as keyword or word token
                    one_of(vec_of_erased![
                        Ref::keyword("PROC"),
                        StringParser::new("PROC", SyntaxKind::Word),
                        Ref::keyword("PROCEDURE"),
                        StringParser::new("PROCEDURE", SyntaxKind::Word)
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
                    one_of(vec_of_erased![
                        Ref::keyword("AS"),
                        // Also accept AS as word token - though this is unusual
                        StringParser::new("AS", SyntaxKind::Keyword)
                    ]),
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
            "ProcedureParameterGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::new("ParameterNameSegment"),
                Ref::new("DatatypeSegment"),
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
                // PRIORITY 1: Word-aware BEGIN...END block first (complete block structure)
                Ref::new("WordAwareBeginEndBlockSegment"),
                // PRIORITY 2: Regular BEGIN...END block
                Ref::new("BeginEndBlockSegment"),
                // PRIORITY 3: For procedures with word tokens after AS, try specific word-aware parsers
                Ref::new("WordAwareIfStatementSegment"),
                // PRIORITY 4: Word-based parsing fallback (for procedure bodies with word tokens)
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("WordAwareStatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.min_times(1);
                    this.parse_mode = ParseMode::Greedy;
                    this.terminators = vec_of_erased![Ref::new("BatchSeparatorGrammar")];
                }),
                // Single statement or block (when keywords are properly lexed)
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
    // NOTE: Removed global ExpressionSegment override as it was too restrictive
    // WordAwareExpressionSegment is now only used in specific targeted contexts
    // rather than replacing all expression parsing globally

    // We need to be careful to ensure CASE expressions can be parsed
    //
    // IMPORTANT: Do NOT add custom grammars here. Instead, override AliasExpressionSegment
    // to support T-SQL's alias = expression syntax.
    // For now, just use ANSI's SelectClauseElementSegment which properly handles CASE

    // Override SelectClauseElementSegment to support T-SQL variable assignment AND prioritize CASE expressions
    dialect.add([(
        "SelectClauseElementSegment".into(),
        NodeMatcher::new(SyntaxKind::SelectClauseElement, |_| {
            one_of(vec_of_erased![
                // T-SQL variable assignment: @var = expression or @var += expression
                Sequence::new(vec_of_erased![
                    Ref::new("TsqlVariableSegment"),
                    Ref::new("AssignmentOperatorSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                // Standalone T-SQL variable: @var
                Sequence::new(vec_of_erased![
                    Ref::new("TsqlVariableSegment"),
                    Ref::new("AliasExpressionSegment").optional(),
                ]),
                // Standard ANSI select clause elements
                Ref::new("WildcardExpressionSegment"),
                // CRITICAL: Try CaseExpressionSegment FIRST to handle T-SQL CASE expressions correctly
                Sequence::new(vec_of_erased![
                    Ref::new("CaseExpressionSegment"),
                    Ref::new("AliasExpressionSegment").optional(),
                ]),
                // Then fallback to standard BaseExpressionElementGrammar for other expressions
                Sequence::new(vec_of_erased![
                    Ref::new("BaseExpressionElementGrammar"),
                    Ref::new("AliasExpressionSegment").optional(),
                ]),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override SelectClause to handle CASE expressions properly by using a custom parsing approach
    // that doesn't terminate on END when inside a CASE expression
    dialect.replace_grammar(
        "SelectClauseSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("SELECT"),
                // Also accept SELECT as word token in T-SQL procedure bodies
                StringParser::new("SELECT", SyntaxKind::Keyword)
            ]),
            Ref::new("SelectClauseModifierSegment").optional(),
            MetaSegment::indent(),
            // Custom T-SQL select clause parsing that can handle edge cases like "column@variable"
            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                // Normal comma-separated elements
                Sequence::new(vec_of_erased![
                    Ref::new("SelectClauseElementSegment"),
                    Ref::new("CommaSegment")
                ]),
                // Final element without comma
                Ref::new("SelectClauseElementSegment")
            ])])
            .config(|this| {
                this.min_times(1);
                // Allow natural boundaries at T-SQL variables
                this.terminators = vec_of_erased![
                    Ref::new("TsqlVariableSegment"),
                    Ref::keyword("FROM"),
                    Ref::keyword("WHERE")
                ];
            }),
        ])
        .terminators(vec_of_erased![
            // Use all standard terminators except END
            Ref::keyword("FROM"),
            Ref::keyword("WHERE"),
            Ref::keyword("INTO"),
            Sequence::new(vec_of_erased![Ref::keyword("ORDER"), Ref::keyword("BY")]),
            Ref::keyword("LIMIT"),
            Ref::keyword("OVERLAPS"),
            Ref::new("SetOperatorSegment"),
            Ref::keyword("FETCH"),
            Ref::keyword("FOR"),
            Ref::new("BatchDelimiterGrammar"),
            Ref::keyword("OPTION"),
            // Statement keywords that should terminate SELECT clause
            Ref::keyword("CREATE"),
            Ref::keyword("DROP"),
            Ref::keyword("ALTER"),
            Ref::keyword("INSERT"),
            Ref::keyword("UPDATE"),
            Ref::keyword("DELETE"),
            Ref::keyword("DECLARE"),
            Ref::keyword("SET"),
            Ref::keyword("BEGIN"),
            // Exclude END here - it will be handled differently
            Ref::keyword("IF"),
            Ref::keyword("WHILE"),
            Ref::keyword("EXEC"),
            Ref::keyword("EXECUTE"),
        ])
        .config(|this| {
            this.parse_mode(ParseMode::GreedyOnceStarted);
        })
        .to_matchable(),
    );

    // Add T-SQL specific WithCheckOptionSegment - must be defined before use
    dialect.add([(
        "WithCheckOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::WithCheckOption, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("CHECK"),
                Ref::keyword("OPTION")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override SelectStatementSegment to add FOR clause and OPTION clause after ORDER BY
    dialect.replace_grammar(
        "SelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec_of_erased![
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("FetchClauseSegment").optional(),
                Ref::new("LimitClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional(),
                // T-SQL specific: FOR JSON/XML/BROWSE clause
                Ref::new("ForClauseSegment")
                    .exclude(LookaheadExclude::new("FOR", "SYSTEM_TIME"))
                    .optional(),
                // T-SQL specific: OPTION clause for query hints
                Ref::new("OptionClauseSegment").optional()
            ]),
            None,
            None,
            None,
            vec_of_erased![
                Ref::new("SetOperatorSegment"),
                // Exclude WITH CHECK OPTION from being consumed by SELECT terminating clauses
                // This allows CREATE VIEW to handle WITH CHECK OPTION properly
                Ref::new("WithNoSchemaBindingClauseSegment")
                    .exclude(LookaheadExclude::new("WITH", "CHECK")),
                Ref::new("WithDataClauseSegment").exclude(LookaheadExclude::new("WITH", "CHECK")),
                // T-SQL specific: GO batch delimiter should terminate statements
                Ref::new("BatchDelimiterGrammar"),
                // Add common statement keywords as terminators to prevent them from being consumed
                Ref::keyword("DROP"),
                Ref::keyword("CREATE"),
                Ref::keyword("ALTER"),
                Ref::keyword("INSERT"),
                Ref::keyword("UPDATE"),
                Ref::keyword("DELETE"),
                // NOTE: MERGE removed from terminators to allow MERGE statements to parse
                // Ref::keyword("MERGE"),
                Ref::keyword("TRUNCATE"),
                Ref::keyword("DECLARE"),
                Ref::keyword("SET"),
                Ref::keyword("PRINT"),
                Ref::keyword("IF"),
                Ref::keyword("WHILE"),
                Ref::keyword("BEGIN"),
                Ref::keyword("EXEC"),
                Ref::keyword("EXECUTE"),
                Ref::keyword("GRANT"),
                Ref::keyword("DENY"),
                Ref::keyword("REVOKE"),
                Ref::keyword("USE"),
                Ref::keyword("BULK"),
                Ref::keyword("WAITFOR"),
                Ref::keyword("GOTO"),
                Ref::keyword("RETURN"),
                Ref::keyword("THROW"),
                Ref::keyword("RAISERROR"),
                Ref::keyword("TRY"),
                Ref::keyword("OPEN"),
                Ref::keyword("CLOSE"),
                // Note: FETCH removed as terminator because it's used in OFFSET/FETCH clause
                // and should only terminate when used as cursor operation
                Ref::keyword("DEALLOCATE"),
                Ref::keyword("DISABLE"),
                Ref::keyword("ENABLE"),
                Ref::keyword("RECONFIGURE"),
                Ref::keyword("BACKUP"),
                Ref::keyword("RESTORE"),
                Ref::keyword("BREAK"),
                Ref::keyword("CONTINUE"),
                Ref::keyword("DBCC"),
                Ref::keyword("RENAME"),
                // CRITICAL: Add ELSE as terminator for IF...ELSE statements (both keyword and word tokens)
                Ref::keyword("ELSE"),
                StringParser::new("ELSE", SyntaxKind::Word)
            ],
            true,
        ),
    );

    // Also add GO as a statement terminator for UnorderedSelectStatementSegment
    // and add OPTION clause support
    dialect.replace_grammar(
        "UnorderedSelectStatementSegment",
        ansi::get_unordered_select_statement_segment_grammar().copy(
            Some(vec_of_erased![
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("FetchClauseSegment").optional(),
                Ref::new("LimitClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional(),
                // T-SQL specific: FOR JSON/XML/BROWSE clause
                Ref::new("ForClauseSegment")
                    .exclude(LookaheadExclude::new("FOR", "SYSTEM_TIME"))
                    .optional(),
                // T-SQL specific: OPTION clause for query hints
                Ref::new("OptionClauseSegment").optional()
            ]),
            None,
            None,
            None,
            vec_of_erased![
                Ref::new("SetOperatorSegment"),
                // T-SQL specific: GO batch delimiter should terminate statements
                Ref::new("BatchDelimiterGrammar"),
                // Add common statement keywords as terminators to prevent them from being consumed
                Ref::keyword("DROP"),
                Ref::keyword("CREATE"),
                Ref::keyword("ALTER"),
                Ref::keyword("INSERT"),
                Ref::keyword("UPDATE"),
                Ref::keyword("DELETE"),
                // NOTE: MERGE removed from terminators to allow MERGE statements to parse
                // Ref::keyword("MERGE"),
                Ref::keyword("TRUNCATE"),
                Ref::keyword("DECLARE"),
                Ref::keyword("SET"),
                Ref::keyword("PRINT"),
                Ref::keyword("IF"),
                Ref::keyword("WHILE"),
                Ref::keyword("BEGIN"),
                Ref::keyword("EXEC"),
                Ref::keyword("EXECUTE"),
                Ref::keyword("GRANT"),
                Ref::keyword("DENY"),
                Ref::keyword("REVOKE"),
                Ref::keyword("USE"),
                Ref::keyword("BULK"),
                Ref::keyword("WAITFOR"),
                Ref::keyword("GOTO"),
                Ref::keyword("RETURN"),
                Ref::keyword("THROW"),
                Ref::keyword("RAISERROR"),
                Ref::keyword("TRY"),
                Ref::keyword("OPEN"),
                Ref::keyword("CLOSE"),
                // Note: FETCH removed as terminator because it's used in OFFSET/FETCH clause
                // and should only terminate when used as cursor operation
                Ref::keyword("DEALLOCATE"),
                Ref::keyword("DISABLE"),
                Ref::keyword("ENABLE"),
                Ref::keyword("RECONFIGURE"),
                Ref::keyword("BACKUP"),
                Ref::keyword("RESTORE"),
                Ref::keyword("BREAK"),
                Ref::keyword("CONTINUE"),
                Ref::keyword("DBCC"),
                Ref::keyword("RENAME"),
                // T-SQL specific: WITH CHECK OPTION terminator for CREATE VIEW
                Ref::new("WithCheckOptionSegment")
            ],
            true,
        ),
    );

    // Override SetExpressionSegment to include OPTION clause after UNION/EXCEPT/INTERSECT
    dialect.replace_grammar(
        "SetExpressionSegment",
        NodeMatcher::new(SyntaxKind::SetExpression, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("NonSetSelectableGrammar"),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("SetOperatorSegment"),
                    Ref::new("NonSetSelectableGrammar"),
                ])])
                .config(|this| this.min_times(1)),
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("LimitClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional(),
                // T-SQL specific: OPTION clause for query hints
                Ref::new("OptionClauseSegment").optional()
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Add T-SQL specific WithCheckOptionSegment
    dialect.add([(
        "WithCheckOptionSegment".into(),
        NodeMatcher::new(SyntaxKind::WithCheckOption, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Ref::keyword("CHECK"),
                Ref::keyword("OPTION")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add ResultSetColumnDefinitionSegment for simple column definitions in WITH RESULT SETS
    // This is much simpler than ColumnDefinitionSegment which includes IDENTITY, FILESTREAM, etc.
    dialect.add([(
        "ResultSetColumnDefinitionSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
            Sequence::new(vec_of_erased![
                // Column name (can be naked or bracketed identifier)
                one_of(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("QuotedIdentifierSegment")
                ]),
                // Data type (use DatatypeSegment for T-SQL specific types like INT)
                Ref::new("DatatypeSegment"),
                // Optional NULL/NOT NULL constraint
                one_of(vec_of_erased![
                    Ref::keyword("NULL"),
                    Sequence::new(vec_of_erased![Ref::keyword("NOT"), Ref::keyword("NULL")])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Create a custom grammar for SELECT statements within CREATE VIEW
    // This ensures WITH CHECK OPTION is not consumed by the SELECT parser
    dialect.add([(
        "CreateViewSelectableGrammar".into(),
        one_of(vec_of_erased![
            // CTE with SELECT - exclude WITH CHECK pattern
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH").exclude(LookaheadExclude::new("WITH", "CHECK")),
                Ref::keyword("RECURSIVE").optional(),
                Conditional::new(MetaSegment::indent()).indented_ctes(),
                Delimited::new(vec_of_erased![Ref::new("CTEDefinitionSegment")]).config(|this| {
                    this.terminators = vec_of_erased![Ref::keyword("SELECT")];
                    this.allow_trailing();
                }),
                Conditional::new(MetaSegment::dedent()).indented_ctes(),
                Ref::new("NonWithSelectableGrammar"),
            ]),
            // Regular SELECT without CTE
            Ref::new("NonWithSelectableGrammar"),
            // Bracketed selectable
            Bracketed::new(vec_of_erased![Ref::new("CreateViewSelectableGrammar")]),
        ])
        .to_matchable()
        .into(),
    )]);

    // Override CREATE VIEW to support CREATE OR ALTER VIEW
    dialect.replace_grammar(
        "CreateViewStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateViewStatement, |_| {
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
                Ref::keyword("VIEW"),
                Ref::new("TableReferenceSegment"),
                // Optional column list
                Ref::new("BracketedColumnReferenceListGrammar").optional(),
                // T-SQL specific view options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Delimited::new(vec_of_erased![
                        Ref::keyword("SCHEMABINDING"),
                        one_of(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("VIEW"),
                                Ref::keyword("METADATA")
                            ]),
                            Ref::keyword("VIEW_METADATA")
                        ]),
                        Ref::keyword("ENCRYPTION")
                    ])
                ])
                .config(|this| this.optional()),
                Ref::keyword("AS"),
                // Parse the SELECT statement, but lookahead to preserve WITH CHECK OPTION
                Sequence::new(vec_of_erased![
                    optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]).config(
                        |this| {
                            this.terminators = vec_of_erased![Ref::new("WithCheckOptionSegment")];
                        }
                    ),
                    // WITH CHECK OPTION at the end using proper segment
                    Ref::new("WithCheckOptionSegment").optional()
                ])
            ])
            .to_matchable()
        })
        .to_matchable(),
    );
    // T-SQL CREATE FUNCTION support with CREATE OR ALTER
    // NOTE: This is overridden later by replace_grammar
    dialect.add([(
        "CreateFunctionStatementSegment_UNUSED".into(),
        NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_| {
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
                Ref::keyword("FUNCTION"),
                Ref::new("ObjectReferenceSegment"),
                Ref::new("FunctionParameterListGrammar"),
                Ref::keyword("RETURNS"),
                one_of(vec_of_erased![
                    // Table-valued function
                    Sequence::new(vec_of_erased![
                        optionally_bracketed(vec_of_erased![Ref::new("TsqlVariableSegment")]),
                        Ref::keyword("TABLE"),
                        // Optional table definition for multi-statement table-valued functions
                        Ref::new("BracketedColumnDefinitionListGrammar").optional()
                    ]),
                    // Scalar function
                    Ref::new("DatatypeSegment")
                ]),
                // Function options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Delimited::new(vec_of_erased![
                        Ref::keyword("SCHEMABINDING"),
                        Ref::keyword("ENCRYPTION"),
                        Ref::new("ExecuteAsClauseGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RETURNS"),
                            Ref::keyword("NULL"),
                            Ref::keyword("ON"),
                            Ref::keyword("NULL"),
                            Ref::keyword("INPUT")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CALLED"),
                            Ref::keyword("ON"),
                            Ref::keyword("NULL"),
                            Ref::keyword("INPUT")
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Function body
                one_of(vec_of_erased![
                    // Inline table-valued function
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::keyword("RETURN"),
                        one_of(vec_of_erased![
                            Ref::new("SelectStatementSegment"),
                            // Handle RETURN ( SELECT ... ) pattern
                            Bracketed::new(vec_of_erased![Ref::new("SelectStatementSegment")])
                        ])
                    ]),
                    // Multi-statement function with BEGIN...END block
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::new("BeginEndBlockSegment")
                    ]),
                    // External CLR function
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::keyword("EXTERNAL"),
                        Ref::keyword("NAME"),
                        Ref::new("ObjectReferenceSegment")
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL ALTER FUNCTION statement
    dialect.add([(
        "AlterFunctionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::AlterFunctionStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("ALTER"),
                Ref::keyword("FUNCTION"),
                Ref::new("ObjectReferenceSegment"),
                Ref::new("FunctionParameterListGrammar"),
                Ref::keyword("RETURNS"),
                one_of(vec_of_erased![
                    // Table-valued function
                    Sequence::new(vec_of_erased![
                        optionally_bracketed(vec_of_erased![Ref::new("TsqlVariableSegment")]),
                        Ref::keyword("TABLE"),
                        // Optional table definition for multi-statement table-valued functions
                        Ref::new("BracketedColumnDefinitionListGrammar").optional()
                    ]),
                    // Scalar function
                    Ref::new("DatatypeSegment")
                ]),
                // Function options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Delimited::new(vec_of_erased![
                        Ref::keyword("SCHEMABINDING"),
                        Ref::keyword("ENCRYPTION"),
                        Ref::new("ExecuteAsClauseGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RETURNS"),
                            Ref::keyword("NULL"),
                            Ref::keyword("ON"),
                            Ref::keyword("NULL"),
                            Ref::keyword("INPUT")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CALLED"),
                            Ref::keyword("ON"),
                            Ref::keyword("NULL"),
                            Ref::keyword("INPUT")
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Function body
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    // Single-statement table-valued function
                    Ref::new("SelectableGrammar"),
                    // Multi-statement function
                    Ref::new("BeginEndBlockSegment"),
                    // Single expression
                    Ref::new("ExpressionSegment")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE OR ALTER FUNCTION statement
    dialect.add([(
        "CreateOrAlterFunctionStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("OR"),
                Ref::keyword("ALTER"),
                Ref::keyword("FUNCTION"),
                Ref::new("ObjectReferenceSegment"),
                Ref::new("FunctionParameterListGrammar"),
                Ref::keyword("RETURNS"),
                one_of(vec_of_erased![
                    // Table-valued function
                    Sequence::new(vec_of_erased![
                        optionally_bracketed(vec_of_erased![Ref::new("TsqlVariableSegment")]),
                        Ref::keyword("TABLE"),
                        // Optional table definition for multi-statement table-valued functions
                        Ref::new("BracketedColumnDefinitionListGrammar").optional()
                    ]),
                    // Scalar function
                    Ref::new("DatatypeSegment")
                ]),
                // Function options
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Delimited::new(vec_of_erased![
                        Ref::keyword("SCHEMABINDING"),
                        Ref::keyword("ENCRYPTION"),
                        Ref::new("ExecuteAsClauseGrammar"),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("RETURNS"),
                            Ref::keyword("NULL"),
                            Ref::keyword("ON"),
                            Ref::keyword("NULL"),
                            Ref::keyword("INPUT")
                        ]),
                        Sequence::new(vec_of_erased![
                            Ref::keyword("CALLED"),
                            Ref::keyword("ON"),
                            Ref::keyword("NULL"),
                            Ref::keyword("INPUT")
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Function body
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    // Single-statement table-valued function
                    Ref::new("SelectableGrammar"),
                    // Multi-statement function
                    Ref::new("BeginEndBlockSegment"),
                    // Single expression
                    Ref::new("ExpressionSegment")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // T-SQL CREATE SCHEMA with AUTHORIZATION support
    dialect.replace_grammar(
        "CreateSchemaStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateSchemaStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("SCHEMA"),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("SchemaReferenceSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AUTHORIZATION"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL CREATE ROLE with AUTHORIZATION support
    dialect.replace_grammar(
        "CreateRoleStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateRoleStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("CREATE"),
                Ref::keyword("ROLE"),
                Ref::new("RoleReferenceSegment"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("AUTHORIZATION"),
                    Ref::new("ObjectReferenceSegment")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL CREATE TABLE with Azure Synapse Analytics support
    dialect.replace_grammar(
        "CreateTableStatementSegment",
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec_of_erased![
                // CREATE as keyword or word token
                one_of(vec_of_erased![
                    Ref::keyword("CREATE"),
                    StringParser::new("CREATE", SyntaxKind::Word)
                ]),
                // TABLE as keyword or word token
                one_of(vec_of_erased![
                    Ref::keyword("TABLE"),
                    StringParser::new("TABLE", SyntaxKind::Word)
                ]),
                Ref::new("IfNotExistsGrammar").optional(),
                Ref::new("TableReferenceSegment"),
                one_of(vec_of_erased![
                    // Regular CREATE TABLE with column definitions (not graph tables)
                    Sequence::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![one_of(vec_of_erased![
                                // Word-aware column definition for cases with word tokens
                                Ref::new("WordAwareColumnDefinitionSegment"),
                                Ref::new("ColumnDefinitionSegment"),
                                Ref::new("TableConstraintSegment"),
                                // T-SQL Graph: CONNECTION constraint for edge tables
                                Ref::new("ConnectionConstraintSegment"),
                                // PERIOD FOR SYSTEM_TIME for temporal tables
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("PERIOD"),
                                    Ref::keyword("FOR"),
                                    Ref::keyword("SYSTEM_TIME"),
                                    Bracketed::new(vec_of_erased![
                                        Ref::new("SingleIdentifierGrammar"),
                                        Ref::new("CommaSegment"),
                                        Ref::new("SingleIdentifierGrammar")
                                    ])
                                ])
                            ])])
                            .config(|this| this.allow_trailing())
                        ]),
                        // Optional WITH clause for table options (after column definitions)
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                Ref::new("TableOptionGrammar")
                            ])])
                        ])
                        .config(|this| this.optional()),
                        // Optional AS NODE/EDGE for graph tables with column definitions
                        Sequence::new(vec_of_erased![
                            Ref::keyword("AS"),
                            one_of(vec_of_erased![Ref::keyword("NODE"), Ref::keyword("EDGE")])
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
                        optionally_bracketed(vec_of_erased![Ref::new("SelectableGrammar")]),
                        // Azure Synapse specific: OPTION clause after AS SELECT
                        Ref::new("OptionClauseSegment").optional()
                    ]),
                    // T-SQL Graph: Simple graph table without column definitions (CREATE TABLE name AS NODE/EDGE)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        one_of(vec_of_erased![Ref::keyword("NODE"), Ref::keyword("EDGE")])
                    ])
                ]),
                // Optional ON filegroup/partition_scheme clause for both table types
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    one_of(vec_of_erased![
                        Ref::new("ObjectReferenceSegment"), // filegroup or partition scheme
                        Ref::keyword("PRIMARY")
                    ])
                ])
                .config(|this| this.optional()),
                // Optional WITH clause for table options (after ON filegroup)
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "TableOptionGrammar"
                    )])])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL Graph: CONNECTION constraint for edge tables
    dialect.add([(
        "ConnectionConstraintSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("CONSTRAINT"),
            Ref::new("ObjectReferenceSegment"), // constraint name
            Ref::keyword("CONNECTION"),
            Bracketed::new(vec_of_erased![
                Ref::new("ObjectReferenceSegment"), // from table
                Ref::keyword("TO"),
                Ref::new("ObjectReferenceSegment"), // to table
            ]),
            // Optional ON DELETE CASCADE
            Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                Ref::keyword("DELETE"),
                Ref::keyword("CASCADE")
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

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
            // Azure Synapse location options
            Sequence::new(vec_of_erased![
                Ref::keyword("LOCATION"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("USER_DB"),
                    Ref::keyword("DW_BIN_TEMP"),
                    Ref::new("ObjectReferenceSegment")
                ])
            ]),
            // Azure Synapse index options
            one_of(vec_of_erased![
                Ref::keyword("HEAP"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTERED"),
                    Ref::keyword("COLUMNSTORE"),
                    Ref::keyword("INDEX"),
                    // Optional ORDER clause for columnstore indexes
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ORDER"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                // Optional ASC/DESC
                                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                    .config(|this| this.optional())
                            ])
                        ])])
                    ])
                    .config(|this| this.optional())
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("CLUSTERED"),
                    Ref::keyword("INDEX"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("ColumnReferenceSegment"),
                            // Optional ASC/DESC
                            one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                                .config(|this| this.optional())
                        ])
                    ])])
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
            ]),
            // SYSTEM_VERSIONING for temporal tables
            Sequence::new(vec_of_erased![
                Ref::keyword("SYSTEM_VERSIONING"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("OFF"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ON"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![
                                // HISTORY_TABLE option
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("HISTORY_TABLE"),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("ObjectReferenceSegment")
                                ]),
                                // HISTORY_RETENTION_PERIOD option
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("HISTORY_RETENTION_PERIOD"),
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![
                                        Ref::keyword("INFINITE"),
                                        Sequence::new(vec_of_erased![
                                            Ref::new("NumericLiteralSegment"),
                                            one_of(vec_of_erased![
                                                Ref::keyword("DAY"),
                                                Ref::keyword("DAYS"),
                                                Ref::keyword("WEEK"),
                                                Ref::keyword("WEEKS"),
                                                Ref::keyword("MONTH"),
                                                Ref::keyword("MONTHS"),
                                                Ref::keyword("YEAR"),
                                                Ref::keyword("YEARS")
                                            ])
                                        ])
                                    ])
                                ]),
                                // DATA_CONSISTENCY_CHECK option
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("DATA_CONSISTENCY_CHECK"),
                                    Ref::new("EqualsSegment"),
                                    one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                                ])
                            ])
                            .config(|this| this.allow_trailing())
                        ])
                    ])
                ])
            ]),
            // DURABILITY option
            Sequence::new(vec_of_erased![
                Ref::keyword("DURABILITY"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("SCHEMA_ONLY"),
                    Ref::keyword("SCHEMA_AND_DATA")
                ])
            ]),
            // MEMORY_OPTIMIZED option
            Sequence::new(vec_of_erased![
                Ref::keyword("MEMORY_OPTIMIZED"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
            ]),
            // DATA_DELETION option
            Sequence::new(vec_of_erased![
                Ref::keyword("DATA_DELETION"),
                Ref::new("EqualsSegment"),
                Ref::keyword("ON"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FILTER_COLUMN"),
                        Ref::new("EqualsSegment"),
                        Ref::new("ColumnReferenceSegment")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETENTION_PERIOD"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::keyword("INFINITE"),
                            Sequence::new(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("DAY"),
                                    Ref::keyword("DAYS"),
                                    Ref::keyword("WEEK"),
                                    Ref::keyword("WEEKS"),
                                    Ref::keyword("MONTH"),
                                    Ref::keyword("MONTHS"),
                                    Ref::keyword("YEAR"),
                                    Ref::keyword("YEARS")
                                ])
                            ])
                        ])
                    ])
                ])])
            ]),
            // FILETABLE options
            Sequence::new(vec_of_erased![
                Ref::keyword("FILETABLE_PRIMARY_KEY_CONSTRAINT_NAME"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILETABLE_DIRECTORY"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILETABLE_COLLATE_FILENAME"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILETABLE_STREAMID_UNIQUE_CONSTRAINT_NAME"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("FILETABLE_FULLPATH_UNIQUE_CONSTRAINT_NAME"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment")
            ]),
            // REMOTE_DATA_ARCHIVE option
            Sequence::new(vec_of_erased![
                Ref::keyword("REMOTE_DATA_ARCHIVE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("OFF"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ON"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILTER_PREDICATE"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("NULL"),
                                    Ref::new("FunctionSegment")
                                ])
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MIGRATION_STATE"),
                                Ref::new("EqualsSegment"),
                                one_of(vec_of_erased![
                                    Ref::keyword("OUTBOUND"),
                                    Ref::keyword("INBOUND"),
                                    Ref::keyword("PAUSED")
                                ])
                            ])
                        ])])
                        .config(|this| this.optional())
                    ])
                ]),
                Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::keyword("MIGRATION_STATE"),
                    Ref::new("EqualsSegment"),
                    one_of(vec_of_erased![
                        Ref::keyword("OUTBOUND"),
                        Ref::keyword("INBOUND"),
                        Ref::keyword("PAUSED")
                    ])
                ])])
                .config(|this| this.optional())
            ]),
            // LEDGER option
            Sequence::new(vec_of_erased![
                Ref::keyword("LEDGER"),
                Ref::new("EqualsSegment"),
                Ref::keyword("ON"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LEDGER_VIEW"),
                        Ref::new("EqualsSegment"),
                        Ref::new("ObjectReferenceSegment"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::keyword("TRANSACTION_ID_COLUMN_NAME"),
                                Ref::new("EqualsSegment"),
                                Ref::new("ColumnReferenceSegment")
                            ]),
                            Sequence::new(vec_of_erased![
                                Ref::keyword("SEQUENCE_NUMBER_COLUMN_NAME"),
                                Ref::new("EqualsSegment"),
                                Ref::new("ColumnReferenceSegment")
                            ])
                        ])])
                        .config(|this| this.optional())
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("APPEND_ONLY"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
                    ])
                ])])
                .config(|this| this.optional())
            ]),
            // DATA_COMPRESSION option
            Sequence::new(vec_of_erased![
                Ref::keyword("DATA_COMPRESSION"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::keyword("NONE"),
                    Ref::keyword("ROW"),
                    Ref::keyword("PAGE"),
                    Ref::keyword("COLUMNSTORE"),
                    Ref::keyword("COLUMNSTORE_ARCHIVE")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("PARTITIONS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::keyword("TO"),
                                Ref::new("NumericLiteralSegment")
                            ])
                        ]
                    )])])
                ])
                .config(|this| this.optional())
            ]),
            // XML_COMPRESSION option
            Sequence::new(vec_of_erased![
                Ref::keyword("XML_COMPRESSION"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    Ref::keyword("PARTITIONS"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("NumericLiteralSegment"),
                                Ref::keyword("TO"),
                                Ref::new("NumericLiteralSegment")
                            ])
                        ]
                    )])])
                ])
                .config(|this| this.optional())
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
            Ref::keyword("COLLATE"),   // T-SQL COLLATE clause for string comparison
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific comparison operators that allow flexible whitespace
    dialect.add([
        // T-SQL >= with flexible spacing: >= or > =
        (
            "TsqlGreaterThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("RawGreaterThanSegment"),
                    Ref::new("RawEqualsSegment"),
                ])
                .allow_gaps(true) // Allow whitespace between > and =
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL <= with flexible spacing: <= or < =
        (
            "TsqlLessThanOrEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("RawLessThanSegment"),
                    Ref::new("RawEqualsSegment"),
                ])
                .allow_gaps(true) // Allow whitespace between < and =
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL <> with flexible spacing: <> or < >
        (
            "TsqlNotEqualToSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("RawLessThanSegment"),
                    Ref::new("RawGreaterThanSegment"),
                ])
                .allow_gaps(true) // Allow whitespace between < and >
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL != with flexible spacing: != or ! =
        (
            "TsqlNotEqualsSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("RawNotSegment"),
                    Ref::new("RawEqualsSegment"),
                ])
                .allow_gaps(true) // Allow whitespace between ! and =
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL !< with flexible spacing: !< or ! <
        (
            "TsqlNotLessThanSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("RawNotSegment"),
                    Ref::new("RawLessThanSegment"),
                ])
                .allow_gaps(true) // Allow whitespace between ! and <
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL !> with flexible spacing: !> or ! >
        (
            "TsqlNotGreaterThanSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("RawNotSegment"),
                    Ref::new("RawGreaterThanSegment"),
                ])
                .allow_gaps(true) // Allow whitespace between ! and >
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override ComparisonOperatorGrammar to include T-SQL specific flexible operators
    dialect.add([(
        "ComparisonOperatorGrammar".into(),
        one_of(vec_of_erased![
            // T-SQL specific operators with flexible spacing - put these FIRST for priority
            Ref::new("TsqlGreaterThanOrEqualToSegment"),
            Ref::new("TsqlLessThanOrEqualToSegment"),
            Ref::new("TsqlNotEqualToSegment"),
            Ref::new("TsqlNotEqualsSegment"),
            Ref::new("TsqlNotLessThanSegment"),
            Ref::new("TsqlNotGreaterThanSegment"),
            // Standard operators (fallback for non-spaced versions)
            Ref::new("EqualsSegment"),
            Ref::new("GreaterThanSegment"),
            Ref::new("LessThanSegment"),
            Ref::new("LikeOperatorSegment"),
            Sequence::new(vec_of_erased![
                Ref::keyword("IS"),
                Ref::keyword("NOT").optional(),
                Ref::keyword("DISTINCT"),
                Ref::keyword("FROM")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific data type identifier - allows case-insensitive user-defined types
    dialect.add([(
        "DatatypeIdentifierSegment".into(),
        SegmentGenerator::new(|_| {
            // Generate the anti template from the set of reserved keywords
            // Exclude keywords that should not be parsed as data types
            let anti_template = format!("^({})$", "NOT|EXECUTE|EXEC|WITH");

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
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ExternalDataSourceOptionGrammar"
                )])])
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
                Ref::new("LiteralGrammar") // Changed to support Unicode strings
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
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
            ]),
            // CONNECTION_OPTIONS = 'options'
            Sequence::new(vec_of_erased![
                Ref::keyword("CONNECTION_OPTIONS"),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar") // Changed to support Unicode strings
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
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ExternalFileFormatOptionGrammar"
                )])])
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
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "FormatOptionGrammar"
                )])])
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
                Ref::new("LiteralGrammar") // Support Unicode strings
            ]),
            // STRING_DELIMITER = 'delimiter'
            Sequence::new(vec_of_erased![
                Ref::keyword("STRING_DELIMITER"),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar") // Support Unicode strings
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
                one_of(vec_of_erased![Ref::keyword("TRUE"), Ref::keyword("FALSE")])
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
                    // WITH PASSWORD = 'password' [MUST_CHANGE] [, options] [FROM WINDOWS]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("PASSWORD"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment"),
                        Ref::keyword("MUST_CHANGE").optional(),
                        // Additional options after MUST_CHANGE
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("CommaSegment"),
                            Ref::new("LoginOptionGrammar")
                        ])]),
                        // Optional FROM WINDOWS after password options
                        Sequence::new(vec_of_erased![
                            Ref::keyword("FROM"),
                            Ref::keyword("WINDOWS")
                        ])
                        .config(|this| this.optional())
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
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
            ]),
            // CHECK_POLICY = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("CHECK_POLICY"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
            ]),
            // DEFAULT_DATABASE = database_name
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT_DATABASE"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("NakedIdentifierSegment")
                ])
            ]),
            // DEFAULT_LANGUAGE = language
            Sequence::new(vec_of_erased![
                Ref::keyword("DEFAULT_LANGUAGE"),
                Ref::new("EqualsSegment"),
                Ref::new("NakedIdentifierSegment")
            ]),
            // SID = 0x... or literal
            Sequence::new(vec_of_erased![
                Ref::keyword("SID"),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar")
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
                        one_of(vec_of_erased![Ref::keyword("FOR"), Ref::keyword("FROM")]),
                        Ref::keyword("LOGIN"),
                        Ref::new("ObjectReferenceSegment"),
                        // Optional WITH options
                        Sequence::new(vec_of_erased![
                            Ref::keyword("WITH"),
                            Delimited::new(vec_of_erased![Ref::new("UserOptionGrammar")])
                        ])
                        .config(|this| this.optional())
                    ]),
                    // WITH PASSWORD = 'password' [, SID = 0x...]
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Delimited::new(vec_of_erased![Ref::new("UserOptionGrammar")])
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
            // SID = 0x... or literal
            Sequence::new(vec_of_erased![
                Ref::keyword("SID"),
                Ref::new("EqualsSegment"),
                Ref::new("LiteralGrammar")
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
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
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
                AnyNumberOf::new(vec_of_erased![Ref::new("SecurityPolicyAddClause")])
                    .config(|this| this.min_times(1)),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "SecurityPolicyOptionGrammar"
                    )])])
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
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
            ]),
            // SCHEMABINDING = ON/OFF
            Sequence::new(vec_of_erased![
                Ref::keyword("SCHEMABINDING"),
                Ref::new("EqualsSegment"),
                one_of(vec_of_erased![Ref::keyword("ON"), Ref::keyword("OFF")])
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
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("SecurityPolicyAddClause"),
                        Ref::new("SecurityPolicyDropClause"),
                        Ref::new("SecurityPolicyAlterClause")
                    ])])
                    .config(|this| this.min_times(1)),
                    // WITH clause only
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "SecurityPolicyOptionGrammar"
                        )])])
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
                Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("ALTER")])
                    .config(|this| this.optional()),
                Ref::keyword("TRIGGER"),
                Ref::new("TriggerReferenceSegment"),
                Ref::keyword("ON"),
                one_of(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Sequence::new(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("SERVER")]),
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
                    Ref::new("ExecuteAsClauseGrammar").optional()
                ])
                .config(|this| this.optional()),
                // Trigger timing
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FOR"),
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            // Common DDL events for DATABASE/ALL SERVER triggers
                            Ref::keyword("CREATE_TABLE"),
                            Ref::keyword("ALTER_TABLE"),
                            Ref::keyword("DROP_TABLE"),
                            Ref::keyword("CREATE_INDEX"),
                            Ref::keyword("DROP_INDEX"),
                            Ref::keyword("CREATE_VIEW"),
                            Ref::keyword("DROP_VIEW"),
                            Ref::keyword("CREATE_PROCEDURE"),
                            Ref::keyword("DROP_PROCEDURE"),
                            Ref::keyword("CREATE_FUNCTION"),
                            Ref::keyword("DROP_FUNCTION"),
                            Ref::keyword("CREATE_SYNONYM"),
                            Ref::keyword("DROP_SYNONYM"),
                            Ref::keyword("CREATE_DATABASE"),
                            Ref::keyword("DROP_DATABASE"),
                            // Fallback for other DDL events as identifiers
                            Ref::new("SingleIdentifierGrammar")
                        ])])
                    ]),
                    Ref::keyword("AFTER"),
                    Sequence::new(vec_of_erased![Ref::keyword("INSTEAD"), Ref::keyword("OF")])
                ])
                .config(|this| this.optional()),
                // Trigger events - only for DML table triggers (not used for DATABASE/ALL SERVER)
                Delimited::new(vec_of_erased![
                    Ref::keyword("INSERT"),
                    Ref::keyword("UPDATE"),
                    Ref::keyword("DELETE")
                ])
                .config(|this| this.optional()),
                // Additional options
                Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("APPEND")])
                    .config(|this| this.optional()),
                Sequence::new(vec_of_erased![
                    Ref::keyword("NOT"),
                    Ref::keyword("FOR"),
                    Ref::keyword("REPLICATION")
                ])
                .config(|this| this.optional()),
                Ref::keyword("AS"),
                one_of(vec_of_erased![
                    // Multiple statements in a BEGIN...END block
                    Ref::new("BeginEndBlockSegment"),
                    // Multiple statements without BEGIN...END (T-SQL allows this)
                    AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                        Ref::new("StatementSegment"),
                        Ref::new("DelimiterGrammar").optional()
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            Ref::new("BatchSeparatorGrammar"), // GO terminates the trigger
                            Ref::keyword("DROP"),              // Next DROP TRIGGER might follow
                            Ref::keyword("CREATE"),            // Next CREATE statement might follow
                        ];
                    })
                ])
            ])
            .config(|this| {
                this.terminators = vec_of_erased![
                    Ref::new("BatchSeparatorGrammar") // GO terminates the trigger definition
                                                      // Removed DelimiterGrammar - semicolons should NOT terminate the entire trigger body
                ]
            })
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override DROP TRIGGER to support T-SQL server/database level triggers
    dialect.replace_grammar(
        "DropTriggerStatementSegment",
        NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DROP"),
                Ref::keyword("TRIGGER"),
                Ref::new("IfExistsGrammar").optional(),
                Ref::new("TriggerReferenceSegment"),
                // Optional ON clause for server/database level triggers
                Sequence::new(vec_of_erased![
                    Ref::keyword("ON"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("SERVER")]),
                        Ref::keyword("DATABASE")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Add DISABLE TRIGGER statement
    dialect.add([(
        "DisableTriggerStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("DISABLE"),
            Ref::keyword("TRIGGER"),
            one_of(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("TriggerReferenceSegment")]),
                Ref::keyword("ALL")
            ]),
            Sequence::new(vec_of_erased![
                Ref::keyword("ON"),
                one_of(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Ref::keyword("DATABASE"),
                    Sequence::new(vec_of_erased![Ref::keyword("ALL"), Ref::keyword("SERVER")])
                ])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware DROP INDEX statement for T-SQL contexts where keywords are lexed as words
    dialect.add([(
        "WordAwareDropIndexStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropIndexStatement, |_| {
            Sequence::new(vec_of_erased![
                StringParser::new("DROP", SyntaxKind::Word),
                StringParser::new("INDEX", SyntaxKind::Word),
                // Index name
                one_of(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"),
                    Ref::new("SingleIdentifierGrammar")
                ]),
                StringParser::new("ON", SyntaxKind::Word),
                // Table name
                Ref::new("TableReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware UPDATE STATISTICS statement for T-SQL contexts where keywords are lexed as words
    dialect.add([(
        "WordAwareUpdateStatisticsStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
            Sequence::new(vec_of_erased![
                StringParser::new("UPDATE", SyntaxKind::Word),
                StringParser::new("STATISTICS", SyntaxKind::Word),
                // Table reference
                Ref::new("TableReferenceSegment"),
                // Optional specific statistics or list
                one_of(vec_of_erased![
                    // Single statistics name
                    Ref::new("ObjectReferenceSegment"),
                    // List of statistics in parentheses
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ObjectReferenceSegment"
                    )])])
                ])
                .config(|this| this.optional()),
                // Optional WITH options
                Sequence::new(vec_of_erased![
                    StringParser::new("WITH", SyntaxKind::Word),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        StringParser::new("FULLSCAN", SyntaxKind::Word),
                        StringParser::new("RESAMPLE", SyntaxKind::Word),
                        StringParser::new("NORECOMPUTE", SyntaxKind::Word),
                        Sequence::new(vec_of_erased![
                            StringParser::new("SAMPLE", SyntaxKind::Word),
                            Ref::new("NumericLiteralSegment"),
                            one_of(vec_of_erased![
                                StringParser::new("PERCENT", SyntaxKind::Word),
                                StringParser::new("ROWS", SyntaxKind::Word)
                            ])
                        ])
                    ])])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware CREATE TRIGGER parser for T-SQL contexts where keywords are lexed as words
    dialect.add([(
        "WordAwareCreateTriggerStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateTriggerStatement, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("CREATE", SyntaxKind::Word),
                    StringParser::new("Create", SyntaxKind::Word),
                    StringParser::new("create", SyntaxKind::Word)
                ]),
                Sequence::new(vec_of_erased![
                    StringParser::new("OR", SyntaxKind::Word),
                    StringParser::new("ALTER", SyntaxKind::Word)
                ])
                .config(|this| this.optional()),
                StringParser::new("TRIGGER", SyntaxKind::Word),
                Ref::new("SingleIdentifierGrammar"), // Trigger name
                StringParser::new("ON", SyntaxKind::Word),
                one_of(vec_of_erased![
                    // Table reference (multi-part identifier)
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DotSegment"),
                            Ref::new("SingleIdentifierGrammar")
                        ])])
                    ]),
                    // Server-level trigger
                    Sequence::new(vec_of_erased![
                        StringParser::new("ALL", SyntaxKind::Word),
                        StringParser::new("SERVER", SyntaxKind::Word)
                    ]),
                    // Database-level trigger
                    StringParser::new("DATABASE", SyntaxKind::Word)
                ]),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    StringParser::new("WITH", SyntaxKind::Word),
                    AnyNumberOf::new(vec_of_erased![
                        StringParser::new("ENCRYPTION", SyntaxKind::Word),
                        StringParser::new("NATIVE_COMPILATION", SyntaxKind::Word),
                        StringParser::new("SCHEMABINDING", SyntaxKind::Word),
                        Sequence::new(vec_of_erased![
                            StringParser::new("EXECUTE", SyntaxKind::Word),
                            StringParser::new("AS", SyntaxKind::Word),
                            Ref::new("QuotedLiteralSegment")
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Trigger timing (FOR/AFTER/INSTEAD OF)
                one_of(vec_of_erased![
                    StringParser::new("FOR", SyntaxKind::Word),
                    StringParser::new("AFTER", SyntaxKind::Word),
                    Sequence::new(vec_of_erased![
                        StringParser::new("INSTEAD", SyntaxKind::Word),
                        StringParser::new("OF", SyntaxKind::Word)
                    ])
                ])
                .config(|this| this.optional()),
                // Trigger events
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    StringParser::new("INSERT", SyntaxKind::Word),
                    StringParser::new("UPDATE", SyntaxKind::Word),
                    StringParser::new("DELETE", SyntaxKind::Word),
                    StringParser::new("CREATE_DATABASE", SyntaxKind::Word),
                    StringParser::new("DROP_DATABASE", SyntaxKind::Word),
                    StringParser::new("DROP_SYNONYM", SyntaxKind::Word),
                    StringParser::new("LOGON", SyntaxKind::Word),
                    // Generic event identifier
                    Ref::new("SingleIdentifierGrammar")
                ])])
                .config(|this| this.optional()),
                // AS keyword
                StringParser::new("AS", SyntaxKind::Word),
                // Trigger body - multiple statements until GO or next CREATE/DROP
                AnyNumberOf::new(vec_of_erased![Ref::new("GenericWordStatementSegment")]).config(
                    |this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            Ref::new("BatchSeparatorGrammar"),
                            StringParser::new("GO", SyntaxKind::Word),
                            StringParser::new("CREATE", SyntaxKind::Word),
                            StringParser::new("DROP", SyntaxKind::Word),
                            StringParser::new("ALTER", SyntaxKind::Word)
                        ];
                    }
                )
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware DROP TRIGGER parser
    dialect.add([(
        "WordAwareDropTriggerStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_| {
            Sequence::new(vec_of_erased![
                StringParser::new("DROP", SyntaxKind::Word),
                StringParser::new("TRIGGER", SyntaxKind::Word),
                Sequence::new(vec_of_erased![
                    StringParser::new("IF", SyntaxKind::Word),
                    StringParser::new("EXISTS", SyntaxKind::Word)
                ])
                .config(|this| this.optional()),
                Ref::new("SingleIdentifierGrammar"), // Trigger name
                // Optional ON clause for server/database level triggers
                Sequence::new(vec_of_erased![
                    StringParser::new("ON", SyntaxKind::Word),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            StringParser::new("ALL", SyntaxKind::Word),
                            StringParser::new("SERVER", SyntaxKind::Word)
                        ]),
                        StringParser::new("DATABASE", SyntaxKind::Word)
                    ])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware DISABLE TRIGGER parser
    dialect.add([(
        "WordAwareDisableTriggerStatementSegment".into(),
        Sequence::new(vec_of_erased![
            StringParser::new("DISABLE", SyntaxKind::Word),
            StringParser::new("TRIGGER", SyntaxKind::Word),
            one_of(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("SingleIdentifierGrammar")]),
                StringParser::new("ALL", SyntaxKind::Word)
            ]),
            Sequence::new(vec_of_erased![
                StringParser::new("ON", SyntaxKind::Word),
                one_of(vec_of_erased![
                    // Table reference (multi-part identifier)
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DotSegment"),
                            Ref::new("SingleIdentifierGrammar")
                        ])])
                    ]),
                    StringParser::new("DATABASE", SyntaxKind::Word),
                    Sequence::new(vec_of_erased![
                        StringParser::new("ALL", SyntaxKind::Word),
                        StringParser::new("SERVER", SyntaxKind::Word)
                    ])
                ])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // RETURN statement (for procedures and functions)
    // Handle T-SQL's lexing behavior where RETURN can be lexed as word in procedure contexts
    dialect.add([(
        "ReturnStatementSegment".into(),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("RETURN"),
                // Also accept RETURN as word token in T-SQL procedure bodies
                StringParser::new("RETURN", SyntaxKind::Keyword)
            ]),
            // Optional return value (for functions)
            Ref::new("ExpressionSegment").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL overrides for base SQL constructs to support word tokens
    // Override FROM clause to accept FROM as word token
    dialect.add([(
        "FromClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::FromClause, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("FROM"),
                    // Also accept FROM as word token in T-SQL procedure bodies
                    StringParser::new("FROM", SyntaxKind::Keyword)
                ]),
                Delimited::new(vec_of_erased![Ref::new("FromExpressionElement")])
            ])
            .terminators(vec_of_erased![
                Ref::new("WhereClauseSegment"),
                Ref::new("GroupByClauseSegment"),
                Ref::new("OrderByClauseSegment"),
                Ref::new("HavingClauseSegment"),
                Ref::new("LimitClauseSegment"),
                Ref::new("OptionClauseSegment"),
                Ref::keyword("FOR"),
                Ref::new("SetOperatorSegment"),
                Ref::new("WithCheckOptionSegment"),
                Ref::new("DelimiterGrammar"),
                Ref::new("BatchDelimiterGrammar"),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override WHERE clause to accept WHERE as word token
    dialect.add([(
        "WhereClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::WhereClause, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("WHERE"),
                    // Also accept WHERE as word token in T-SQL procedure bodies
                    StringParser::new("WHERE", SyntaxKind::Keyword)
                ]),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Ref::new("ExpressionSegment")])
                    .config(|this| this.min_times(1)),
                MetaSegment::dedent(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override join keywords to support word tokens
    dialect.add([(
        "JoinTypeKeywords".into(),
        one_of(vec_of_erased![
            // Regular keywords
            Ref::keyword("JOIN"),
            Sequence::new(vec_of_erased![Ref::keyword("INNER"), Ref::keyword("JOIN")]),
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("LEFT"),
                    Ref::keyword("RIGHT"),
                    Ref::keyword("FULL")
                ]),
                Ref::keyword("OUTER").optional(),
                Ref::keyword("JOIN")
            ]),
            Sequence::new(vec_of_erased![Ref::keyword("CROSS"), Ref::keyword("JOIN")]),
            // Also accept as word tokens in T-SQL procedure bodies
            StringParser::new("JOIN", SyntaxKind::Keyword),
            Sequence::new(vec_of_erased![
                StringParser::new("INNER", SyntaxKind::Keyword),
                StringParser::new("JOIN", SyntaxKind::Keyword)
            ]),
            // LEFT/RIGHT/FULL JOIN (without OUTER)
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("LEFT", SyntaxKind::Keyword),
                    StringParser::new("RIGHT", SyntaxKind::Keyword),
                    StringParser::new("FULL", SyntaxKind::Keyword)
                ]),
                StringParser::new("JOIN", SyntaxKind::Keyword)
            ]),
            // LEFT/RIGHT/FULL OUTER JOIN
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("LEFT", SyntaxKind::Keyword),
                    StringParser::new("RIGHT", SyntaxKind::Keyword),
                    StringParser::new("FULL", SyntaxKind::Keyword)
                ]),
                StringParser::new("OUTER", SyntaxKind::Keyword),
                StringParser::new("JOIN", SyntaxKind::Keyword)
            ]),
            Sequence::new(vec_of_erased![
                StringParser::new("CROSS", SyntaxKind::Keyword),
                StringParser::new("JOIN", SyntaxKind::Keyword)
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Override join ON clause to support word tokens
    dialect.add([(
        "JoinOnConditionSegment".into(),
        NodeMatcher::new(SyntaxKind::JoinOnCondition, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    // Also accept ON as word token in T-SQL procedure bodies
                    StringParser::new("ON", SyntaxKind::Keyword)
                ]),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Ref::new("ExpressionSegment")])
                    .config(|this| this.min_times(1)),
                MetaSegment::dedent(),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override IS NULL/IS NOT NULL to support word tokens
    dialect.add([(
        "IsNullGrammar".into(),
        one_of(vec_of_erased![
            // IS NULL
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("IS"),
                    // Also accept IS as word token in T-SQL procedure bodies
                    StringParser::new("IS", SyntaxKind::Keyword)
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("NULL"),
                    // Also accept NULL as word token
                    StringParser::new("NULL", SyntaxKind::Keyword)
                ])
            ]),
            // IS NOT NULL
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("IS"),
                    StringParser::new("IS", SyntaxKind::Keyword)
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("NOT"),
                    StringParser::new("NOT", SyntaxKind::Keyword)
                ]),
                one_of(vec_of_erased![
                    Ref::keyword("NULL"),
                    StringParser::new("NULL", SyntaxKind::Keyword)
                ])
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Override NULL literal to support word tokens
    dialect.add([(
        "NullLiteralSegment".into(),
        NodeMatcher::new(SyntaxKind::NullLiteral, |_| {
            one_of(vec_of_erased![
                Ref::keyword("NULL"),
                // Also accept NULL as word token in T-SQL procedure bodies
                StringParser::new("NULL", SyntaxKind::Keyword)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // RAISERROR statement
    dialect.add([(
        "RaiserrorStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("RAISERROR"),
            Bracketed::new(vec_of_erased![Sequence::new(vec_of_erased![
                // Message: expression (numeric ID, string literal, or variable)
                Ref::new("ExpressionSegment"),
                Ref::new("CommaSegment"),
                // Severity: expression (allows negative numbers like -1)
                Ref::new("ExpressionSegment"),
                Ref::new("CommaSegment"),
                // State: expression (allows negative numbers like -1)
                Ref::new("ExpressionSegment"),
                // Optional additional arguments for message formatting
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("CommaSegment"),
                    Ref::new("ExpressionSegment")
                ])
            ])]),
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
        Sequence::new(vec_of_erased![
            Ref::keyword("OPEN"),
            Ref::new("CursorNameGrammar")
        ])
        .to_matchable()
        .into(),
    )]);

    // CLOSE cursor statement
    dialect.add([(
        "CloseCursorStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("CLOSE"),
            Ref::new("CursorNameGrammar")
        ])
        .to_matchable()
        .into(),
    )]);

    // DEALLOCATE cursor statement
    dialect.add([(
        "DeallocateCursorStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("DEALLOCATE"),
            Ref::new("CursorNameGrammar")
        ])
        .to_matchable()
        .into(),
    )]);

    // FETCH cursor statement
    dialect.add([(
        "FetchCursorStatementSegment".into(),
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
                Delimited::new(vec_of_erased![Ref::new("ParameterNameSegment")])
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // OPEN SYMMETRIC KEY statement
    dialect.add([(
        "OpenSymmetricKeyStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("OPEN"),
            Ref::keyword("SYMMETRIC"),
            Ref::keyword("KEY"),
            Ref::new("ObjectReferenceSegment"), // Key name
            Ref::keyword("DECRYPTION"),
            Ref::keyword("BY"),
            // Decryption mechanism
            one_of(vec_of_erased![
                // CERTIFICATE certificate_name [WITH PASSWORD = 'password']
                Sequence::new(vec_of_erased![
                    Ref::keyword("CERTIFICATE"),
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("PASSWORD"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment")
                    ])
                    .config(|this| this.optional())
                ]),
                // ASYMMETRIC KEY asym_key_name [WITH PASSWORD = 'password']
                Sequence::new(vec_of_erased![
                    Ref::keyword("ASYMMETRIC"),
                    Ref::keyword("KEY"),
                    Ref::new("ObjectReferenceSegment"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("PASSWORD"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment")
                    ])
                    .config(|this| this.optional())
                ]),
                // SYMMETRIC KEY decrypting_key_name
                Sequence::new(vec_of_erased![
                    Ref::keyword("SYMMETRIC"),
                    Ref::keyword("KEY"),
                    Ref::new("ObjectReferenceSegment")
                ]),
                // PASSWORD = 'password'
                Sequence::new(vec_of_erased![
                    Ref::keyword("PASSWORD"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment")
                ])
            ])
        ])
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
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("DotSegment"),
                    Ref::new("SingleIdentifierGrammar")
                ])])
                .config(|this| this.max_times(1)) // Only allow schema.synonym, not server.db.schema.synonym
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // OFFSET clause
    dialect.add([(
        "OffsetClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::OffsetClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("OFFSET"),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("ROWS")])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // FETCH clause
    dialect.add([(
        "FetchClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::FetchClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("FETCH"),
                one_of(vec_of_erased![Ref::keyword("FIRST"), Ref::keyword("NEXT")]),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment")
                ])
                .config(|this| this.optional()),
                one_of(vec_of_erased![Ref::keyword("ROW"), Ref::keyword("ROWS")]),
                one_of(vec_of_erased![
                    Ref::keyword("ONLY"),
                    Sequence::new(vec_of_erased![Ref::keyword("WITH"), Ref::keyword("TIES")])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override OrderByClauseSegment to include OFFSET...FETCH support
    dialect.replace_grammar(
        "OrderByClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("ORDER"),
            Ref::keyword("BY"),
            MetaSegment::indent(),
            Delimited::new(vec_of_erased![Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("ColumnReferenceSegment"),
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                one_of(vec_of_erased![Ref::keyword("ASC"), Ref::keyword("DESC")])
                    .config(|this| this.optional())
            ])])
            .config(|this| this.allow_trailing()),
            Sequence::new(vec_of_erased![
                Ref::new("OffsetClauseSegment"),
                Ref::new("FetchClauseSegment").optional()
            ])
            .config(|this| this.optional()),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    // Add T-SQL specific permission statement segments
    dialect.add([
        (
            "TsqlGrantStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("GRANT"),
                    Ref::new("TsqlPermissionGrammar"),
                    Ref::keyword("ON"),
                    Ref::new("TsqlObjectReferenceGrammar"),
                    Ref::keyword("TO"),
                    Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Ref::keyword("GRANT"),
                        Ref::keyword("OPTION")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TsqlDenyStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DENY"),
                    Ref::new("TsqlPermissionGrammar"),
                    Ref::keyword("ON"),
                    Ref::new("TsqlObjectReferenceGrammar"),
                    Ref::keyword("TO"),
                    Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")]),
                    Ref::keyword("CASCADE").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "TsqlRevokeStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AccessStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("REVOKE"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("GRANT"),
                        Ref::keyword("OPTION"),
                        Ref::keyword("FOR")
                    ])
                    .config(|this| this.optional()),
                    Ref::new("TsqlPermissionGrammar"),
                    Ref::keyword("ON"),
                    Ref::new("TsqlObjectReferenceGrammar"),
                    one_of(vec_of_erased![Ref::keyword("FROM"), Ref::keyword("TO")]),
                    Delimited::new(vec_of_erased![Ref::new("ObjectReferenceSegment")]),
                    Ref::keyword("CASCADE").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL permission types
        (
            "TsqlPermissionGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("SELECT"),
                Ref::keyword("INSERT"),
                Ref::keyword("UPDATE"),
                Ref::keyword("DELETE"),
                Ref::keyword("EXECUTE"),
                Ref::keyword("REFERENCES"),
                Ref::keyword("ALTER"),
                Ref::keyword("CONTROL"),
                Ref::keyword("TAKE"),
                Ref::keyword("VIEW"),
                Ref::keyword("IMPERSONATE"),
                Sequence::new(vec_of_erased![
                    Ref::keyword("REFERENCES"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnReferenceSegment"
                    )])])
                ])
            ])
            .to_matchable()
            .into(),
        ),
        // T-SQL object reference with OBJECT:: and SCHEMA:: support
        (
            "TsqlObjectReferenceGrammar".into(),
            one_of(vec_of_erased![
                // OBJECT::schema.object syntax
                Sequence::new(vec_of_erased![
                    Ref::keyword("OBJECT"),
                    Ref::new("CastOperatorSegment"), // ::
                    Ref::new("ObjectReferenceSegment")
                ]),
                // SCHEMA::schema syntax
                Sequence::new(vec_of_erased![
                    Ref::keyword("SCHEMA"),
                    Ref::new("CastOperatorSegment"), // ::
                    Ref::new("ObjectReferenceSegment")
                ]),
                // Regular object reference
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
            .into(),
        ),
        // T-SQL JSON null handling clause for JSON_ARRAY and JSON_OBJECT
        (
            "TsqlJsonNullClause".into(),
            NodeMatcher::new(SyntaxKind::JsonNullClause, |_| {
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::keyword("NULL"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ABSENT"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL")
                    ]),
                    // Just ON NULL by itself
                    Sequence::new(vec_of_erased![Ref::keyword("ON"), Ref::keyword("NULL")])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // T-SQL JSON_OBJECT function with key:value syntax
        (
            "TsqlJsonObjectSegment".into(),
            NodeMatcher::new(SyntaxKind::Function, |_| {
                Sequence::new(vec_of_erased![
                    NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                        StringParser::new("JSON_OBJECT", SyntaxKind::FunctionNameIdentifier)
                            .to_matchable()
                    }),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        // Just the null clause for JSON_OBJECT(ABSENT ON NULL)
                        Ref::new("TsqlJsonNullClause"),
                        // Key-value pairs with optional null clause
                        Sequence::new(vec_of_erased![
                            Delimited::new(vec_of_erased![
                                // Key-value pairs with colon syntax
                                Sequence::new(vec_of_erased![
                                    Ref::new("ExpressionSegment"), // key
                                    Ref::new("ColonSegment"),      // :
                                    Ref::new("ExpressionSegment")  // value
                                ])
                            ])
                            .config(|this| {
                                this.allow_trailing = true;
                            }),
                            Ref::new("TsqlJsonNullClause").optional()
                        ])
                    ])])
                    .config(|this| this.parse_mode(ParseMode::Greedy))
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // Custom element for JSON_ARRAY that can be an expression or null clause
        (
            "TsqlJsonArrayElementGrammar".into(),
            one_of(vec_of_erased![
                Ref::new("ExpressionSegment"),
                Ref::new("TsqlJsonNullClause")
            ])
            .to_matchable()
            .into(),
        ),
        // Grammar for JSON_ARRAY function contents
        (
            "TsqlJsonArrayContentsGrammar".into(),
            one_of(vec_of_erased![
                // Just null clause for JSON_ARRAY(NULL ON NULL) or JSON_ARRAY(ABSENT ON NULL)
                Ref::new("TsqlJsonNullClause"),
                // List of expressions followed by optional null clause (without comma)
                Sequence::new(vec_of_erased![
                    Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")]).config(|this| {
                        this.allow_trailing = false; // Don't allow trailing comma before null clause
                    }),
                    // Allow the NULL ON NULL clause directly after the expressions
                    Ref::new("TsqlJsonNullClause").optional()
                ])
            ])
            .to_matchable()
            .into(),
        ),
        // T-SQL JSON_ARRAY function
        (
            "TsqlJsonArraySegment".into(),
            NodeMatcher::new(SyntaxKind::Function, |_| {
                Sequence::new(vec_of_erased![
                    NodeMatcher::new(SyntaxKind::FunctionName, |_| {
                        StringParser::new("JSON_ARRAY", SyntaxKind::FunctionNameIdentifier)
                            .to_matchable()
                    }),
                    Bracketed::new(vec_of_erased![Ref::new("TsqlJsonArrayContentsGrammar")])
                        .config(|this| this.parse_mode(ParseMode::Greedy))
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // Override FunctionContentsGrammar to handle T-SQL JSON functions with special syntax
    dialect.add([(
        "FunctionContentsGrammar".into(),
        AnyNumberOf::new(vec![
            // Standard expressions (which will include functions via BaseExpressionElementGrammar)
            Ref::new("ExpressionSegment").to_matchable(),
            // A Cast-like function
            Sequence::new(vec![
                Ref::new("ExpressionSegment").to_matchable(),
                Ref::keyword("AS").to_matchable(),
                Ref::new("DatatypeSegment").to_matchable(),
            ])
            .to_matchable(),
            // Trim function
            Sequence::new(vec![
                Ref::new("TrimParametersGrammar").to_matchable(),
                Ref::new("ExpressionSegment")
                    .optional()
                    .exclude(Ref::keyword("FROM"))
                    .to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable(),
            // An extract-like or substring-like function
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("DatetimeUnitSegment").to_matchable(),
                    Ref::new("ExpressionSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("FROM").to_matchable(),
                Ref::new("ExpressionSegment").to_matchable(),
            ])
            .to_matchable(),
            Sequence::new(vec![
                // Allow an optional distinct keyword here.
                Ref::keyword("DISTINCT").optional().to_matchable(),
                one_of(vec![
                    // For COUNT(*) or similar
                    Ref::new("StarSegment").to_matchable(),
                    Delimited::new(vec![
                        Ref::new("FunctionContentsExpressionGrammar").to_matchable(),
                    ])
                    .to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("AggregateOrderByClause").to_matchable(),
            Sequence::new(vec![
                Ref::keyword("SEPARATOR").to_matchable(),
                Ref::new("LiteralGrammar").to_matchable(),
            ])
            .to_matchable(),
            // Position-like function
            Sequence::new(vec![
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("IN").to_matchable(),
                one_of(vec![
                    Ref::new("QuotedLiteralSegment").to_matchable(),
                    Ref::new("SingleIdentifierGrammar").to_matchable(),
                    Ref::new("ColumnReferenceSegment").to_matchable(),
                ])
                .to_matchable(),
            ])
            .to_matchable(),
            Ref::new("IgnoreRespectNullsGrammar").to_matchable(),
            Ref::new("IndexColumnDefinitionSegment").to_matchable(),
            Ref::new("EmptyStructLiteralSegment").to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // NOTE: This FunctionSegment definition is commented out because it's superseded by the later definition
    // that consolidates all function patterns. Keeping for reference.
    // // Override FunctionSegment to include T-SQL specific JSON functions
    // dialect.replace_grammar("FunctionSegment", ...);

    // Add COPY INTO statement support for Azure blob storage
    dialect.add([
        // Credential grammar for COPY INTO WITH clause
        (
            "TsqlCredentialGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("IDENTITY"),
                Ref::new("EqualsSegment"),
                Ref::new("QuotedLiteralSegment"),
                Sequence::new(vec_of_erased![
                    Ref::new("CommaSegment"),
                    Ref::keyword("SECRET"),
                    Ref::new("EqualsSegment"),
                    Ref::new("QuotedLiteralSegment")
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
        // Azure storage location segment
        (
            "TsqlStorageLocationSegment".into(),
            one_of(vec_of_erased![
                Ref::new("LiteralGrammar") // Azure blob URLs - supports Unicode strings
            ])
            .to_matchable()
            .into(),
        ),
        // COPY INTO statement
        (
            "TsqlCopyIntoStatementSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("COPY"),
                Ref::keyword("INTO"),
                Ref::new("TableReferenceSegment"),
                // Optional column list
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ColumnReferenceSegment"
                )])])
                .config(|this| this.optional()),
                // FROM clause with storage locations
                Sequence::new(vec_of_erased![
                    Ref::keyword("FROM"),
                    Delimited::new(vec_of_erased![Ref::new("TsqlStorageLocationSegment")])
                ]),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            // FILE_TYPE = 'CSV'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILE_TYPE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("LiteralGrammar") // Support Unicode strings
                            ]),
                            // FILE_FORMAT = object_ref
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FILE_FORMAT"),
                                Ref::new("EqualsSegment"),
                                Ref::new("ObjectReferenceSegment")
                            ]),
                            // CREDENTIAL = (credential_grammar)
                            Sequence::new(vec_of_erased![
                                Ref::keyword("CREDENTIAL"),
                                Ref::new("EqualsSegment"),
                                Bracketed::new(vec_of_erased![Ref::new("TsqlCredentialGrammar")])
                            ]),
                            // ERRORFILE = 'path'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ERRORFILE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // MAXERRORS = number
                            Sequence::new(vec_of_erased![
                                Ref::keyword("MAXERRORS"),
                                Ref::new("EqualsSegment"),
                                Ref::new("NumericLiteralSegment")
                            ]),
                            // COMPRESSION = 'type'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("COMPRESSION"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // FIELDQUOTE = 'char'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FIELDQUOTE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // FIELDTERMINATOR = 'char'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FIELDTERMINATOR"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // ROWTERMINATOR = 'char'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ROWTERMINATOR"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // FIRSTROW = number
                            Sequence::new(vec_of_erased![
                                Ref::keyword("FIRSTROW"),
                                Ref::new("EqualsSegment"),
                                Ref::new("NumericLiteralSegment")
                            ]),
                            // DATEFORMAT = 'format'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("DATEFORMAT"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // ENCODING = 'encoding'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("ENCODING"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // IDENTITY_INSERT = 'ON'/'OFF'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("IDENTITY_INSERT"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ]),
                            // AUTO_CREATE_TABLE = 'ON'/'OFF'
                            Sequence::new(vec_of_erased![
                                Ref::keyword("AUTO_CREATE_TABLE"),
                                Ref::new("EqualsSegment"),
                                Ref::new("QuotedLiteralSegment")
                            ])
                        ]
                    )])])
                ])
                .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Add CREATE DATABASE SCOPED CREDENTIAL statement
    dialect.add([
        (
            "CreateDatabaseScopedCredentialStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseScopedCredentialStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("DATABASE"),
                    Ref::keyword("SCOPED"),
                    Ref::keyword("CREDENTIAL"),
                    Ref::new("ObjectReferenceSegment"), // credential_name
                    Ref::keyword("WITH"),
                    Ref::new("TsqlCredentialGrammar") // IDENTITY = 'value' [, SECRET = 'value']
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // CREATE MASTER KEY statement
        (
            "CreateMasterKeyStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateMasterKeyStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    StringParser::new("MASTER", SyntaxKind::Keyword),
                    Ref::keyword("KEY"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("ENCRYPTION"),
                        Ref::keyword("BY"),
                        Ref::keyword("PASSWORD"),
                        Ref::new("EqualsSegment"),
                        Ref::new("QuotedLiteralSegment")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // ALTER MASTER KEY statement
        (
            "AlterMasterKeyStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterMasterKeyStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("ALTER"),
                    StringParser::new("MASTER", SyntaxKind::Keyword),
                    Ref::keyword("KEY"),
                    one_of(vec_of_erased![
                        // FORCE REGENERATE WITH ENCRYPTION BY PASSWORD = 'password'
                        Sequence::new(vec_of_erased![
                            Sequence::new(vec_of_erased![StringParser::new(
                                "FORCE",
                                SyntaxKind::Keyword
                            ),])
                            .config(|this| this.optional()),
                            StringParser::new("REGENERATE", SyntaxKind::Keyword),
                            Ref::keyword("WITH"),
                            Ref::keyword("ENCRYPTION"),
                            Ref::keyword("BY"),
                            Ref::keyword("PASSWORD"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment")
                        ]),
                        // ADD ENCRYPTION BY PASSWORD = 'password'
                        Sequence::new(vec_of_erased![
                            Ref::keyword("ADD"),
                            Ref::keyword("ENCRYPTION"),
                            Ref::keyword("BY"),
                            one_of(vec_of_erased![
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("PASSWORD"),
                                    Ref::new("EqualsSegment"),
                                    Ref::new("QuotedLiteralSegment")
                                ]),
                                Sequence::new(vec_of_erased![
                                    Ref::keyword("SERVICE"),
                                    StringParser::new("MASTER", SyntaxKind::Keyword),
                                    Ref::keyword("KEY")
                                ])
                            ])
                        ]),
                        // DROP ENCRYPTION BY PASSWORD = 'password'
                        Sequence::new(vec_of_erased![
                            Ref::keyword("DROP"),
                            Ref::keyword("ENCRYPTION"),
                            Ref::keyword("BY"),
                            Ref::keyword("PASSWORD"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment")
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // DROP MASTER KEY statement
        (
            "DropMasterKeyStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropMasterKeyStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("DROP"),
                    StringParser::new("MASTER", SyntaxKind::Keyword),
                    Ref::keyword("KEY")
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        // Grammar for PERCENT/ROWS in TABLESAMPLE
        (
            "PercentRowsGrammar".into(),
            one_of(vec_of_erased![
                Ref::keyword("PERCENT"),
                Ref::keyword("ROWS")
            ])
            .to_matchable()
            .into(),
        ),
        // T-SQL-specific TABLESAMPLE clause
        (
            "SamplingExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::SampleExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("TABLESAMPLE"),
                    // SYSTEM is optional in T-SQL
                    Ref::keyword("SYSTEM").optional(),
                    Bracketed::new(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        // PERCENT or ROWS is optional (default is ROWS)
                        Ref::new("PercentRowsGrammar").optional()
                    ]),
                    // REPEATABLE clause is optional
                    Sequence::new(vec_of_erased![
                        Ref::keyword("REPEATABLE"),
                        Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")]),
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL OUTPUT clause needed by INSERT, UPDATE, DELETE, MERGE statements
    dialect.add([(
        "OutputClauseSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("OUTPUT"),
            Delimited::new(vec_of_erased![
                // Use SelectClauseElementSegment which already handles expressions with aliases
                Ref::new("SelectClauseElementSegment"),
            ]),
            // Optional INTO clause
            Sequence::new(vec_of_erased![
                Ref::keyword("INTO"),
                Ref::new("TableReferenceSegment"),
                // Optional column list
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "ColumnReferenceSegment"
                ),])])
                .config(|this| this.optional()),
            ])
            .config(|this| this.optional()),
        ])
        .to_matchable()
        .into(),
    )]);

    // Override MergeStatementSegment for T-SQL specific features (OUTPUT clause support)
    dialect.replace_grammar(
        "MergeStatementSegment",
        NodeMatcher::new(SyntaxKind::MergeStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("MergeIntoLiteralGrammar"),
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    // T-SQL specific: Table reference with optional hints
                    Sequence::new(vec_of_erased![
                        Ref::new("TableReferenceSegment"),
                        Ref::new("TableHintSegment").optional(),
                        Ref::new("AliasExpressionSegment").optional(),
                    ]),
                    Ref::new("AliasedTableReferenceGrammar"),
                ]),
                MetaSegment::dedent(),
                Ref::keyword("USING"),
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::new("AliasedTableReferenceGrammar"),
                    Sequence::new(vec_of_erased![
                        Bracketed::new(vec_of_erased![Ref::new("SelectableGrammar")]),
                        Ref::new("AliasExpressionSegment").optional(),
                    ])
                ]),
                MetaSegment::dedent(),
                Conditional::new(MetaSegment::indent()).indented_using_on(),
                Ref::new("JoinOnConditionSegment"),
                Conditional::new(MetaSegment::dedent()).indented_using_on(),
                Ref::new("MergeMatchSegment"),
                // T-SQL specific: OUTPUT clause support
                Ref::new("OutputClauseSegment").optional(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Override UPDATE statement for T-SQL specific features
    dialect.replace_grammar(
        "UpdateStatementSegment",
        NodeMatcher::new(SyntaxKind::UpdateStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("UPDATE"),
                // T-SQL specific: TOP clause for UPDATE
                Ref::new("TopClauseSegment").optional(),
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    Ref::new("AliasedTableReferenceGrammar"),
                    // T-SQL specific: OPENQUERY/OPENROWSET/OPENDATASOURCE support
                    Ref::new("OpenQuerySegment"),
                    Ref::new("OpenRowSetSegment"),
                    Ref::new("OpenDataSourceSegment"),
                    // Allow OPENDATASOURCE/OPENROWSET/OPENQUERY with chained object references
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("OpenQuerySegment"),
                            Ref::new("OpenDataSourceSegment"),
                            Ref::new("OpenRowSetSegment")
                        ]),
                        // Allow .database.schema.table after OPENDATASOURCE/OPENROWSET/OPENQUERY
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DotSegment"),
                            Ref::new("SingleIdentifierGrammar")
                        ])])
                        .config(|this| this.min_times(1))
                    ]),
                    Ref::new("FunctionSegment"),
                ]),
                // T-SQL specific: Table hints
                Ref::new("PostTableExpressionGrammar").optional(),
                MetaSegment::dedent(),
                Ref::new("SetClauseListSegment"),
                // T-SQL specific: OUTPUT clause (after SET)
                Ref::new("OutputClauseSegment").optional(),
                Ref::new("FromClauseSegment").optional(),
                Ref::new("WhereClauseSegment").optional(),
                // T-SQL specific: OPTION clause
                Ref::new("OptionClauseSegment").optional(),
                Ref::new("DelimiterGrammar").optional()
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Override SetClauseListSegment for T-SQL (without comma delimiting)
    dialect.replace_grammar(
        "SetClauseListSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("SET"),
            MetaSegment::indent(),
            Ref::new("SetClauseSegment"),
            AnyNumberOf::new(vec_of_erased![
                Ref::new("CommaSegment"),
                Ref::new("SetClauseSegment")
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    // Override SetClauseSegment to support T-SQL compound assignment operators
    dialect.replace_grammar(
        "SetClauseSegment",
        NodeMatcher::new(SyntaxKind::SetClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("ColumnReferenceSegment"),
                // Use the already-defined AssignmentOperatorSegment which includes compound assignments
                Ref::new("AssignmentOperatorSegment"),
                Ref::new("ExpressionSegment"),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Override INSERT statement for T-SQL specific features
    dialect.replace_grammar(
        "InsertStatementSegment",
        NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("INSERT"),
                // T-SQL specific: TOP clause for INSERT
                Ref::new("TopClauseSegment").optional(),
                Ref::keyword("OVERWRITE").optional(),
                // T-SQL allows omitting INTO when using OPENQUERY
                Ref::keyword("INTO").optional(),
                one_of(vec_of_erased![
                    Ref::new("TableReferenceSegment"),
                    // T-SQL specific: OPENQUERY/OPENROWSET/OPENDATASOURCE support
                    Ref::new("OpenQuerySegment"),
                    Ref::new("OpenRowSetSegment"),
                    Ref::new("OpenDataSourceSegment"),
                    // Allow OPENDATASOURCE/OPENROWSET/OPENQUERY with chained object references
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("OpenQuerySegment"),
                            Ref::new("OpenDataSourceSegment"),
                            Ref::new("OpenRowSetSegment")
                        ]),
                        // Allow .database.schema.table after OPENDATASOURCE/OPENROWSET/OPENQUERY
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DotSegment"),
                            Ref::new("SingleIdentifierGrammar")
                        ])])
                        .config(|this| this.min_times(1))
                    ]),
                ]),
                // T-SQL specific: Table hints
                Ref::new("PostTableExpressionGrammar").optional(),
                // T-SQL specific: OUTPUT clause before VALUES/SELECT
                Ref::new("OutputClauseSegment").optional(),
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("BracketedColumnReferenceListGrammar"),
                        // OUTPUT clause can also be here
                        Ref::new("OutputClauseSegment").optional(),
                        Ref::new("SelectableGrammar")
                    ]),
                    Ref::new("SelectableGrammar"),
                    Ref::new("DefaultValuesGrammar"),
                    // T-SQL specific: EXEC/EXECUTE for INSERT...EXEC
                    Ref::new("ExecuteStatementGrammar")
                ]),
                // T-SQL specific: OPTION clause
                Ref::new("OptionClauseSegment").optional(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // NOTE: Removed T-SQL SelectableGrammar override to fix MERGE statement parsing
    // The EXEC statement support should be added differently without breaking MERGE

    // Override DELETE statement for T-SQL specific features
    dialect.replace_grammar(
        "DeleteStatementSegment",
        NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("DELETE"),
                // T-SQL specific: TOP clause for DELETE
                Ref::new("TopClauseSegment").optional(),
                // T-SQL allows omitting FROM when using OPENQUERY
                Ref::keyword("FROM").optional(),
                one_of(vec_of_erased![
                    // Handle table with alias (but exclude OUTPUT and WHERE as aliases)
                    Sequence::new(vec_of_erased![
                        Ref::new("TableReferenceSegment"),
                        Ref::new("AliasExpressionSegment")
                            .exclude(one_of(vec_of_erased![
                                Ref::keyword("OUTPUT"),
                                Ref::keyword("WHERE"),
                                Ref::keyword("OPTION")
                            ]))
                            .optional()
                    ]),
                    // T-SQL specific: OPENQUERY/OPENROWSET/OPENDATASOURCE support
                    Ref::new("OpenQuerySegment"),
                    Ref::new("OpenRowSetSegment"),
                    Ref::new("OpenDataSourceSegment"),
                    // Allow OPENDATASOURCE/OPENROWSET/OPENQUERY with chained object references
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::new("OpenQuerySegment"),
                            Ref::new("OpenDataSourceSegment"),
                            Ref::new("OpenRowSetSegment")
                        ]),
                        // Allow .database.schema.table after OPENDATASOURCE/OPENROWSET/OPENQUERY
                        AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                            Ref::new("DotSegment"),
                            Ref::new("SingleIdentifierGrammar")
                        ])])
                        .config(|this| this.min_times(1))
                    ]),
                    Ref::new("FunctionSegment"),
                ]),
                // T-SQL specific: Table hints
                Ref::new("PostTableExpressionGrammar").optional(),
                // T-SQL specific: OUTPUT clause
                Ref::new("OutputClauseSegment").optional(),
                // FROM clause for joins
                Ref::new("FromClauseSegment").optional(),
                Ref::new("WhereClauseSegment").optional(),
                // T-SQL specific: OPTION clause
                Ref::new("OptionClauseSegment").optional(),
                Ref::new("DelimiterGrammar").optional()
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Override WhereClauseSegment to support T-SQL CURRENT OF clause
    dialect.replace_grammar(
        "WhereClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("WHERE"),
            MetaSegment::indent(),
            one_of(vec_of_erased![
                // Regular WHERE with expression - exclude WITH CHECK OPTION
                Ref::new("ExpressionSegment").exclude(LookaheadExclude::new("WITH", "CHECK")),
                // T-SQL CURRENT OF clause for cursors
                Sequence::new(vec_of_erased![
                    Ref::keyword("CURRENT"),
                    Ref::keyword("OF"),
                    Ref::new("ObjectReferenceSegment") // cursor name
                ])
            ]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    // BREAK and CONTINUE statements for loops
    dialect.add([
        (
            "BreakStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::BreakStatement, |_| {
                Ref::keyword("BREAK").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ContinueStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ContinueStatement, |_| {
                Ref::keyword("CONTINUE").to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL specific GROUP BY extensions
    dialect.replace_grammar(
        "GroupByClauseSegment",
        NodeMatcher::new(SyntaxKind::GroupbyClause, |_| {
            Sequence::new(vec![
                Ref::keyword("GROUP").to_matchable(),
                Ref::keyword("BY").to_matchable(),
                MetaSegment::indent().to_matchable(),
                one_of(vec_of_erased![
                    Ref::new("CubeRollupClauseSegment"),
                    Delimited::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("ExpressionSegment")
                    ])])
                ])
                .config(|this| this.optional())
                .to_matchable(),
                MetaSegment::dedent().to_matchable(),
                // T-SQL specific WITH ROLLUP/CUBE syntax
                Sequence::new(vec_of_erased![
                    Ref::keyword("WITH"),
                    one_of(vec_of_erased![Ref::keyword("ROLLUP"), Ref::keyword("CUBE")])
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // T-SQL PIVOT and UNPIVOT support
    dialect.add([
        (
            "PivotUnpivotStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotExpression, |_| {
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        // PIVOT
                        Sequence::new(vec_of_erased![
                            Ref::keyword("PIVOT"),
                            Bracketed::new(vec_of_erased![
                                // Aggregate function (e.g., AVG(StandardCost))
                                Ref::new("FunctionSegment"),
                                Ref::keyword("FOR"),
                                // Column to pivot on
                                Ref::new("ColumnReferenceSegment"),
                                Ref::keyword("IN"),
                                // List of values to become column headers
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    Ref::new("PivotColumnReferenceSegment")
                                ])])
                            ])
                        ]),
                        // UNPIVOT
                        Sequence::new(vec_of_erased![
                            Ref::keyword("UNPIVOT"),
                            Bracketed::new(vec_of_erased![
                                // Value column (e.g., Quantity)
                                Ref::new("ColumnReferenceSegment"),
                                Ref::keyword("FOR"),
                                // Column that will hold the unpivoted column names
                                Ref::new("ColumnReferenceSegment"),
                                Ref::keyword("IN"),
                                // List of columns to unpivot
                                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                                    Ref::new("PivotColumnReferenceSegment")
                                ])])
                            ])
                        ])
                    ]),
                    // Optional AS alias after PIVOT/UNPIVOT
                    Ref::new("AliasExpressionSegment").optional()
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "PivotColumnReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::PivotColumnReference, |_| {
                // Can be quoted identifiers like [0], [1] or regular column names
                one_of(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("QuotedIdentifierSegment"),
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL THROW statement support
    dialect.add([(
        "ThrowStatementSegment".into(),
        Sequence::new(vec_of_erased![
            Ref::keyword("THROW"),
            // Optional error number, message and state
            Sequence::new(vec_of_erased![
                Ref::new("NumericLiteralSegment"),
                Ref::new("CommaSegment"),
                Ref::new("ExpressionSegment"),
                Ref::new("CommaSegment"),
                Ref::new("NumericLiteralSegment"),
            ])
            .config(|this| this.optional())
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL Function Parameter Support
    dialect.add([
        (
            "FunctionParameterListGrammar".into(),
            Bracketed::new(vec_of_erased![
                Delimited::new(vec_of_erased![Ref::new("FunctionParameterSegment")])
                    .config(|this| this.optional())
            ])
            .to_matchable()
            .into(),
        ),
        (
            "FunctionParameterSegment".into(),
            NodeMatcher::new(SyntaxKind::Parameter, |_| {
                Sequence::new(vec_of_erased![
                    Ref::new("ParameterNameSegment"),
                    // Optional AS keyword
                    Ref::keyword("AS").optional(),
                    one_of(vec_of_erased![
                        // Regular parameter type
                        Ref::new("DatatypeSegment"),
                        // User-defined table type with READONLY
                        Sequence::new(vec_of_erased![
                            Ref::new("DatatypeSegment"),
                            Ref::keyword("READONLY")
                        ])
                    ]),
                    // Optional default value
                    Sequence::new(vec_of_erased![
                        Ref::new("EqualsSegment"),
                        Ref::new("ExpressionSegment")
                    ])
                    .config(|this| this.optional())
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL CREATE EXTERNAL TABLE statement (Azure Synapse Analytics)
    dialect.add([
        (
            "CreateExternalTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("CREATE"),
                    Ref::keyword("EXTERNAL"),
                    Ref::keyword("TABLE"),
                    // Table name (can be schema qualified)
                    Ref::new("TableReferenceSegment"),
                    // Column definitions (optional)
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                        "ColumnDefinitionSegment"
                    )])])
                    .config(|this| this.optional()),
                    // WITH clause for external table options
                    Sequence::new(vec_of_erased![
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ExternalTableOptionSegment"
                        )])])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
        (
            "ExternalTableOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::TableConstraint, |_| {
                one_of(vec_of_erased![
                    // LOCATION = 'path'
                    Sequence::new(vec_of_erased![
                        Ref::keyword("LOCATION"),
                        Ref::new("EqualsSegment"),
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("UnicodeLiteralSegment")
                        ])
                    ]),
                    // DATA_SOURCE = name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("DATA_SOURCE"),
                        Ref::new("EqualsSegment"),
                        Ref::new("NakedIdentifierSegment")
                    ]),
                    // FILE_FORMAT = name
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FILE_FORMAT"),
                        Ref::new("EqualsSegment"),
                        Ref::new("NakedIdentifierSegment")
                    ]),
                    // REJECT_TYPE = VALUE|PERCENTAGE
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
                        one_of(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            Ref::new("UnicodeLiteralSegment")
                        ])
                    ])
                ])
                .to_matchable()
            })
            .to_matchable()
            .into(),
        ),
    ]);

    // T-SQL FOR SYSTEM_TIME temporal table queries
    dialect.add([(
        "ForSystemTimeClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::WithDataClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("FOR"),
                Ref::keyword("SYSTEM_TIME"),
                one_of(vec_of_erased![
                    // FOR SYSTEM_TIME ALL
                    Ref::keyword("ALL"),
                    // FOR SYSTEM_TIME BETWEEN datetime AND datetime
                    Sequence::new(vec_of_erased![
                        Ref::keyword("BETWEEN"),
                        Ref::new("LiteralGrammar"),
                        Ref::keyword("AND"),
                        Ref::new("LiteralGrammar")
                    ]),
                    // FOR SYSTEM_TIME FROM datetime TO datetime
                    Sequence::new(vec_of_erased![
                        Ref::keyword("FROM"),
                        Ref::new("ExpressionSegment"),
                        Ref::keyword("TO"),
                        Ref::new("ExpressionSegment")
                    ]),
                    // FOR SYSTEM_TIME AS OF datetime
                    Sequence::new(vec_of_erased![
                        Ref::keyword("AS"),
                        Ref::keyword("OF"),
                        Ref::new("ExpressionSegment")
                    ]),
                    // FOR SYSTEM_TIME CONTAINED IN (datetime, datetime)
                    Sequence::new(vec_of_erased![
                        Ref::keyword("CONTAINED"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "ExpressionSegment"
                        )])])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Update PostTableExpressionGrammar to include ForSystemTimeClauseSegment
    dialect.add([(
        "PostTableExpressionGrammar".into(),
        one_of(vec_of_erased![
            // WITH (hints) syntax
            Ref::new("TableHintSegment"),
            // Simplified (hint) syntax - just bracketed hints without WITH
            Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                "TableHintElement"
            )])]),
            // PIVOT/UNPIVOT
            Ref::new("PivotUnpivotStatementSegment"),
            // FOR SYSTEM_TIME temporal table queries
            Ref::new("ForSystemTimeClauseSegment"),
        ])
        .config(|this| this.optional())
        .to_matchable()
        .into(),
    )]);

    // Override CREATE FUNCTION for T-SQL specific features
    dialect.replace_grammar("CreateFunctionStatementSegment", {
        Sequence::new(vec_of_erased![
            Ref::keyword("CREATE"),
            Sequence::new(vec_of_erased![Ref::keyword("OR"), Ref::keyword("ALTER")])
                .config(|this| this.optional()),
            Ref::keyword("FUNCTION"),
            Ref::new("ObjectReferenceSegment"), // T-SQL functions can have schema names
            Ref::new("FunctionParameterListGrammar"),
            // RETURNS clause - can be simple type or table type
            Sequence::new(vec_of_erased![
                Ref::keyword("RETURNS"),
                one_of(vec_of_erased![
                    // Table variable: RETURNS @var TABLE (...)
                    Sequence::new(vec_of_erased![
                        Ref::new("TsqlVariableSegment"),
                        Ref::keyword("TABLE"),
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                Ref::new("ColumnReferenceSegment"),
                                Ref::new("DatatypeSegment")
                            ])
                        ])])
                    ]),
                    // Simple data type
                    Ref::new("DatatypeSegment")
                ])
            ]),
            // Optional WITH clauses
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Delimited::new(vec_of_erased![one_of(vec_of_erased![
                    Ref::keyword("SCHEMABINDING"),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("RETURNS"),
                        Ref::keyword("NULL"),
                        Ref::keyword("ON"),
                        Ref::keyword("NULL"),
                        Ref::keyword("INPUT")
                    ]),
                    Sequence::new(vec_of_erased![
                        Ref::keyword("EXECUTE"),
                        Ref::keyword("AS"),
                        one_of(vec_of_erased![
                            Ref::keyword("CALLER"),
                            Ref::keyword("SELF"),
                            Ref::keyword("OWNER"),
                            Ref::new("QuotedLiteralSegment")
                        ])
                    ])
                ])])
            ])
            .config(|this| this.optional()),
            // Function body
            one_of(vec_of_erased![
                // AS BEGIN ... END
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::keyword("BEGIN"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("StatementSegment")]),
                    Ref::keyword("END")
                ]),
                // Just AS for inline functions
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS"),
                    Ref::new("StatementSegment")
                ]),
                // BEGIN ... END without AS (T-SQL allows this)
                Sequence::new(vec_of_erased![
                    Ref::keyword("BEGIN"),
                    AnyNumberOf::new(vec_of_erased![Ref::new("StatementSegment")]),
                    Ref::keyword("END")
                ])
            ])
        ])
        .to_matchable()
    });

    // Override MergeMatchSegment to allow multiple MERGE clauses including T-SQL specific syntax
    dialect.replace_grammar("MergeMatchSegment", {
        NodeMatcher::new(SyntaxKind::MergeMatch, |_| {
            AnyNumberOf::new(vec_of_erased![
                Ref::new("MergeMatchedClauseSegment"),
                Ref::new("MergeNotMatchedClauseSegment")
            ])
            .config(|this| this.min_times(1))
            .to_matchable()
        })
        .to_matchable()
    });

    // Remove the problematic StatementSegment override
    // Instead, modify ProcedureDefinitionGrammar directly to use WordAwareStatementSegment

    // T-SQL MergeIntoLiteralGrammar override - INTO is optional in T-SQL
    // Try to add it first in case ANSI doesn't have it
    dialect.add([(
        "MergeIntoLiteralGrammar".into(),
        Sequence::new(vec![
            Ref::keyword("MERGE").to_matchable(),
            Ref::keyword("INTO").optional().to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // T-SQL specific JOIN hints grammar - add HASH/MERGE/LOOP hints to JoinTypeKeywordsGrammar
    dialect.add([(
        "JoinTypeKeywordsGrammar".into(),
        one_of(vec![
            Ref::keyword("CROSS").to_matchable(),
            // T-SQL specific: INNER with optional hints
            Sequence::new(vec![
                Ref::keyword("INNER").to_matchable(),
                one_of(vec![
                    Ref::keyword("HASH").to_matchable(),
                    Ref::keyword("MERGE").to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
            // T-SQL specific: OUTER joins with optional hints
            Sequence::new(vec![
                one_of(vec![
                    Ref::keyword("FULL").to_matchable(),
                    Ref::keyword("LEFT").to_matchable(),
                    Ref::keyword("RIGHT").to_matchable(),
                ])
                .to_matchable(),
                Ref::keyword("OUTER").optional().to_matchable(),
                one_of(vec![
                    Ref::keyword("HASH").to_matchable(),
                    Ref::keyword("MERGE").to_matchable(),
                    Ref::keyword("LOOP").to_matchable(),
                ])
                .config(|this| this.optional())
                .to_matchable(),
            ])
            .to_matchable(),
        ])
        .config(|this| this.optional())
        .to_matchable()
        .into(),
    )]);

    // T-SQL AliasedTableReferenceGrammar override - support inline aliases without indentation
    // This fixes MERGE statements like "MERGE table alias" where alias is on same line
    dialect.add([(
        "AliasedTableReferenceGrammar".into(),
        Sequence::new(vec_of_erased![
            Ref::new("TableReferenceSegment"),
            // T-SQL specific inline alias support (no indent requirement)
            NodeMatcher::new(SyntaxKind::AliasExpression, |_| {
                Sequence::new(vec_of_erased![
                    Ref::keyword("AS").optional(),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("SingleIdentifierGrammar"),
                            Bracketed::new(vec_of_erased![Ref::new("SingleIdentifierListSegment")])
                                .config(|this| this.optional())
                        ]),
                        Ref::new("SingleQuotedIdentifierSegment")
                    ]),
                ])
                .to_matchable()
            }),
        ])
        .to_matchable()
        .into(),
    )]);

    // Override MERGE NOT MATCHED clause to support "BY TARGET" and "BY SOURCE"
    dialect.replace_grammar("MergeNotMatchedClauseSegment", {
        NodeMatcher::new(SyntaxKind::MergeWhenNotMatchedClause, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("WHEN"),
                Ref::keyword("NOT"),
                Ref::keyword("MATCHED"),
                // Optional BY TARGET or BY SOURCE
                Sequence::new(vec_of_erased![
                    Ref::keyword("BY"),
                    one_of(vec_of_erased![
                        Ref::keyword("TARGET"),
                        Ref::keyword("SOURCE")
                    ])
                ])
                .config(|this| this.optional()),
                // Optional AND condition
                Sequence::new(vec_of_erased![
                    Ref::keyword("AND"),
                    Ref::new("ExpressionSegment")
                ])
                .config(|this| this.optional()),
                Ref::keyword("THEN"),
                MetaSegment::indent(),
                one_of(vec_of_erased![
                    Ref::new("MergeInsertClauseSegment"),
                    Ref::new("MergeDeleteClauseSegment"),
                    Ref::new("MergeUpdateClauseSegment")
                ]),
                MetaSegment::dedent(),
            ])
            .to_matchable()
        })
        .to_matchable()
    });

    // Add MergeDeleteClauseSegment for DELETE in MERGE
    dialect.add([(
        "MergeDeleteClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::DeleteStatement, |_| {
            Ref::keyword("DELETE").to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override ColumnDefinitionSegment to support IDENTITY and computed columns
    dialect.replace_grammar("ColumnDefinitionSegment", {
        NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
            one_of(vec_of_erased![
                // Computed column: column AS expression [PERSISTED]
                // Put this first to prioritize AS as a keyword over AS as a data type
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::keyword("AS"),
                    Ref::new("ExpressionSegment"),
                    Ref::keyword("PERSISTED").optional(),
                    // Column constraints after PERSISTED
                    AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")])
                ]),
                // Regular column definition
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("DatatypeSegment"),
                    // Add IDENTITY support
                    Sequence::new(vec_of_erased![
                        Ref::keyword("IDENTITY"),
                        Bracketed::new(vec_of_erased![
                            Ref::new("NumericLiteralSegment"),
                            Ref::new("CommaSegment"),
                            Ref::new("NumericLiteralSegment")
                        ])
                        .config(|this| this.optional())
                    ])
                    .config(|this| this.optional()),
                    // FILESTREAM
                    Ref::keyword("FILESTREAM").optional(),
                    // MASKED WITH (FUNCTION = '...')
                    Sequence::new(vec_of_erased![
                        Ref::keyword("MASKED"),
                        Ref::keyword("WITH"),
                        Bracketed::new(vec_of_erased![
                            Ref::keyword("FUNCTION"),
                            Ref::new("EqualsSegment"),
                            Ref::new("QuotedLiteralSegment")
                        ])
                    ])
                    .config(|this| this.optional()),
                    // Column constraints
                    AnyNumberOf::new(vec_of_erased![Ref::new("ColumnConstraintSegment")])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
    });

    // Add NEXT VALUE FOR sequence expression support
    dialect.add([(
        "NextValueForSegment".into(),
        NodeMatcher::new(SyntaxKind::Expression, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("NEXT"),
                Ref::keyword("VALUE"),
                Ref::keyword("FOR"),
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware NEXT VALUE FOR sequence expression for word token contexts
    dialect.add([(
        "WordAwareNextValueForSegment".into(),
        NodeMatcher::new(SyntaxKind::Expression, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("NEXT", SyntaxKind::Word),
                    StringParser::new("next", SyntaxKind::Word)
                ]),
                one_of(vec_of_erased![
                    StringParser::new("VALUE", SyntaxKind::Word),
                    StringParser::new("value", SyntaxKind::Word)
                ]),
                one_of(vec_of_erased![
                    StringParser::new("FOR", SyntaxKind::Word),
                    StringParser::new("for", SyntaxKind::Word)
                ]),
                Ref::new("ObjectReferenceSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add OPENQUERY support
    dialect.add([(
        "OpenQuerySegment".into(),
        NodeMatcher::new(SyntaxKind::TableReference, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("OPENQUERY"),
                Bracketed::new(vec_of_erased![
                    // Linked server name
                    one_of(vec_of_erased![
                        Ref::new("NakedIdentifierSegment"),
                        Ref::new("QuotedIdentifierSegment")
                    ]),
                    Ref::new("CommaSegment"),
                    // Query string
                    Ref::new("QuotedLiteralSegment")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add OPENDATASOURCE support
    dialect.add([(
        "OpenDataSourceSegment".into(),
        NodeMatcher::new(SyntaxKind::TableReference, |_| {
            Sequence::new(vec_of_erased![
                Ref::keyword("OPENDATASOURCE"),
                Bracketed::new(vec_of_erased![
                    // Provider name
                    Ref::new("QuotedLiteralSegment"),
                    Ref::new("CommaSegment"),
                    // Connection string
                    Ref::new("QuotedLiteralSegment")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // (Removed duplicate OpenRowsetSegment - using the better one defined earlier)

    // (Removed duplicate FromExpressionElementSegment - using the better one defined earlier)

    // Add JSON NULL clause support
    dialect.add([(
        "JsonNullClauseSegment".into(),
        NodeMatcher::new(SyntaxKind::Expression, |_| {
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("NULL"),
                    Ref::keyword("ON"),
                    Ref::keyword("NULL")
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("ABSENT"),
                    Ref::keyword("ON"),
                    Ref::keyword("NULL")
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // NOTE: This FunctionSegment definition is commented out because it's superseded by the later definition
    // that consolidates all function patterns. Keeping for reference.
    // // Override FunctionSegment to add JSON NULL clause support
    // dialect.replace_grammar("FunctionSegment", ...);

    // Add JSON function names to T-SQL as a set
    dialect.sets_mut("json_function_names").extend([
        "JSON_OBJECT",
        "JSON_ARRAY",
        "JSON_VALUE",
        "JSON_QUERY",
        "JSON_MODIFY",
        "ISJSON",
        "JSON_PATH_EXISTS",
    ]);

    // Add datetime unit segment for date part functions
    dialect.add([(
        "DatetimeUnitSegment".into(),
        NodeMatcher::new(SyntaxKind::DatetimeTypeIdentifier, |_| {
            one_of(vec_of_erased![
                // Standard date parts
                StringParser::new("year", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("yy", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("yyyy", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("quarter", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("qq", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("q", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("month", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("mm", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("m", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("dayofyear", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("dy", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("y", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("day", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("dd", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("d", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("week", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("wk", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("ww", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("weekday", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("dw", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("hour", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("hh", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("minute", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("mi", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("n", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("second", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("ss", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("s", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("millisecond", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("ms", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("microsecond", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("mcs", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("nanosecond", SyntaxKind::DatetimeTypeIdentifier),
                StringParser::new("ns", SyntaxKind::DatetimeTypeIdentifier),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add date part function names for proper function recognition
    // Note: Removing DATEADD/DATEDIFF from here so they use regular function pattern
    dialect.add([(
        "DatePartFunctionNameSegment".into(),
        NodeMatcher::new(SyntaxKind::FunctionName, |_| {
            one_of(vec_of_erased![
                StringParser::new("DATENAME", SyntaxKind::FunctionNameIdentifier),
                StringParser::new("DATEPART", SyntaxKind::FunctionNameIdentifier),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Override FunctionSegment to include all T-SQL function patterns
    // This consolidates all previous FunctionSegment definitions
    dialect.replace_grammar("FunctionSegment", {
        one_of(vec_of_erased![
            // JSON functions
            Ref::new("TsqlJsonObjectSegment"),
            Ref::new("TsqlJsonArraySegment"),
            // Date part functions with optional PostFunctionGrammar
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("DatePartFunctionNameSegment"),
                    Ref::new("DateTimeFunctionContentsSegment")
                ]),
                Ref::new("PostFunctionGrammar").optional()
            ]),
            // JSON functions with NULL clause support (from second definition)
            Sequence::new(vec_of_erased![
                Ref::new("FunctionNameSegment"),
                Ref::new("FunctionParameterListGrammar"),
                Ref::new("JsonNullClauseSegment")
            ]),
            // General function pattern with PostFunctionGrammar
            Sequence::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("FunctionNameSegment").exclude(one_of(vec_of_erased![
                        Ref::new("DatePartFunctionNameSegment"),
                        Ref::new("ValuesClauseSegment")
                    ])),
                    Ref::new("FunctionContentsSegment")
                ]),
                Ref::new("PostFunctionGrammar").optional()
            ])
        ])
        .to_matchable()
    });

    // Word token support for T-SQL procedure bodies
    // Keywords inside procedure bodies (after AS) are lexed as 'word' tokens instead of keyword tokens
    // This section adds support for parsing these word tokens as their intended keywords

    // Override IsNullGrammar (defined as Nothing in ANSI) to support both keyword and word tokens
    dialect.add([(
        "IsNullGrammar".into(),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("IS"),
                // Also accept IS as word token in T-SQL procedure bodies
                StringParser::new("IS", SyntaxKind::Keyword)
            ]),
            Ref::keyword("NOT").optional(),
            Ref::new("NullLiteralSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // Override NullLiteralSegment to handle word tokens in procedure bodies
    dialect.add([(
        "NullLiteralSegment".into(),
        one_of(vec_of_erased![
            StringParser::new("null", SyntaxKind::NullLiteral),
            // Also accept NULL as word token in T-SQL procedure bodies
            StringParser::new("NULL", SyntaxKind::Keyword)
        ])
        .to_matchable()
        .into(),
    )]);

    // Override FromClauseSegment to handle word tokens in procedure bodies
    dialect.replace_grammar(
        "FromClauseSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("FROM"),
                // Also accept FROM as word token in T-SQL procedure bodies
                StringParser::new("FROM", SyntaxKind::Keyword)
            ]),
            Delimited::new(vec_of_erased![Ref::new("FromExpressionSegment")]),
        ])
        .to_matchable(),
    );

    // Override WhereClauseSegment to handle word tokens in procedure bodies
    dialect.replace_grammar(
        "WhereClauseSegment",
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                Ref::keyword("WHERE"),
                // Also accept WHERE as word token in T-SQL procedure bodies
                StringParser::new("WHERE", SyntaxKind::Keyword)
            ]),
            MetaSegment::implicit_indent(),
            optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
            MetaSegment::dedent()
        ])
        .to_matchable(),
    );

    // Override JoinKeywordsGrammar to handle word tokens in procedure bodies
    dialect.add([(
        "JoinKeywordsGrammar".into(),
        Sequence::new(vec![
            one_of(vec_of_erased![
                Ref::keyword("JOIN"),
                // Also accept JOIN as word token in T-SQL procedure bodies
                StringParser::new("JOIN", SyntaxKind::Keyword)
            ])
            .to_matchable(),
        ])
        .to_matchable()
        .into(),
    )]);

    // Override JoinOnConditionSegment to handle word tokens in procedure bodies
    dialect.replace_grammar(
        "JoinOnConditionSegment",
        NodeMatcher::new(SyntaxKind::JoinOnCondition, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    // Also accept ON as word token in T-SQL procedure bodies
                    StringParser::new("ON", SyntaxKind::Keyword)
                ]),
                Conditional::new(MetaSegment::implicit_indent()).indented_on_contents(),
                optionally_bracketed(vec_of_erased![Ref::new("ExpressionSegment")]),
                Conditional::new(MetaSegment::dedent()).indented_on_contents()
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    // Word-aware expression parser for T-SQL contexts where keywords are lexed as words
    // This handles expressions like "@var IS NULL" where IS and NULL are word tokens
    dialect.add([(
        "WordAwareExpressionSegment".into(),
        Sequence::new(vec_of_erased![
            // First part of expression (e.g., variable, column, literal)
            one_of(vec_of_erased![
                // Word-aware NEXT VALUE FOR expression
                Ref::new("WordAwareNextValueForSegment"),
                // EXISTS function with word token
                Ref::new("WordAwareExistsFunctionSegment"),
                Ref::new("TsqlVariableSegment"),
                Ref::new("ColumnReferenceSegment"),
                Ref::new("LiteralGrammar"),
                Ref::new("FunctionSegment"),
                // Fixed: Remove recursive reference to prevent malformed AST
                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
            ]),
            // Optional operators and additional expressions
            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                // IS NULL / IS NOT NULL with word tokens
                Sequence::new(vec_of_erased![
                    StringParser::new("IS", SyntaxKind::Word),
                    one_of(vec_of_erased![
                        StringParser::new("NULL", SyntaxKind::Word),
                        Sequence::new(vec_of_erased![
                            StringParser::new("NOT", SyntaxKind::Word),
                            StringParser::new("NULL", SyntaxKind::Word)
                        ])
                    ])
                ]),
                // Comparison operators
                Sequence::new(vec_of_erased![
                    Ref::new("ComparisonOperatorGrammar"),
                    // Fixed: Remove recursive reference to prevent malformed AST
                    Ref::new("ExpressionSegment")
                ]),
                // Binary operators (AND, OR)
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("AND"),
                        Ref::keyword("OR"),
                        StringParser::new("AND", SyntaxKind::Word),
                        StringParser::new("OR", SyntaxKind::Word)
                    ]),
                    // Fixed: Remove recursive reference to prevent malformed AST
                    Ref::new("ExpressionSegment")
                ])
            ])])
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware PRINT statement for when PRINT is lexed as word
    dialect.add([(
        "WordAwarePrintStatementSegment".into(),
        Sequence::new(vec_of_erased![
            StringParser::new("PRINT", SyntaxKind::Word),
            Ref::new("ExpressionSegment") // Expression should work fine with literals
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware SELECT statement for when SELECT is lexed as word
    dialect.add([(
        "WordAwareSelectStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SelectStatement, |_| {
            Sequence::new(vec_of_erased![
                // SELECT clause with word token
                NodeMatcher::new(SyntaxKind::SelectClause, |_| {
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            StringParser::new("SELECT", SyntaxKind::Word),
                            // Also accept lowercase 'select' as word token
                            StringParser::new("select", SyntaxKind::Word)
                        ]),
                        MetaSegment::indent(),
                        Delimited::new(vec_of_erased![one_of(vec_of_erased![
                            // Wildcard expression (* or table.*)
                            Ref::new("WildcardExpressionSegment"),
                            // Simple column reference (o.name)
                            Ref::new("ColumnReferenceSegment"),
                            // Expression (for complex select items)
                            Ref::new("ExpressionSegment")
                        ])])
                        .config(|this| this.allow_trailing()),
                    ])
                    .terminators(vec_of_erased![
                        StringParser::new("FROM", SyntaxKind::Word),
                        Ref::keyword("FROM")
                    ])
                    .to_matchable()
                }),
                // FROM clause with word token
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        StringParser::new("FROM", SyntaxKind::Word),
                        // Also accept lowercase 'from' as word token
                        StringParser::new("from", SyntaxKind::Word)
                    ]),
                    Ref::new("FromExpressionSegment")
                ])
                .config(|this| this.optional()),
                // WHERE clause with word token
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        StringParser::new("WHERE", SyntaxKind::Word),
                        // Also accept lowercase 'where' as word token
                        StringParser::new("where", SyntaxKind::Word)
                    ]),
                    Ref::new("ExpressionSegment")
                ])
                .config(|this| this.optional()),
                // UNION clause with word tokens
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        StringParser::new("UNION", SyntaxKind::Keyword),
                        // Also accept lowercase 'union' as word token
                        StringParser::new("union", SyntaxKind::Keyword)
                    ]),
                    Sequence::new(vec_of_erased![one_of(vec_of_erased![
                        StringParser::new("ALL", SyntaxKind::Keyword),
                        // Also accept lowercase 'all' as word token
                        StringParser::new("all", SyntaxKind::Keyword)
                    ])])
                    .config(|this| this.optional()),
                    Ref::new("WordAwareSelectStatementSegment")
                ])
                .config(|this| this.optional())
            ])
            .terminators(vec_of_erased![
                // Critical: Terminate at ELSE when inside IF statements
                StringParser::new("ELSE", SyntaxKind::Word),
                StringParser::new("else", SyntaxKind::Word),
                StringParser::new("ELSE", SyntaxKind::Keyword),
                Ref::keyword("ELSE"),
                // Also terminate at other common statement keywords
                StringParser::new("END", SyntaxKind::Word),
                StringParser::new("end", SyntaxKind::Word),
                Ref::keyword("END"),
                StringParser::new("GO", SyntaxKind::Word),
                Ref::new("SemicolonSegment")
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware RETURN statement for when RETURN is lexed as word
    dialect.add([(
        "WordAwareReturnStatementSegment".into(),
        Sequence::new(vec_of_erased![
            StringParser::new("RETURN", SyntaxKind::Word),
            // Optional return value
            Ref::new("ExpressionSegment").optional()
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware BEGIN...END block that uses word-aware statements
    dialect.add([(
        "WordAwareBeginEndBlockSegment".into(),
        NodeMatcher::new(SyntaxKind::BeginEndBlock, |_| {
            Sequence::new(vec_of_erased![
                StringParser::new("BEGIN", SyntaxKind::Word),
                Ref::new("DelimiterGrammar").optional(),
                MetaSegment::indent(),
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    Ref::new("WordAwareStatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ])])
                .config(|this| {
                    this.min_times(1);
                    this.terminators = vec_of_erased![
                        StringParser::new("END", SyntaxKind::Word),
                        Ref::keyword("END") // Also check for proper keywords
                    ];
                }),
                MetaSegment::dedent(),
                StringParser::new("END", SyntaxKind::Word)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware IF statement that expects and transforms word tokens  
    dialect.add([(
        "WordAwareIfStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::IfStatement, |_| {
            Sequence::new(vec_of_erased![
                // IF as word token (both uppercase and lowercase)
                one_of(vec_of_erased![
                    StringParser::new("IF", SyntaxKind::Word),
                    StringParser::new("if", SyntaxKind::Word)
                ]),
                // Handle optional NOT keyword for IF NOT EXISTS patterns
                Sequence::new(vec_of_erased![one_of(vec_of_erased![
                    StringParser::new("NOT", SyntaxKind::Word),
                    StringParser::new("not", SyntaxKind::Word)
                ])])
                .config(|this| this.optional()),
                // Expression with word tokens - use targeted parser for exists(select...)
                one_of(vec_of_erased![
                    Ref::new("WordAwareExpressionSegment"),
                    Ref::new("ExpressionSegment")
                ]),
                // Add indentation for the IF body
                MetaSegment::indent(),
                // Use the IF statements container for proper indentation handling
                Ref::new("IfStatementsSegment"),
                // Close indentation for the IF body
                MetaSegment::dedent(),
                // ELSE IF clauses: ELSE IF condition (two keywords)
                AnyNumberOf::new(vec_of_erased![Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ELSE"),
                        StringParser::new("ELSE", SyntaxKind::Word),
                        StringParser::new("else", SyntaxKind::Word)
                    ]),
                    one_of(vec_of_erased![
                        Ref::keyword("IF"),
                        StringParser::new("IF", SyntaxKind::Word),
                        StringParser::new("if", SyntaxKind::Word)
                    ]),
                    one_of(vec_of_erased![
                        Ref::new("WordAwareExpressionSegment"),
                        Ref::new("ExpressionSegment")
                    ]),
                    MetaSegment::indent(),
                    Ref::new("IfStatementsSegment"),
                    MetaSegment::dedent()
                ])]),
                // Optional ELSE clause
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("ELSE"),
                        StringParser::new("ELSE", SyntaxKind::Word),
                        StringParser::new("else", SyntaxKind::Word)
                    ]),
                    MetaSegment::indent(),
                    Ref::new("IfStatementsSegment"),
                    MetaSegment::dedent()
                ])
                .config(|this| this.optional()),
                // Optional delimiter
                Ref::new("DelimiterGrammar").optional()
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // ELSE-aware statement parser for IF statement bodies
    // This ensures statements inside IF bodies properly terminate at ELSE
    dialect.add([(
        "ElseAwareStatementSegment".into(),
        one_of(vec_of_erased![
            // Core statements that I know exist for sure
            Ref::new("BeginEndBlockSegment"),
            Ref::new("DeclareStatementSegment"),
            Ref::new("SetVariableStatementSegment"),
            Ref::new("PrintStatementSegment"),
            Ref::new("IfStatementSegment"),
            Ref::new("WhileStatementSegment"),
            Ref::new("BreakStatementSegment"),
            Ref::new("ContinueStatementSegment"),
            Ref::new("ExecuteStatementSegment"),
            // CRITICAL: Use ELSE-aware selectable grammar instead of regular SelectableGrammar
            Ref::new("ElseAwareSelectableGrammar"),
            // Include some basic ANSI statements that should exist
            Ref::new("InsertStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            // Word-aware CREATE TABLE must come before regular CREATE TABLE for compound statements
            Ref::new("WordAwareCreateTableStatementSegment"),
            // Enhanced CREATE TABLE handles both keywords and word tokens
            Ref::new("CreateTableStatementSegment"),
            // Enhanced CREATE PROCEDURE handles both keywords and word tokens for compound statements
            Ref::new("CreateProcedureStatementSegment"),
            Ref::new("DropTableStatementSegment")
        ])
        .config(|this| {
            this.terminators = vec_of_erased![
                Ref::new("BatchSeparatorGrammar"), // GO statements should terminate
                Ref::keyword("ELSE"),              // ELSE keywords should terminate
                StringParser::new("ELSE", SyntaxKind::NakedIdentifier),
                StringParser::new("ELSE", SyntaxKind::Keyword)
            ]
        })
        .to_matchable()
        .into(),
    )]);

    // ELSE-aware selectable grammar that properly terminates at ELSE keywords
    dialect.add([(
        "ElseAwareSelectableGrammar".into(),
        one_of(vec_of_erased![
            // Just use simple SELECT statement with ELSE termination
            Ref::new("ElseAwareSelectStatementSegment"),
            // Bracketed selectable with ELSE termination
            optionally_bracketed(vec_of_erased![Ref::new("ElseAwareSelectStatementSegment")])
        ])
        .to_matchable()
        .into(),
    )]);

    // ELSE-aware SELECT statement that properly terminates at ELSE
    dialect.add([(
        "ElseAwareSelectStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::SelectStatement, |_| {
            Sequence::new(vec_of_erased![
                Ref::new("SelectClauseSegment"),
                Ref::new("FromClauseSegment").optional(),
                Ref::new("WhereClauseSegment").optional(),
                Ref::new("GroupByClauseSegment").optional(),
                Ref::new("HavingClauseSegment").optional(),
                Ref::new("OrderByClauseSegment").optional(),
                Ref::new("LimitClauseSegment").optional(),
                Ref::new("NamedWindowSegment").optional(),
            ])
            .terminators(vec_of_erased![
                // CRITICAL: Terminate at ELSE for IF statement bodies
                Ref::keyword("ELSE"),
                StringParser::new("ELSE", SyntaxKind::Word),
                StringParser::new("else", SyntaxKind::Word),
                // Also terminate at other logical boundaries
                Ref::keyword("UNION"),
                Ref::keyword("INTERSECT"),
                Ref::keyword("EXCEPT"),
                Ref::keyword("ORDER"),
                Ref::keyword("LIMIT"),
                Ref::keyword("OFFSET"),
                Ref::keyword("FOR"),
                Ref::keyword("OPTION"),
                Ref::keyword("INTO"),
                Ref::keyword("GO"),
                Ref::keyword("RENAME"),
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware WHILE statement for when WHILE is lexed as word
    dialect.add([(
        "WordAwareWhileStatementSegment".into(),
        Sequence::new(vec_of_erased![
            StringParser::new("WHILE", SyntaxKind::Keyword),
            Ref::new("ExpressionSegment"),
            one_of(vec_of_erased![
                // Try word-aware parsers first
                Ref::new("WordAwareBeginEndBlockSegment"),
                Ref::new("WordAwareStatementSegment"),
                Ref::new("BeginEndBlockSegment"),
                Ref::new("StatementSegment")
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware BREAK statement for when BREAK is lexed as word
    dialect.add([(
        "WordAwareBreakStatementSegment".into(),
        StringParser::new("BREAK", SyntaxKind::Keyword)
            .to_matchable()
            .into(),
    )]);

    // Word-aware DROP TABLE statement for when DROP is lexed as word
    dialect.add([(
        "WordAwareDropTableStatementSegment".into(),
        Sequence::new(vec_of_erased![
            // DROP as keyword or word token
            one_of(vec_of_erased![
                StringParser::new("DROP", SyntaxKind::Keyword),
                StringParser::new("DROP", SyntaxKind::Word)
            ]),
            // TABLE as keyword or word token
            one_of(vec_of_erased![
                StringParser::new("TABLE", SyntaxKind::Keyword),
                StringParser::new("TABLE", SyntaxKind::Word)
            ]),
            // Table reference - handle multi-part names with word tokens
            one_of(vec_of_erased![
                Ref::new("TableReferenceSegment"),
                // Fallback: parse as word tokens for procedure contexts
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier)
                    ]),
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        one_of(vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier)
                        ])
                    ])
                    .config(|this| this.optional())
                ])
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware DECLARE statement for when DECLARE is lexed as word
    dialect.add([(
        "WordAwareDeclareStatementSegment".into(),
        Sequence::new(vec_of_erased![
            // DECLARE as keyword or word token
            one_of(vec_of_erased![
                StringParser::new("DECLARE", SyntaxKind::Keyword),
                StringParser::new("DECLARE", SyntaxKind::Word)
            ]),
            Ref::new("TsqlVariableSegment"),
            // Optional AS keyword or word
            Sequence::new(vec_of_erased![one_of(vec_of_erased![
                StringParser::new("AS", SyntaxKind::Keyword),
                StringParser::new("AS", SyntaxKind::Word)
            ])])
            .config(|this| this.optional()),
            // Data type - handle as word tokens in procedure contexts
            one_of(vec_of_erased![
                Ref::new("DatatypeSegment"),
                // Fallback: parse as word token
                TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier)
            ]),
            // Optional assignment with expression - be more restrictive
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    Ref::new("EqualsSegment"),
                    TypedParser::new(
                        SyntaxKind::RawComparisonOperator,
                        SyntaxKind::ComparisonOperator
                    )
                ]),
                // Expression - parse ONLY function calls and literals, don't consume everything
                one_of(vec_of_erased![
                    // Parse simple function call pattern: WORD()
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        Bracketed::new(vec_of_erased![
                            // Empty brackets or simple content
                            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                                TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                                TypedParser::new(
                                    SyntaxKind::NumericLiteral,
                                    SyntaxKind::NumericLiteral
                                ),
                                TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma)
                            ])])
                            .config(|this| this.optional())
                        ])
                    ]),
                    // Parse literals
                    TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                    TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral),
                    // Parse simple identifiers
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier)
                ])
            ])
            .config(|this| this.optional())
        ])
        .terminators(vec_of_erased![
            // Terminate at other statement keywords
            StringParser::new("DROP", SyntaxKind::Word),
            StringParser::new("CREATE", SyntaxKind::Word),
            StringParser::new("INSERT", SyntaxKind::Word),
            StringParser::new("SELECT", SyntaxKind::Word),
            StringParser::new("UPDATE", SyntaxKind::Word),
            StringParser::new("DELETE", SyntaxKind::Word),
            StringParser::new("SET", SyntaxKind::Word),
            StringParser::new("BEGIN", SyntaxKind::Word),
            StringParser::new("IF", SyntaxKind::Word),
            StringParser::new("WHILE", SyntaxKind::Word),
            // Also terminate at keywords
            Ref::keyword("DROP"),
            Ref::keyword("CREATE"),
            Ref::keyword("INSERT"),
            Ref::keyword("SELECT"),
            Ref::keyword("UPDATE"),
            Ref::keyword("DELETE"),
            Ref::keyword("SET"),
            Ref::keyword("BEGIN"),
            Ref::keyword("IF"),
            Ref::keyword("WHILE"),
            // Terminate at semicolons and GO
            TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon),
            StringParser::new("GO", SyntaxKind::Word),
            Ref::keyword("GO")
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware SET statement for when SET is lexed as word
    dialect.add([(
        "WordAwareSetStatementSegment".into(),
        Sequence::new(vec_of_erased![
            one_of(vec_of_erased![
                StringParser::new("SET", SyntaxKind::Keyword),
                // Also accept lowercase 'set' as word token
                StringParser::new("set", SyntaxKind::Keyword)
            ]),
            Ref::new("TsqlVariableSegment"),
            Ref::new("EqualsSegment"),
            Ref::new("ExpressionSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware EXISTS function for when EXISTS is lexed as word
    dialect.add([(
        "WordAwareExistsFunctionSegment".into(),
        NodeMatcher::new(SyntaxKind::Expression, |_| {
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![
                    StringParser::new("EXISTS", SyntaxKind::Keyword),
                    // Also accept lowercase 'exists' as word token
                    StringParser::new("exists", SyntaxKind::Keyword)
                ]),
                Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                    // Try word-aware SELECT first
                    Ref::new("WordAwareSelectStatementSegment"),
                    // Regular SELECT statement as fallback
                    Ref::new("SelectStatementSegment"),
                    // Fallback: SelectableGrammar (like regular EXISTS)
                    Ref::new("SelectableGrammar"),
                    // Last resort: Generic word-aware statement that can handle anything
                    Ref::new("WordAwareGenericSelectSegment")
                ])])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Generic word-aware SELECT segment for robust parsing of SELECT with word tokens
    dialect.add([(
        "WordAwareGenericSelectSegment".into(),
        NodeMatcher::new(SyntaxKind::SelectStatement, |_| {
            Sequence::new(vec_of_erased![
                // Start with SELECT (keyword or word)
                one_of(vec_of_erased![
                    StringParser::new("SELECT", SyntaxKind::Keyword),
                    StringParser::new("select", SyntaxKind::Keyword),
                    Ref::keyword("SELECT")
                ]),
                // Consume everything until we hit a closing bracket or terminator
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(SyntaxKind::Star, SyntaxKind::Star),
                    TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                    TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::SingleQuote),
                    TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                    TypedParser::new(
                        SyntaxKind::RawComparisonOperator,
                        SyntaxKind::RawComparisonOperator
                    ),
                    TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable),
                    Ref::new("LiteralGrammar"),
                    // Allow nested subqueries
                    Bracketed::new(vec_of_erased![Ref::new("WordAwareGenericSelectSegment")])
                ])])
                .config(|this| {
                    this.min_times(1);
                    // Stop at closing bracket or common terminators
                    this.terminators = vec_of_erased![
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        StringParser::new(")", SyntaxKind::EndBracket),
                        // Also stop at statement terminators
                        StringParser::new("GO", SyntaxKind::Word),
                        Ref::new("SemicolonSegment")
                    ];
                })
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Special parser for single statements that must stop before ELSE
    dialect.add([(
        "WordAwareSingleStatementBeforeElse".into(),
        // Try word-aware parsers that know to stop at ELSE
        one_of(vec_of_erased![
            // Word-aware SELECT that stops at ELSE
            Ref::new("WordAwareSelectStatementSegment"),
            // Word-aware SET statement (for variable assignments)
            Ref::new("WordAwareSetStatementSegment"),
            // Word-aware PRINT statement
            Ref::new("WordAwarePrintStatementSegment"),
            // Word-aware DECLARE statement
            Ref::new("WordAwareDeclareStatementSegment"),
            // Generic word statement that stops at ELSE with improved terminators
            AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable),
                TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                TypedParser::new(
                    SyntaxKind::RawComparisonOperator,
                    SyntaxKind::RawComparisonOperator
                ),
                TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::SingleQuote),
                TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon),
                TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                TypedParser::new(SyntaxKind::Star, SyntaxKind::Star),
                Ref::new("LiteralGrammar"),
            ])])
            .config(|this| {
                this.min_times(1);
                // Critical: Stop at ELSE when parsing IF body - improved terminators
                this.terminators = vec_of_erased![
                    StringParser::new("ELSE", SyntaxKind::Word),
                    StringParser::new("else", SyntaxKind::Word),
                    StringParser::new("ELSE", SyntaxKind::Keyword),
                    StringParser::new("else", SyntaxKind::Keyword),
                    Ref::keyword("ELSE"),
                    StringParser::new("IF", SyntaxKind::Word),
                    StringParser::new("if", SyntaxKind::Word),
                    StringParser::new("BEGIN", SyntaxKind::Word),
                    StringParser::new("begin", SyntaxKind::Word),
                    StringParser::new("END", SyntaxKind::Word),
                    StringParser::new("end", SyntaxKind::Word),
                    StringParser::new("GO", SyntaxKind::Word),
                ];
            })
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware CREATE INDEX statement for when CREATE is lexed as word
    dialect.add([(
        "WordAwareCreateIndexStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_| {
            Sequence::new(vec_of_erased![
                // CREATE as keyword or word token
                one_of(vec_of_erased![
                    Ref::keyword("CREATE"),
                    StringParser::new("CREATE", SyntaxKind::Word)
                ]),
                // UNIQUE is optional - handle as keyword or word
                one_of(vec_of_erased![
                    Ref::keyword("UNIQUE"),
                    StringParser::new("UNIQUE", SyntaxKind::Word)
                ])
                .config(|this| this.optional()),
                // CLUSTERED or NONCLUSTERED - handle as keyword or word
                one_of(vec_of_erased![
                    Ref::keyword("CLUSTERED"),
                    StringParser::new("CLUSTERED", SyntaxKind::Word),
                    Ref::keyword("NONCLUSTERED"),
                    StringParser::new("NONCLUSTERED", SyntaxKind::Word)
                ])
                .config(|this| this.optional()),
                // INDEX as keyword or word token
                one_of(vec_of_erased![
                    Ref::keyword("INDEX"),
                    StringParser::new("INDEX", SyntaxKind::Word)
                ]),
                // Index name - consume until ON
                one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                    Ref::new("SingleIdentifierGrammar")
                ]),
                // ON as keyword or word token
                one_of(vec_of_erased![
                    Ref::keyword("ON"),
                    StringParser::new("ON", SyntaxKind::Word)
                ]),
                // Table reference - handle multi-part names
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        Ref::new("SingleIdentifierGrammar")
                    ]),
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        one_of(vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                            Ref::new("SingleIdentifierGrammar")
                        ])
                    ])
                    .config(|this| this.optional())
                ]),
                // Column list
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                            Ref::new("SingleIdentifierGrammar")
                        ]),
                        // Optional ASC/DESC
                        one_of(vec_of_erased![
                            StringParser::new("ASC", SyntaxKind::Word),
                            StringParser::new("DESC", SyntaxKind::Word)
                        ])
                        .config(|this| this.optional())
                    ])
                ])]),
                // Optional INCLUDE clause
                Sequence::new(vec_of_erased![
                    StringParser::new("INCLUDE", SyntaxKind::Word),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                            Ref::new("SingleIdentifierGrammar")
                        ]
                    )])])
                ])
                .config(|this| this.optional()),
                // Optional WHERE clause for filtered indexes
                Sequence::new(vec_of_erased![
                    StringParser::new("WHERE", SyntaxKind::Word),
                    // Simple expression parsing for WHERE clause
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        TypedParser::new(
                            SyntaxKind::RawComparisonOperator,
                            SyntaxKind::RawComparisonOperator
                        ),
                        Ref::new("LiteralGrammar")
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            StringParser::new("WITH", SyntaxKind::Word),
                            StringParser::new("ON", SyntaxKind::Word),
                            TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon),
                            StringParser::new("GO", SyntaxKind::Word)
                        ];
                    })
                ])
                .config(|this| this.optional()),
                // Optional WITH clause
                Sequence::new(vec_of_erased![
                    StringParser::new("WITH", SyntaxKind::Word),
                    one_of(vec_of_erased![
                        // WITH (options)
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![
                            Sequence::new(vec_of_erased![
                                TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                                TypedParser::new(
                                    SyntaxKind::RawComparisonOperator,
                                    SyntaxKind::RawComparisonOperator
                                ),
                                one_of(vec_of_erased![
                                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                                    TypedParser::new(
                                        SyntaxKind::NumericLiteral,
                                        SyntaxKind::NumericLiteral
                                    ),
                                    // Handle nested ONLINE = ON (...)
                                    Sequence::new(vec_of_erased![
                                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                                        Bracketed::new(vec_of_erased![AnyNumberOf::new(
                                            vec_of_erased![one_of(vec_of_erased![
                                                TypedParser::new(
                                                    SyntaxKind::Word,
                                                    SyntaxKind::Word
                                                ),
                                                TypedParser::new(
                                                    SyntaxKind::Comma,
                                                    SyntaxKind::Comma
                                                ),
                                                TypedParser::new(
                                                    SyntaxKind::RawComparisonOperator,
                                                    SyntaxKind::RawComparisonOperator
                                                ),
                                                TypedParser::new(
                                                    SyntaxKind::NumericLiteral,
                                                    SyntaxKind::NumericLiteral
                                                ),
                                                TypedParser::new(
                                                    SyntaxKind::StartBracket,
                                                    SyntaxKind::StartBracket
                                                ),
                                                TypedParser::new(
                                                    SyntaxKind::EndBracket,
                                                    SyntaxKind::EndBracket
                                                )
                                            ])]
                                        )])
                                    ])
                                ])
                            ])
                        ])]),
                        // WITH FILLFACTOR = n
                        Sequence::new(vec_of_erased![
                            StringParser::new("FILLFACTOR", SyntaxKind::Word),
                            TypedParser::new(
                                SyntaxKind::RawComparisonOperator,
                                SyntaxKind::RawComparisonOperator
                            ),
                            TypedParser::new(
                                SyntaxKind::NumericLiteral,
                                SyntaxKind::NumericLiteral
                            )
                        ]),
                        // WITH DATA_COMPRESSION = ROW/PAGE
                        Sequence::new(vec_of_erased![
                            StringParser::new("DATA_COMPRESSION", SyntaxKind::Word),
                            TypedParser::new(
                                SyntaxKind::RawComparisonOperator,
                                SyntaxKind::RawComparisonOperator
                            ),
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                            // Optional ON PARTITIONS clause
                            Sequence::new(vec_of_erased![
                                StringParser::new("ON", SyntaxKind::Word),
                                StringParser::new("PARTITIONS", SyntaxKind::Word),
                                Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![
                                    one_of(vec_of_erased![
                                        TypedParser::new(
                                            SyntaxKind::NumericLiteral,
                                            SyntaxKind::NumericLiteral
                                        ),
                                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                                        StringParser::new("TO", SyntaxKind::Word)
                                    ])
                                ])])
                            ])
                            .config(|this| this.optional())
                        ])
                    ])
                ])
                .config(|this| this.optional()),
                // Optional ON filegroup/partition
                Sequence::new(vec_of_erased![
                    StringParser::new("ON", SyntaxKind::Word),
                    one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        Ref::new("SingleIdentifierGrammar")
                    ])
                ])
                .config(|this| this.optional())
            ])
            .terminators(vec_of_erased![
                // Terminate at GO keywords to allow next statements
                StringParser::new("GO", SyntaxKind::Word),
                Ref::keyword("GO"),
                // Terminate at other CREATE statements
                StringParser::new("CREATE", SyntaxKind::Word),
                Ref::keyword("CREATE"),
                // Terminate at semicolons
                TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Add back WordAwareCreateTableStatementSegment for batch contexts where CREATE TABLE appears as word tokens
    // Add WordAwareInsertStatementSegment for procedure bodies with word tokens
    // Add compound statement parser for DECLARE followed by other statements
    // Add compound parser for CREATE statements that appear together
    dialect.add([(
        "CompoundCreateStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            one_of(vec_of_erased![
                // Handle CREATE TABLE that appears after other CREATE statements
                Sequence::new(vec_of_erased![
                    StringParser::new("CREATE", SyntaxKind::Word),
                    StringParser::new("TABLE", SyntaxKind::Word),
                    // Table name with multi-part reference
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        Sequence::new(vec_of_erased![
                            TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier)
                        ])
                        .config(|this| this.optional())
                    ]),
                    // Column definitions in brackets - consume all content
                    Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![one_of(
                        vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                            TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                            TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                            TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                            TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                            TypedParser::new(
                                SyntaxKind::NumericLiteral,
                                SyntaxKind::NumericLiteral
                            ),
                            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral)
                        ]
                    )])])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "CompoundDeclareStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                // First: DECLARE statement
                Sequence::new(vec_of_erased![
                    StringParser::new("DECLARE", SyntaxKind::Word),
                    Ref::new("TsqlVariableSegment"),
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier), // Data type
                    // Optional assignment
                    Sequence::new(vec_of_erased![
                        TypedParser::new(
                            SyntaxKind::RawComparisonOperator,
                            SyntaxKind::ComparisonOperator
                        ),
                        // Function call: WORD()
                        Sequence::new(vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            Bracketed::new(vec_of_erased![]).config(|this| this.optional())
                        ])
                    ])
                    .config(|this| this.optional())
                ]),
                // Second: DROP TABLE statement
                Sequence::new(vec_of_erased![
                    StringParser::new("DROP", SyntaxKind::Word),
                    StringParser::new("TABLE", SyntaxKind::Word),
                    // Table reference with quoted identifiers
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier)
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // CompoundBatchStatementSegment for complex CREATE INDEX GO CREATE INDEX ; CREATE TABLE patterns
    dialect.add([(
        "CompoundBatchStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            Sequence::new(vec_of_erased![
                // First CREATE INDEX (CLUSTERED)
                Sequence::new(vec_of_erased![
                    StringParser::new("CREATE", SyntaxKind::Word),
                    StringParser::new("CLUSTERED", SyntaxKind::Word),
                    StringParser::new("INDEX", SyntaxKind::Word),
                    // Consume tokens until GO or end
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket)
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators =
                            vec_of_erased![StringParser::new("GO", SyntaxKind::Word)];
                    })
                ]),
                // GO separator
                StringParser::new("GO", SyntaxKind::Word),
                // Second CREATE INDEX (NONCLUSTERED)
                Sequence::new(vec_of_erased![
                    StringParser::new("CREATE", SyntaxKind::Word),
                    StringParser::new("NONCLUSTERED", SyntaxKind::Word),
                    StringParser::new("INDEX", SyntaxKind::Word),
                    // Consume tokens until semicolon
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma)
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![TypedParser::new(
                            SyntaxKind::Semicolon,
                            SyntaxKind::Semicolon
                        )];
                    })
                ]),
                // Semicolon separator
                TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon),
                // CREATE TABLE statement
                Sequence::new(vec_of_erased![
                    StringParser::new("CREATE", SyntaxKind::Word),
                    StringParser::new("TABLE", SyntaxKind::Word),
                    // Table name (schema.table)
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word)
                    ]),
                    // Column definitions in brackets - use generic token consumption for maximum compatibility
                    Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![one_of(
                        vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                            TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                            TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                            TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                            TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                            TypedParser::new(
                                SyntaxKind::NumericLiteral,
                                SyntaxKind::NumericLiteral
                            ),
                            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral),
                            TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
                        ]
                    )])])
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // FallbackCreateTableSegment for CREATE TABLE patterns that appear in complex compound contexts
    dialect.add([(
        "FallbackCreateTableSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec_of_erased![
                // Look for CREATE TABLE pattern anywhere in the token stream
                StringParser::new("CREATE", SyntaxKind::Word),
                StringParser::new("TABLE", SyntaxKind::Word),
                // Table name (handle schema.table or just table)
                TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                // Optional schema.table pattern
                Sequence::new(vec_of_erased![
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word)
                ])
                .config(|this| {
                    this.optional();
                }),
                // Opening bracket for column definitions
                TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                // Column definitions - consume everything until closing bracket
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                    TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                    TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                    TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                    TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                    TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral)
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        // Stop at the closing bracket of the column definitions
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket)
                    ];
                }),
                // Closing bracket for column definitions
                TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "WordAwareInsertStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::InsertStatement, |_| {
            Sequence::new(vec_of_erased![
                // INSERT as word token (for procedure contexts)
                StringParser::new("INSERT", SyntaxKind::Word),
                // INTO as word token
                StringParser::new("INTO", SyntaxKind::Word),
                // Table reference - handle multi-part names
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        Ref::new("SingleIdentifierGrammar")
                    ]),
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        one_of(vec_of_erased![
                            TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                            Ref::new("SingleIdentifierGrammar")
                        ])
                    ])
                    .config(|this| this.optional())
                ]),
                // Column list in brackets (optional)
                Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![one_of(
                    vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma)
                    ]
                )])])
                .config(|this| this.optional()),
                // SELECT statement or VALUES - consume rest as tokens for now
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                    TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                    TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                    TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                    TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                    TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral)
                ])])
            ])
            .terminators(vec_of_erased![
                // Terminate at GO keywords to allow next statements
                StringParser::new("GO", SyntaxKind::Word),
                Ref::keyword("GO"),
                // Terminate at semicolons
                TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    dialect.add([(
        "WordAwareCreateTableStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateTableStatement, |_| {
            Sequence::new(vec_of_erased![
                // CREATE as word token (for batch contexts)
                StringParser::new("CREATE", SyntaxKind::Word),
                // TABLE as word token
                StringParser::new("TABLE", SyntaxKind::Word),
                // Simplified: consume all tokens until opening bracket
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot)
                ])])
                .config(|this| this.min_times(1)),
                // Column definitions in brackets - use generic token consumption for maximum compatibility
                Bracketed::new(vec_of_erased![AnyNumberOf::new(vec_of_erased![one_of(
                    vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                        TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral),
                        TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
                    ]
                )])])
            ])
            .terminators(vec_of_erased![
                // Terminate at GO keywords to allow next statements
                StringParser::new("GO", SyntaxKind::Word),
                Ref::keyword("GO"),
                // Terminate at other CREATE statements
                StringParser::new("CREATE", SyntaxKind::Word),
                Ref::keyword("CREATE"),
                // Terminate at semicolons
                TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware column definition for CREATE TABLE contexts where keywords are lexed as words
    dialect.add([(
        "WordAwareColumnDefinitionSegment".into(),
        NodeMatcher::new(SyntaxKind::ColumnDefinition, |_| {
            Sequence::new(vec_of_erased![
                // Column name - handle bracketed identifiers
                one_of(vec_of_erased![
                    // Bracketed identifier like [ID]
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
                    // Regular word token
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::NakedIdentifier),
                    // Standard identifier grammar
                    Ref::new("SingleIdentifierGrammar")
                ]),
                // Column type as word token
                one_of(vec_of_erased![
                    // Simple types: INT, BIGINT, etc.
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    // Types with parameters: VARCHAR(100), DECIMAL(16,2)
                    Sequence::new(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word), // type name
                        Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                            // Single parameter: VARCHAR(100)
                            TypedParser::new(
                                SyntaxKind::NumericLiteral,
                                SyntaxKind::NumericLiteral
                            ),
                            // Two parameters: DECIMAL(16,2)
                            Sequence::new(vec_of_erased![
                                TypedParser::new(
                                    SyntaxKind::NumericLiteral,
                                    SyntaxKind::NumericLiteral
                                ),
                                TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                                TypedParser::new(
                                    SyntaxKind::NumericLiteral,
                                    SyntaxKind::NumericLiteral
                                )
                            ])
                        ])])
                    ])
                ]),
                // Column constraints as word tokens
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    // PRIMARY KEY
                    Sequence::new(vec_of_erased![
                        StringParser::new("PRIMARY", SyntaxKind::Word),
                        StringParser::new("KEY", SyntaxKind::Word)
                    ]),
                    // NOT NULL
                    Sequence::new(vec_of_erased![
                        StringParser::new("NOT", SyntaxKind::Word),
                        StringParser::new("NULL", SyntaxKind::Word)
                    ]),
                    // NULL
                    StringParser::new("NULL", SyntaxKind::Word),
                    // DEFAULT expression
                    Sequence::new(vec_of_erased![
                        StringParser::new("DEFAULT", SyntaxKind::Word),
                        one_of(vec_of_erased![
                            // DEFAULT (NEXT VALUE FOR ...)
                            Bracketed::new(vec_of_erased![Ref::new(
                                "WordAwareNextValueForSegment"
                            )]),
                            // DEFAULT literal
                            Ref::new("LiteralGrammar")
                        ])
                    ])
                ])])
                .config(|this| this.optional())
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware CREATE/UPDATE/DROP STATISTICS statements
    dialect.add([(
        "WordAwareStatisticsStatementSegment".into(),
        NodeMatcher::new(SyntaxKind::Statement, |_| {
            one_of(vec_of_erased![
                // CREATE STATISTICS
                Sequence::new(vec_of_erased![
                    StringParser::new("CREATE", SyntaxKind::Keyword),
                    StringParser::new("STATISTICS", SyntaxKind::Keyword),
                    // Consume rest as tokens
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DoubleQuote),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        Ref::new("LiteralGrammar")
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            StringParser::new("GO", SyntaxKind::Word),
                            StringParser::new("CREATE", SyntaxKind::Word),
                            StringParser::new("UPDATE", SyntaxKind::Word),
                            StringParser::new("DROP", SyntaxKind::Word),
                            Ref::new("SemicolonSegment")
                        ];
                    })
                ]),
                // UPDATE STATISTICS
                Sequence::new(vec_of_erased![
                    StringParser::new("UPDATE", SyntaxKind::Keyword),
                    StringParser::new("STATISTICS", SyntaxKind::Keyword),
                    // Consume rest as tokens
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DoubleQuote),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        Ref::new("LiteralGrammar")
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            StringParser::new("GO", SyntaxKind::Word),
                            StringParser::new("CREATE", SyntaxKind::Word),
                            StringParser::new("UPDATE", SyntaxKind::Word),
                            StringParser::new("DROP", SyntaxKind::Word),
                            Ref::new("SemicolonSegment")
                        ];
                    })
                ]),
                // DROP STATISTICS
                Sequence::new(vec_of_erased![
                    StringParser::new("DROP", SyntaxKind::Keyword),
                    StringParser::new("STATISTICS", SyntaxKind::Keyword),
                    // Consume rest as tokens
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DoubleQuote),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        Ref::new("LiteralGrammar")
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            StringParser::new("GO", SyntaxKind::Word),
                            StringParser::new("CREATE", SyntaxKind::Word),
                            StringParser::new("UPDATE", SyntaxKind::Word),
                            StringParser::new("DROP", SyntaxKind::Word),
                            Ref::new("SemicolonSegment")
                        ];
                    })
                ]),
                // DROP INDEX
                Sequence::new(vec_of_erased![
                    StringParser::new("DROP", SyntaxKind::Keyword),
                    StringParser::new("INDEX", SyntaxKind::Keyword),
                    // Consume rest as tokens
                    AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                        TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DoubleQuote),
                        TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                        TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                        TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                        TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                        Ref::new("LiteralGrammar")
                    ])])
                    .config(|this| {
                        this.min_times(1);
                        this.terminators = vec_of_erased![
                            StringParser::new("GO", SyntaxKind::Word),
                            StringParser::new("CREATE", SyntaxKind::Word),
                            StringParser::new("UPDATE", SyntaxKind::Word),
                            StringParser::new("DROP", SyntaxKind::Word),
                            Ref::new("SemicolonSegment")
                        ];
                    })
                ])
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Word-aware statement segment for parsing statements when keywords are lexed as words
    dialect.add([(
        "WordAwareStatementSegment".into(),
        // Try to parse word tokens as proper statements
        one_of(vec_of_erased![
            // Use word-aware parsers for statements with word tokens
            // Put CREATE TABLE FIRST with ABSOLUTE HIGHEST priority
            Ref::new("WordAwareCreateTableStatementSegment"),
            // Add fallback CREATE TABLE parser for complex compound contexts
            Ref::new("FallbackCreateTableSegment"),
            // Then compound CREATE statement (which also handles CREATE TABLE patterns)
            Ref::new("CompoundCreateStatementSegment"),
            // Put compound statements next to handle complex patterns
            Ref::new("CompoundDeclareStatementSegment"),
            Ref::new("CompoundBatchStatementSegment"),
            // Put DROP TABLE next to prevent it from being consumed by other parsers
            Ref::new("WordAwareDropTableStatementSegment"),
            // Word-aware TRY/CATCH should come before IF/WHILE to prevent mismatching
            Ref::new("WordAwareTryCatchSegment"),
            Ref::new("WordAwareBeginEndBlockSegment"), // MUST come after TRY/CATCH to avoid consuming BEGIN TRY
            Ref::new("WordAwareIfStatementSegment"),
            Ref::new("WordAwareWhileStatementSegment"),
            Ref::new("WordAwareBreakStatementSegment"),
            Ref::new("WordAwareDeclareStatementSegment"),
            Ref::new("WordAwareSetStatementSegment"),
            Ref::new("WordAwarePrintStatementSegment"),
            Ref::new("WordAwareReturnStatementSegment"),
            Ref::new("WordAwareSelectStatementSegment"), // Re-enabled: Focus on core parsing issues
            Ref::new("WordAwareInsertStatementSegment"), // Handle INSERT INTO in procedure bodies
            Ref::new("WordAwareCreateIndexStatementSegment"),
            Ref::new("WordAwareDropIndexStatementSegment"),
            Ref::new("WordAwareUpdateStatisticsStatementSegment"),
            Ref::new("WordAwareCreateTriggerStatementSegment"),
            Ref::new("WordAwareDropTriggerStatementSegment"),
            Ref::new("WordAwareDisableTriggerStatementSegment"),
            // Enhanced CREATE PROCEDURE handles both keywords and word tokens
            Ref::new("CreateProcedureStatementSegment"),
            Ref::new("WordAwareStatisticsStatementSegment"),
            // GOTO statement (handles both keyword and word forms)
            Ref::new("GotoStatementSegment"),
            // Try regular parsers that already handle word tokens
            Ref::new("BeginEndBlockSegment"),
            Ref::new("DeclareStatementSegment"),
            Ref::new("DropTableStatementSegment"),
            Ref::new("BreakStatementSegment"),
            Ref::new("PrintStatementSegment"),
            Ref::new("ReturnStatementSegment"),
            Ref::new("SelectStatementSegment"),
            Ref::new("TryBlockSegment"),
            Ref::new("ExecuteStatementSegment"),
            Ref::new("SetVariableStatementSegment"),
            Ref::new("CreateIndexStatementSegment"),
            // Fallback to regular parsers
            Ref::new("IfStatementSegment"),
            Ref::new("WhileStatementSegment"),
            // Fallback CREATE parsers for compound statement contexts
            Ref::new("FallbackWordCreateStatementSegment"),
            // Fallback: For truly unparsable content, consume tokens to prevent errors
            Ref::new("GenericWordStatementSegment")
        ])
        .to_matchable()
        .into(),
    )]);

    // Word-aware batch segment for when entire batches have word tokens
    dialect.add([(
        "WordAwareBatchSegment".into(),
        AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
            // Try word-aware statement parsing first
            Sequence::new(vec_of_erased![
                Ref::new("WordAwareStatementSegment"),
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("DelimiterGrammar"),
                    Ref::new("BatchDelimiterGrammar"),
                    StringParser::new("GO", SyntaxKind::Keyword),
                    // Also handle GO as word token
                    StringParser::new("GO", SyntaxKind::Word),
                    // Handle semicolons as statement separators in word contexts
                    TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
                ])
                .config(|this| this.optional())
            ]),
            // Handle WHILE statements at batch level
            Ref::new("WordAwareWhileStatementSegment"),
            Ref::new("WhileStatementSegment"),
            // Also try specific word-aware parsers
            Ref::new("WordAwareCreateProcedureSegment"),
            // Handle standalone TRY-CATCH blocks with word tokens
            Ref::new("WordAwareTryCatchSegment"),
            Ref::new("TryBlockSegment"),
            // Fallback: consume tokens to prevent errors
            Ref::new("GenericWordStatementSegment")
        ])])
        .config(|this| this.min_times(1))
        .to_matchable()
        .into(),
    )]);

    // Word-aware CREATE PROCEDURE segment for when CREATE PROCEDURE is lexed as words
    dialect.add([(
        "WordAwareCreateProcedureSegment".into(),
        NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_| {
            Sequence::new(vec_of_erased![
                // CREATE OR ALTER PROCEDURE
                one_of(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        StringParser::new("CREATE", SyntaxKind::Word),
                        Sequence::new(vec_of_erased![
                            StringParser::new("OR", SyntaxKind::Word),
                            StringParser::new("ALTER", SyntaxKind::Word)
                        ])
                        .config(|this| this.optional()),
                        one_of(vec_of_erased![
                            StringParser::new("PROCEDURE", SyntaxKind::Word),
                            StringParser::new("PROC", SyntaxKind::Word)
                        ])
                    ]),
                    Sequence::new(vec_of_erased![
                        StringParser::new("ALTER", SyntaxKind::Word),
                        one_of(vec_of_erased![
                            StringParser::new("PROCEDURE", SyntaxKind::Word),
                            StringParser::new("PROC", SyntaxKind::Word)
                        ])
                    ])
                ]),
                // Procedure name - could be multi-part
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(SyntaxKind::NakedIdentifier, SyntaxKind::NakedIdentifier),
                    TypedParser::new(SyntaxKind::QuotedIdentifier, SyntaxKind::QuotedIdentifier),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot)
                ])])
                .config(|this| {
                    this.min_times(1);
                    this.terminators = vec_of_erased![
                        TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable),
                        StringParser::new("AS", SyntaxKind::Word)
                    ];
                }),
                // Optional parameters
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable),
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(
                        SyntaxKind::RawComparisonOperator,
                        SyntaxKind::RawComparisonOperator
                    ),
                    TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                    Ref::new("LiteralGrammar")
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![StringParser::new("AS", SyntaxKind::Word)];
                }),
                // AS keyword
                StringParser::new("AS", SyntaxKind::Word),
                // Procedure body - use existing word-aware parsers
                AnyNumberOf::new(vec_of_erased![
                    Ref::new("WordAwareStatementSegment"),
                    Ref::new("DelimiterGrammar").optional()
                ])
                .config(|this| {
                    this.min_times(1);
                    this.parse_mode = ParseMode::Greedy;
                    this.terminators = vec_of_erased![
                        Ref::new("BatchSeparatorGrammar"),
                        StringParser::new("GO", SyntaxKind::Word)
                    ];
                })
            ])
            .to_matchable()
        })
        .to_matchable()
        .into(),
    )]);

    // Fallback CREATE statement parser for compound statement contexts - consumes tokens to prevent unparsable
    dialect.add([(
        "FallbackWordCreateStatementSegment".into(),
        one_of(vec_of_erased![
            // CREATE TABLE with word tokens - simple token consumption to prevent unparsable
            Sequence::new(vec_of_erased![
                StringParser::new("CREATE", SyntaxKind::Word),
                StringParser::new("TABLE", SyntaxKind::Word),
                // Consume table name tokens
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot)
                ])])
                .config(|this| {
                    this.min_times(1);
                    this.max_times(3); // schema.table max
                }),
                // Consume everything until we find what looks like the next statement
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                    TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::DoubleQuote),
                    TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                    TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
                    TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::SingleQuote)
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        StringParser::new("GO", SyntaxKind::Word),
                        StringParser::new("CREATE", SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
                    ];
                })
            ]),
            // CREATE INDEX with word tokens - simple token consumption
            Sequence::new(vec_of_erased![
                StringParser::new("CREATE", SyntaxKind::Word),
                // Optional modifiers
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    StringParser::new("CLUSTERED", SyntaxKind::Word),
                    StringParser::new("NONCLUSTERED", SyntaxKind::Word),
                    StringParser::new("UNIQUE", SyntaxKind::Word)
                ])])
                .config(|this| this.max_times(2)),
                StringParser::new("INDEX", SyntaxKind::Word),
                // Consume everything for the INDEX statement
                AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
                    TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
                    TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
                    TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
                    TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
                    TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma)
                ])])
                .config(|this| {
                    this.terminators = vec_of_erased![
                        StringParser::new("GO", SyntaxKind::Word),
                        StringParser::new("CREATE", SyntaxKind::Word),
                        TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon)
                    ];
                })
            ])
        ])
        .to_matchable()
        .into(),
    )]);

    // Generic word statement for fallback parsing
    dialect.add([(
        "GenericWordStatementSegment".into(),
        AnyNumberOf::new(vec_of_erased![one_of(vec_of_erased![
            TypedParser::new(SyntaxKind::Word, SyntaxKind::Word),
            TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable),
            TypedParser::new(SyntaxKind::Dot, SyntaxKind::Dot),
            TypedParser::new(SyntaxKind::Comma, SyntaxKind::Comma),
            TypedParser::new(
                SyntaxKind::RawComparisonOperator,
                SyntaxKind::RawComparisonOperator
            ),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::SingleQuote),
            TypedParser::new(SyntaxKind::NumericLiteral, SyntaxKind::NumericLiteral),
            TypedParser::new(SyntaxKind::Semicolon, SyntaxKind::Semicolon),
            TypedParser::new(SyntaxKind::StartBracket, SyntaxKind::StartBracket),
            TypedParser::new(SyntaxKind::EndBracket, SyntaxKind::EndBracket),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier),
            TypedParser::new(SyntaxKind::Star, SyntaxKind::Star),
            TypedParser::new(SyntaxKind::UnicodeSingleQuote, SyntaxKind::QuotedLiteral),
            TypedParser::new(SyntaxKind::Plus, SyntaxKind::Plus),
            TypedParser::new(SyntaxKind::Minus, SyntaxKind::Minus),
            Ref::new("LiteralGrammar"),
        ])])
        .config(|this| {
            this.min_times(1);
            // Add terminators to prevent consuming too many tokens
            // Use both word and keyword forms since we might encounter either
            this.terminators = vec_of_erased![
                StringParser::new("IF", SyntaxKind::Word),
                StringParser::new("ELSE", SyntaxKind::Word),
                StringParser::new("BEGIN", SyntaxKind::Word),
                StringParser::new("END", SyntaxKind::Word),
                StringParser::new("TRY", SyntaxKind::Word),
                StringParser::new("CATCH", SyntaxKind::Word),
                StringParser::new("WHILE", SyntaxKind::Word),
                StringParser::new("RETURN", SyntaxKind::Word),
                StringParser::new("PRINT", SyntaxKind::Word),
                StringParser::new("SELECT", SyntaxKind::Word),
                StringParser::new("INSERT", SyntaxKind::Word),
                StringParser::new("UPDATE", SyntaxKind::Word),
                StringParser::new("DELETE", SyntaxKind::Word),
                StringParser::new("SET", SyntaxKind::Word),
                StringParser::new("DECLARE", SyntaxKind::Word),
                StringParser::new("CREATE", SyntaxKind::Word),
                StringParser::new("DROP", SyntaxKind::Word),
                StringParser::new("ALTER", SyntaxKind::Word),
                StringParser::new("GOTO", SyntaxKind::Word),
                // Also add lowercase versions
                StringParser::new("if", SyntaxKind::Word),
                StringParser::new("else", SyntaxKind::Word),
                StringParser::new("begin", SyntaxKind::Word),
                StringParser::new("end", SyntaxKind::Word),
                StringParser::new("while", SyntaxKind::Word),
                StringParser::new("return", SyntaxKind::Word),
                StringParser::new("goto", SyntaxKind::Word),
                StringParser::new("print", SyntaxKind::Word),
                StringParser::new("select", SyntaxKind::Word),
                StringParser::new("insert", SyntaxKind::Word),
                StringParser::new("update", SyntaxKind::Word),
                StringParser::new("delete", SyntaxKind::Word),
                StringParser::new("set", SyntaxKind::Word),
                StringParser::new("declare", SyntaxKind::Word),
                StringParser::new("create", SyntaxKind::Word),
                StringParser::new("drop", SyntaxKind::Word),
                StringParser::new("alter", SyntaxKind::Word),
                // Also add keyword versions
                Ref::keyword("IF"),
                Ref::keyword("ELSE"),
                Ref::keyword("BEGIN"),
                Ref::keyword("END"),
                Ref::keyword("WHILE"),
                Ref::keyword("RETURN"),
                Ref::keyword("PRINT"),
                Ref::keyword("SELECT"),
                Ref::keyword("INSERT"),
                Ref::keyword("UPDATE"),
                Ref::keyword("DELETE"),
                Ref::keyword("SET"),
                Ref::keyword("DECLARE"),
                Ref::keyword("CREATE"),
                Ref::keyword("DROP"),
                Ref::keyword("ALTER"),
                StringParser::new("GO", SyntaxKind::Word),
                Ref::new("SemicolonSegment"),
            ];
        })
        .to_matchable()
        .into(),
    )]);

    // Replace DatatypeSegment to add T-SQL specific types
    dialect.replace_grammar(
        "DatatypeSegment",
        NodeMatcher::new(SyntaxKind::DataType, |_| {
            one_of(vec_of_erased![
                // TABLE type with inline table definition
                Sequence::new(vec_of_erased![
                    StringParser::new("TABLE", SyntaxKind::DataTypeIdentifier),
                    Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![one_of(
                        vec_of_erased![
                            // Column definition
                            Ref::new("ColumnDefinitionSegment"),
                            // Table constraint
                            Ref::new("TableConstraintSegment")
                        ]
                    )])])
                ]),
                // Square bracket data type like [int], [varchar](100)
                Sequence::new(vec_of_erased![
                    Ref::new("QuotedIdentifierSegment"), // Bracketed data type name
                    Ref::new("BracketedArguments").optional()
                ]),
                // ANSI data types
                Sequence::new(vec_of_erased![
                    one_of(vec_of_erased![
                        Ref::keyword("TIME"),
                        Ref::keyword("TIMESTAMP")
                    ]),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                        .config(|this| this.optional()),
                    Sequence::new(vec_of_erased![
                        one_of(vec_of_erased![
                            Ref::keyword("WITH"),
                            Ref::keyword("WITHOUT")
                        ]),
                        Ref::keyword("TIME"),
                        Ref::keyword("ZONE")
                    ])
                    .config(|this| this.optional())
                ]),
                // Array types
                Sequence::new(vec_of_erased![
                    Ref::new("DatatypeIdentifierSegment"),
                    Ref::new("ArrayTypeSegment").optional()
                ]),
                // Parameterized types
                Sequence::new(vec_of_erased![
                    Ref::new("DatatypeIdentifierSegment"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Delimited::new(vec_of_erased![Ref::new("ExpressionSegment")]),
                        Delimited::new(vec_of_erased![Ref::new("DatatypeSegment")])
                    ])])
                ]),
                Ref::new("DatatypeIdentifierSegment"),
                // Interval type
                Sequence::new(vec_of_erased![
                    Ref::keyword("INTERVAL"),
                    one_of(vec_of_erased![
                        Sequence::new(vec_of_erased![
                            Ref::new("QuotedLiteralSegment"),
                            one_of(vec_of_erased![
                                Ref::new("DatetimeUnitSegment"),
                                Sequence::new(vec_of_erased![
                                    Ref::new("DatetimeUnitSegment"),
                                    Ref::keyword("TO"),
                                    Ref::new("DatetimeUnitSegment")
                                ])
                            ])
                        ]),
                        one_of(vec_of_erased![
                            Ref::new("DatetimeUnitSegment"),
                            Sequence::new(vec_of_erased![
                                Ref::new("DatetimeUnitSegment"),
                                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                                    .config(|this| this.optional()),
                                Ref::keyword("TO"),
                                Ref::new("DatetimeUnitSegment"),
                                Bracketed::new(vec_of_erased![Ref::new("ExpressionSegment")])
                                    .config(|this| this.optional())
                            ])
                        ])
                    ])
                ])
            ])
            .to_matchable()
        })
        .to_matchable(),
    );

    dialect.expand();
    dialect
}

