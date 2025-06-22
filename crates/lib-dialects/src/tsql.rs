use sqruff_lib_core::dialects::base::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of};
use sqruff_lib_core::parser::grammar::base::Ref;
use sqruff_lib_core::parser::grammar::conditional::Conditional;
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::MatchableTrait;
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::TypedParser;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;
use sqruff_lib_core::vec_of_erased;

use crate::{ansi, tsql_keywords};

pub fn dialect() -> Dialect {
    raw_dialect().config(|dialect| {
        // T-SQL supports alternative alias syntax: AliasName = Expression
        dialect.replace_grammar(
            "SelectClauseElementSegment",
            ansi::select_clause_element().copy(
                Some(vec_of_erased![
                    // T-SQL alternative alias syntax: AliasName = Expression
                    Sequence::new(vec_of_erased![
                        Ref::new("SingleIdentifierGrammar"),
                        Ref::new("AssignmentOperatorSegment"),
                        Ref::new("ExpressionSegment")
                    ])
                ]),
                None,
                None,
                None,
                Vec::new(),
                false,
            ),
        );
        
        dialect.expand();
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
    dialect
        .sets_mut("operator_symbols")
        .extend(["%=", "&=", "*=", "+=", "-=", "/=", "^=", "|=", "!<", "!>"]);

    // T-SQL supports square brackets for identifiers and @ for variables
    dialect.insert_lexer_matchers(
        vec![
            Matcher::regex(
                "tsql_square_bracket_identifier",
                r"\[[^\]]*\]",
                SyntaxKind::DoubleQuote,
            ),
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
    dialect.replace_grammar(
        "SelectClauseModifierSegment",
        NodeMatcher::new(
            SyntaxKind::SelectClauseModifier,
            Sequence::new(vec_of_erased![
                Ref::keyword("TOP"),
                one_of(vec_of_erased![
                    Ref::new("NumericLiteralSegment"),
                    Ref::new("TsqlVariableSegment"),
                    Bracketed::new(vec_of_erased![one_of(vec_of_erased![
                        Ref::new("NumericLiteralSegment"),
                        Ref::new("TsqlVariableSegment"),
                        Ref::new("ExpressionSegment")
                    ])])
                ]),
                Ref::keyword("PERCENT").optional(),
                Ref::keyword("WITH").optional(),
                Ref::keyword("TIES").optional()
            ])
            .to_matchable(),
        )
        .to_matchable(),
    );

    // Add T-SQL assignment operator segment
    dialect.add([(
        "AssignmentOperatorSegment".into(),
        NodeMatcher::new(
            SyntaxKind::AssignmentOperator,
            Ref::new("EqualsSegment").to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // DECLARE statement
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
            Ref::new("SetVariableStatementGrammar").to_matchable().into(),
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

    // BEGIN...END blocks
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
                AnyNumberOf::new(vec_of_erased![
                    Sequence::new(vec_of_erased![
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
                            Ref::new("BeginEndBlockSegment")  // Allow nested BEGIN...END
                        ]),
                        Ref::new("DelimiterGrammar").optional()
                    ])
                ])
                .config(|this| this.min_times(0)),  // Allow empty blocks
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
            NodeMatcher::new(SyntaxKind::TableExpression, Ref::new("PivotUnpivotGrammar").to_matchable())
                .to_matchable()
                .into(),
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
                        Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                            "LiteralGrammar"
                        )])])
                    ])
                ]),
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
    dialect.replace_grammar(
        "StatementSegment",
        NodeMatcher::new(
            SyntaxKind::Statement,
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

    // Add variable reference support
    // Note: We add this as a new segment type instead of replacing SingleIdentifierGrammar
    dialect.add([
        (
            "TsqlVariableSegment".into(),
            TypedParser::new(SyntaxKind::TsqlVariable, SyntaxKind::TsqlVariable)
                .to_matchable()
                .into(),
        ),
        (
            "ParameterizedSegment".into(),
            NodeMatcher::new(
                SyntaxKind::ParameterizedExpression,
                Ref::new("TsqlVariableSegment").to_matchable(),
            )
            .to_matchable()
            .into(),
        ),
        (
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
    dialect.add([
        (
            "TableHintSegment".into(),
            Sequence::new(vec_of_erased![
                Ref::keyword("WITH"),
                Bracketed::new(vec_of_erased![Delimited::new(vec_of_erased![Ref::new(
                    "TableHintElement"
                )])])
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

    // Override PostTableExpressionGrammar to include table hints
    dialect.add([(
        "PostTableExpressionGrammar".into(),
        Ref::new("TableHintSegment")
            .optional()
            .to_matchable()
            .into(),
    )]);
    
    // Also update JoinClauseSegment to handle APPLY syntax properly
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
                    one_of(vec_of_erased![
                        Ref::keyword("CROSS"),
                        Ref::keyword("OUTER")
                    ]),
                    Ref::keyword("APPLY"),
                    MetaSegment::indent(),
                    Ref::new("FromExpressionElementSegment"),
                    MetaSegment::dedent(),
                ])
            ])
            .to_matchable()
        )
        .to_matchable()
    );

    // T-SQL specific data type handling for MAX keyword
    // TODO: Fix this - DatatypeIdentifierSegment is not a grammar, it's a SegmentGenerator
    // dialect.replace_grammar(
    //     "DatatypeIdentifierSegment",
    //     one_of(vec_of_erased![
    //         Ref::new("SingleIdentifierGrammar"),
    //         Ref::keyword("MAX")  // T-SQL allows MAX as a data type length
    //     ])
    //     .to_matchable()
    // );

    // APPLY clause support (CROSS APPLY and OUTER APPLY)
    dialect.add([(
        "ApplyClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::JoinClause,
            Sequence::new(vec_of_erased![
                one_of(vec_of_erased![Ref::keyword("CROSS"), Ref::keyword("OUTER")]),
                Ref::keyword("APPLY"),
                MetaSegment::indent(),
                Ref::new("FromExpressionElementSegment"),
                MetaSegment::dedent()
            ])
            .to_matchable(),
        )
        .to_matchable()
        .into(),
    )]);

    // Add JoinLikeClauseGrammar for T-SQL to include APPLY
    dialect.add([(
        "JoinLikeClauseGrammar".into(),
        Ref::new("ApplyClauseSegment").to_matchable().into(),
    )]);

    // WITHIN GROUP support for aggregate functions like STRING_AGG
    dialect.add([(
        "WithinGroupClauseSegment".into(),
        NodeMatcher::new(
            SyntaxKind::WithingroupClause,
            Sequence::new(vec_of_erased![
                Ref::keyword("WITHIN"),
                Ref::keyword("GROUP"),
                Bracketed::new(vec_of_erased![Ref::new("OrderByClauseSegment").optional()])
            ])
            .to_matchable(),
        )
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
            Ref::new("ParameterizedSegment")  // Add T-SQL variables
        ])
        .to_matchable()
        .into(),
    )]);

    dialect
}
