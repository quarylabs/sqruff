use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::base::{Nothing, Ref};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::vec_of_erased;

use crate::{ansi, tsql_keywords};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| {
        dialect.expand();
        
        // Add T-SQL variable support to LiteralGrammar for use in expressions
        // This enables variables to work inside parentheses and other expression contexts
        dialect.add([
            (
                "LiteralGrammar".into(),
                dialect
                    .grammar("LiteralGrammar")
                    .copy(
                        Some(vec_of_erased![Ref::new("ParameterizedSegment")]),
                        None,
                        None,
                        None,
                        Vec::new(),
                        false,
                    )
                    .into(),
            ),
        ]);
        
        // Add T-SQL table variable support for table references
        // This enables @TableVariable to work in FROM clauses
        dialect.replace_grammar(
            "TableReferenceSegment",
            NodeMatcher::new(
                SyntaxKind::TableReference,
                one_of(vec_of_erased![
                    Ref::new("ObjectReferenceSegment"), // Original object references
                    Ref::new("TsqlVariableSegment"),    // T-SQL table variables like @MyTable
                ])
                .to_matchable(),
            )
            .to_matchable()
            .into(),
        );
        
        // T-SQL supports alternative alias syntax: AliasName = Expression
        dialect.replace_grammar(
            "SelectClauseElementSegment",
            ansi::select_clause_element().copy(
                Some(vec_of_erased![
                    // T-SQL alternative alias syntax: AliasName = Expression
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("EqualsSegment"),
                        Ref::new("BaseExpressionElementGrammar")
                    ])
                ]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ).into(),
        );
    })
}

pub fn raw_dialect() -> Dialect {
    let mut dialect = ansi::raw_dialect();
    dialect.name = DialectKind::Tsql;

    // Set T-SQL specific keywords
    dialect.sets_mut("reserved_keywords").clear();
    dialect
        .sets_mut("reserved_keywords")
        .extend(tsql_keywords::tsql_reserved_keywords());
    dialect.sets_mut("unreserved_keywords").clear();
    dialect
        .sets_mut("unreserved_keywords")
        .extend(tsql_keywords::tsql_unreserved_keywords());
    
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
        "%=", "&=", "*=", "+=", "-=", "/=", "^=", "|=", "!<", "!>",
    ]);

    // T-SQL supports square brackets for identifiers and @ for variables
    dialect.insert_lexer_matchers(
        vec![
            Matcher::regex("tsql_square_bracket_identifier", r"\[[^\]]*\]", SyntaxKind::DoubleQuote),
            Matcher::regex("tsql_variable", r"@[a-zA-Z_][a-zA-Z0-9_]*", SyntaxKind::TsqlVariable),
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
    dialect.sets_mut("aggregate_functions").extend([
        "STRING_AGG",
    ]);
    
    dialect.sets_mut("special_functions").extend([
        "COALESCE",
        "NULLIF",
        "ISNULL",
    ]);

    // T-SQL datetime units
    dialect.sets_mut("datetime_units").extend([
        "YEAR", "YY", "YYYY",
        "QUARTER", "QQ", "Q",
        "MONTH", "MM", "M",
        "DAYOFYEAR", "DY", "Y",
        "DAY", "DD", "D",
        "WEEK", "WK", "WW",
        "WEEKDAY", "DW",
        "HOUR", "HH",
        "MINUTE", "MI", "N",
        "SECOND", "SS", "S",
        "MILLISECOND", "MS",
        "MICROSECOND", "MCS",
        "NANOSECOND", "NS",
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
    
    // TOP clause support
    dialect.add([
        (
            "TopClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseModifier, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "TopClauseGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("TOP"),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Bracketed::new(vec_of_erased![Ref::new("NumericLiteralSegment")])
                ]),
                Ref::keyword("PERCENT").optional(),
                Ref::keyword("WITH").optional(),
                Ref::keyword("TIES").optional()
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // DECLARE statement
    dialect.add([
        (
            "DeclareStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "DeclareStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("DECLARE"),
                Delimited::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
                        Ref::new("TsqlVariableSegment"),
                        Ref::new("DatatypeSegment"),
                        Sequence::new(vec_of_erased![
                            Ref::new("EqualsSegment"),
                            Ref::new("ExpressionSegment")
                        ])
                        .config(|this| this.optional())
                    ])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // SET statement for variables
    dialect.add([
        (
            "SetVariableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "SetVariableStatementGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("SET"),
                Ref::new("TsqlVariableSegment"),
                Ref::new("EqualsSegment"),
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
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
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

    // BEGIN...END blocks
    dialect.add([
        (
            "BeginEndBlockSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "BeginEndBlockGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("BEGIN"),
                AnyNumberOf::new(vec_of_erased![Ref::new("StatementSegment")])
                    .config(|this| this.min_times(1)),
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
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
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
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
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
            NodeMatcher::new(SyntaxKind::TableExpression, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "PivotUnpivotGrammar".into(),
            one_of(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::keyword("PIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("FunctionSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("LiteralGrammar")])
                        ])
                    ])
                ]),
                Sequence::new(vec_of_erased![
                    Ref::keyword("UNPIVOT"),
                    Bracketed::new(vec_of_erased![
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("FOR"),
                        Ref::new("ColumnReferenceSegment"),
                        Ref::keyword("IN"),
                        Bracketed::new(vec_of_erased![
                            Delimited::new(vec_of_erased![Ref::new("ColumnReferenceSegment")])
                        ])
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
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
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
            Ref::new("DeclareStatementGrammar"),
            Ref::new("SetVariableStatementGrammar"),
            Ref::new("PrintStatementGrammar"),
            Ref::new("BeginEndBlockGrammar"),
            Ref::new("IfStatementGrammar"),
            Ref::new("WhileStatementGrammar"),
            Ref::new("BatchSeparatorGrammar"),
            Ref::new("UseStatementGrammar"),
            // Include base statement types that exist in ANSI
            Ref::new("SelectableGrammar"),
            Ref::new("InsertStatementSegment"),
            Ref::new("UpdateStatementSegment"),
            Ref::new("DeleteStatementSegment"),
            Ref::new("CreateTableStatementSegment"),
            Ref::new("AlterTableStatementSegment"),
            Ref::new("DropTableStatementSegment")
        ])
        .to_matchable()
        .into(),
    );

    // Update SELECT to include TOP clause
    dialect.replace_grammar(
        "SelectClauseSegment",
        Sequence::new(vec_of_erased![
            Ref::keyword("SELECT"),
            Ref::new("SelectClauseModifierSegment").optional(),
            Ref::new("TopClauseGrammar").optional(),
            Ref::new("SelectClauseElementSegment"),
            AnyNumberOf::new(vec_of_erased![
                Sequence::new(vec_of_erased![
                    Ref::new("CommaSegment"),
                    Ref::new("SelectClauseElementSegment")
                ])
            ])
        ])
        .to_matchable()
        .into(),
    );

    // USE statement for changing database context
    dialect.add([
        (
            "UseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, Nothing::new().to_matchable()).to_matchable().into(),
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
    
    // Add variable reference support
    // Note: We add this as a new segment type instead of replacing SingleIdentifierGrammar
    dialect.add([
        (
            "TsqlVariableSegment".into(),
            TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable).to_matchable().into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ParameterizedExpression,
                Ref::new("TsqlVariableSegment").to_matchable(),
            ).to_matchable().into(),
        ),
        (
            "TsqlTableVariableSegment".into(),
            NodeMatcher::new(
                SyntaxKind::TableReference,
                Ref::new("TsqlVariableSegment").to_matchable(),
            ).to_matchable().into(),
        ),
    ]);
    
    // Update TableReferenceSegment to support T-SQL table variables directly
    // (Must be done after TsqlVariableSegment is defined)
    dialect.replace_grammar(
        "TableReferenceSegment",
        NodeMatcher::new(
            SyntaxKind::TableReference,
            one_of(vec_of_erased![
                Ref::new("ObjectReferenceSegment"), // Original object references
                Ref::new("TsqlVariableSegment"),    // T-SQL table variables like @MyTable
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
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
        .to_matchable()
        .into(),
    );
    
    
    // Table hints support
    dialect.add([
        (
            "TableHintSegment".into(),
            NodeMatcher::new(SyntaxKind::Expression, Nothing::new().to_matchable()).to_matchable().into(),
        ),
        (
            "TableHintGrammar".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![
                    Delimited::new(vec_of_erased![
                        one_of(vec_of_erased![
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
                            Ref::keyword("INDEX"),
                            Ref::keyword("FORCESEEK"),
                            Ref::keyword("FORCESCAN"),
                            Ref::keyword("HOLDLOCK"),
                            Ref::keyword("SNAPSHOT"),
                            Ref::new("NumericLiteralSegment"), // For INDEX(...) and other hints with parameters
                            Ref::new("NakedIdentifierSegment"), // For dynamic hint values
                        ])
                    ])
                ])
            ])
            .to_matchable()
            .into(),
        ),
    ]);

    // Override PostTableExpressionGrammar to include table hints
    dialect.add([
        (
            "PostTableExpressionGrammar".into(),
            Ref::new("TableHintGrammar").optional().to_matchable().into(),
        ),
    ]);
    
    // T-SQL alternative alias syntax: AliasName = Expression
    // This allows SELECT UUID = CAST(u_uuid AS CHAR(36)) syntax
    dialect.replace_grammar(
        "SelectClauseElementSegment",
        ansi::select_clause_element().copy(
            Some(vec_of_erased![
                // T-SQL alternative alias syntax: AliasName = Expression
                Sequence::new(vec_of_erased![
                    Ref::new("SingleIdentifierGrammar"),
                    Ref::new("EqualsSegment"),
                    Ref::new("BaseExpressionElementGrammar")
                ])
            ]),
            None,
            None,
            None,
            Vec::new(),
            false,
        ).into(),
    );

    dialect
}